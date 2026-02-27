use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::api::WeatherData;

#[derive(Serialize, Deserialize)]
pub struct CacheEntry {
    pub weather: WeatherData,
    pub city: String,
    pub location_query: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub timestamp: i64,
}

pub fn save(entry: &CacheEntry, cache_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(cache_dir).map_err(|e| format!("failed to create cache dir: {e}"))?;

    let json =
        serde_json::to_string(entry).map_err(|e| format!("failed to serialize cache: {e}"))?;

    let tmp_path = cache_dir.join("last.json.tmp");
    let final_path = cache_dir.join("last.json");

    let mut file =
        fs::File::create(&tmp_path).map_err(|e| format!("failed to create temp cache: {e}"))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("failed to write cache: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("failed to sync cache: {e}"))?;

    fs::rename(&tmp_path, &final_path).map_err(|e| format!("failed to rename cache: {e}"))?;

    Ok(())
}

pub fn load(cache_dir: &Path) -> Result<CacheEntry, String> {
    let path = cache_dir.join("last.json");
    let contents = fs::read_to_string(&path).map_err(|e| format!("failed to read cache: {e}"))?;

    match serde_json::from_str::<CacheEntry>(&contents) {
        Ok(entry) => Ok(entry),
        Err(e) => {
            let _ = fs::remove_file(&path);
            Err(format!("malformed cache (deleted): {e}"))
        }
    }
}

pub fn get_cached_location(cache_dir: &Path, location_query: &str) -> Option<(f64, f64)> {
    let entry = load(cache_dir).ok()?;
    let cached_query = entry.location_query.as_deref()?;
    if cached_query.to_lowercase() == location_query.to_lowercase() {
        Some((entry.lat, entry.lon))
    } else {
        None
    }
}
