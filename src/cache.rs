use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fs2::FileExt;

const MIN_TTL_SECS: u64 = 60;

/// Open a file for writing, refusing anything that is not a regular file.
///
/// `open(2)` blocks on a FIFO in BOTH directions: `O_WRONLY` waits for a
/// reader exactly as `O_RDONLY` waits for a writer. `safe_read` closes the
/// read side; these two paths are the write side of the same hole. Both sit at
/// a name anyone can predict inside a world-readable cache directory, so
/// whoever gets there first with `mkfifo` decides whether this process ever
/// returns — and meteobar returning never means the omarchy-shell process
/// hangs, not meteobar.
///
/// Same order as `safe_read::read_bounded`, for the same reasons: `O_NONBLOCK`
/// on the open so a FIFO fails instead of waiting, the type check on the OPEN
/// DESCRIPTOR so nothing can swap the path afterwards, and the flag cleared
/// once the type is settled so it cannot turn a good write into a spurious
/// `WouldBlock` on a filesystem that honours it. There is no byte cap here:
/// this only ever writes what this program produced.
fn open_regular_write(path: &Path, truncate: bool) -> std::io::Result<fs::File> {
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(truncate);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NONBLOCK);
    }
    let file = opts.open(path)?;

    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} is not a regular file", path.display()),
        ));
    }

    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let fd = file.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags != -1 {
            unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) };
        }
    }
    Ok(file)
}

/// Canonical request descriptor. Requests with different parameters get
/// different cache files, so e.g. Waybar (`--hours 0`) and the Omarchy plugin
/// (`--hours 12 --units imperial`) never cross-serve each other's payloads.
/// The location component is the input as given (named location string, a
/// coords pair, or the auto/IP marker) — building the key never geocodes.
pub struct CacheKey {
    pub location: String,
    /// Both units, not a system name: the cached payload holds already
    /// converted numbers, so a °C/mph request must never be served a cached
    /// °C/km-h one.
    pub units: String,
    pub days: u8,
    pub hours: u8,
}

impl CacheKey {
    pub fn canonical(&self) -> String {
        format!(
            "{}|units:{}|days:{}|hours:{}",
            self.location, self.units, self.days, self.hours
        )
    }

    /// 16-hex digest of the canonical descriptor (FNV-1a 64-bit; stable, no
    /// extra dependencies, not security-sensitive).
    pub fn digest(&self) -> String {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in self.canonical().as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("{hash:016x}")
    }
}

/// How the returned payload relates to the network: when it was fetched, and
/// whether it is a stale fallback served because a fresh fetch failed.
#[derive(Debug)]
pub struct Freshness {
    pub fetched_at: Option<std::time::SystemTime>,
    pub stale: bool,
    pub stale_reason: Option<&'static str>,
}

pub struct Cache {
    dir: PathBuf,
    file_name: String,
    ttl: Duration,
}

impl Cache {
    /// Returns the modification time of the cache file (= last successful fetch).
    pub fn last_fetched(&self) -> Option<std::time::SystemTime> {
        fs::metadata(self.dir.join(&self.file_name))
            .ok()?
            .modified()
            .ok()
    }

    pub fn new(key: &CacheKey) -> Self {
        let dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("meteobar");
        fs::create_dir_all(&dir).ok();
        // Pre-keyed versions used a single shared weather.json; clean it up.
        fs::remove_file(dir.join("weather.json")).ok();
        Self::with_dir(dir, key, Duration::from_secs(MIN_TTL_SECS))
    }

    fn with_dir(dir: PathBuf, key: &CacheKey, ttl: Duration) -> Self {
        Self {
            dir,
            file_name: format!("weather-{}.json", key.digest()),
            ttl,
        }
    }

    /// Try to read fresh cached data. Returns None if cache is missing or stale.
    fn read_fresh(&self) -> Option<String> {
        let path = self.dir.join(&self.file_name);
        let meta = fs::metadata(&path).ok()?;
        let age = meta.modified().ok()?.elapsed().unwrap_or(Duration::MAX);
        if age < self.ttl {
            // A refused entry reads as a cache miss, which is already the
            // path for a corrupt or absent one: refetch. Nothing here needs to
            // distinguish "someone put a FIFO in the cache dir" from "no cache
            // yet" — both mean the network is the only source left.
            crate::safe_read::read_bounded(&path, crate::safe_read::STATE_LIMIT).ok()
        } else {
            None
        }
    }

    /// Read stale cache as fallback (any age).
    fn read_stale(&self) -> Option<String> {
        crate::safe_read::read_bounded(
            &self.dir.join(&self.file_name),
            crate::safe_read::STATE_LIMIT,
        )
        .ok()
    }

    /// Atomically write data to cache.
    fn write(&self, data: &str) {
        let tmp = self.dir.join(format!(".{}.tmp", self.file_name));
        let dest = self.dir.join(&self.file_name);
        if let Ok(mut f) = open_regular_write(&tmp, true) {
            if f.write_all(data.as_bytes()).is_ok() {
                fs::rename(&tmp, &dest).ok();
            }
        }
    }

    /// Run a fetch function with file-lock serialization and caching.
    /// Only one process fetches a given request at a time; a second one does
    /// NOT wait for it — it serves what is already cached. Returns the payload
    /// plus its freshness metadata.
    ///
    /// The wait is what had to go. `lock_exclusive` has no timeout, so one
    /// process that holds this lock — wedged mid-fetch, stopped under a
    /// debugger, or simply planted there — stops every later run for ever,
    /// and this widget runs inside the long-lived omarchy-shell process.
    /// Refusing to wait costs nothing: whoever holds the lock is fetching the
    /// same payload, and serving cache without fetching is a path this already
    /// had for the case where the cache is fresh.
    pub fn fetch_or_cached<F>(&self, fetch_fn: F) -> Result<(String, Freshness), String>
    where
        F: FnOnce() -> Result<String, String>,
    {
        let lock_path = self.dir.join(format!(".{}.lock", self.file_name));
        let lock_file =
            open_regular_write(&lock_path, false).map_err(|e| format!("lock open failed: {e}"))?;

        // Any error means "not ours to take" and takes the same degraded path:
        // contention, and anything else the platform reports, are equally
        // reasons not to fetch and not to block.
        if lock_file.try_lock_exclusive().is_err() {
            return self.serve_cached_without_fetching();
        }

        let result = self.fetch_inner(fetch_fn);

        lock_file.unlock().ok();
        result
    }

    /// Another instance holds the lock: return what is on disk, never fetch.
    fn serve_cached_without_fetching(&self) -> Result<(String, Freshness), String> {
        if let Some(fresh) = self.read_fresh() {
            return Ok((
                fresh,
                Freshness {
                    fetched_at: self.last_fetched(),
                    stale: false,
                    stale_reason: None,
                },
            ));
        }
        match self.read_stale() {
            Some(stale) => Ok((
                stale,
                Freshness {
                    fetched_at: self.last_fetched(),
                    stale: true,
                    stale_reason: Some("lock_busy"),
                },
            )),
            None => Err("another instance holds the cache lock".to_string()),
        }
    }

    fn fetch_inner<F>(&self, fetch_fn: F) -> Result<(String, Freshness), String>
    where
        F: FnOnce() -> Result<String, String>,
    {
        // Check cache inside lock (another instance may have just refreshed it)
        if let Some(cached) = self.read_fresh() {
            return Ok((
                cached,
                Freshness {
                    fetched_at: self.last_fetched(),
                    stale: false,
                    stale_reason: None,
                },
            ));
        }

        match fetch_fn() {
            Ok(data) => {
                self.write(&data);
                Ok((
                    data,
                    Freshness {
                        fetched_at: self.last_fetched().or(Some(std::time::SystemTime::now())),
                        stale: false,
                        stale_reason: None,
                    },
                ))
            }
            Err(e) => {
                // Stale fallback: serve the last payload we have, flagged as
                // such. "fetch_error" — the failure is not classified further.
                if let Some(stale) = self.read_stale() {
                    Ok((
                        stale,
                        Freshness {
                            fetched_at: self.last_fetched(),
                            stale: true,
                            stale_reason: Some("fetch_error"),
                        },
                    ))
                } else {
                    Err(e)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key(location: &str, units: &str, days: u8, hours: u8) -> CacheKey {
        CacheKey {
            location: location.to_string(),
            units: units.to_string(),
            days,
            hours,
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("meteobar-test-{}-{}", tag, std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_regular_path_opens_for_writing_in_both_truncate_modes() {
        let dir = temp_dir("write-ok");
        let p = dir.join("f");
        fs::write(&p, "old").unwrap();
        assert!(open_regular_write(&p, false).is_ok());
        assert!(open_regular_write(&p, true).is_ok());
        assert_eq!(fs::read_to_string(&p).unwrap(), "");
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_at_the_lock_path_is_refused_instead_of_blocking_for_ever() {
        // The lock and the temp file sit at predictable names in a cache
        // directory. Without O_NONBLOCK this open never returns, and it takes
        // the whole shell process with it -- which is the failure this exists
        // to stop, and it is invisible in every test that does not plant one.
        let dir = temp_dir("write-fifo");
        let p = dir.join(".weather-deadbeef.json.lock");
        let c = std::ffi::CString::new(p.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert!(open_regular_write(&p, false).is_err());
        assert!(open_regular_write(&p, true).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_in_place_of_a_cache_file_is_refused_instead_of_written() {
        let dir = temp_dir("write-dir");
        let p = dir.join("d");
        fs::create_dir_all(&p).unwrap();
        assert!(open_regular_write(&p, false).is_err());
    }

    #[test]
    fn different_params_get_different_cache_files() {
        let base = test_key("loc:Berlin", "metric", 3, 0);
        let same = test_key("loc:Berlin", "metric", 3, 0);
        assert_eq!(base.digest(), same.digest());
        assert_eq!(base.digest().len(), 16);
        assert!(base.digest().chars().all(|c| c.is_ascii_hexdigit()));

        for other in [
            test_key("loc:Berlin", "imperial", 3, 0),
            test_key("loc:Berlin", "metric", 5, 0),
            test_key("loc:Berlin", "metric", 3, 12),
            test_key("loc:Paris", "metric", 3, 0),
            test_key("auto", "metric", 3, 0),
        ] {
            assert_ne!(base.digest(), other.digest());
        }
    }

    #[test]
    fn fresh_fetch_is_not_stale() {
        let dir = temp_dir("fresh");
        let cache = Cache::with_dir(
            dir.clone(),
            &test_key("auto", "metric", 3, 0),
            Duration::ZERO,
        );
        let (data, freshness) = cache.fetch_or_cached(|| Ok("payload-1".into())).unwrap();
        assert_eq!(data, "payload-1");
        assert!(!freshness.stale);
        assert_eq!(freshness.stale_reason, None);
        assert!(freshness.fetched_at.is_some());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn failed_fetch_serves_stale_with_reason() {
        let dir = temp_dir("stale");
        // TTL zero: every read is expired, forcing a re-fetch each call.
        let cache = Cache::with_dir(
            dir.clone(),
            &test_key("auto", "metric", 3, 0),
            Duration::ZERO,
        );
        cache.fetch_or_cached(|| Ok("payload-1".into())).unwrap();

        let (data, freshness) = cache.fetch_or_cached(|| Err("boom".into())).unwrap();
        assert_eq!(data, "payload-1");
        assert!(freshness.stale);
        assert_eq!(freshness.stale_reason, Some("fetch_error"));
        assert!(freshness.fetched_at.is_some());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn failed_fetch_without_cache_is_an_error() {
        let dir = temp_dir("nocache");
        let cache = Cache::with_dir(
            dir.clone(),
            &test_key("auto", "metric", 3, 0),
            Duration::ZERO,
        );
        let err = cache.fetch_or_cached(|| Err("boom".into())).unwrap_err();
        assert_eq!(err, "boom");
        fs::remove_dir_all(&dir).ok();
    }

    /// The lock is only worth taking if not taking it is survivable. Waiting on
    /// it is not: this runs inside omarchy-shell, and one holder that never
    /// lets go used to stop every later run for ever.
    ///
    /// The holder here is a second descriptor, not a second process, and that
    /// is the same conflict: `flock(2)` owns the lock per open file
    /// description, not per PID, so two independent `open`s contend even from
    /// one process.
    ///
    /// The call under test runs on a thread and reports through a channel, so
    /// a regression fails this test in seconds instead of hanging the suite.
    #[cfg(unix)]
    #[test]
    fn a_lock_held_by_someone_else_serves_cache_instead_of_waiting() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::mpsc;

        let dir = temp_dir("lock-held");
        let key = test_key("auto", "metric", 3, 0);
        let cache = Cache::with_dir(dir.clone(), &key, Duration::ZERO);
        cache.fetch_or_cached(|| Ok("payload-1".into())).unwrap();

        // Independent open, independent flock owner.
        let lock_path = dir.join(format!(".weather-{}.json.lock", key.digest()));
        let holder = open_regular_write(&lock_path, false).unwrap();
        holder.lock_exclusive().unwrap();
        assert!(
            open_regular_write(&lock_path, false)
                .unwrap()
                .try_lock_exclusive()
                .is_err(),
            "the lock is not actually held — this test would prove nothing"
        );

        static FETCHED: AtomicBool = AtomicBool::new(false);
        FETCHED.store(false, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        let thread_dir = dir.clone();
        std::thread::spawn(move || {
            let cache = Cache::with_dir(
                thread_dir,
                &test_key("auto", "metric", 3, 0),
                Duration::ZERO,
            );
            let out = cache.fetch_or_cached(|| {
                FETCHED.store(true, Ordering::SeqCst);
                Ok("payload-2".into())
            });
            tx.send(out).ok();
        });

        let got = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("fetch_or_cached blocked on a lock held by someone else");
        let (data, freshness) = got.unwrap();
        assert_eq!(data, "payload-1");
        assert!(freshness.stale);
        assert_eq!(freshness.stale_reason, Some("lock_busy"));
        assert!(
            !FETCHED.load(Ordering::SeqCst),
            "a run that could not take the lock must not fetch"
        );

        FileExt::unlock(&holder).ok();
        fs::remove_dir_all(&dir).ok();
    }
}
