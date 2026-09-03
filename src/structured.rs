//! Structured JSON output (`--output json`).
//!
//! Raw machine-readable data for frontends that own their presentation (e.g.
//! the Omarchy shell plugin in `omarchy/`): no Pango markup, no colors, no
//! pre-rendered layout. The icon glyph is still resolved here — the icon-set
//! logic is meteobar's single source of truth — but it is a bare glyph string.

use serde::Serialize;

use crate::api::WeatherData;
use crate::cache::Freshness;
use crate::forecast;
use crate::format::degrees_to_cardinal;
use crate::i18n::Language;
use crate::icons::{get_icon_plain, IconSet};
use crate::theme::{RampStop, ThemeColors};

pub const SCHEMA_VERSION: u8 = 1;

#[derive(Serialize)]
pub struct StructuredOutput {
    pub schema_version: u8,
    /// `null` on success. The process still exits 0 on errors.
    pub error: Option<ErrorInfo>,
    /// Resolved display name ("City, Province, CC"); `null` on error.
    pub location: Option<String>,
    pub location_short: Option<String>,
    pub units: UnitLabels,
    pub icon_set: &'static str,
    pub cache: CacheInfo,
    pub palette: Palette,
    pub current: Option<Current>,
    pub hourly: Vec<HourlyEntry>,
    pub daily: Vec<DailyEntry>,
}

/// Colors resolved by the core from the active theme, published so the panel
/// renders the same values the same way the Waybar tooltip does — including on
/// a pywal-only machine, where the panel has no other way to reach them. The
/// ramp carries its stop *positions*, not just colors, so a threshold change
/// moves both frontends at once.
#[derive(Serialize)]
pub struct Palette {
    pub text: String,
    pub dim: String,
    pub accent: String,
    /// Anchors for temperature read-outs (daily min/max, hourly warmth).
    pub temp_cold: String,
    pub temp_warm: String,
    /// Precipitation probability ramp, ascending by `pct`.
    pub precip_ramp: Vec<RampStop>,
}

impl Palette {
    pub fn from_theme(colors: &ThemeColors) -> Self {
        Self {
            text: colors.text.clone(),
            dim: colors.dim.clone(),
            accent: colors.accent.clone(),
            temp_cold: colors.temp_cold(),
            temp_warm: colors.temp_warm(),
            precip_ramp: colors.precip_ramp(),
        }
    }
}

#[derive(Serialize)]
pub struct ErrorInfo {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Provenance of the payload: when it was fetched and whether it is a stale
/// fallback (a fresh fetch failed and the last cached payload was served).
#[derive(Serialize)]
pub struct CacheInfo {
    /// Last successful fetch, ISO-8601 with offset.
    pub fetched_at: Option<String>,
    pub stale: bool,
    pub stale_reason: Option<&'static str>,
}

impl CacheInfo {
    pub fn from_freshness(freshness: &Freshness) -> Self {
        Self {
            fetched_at: freshness.fetched_at.map(|st| {
                chrono::DateTime::<chrono::Local>::from(st)
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
            }),
            stale: freshness.stale,
            stale_reason: freshness.stale_reason,
        }
    }

    pub fn empty() -> Self {
        Self {
            fetched_at: None,
            stale: false,
            stale_reason: None,
        }
    }
}

#[derive(Serialize)]
pub struct UnitLabels {
    pub temperature: &'static str,
    pub wind_speed: &'static str,
    pub pressure: &'static str,
}

#[derive(Serialize)]
pub struct Current {
    pub temperature: f64,
    pub feels_like: Option<f64>,
    pub humidity_pct: Option<f64>,
    pub wind_speed: Option<f64>,
    pub wind_direction_deg: Option<f64>,
    pub wind_direction: Option<&'static str>,
    pub pressure: Option<f64>,
    pub precipitation: Option<f64>,
    pub weather_code: u8,
    pub is_day: bool,
    pub icon: String,
    pub description: &'static str,
    /// Condition family, same vocabulary as the waybar CSS class
    /// (clear|cloudy|rainy|snowy|stormy|foggy).
    pub condition: &'static str,
}

#[derive(Serialize)]
pub struct HourlyEntry {
    /// "YYYY-MM-DDTHH:MM" in the location's timezone.
    pub time: String,
    pub temperature: f64,
    pub weather_code: u8,
    pub is_day: bool,
    pub icon: String,
    pub description: &'static str,
    /// Precipitation probability, 0-100.
    pub precip_pct: Option<u8>,
}

#[derive(Serialize)]
pub struct DailyEntry {
    /// "YYYY-MM-DD".
    pub date: String,
    pub temperature_min: f64,
    pub temperature_max: f64,
    pub weather_code: u8,
    pub icon: String,
    pub description: &'static str,
    /// Maximum precipitation probability for the day, 0-100.
    pub precip_pct: Option<u8>,
    pub sunrise: String,
    pub sunset: String,
}

fn unit_labels(imperial: bool) -> UnitLabels {
    UnitLabels {
        temperature: if imperial { "°F" } else { "°C" },
        wind_speed: if imperial { "mph" } else { "km/h" },
        pressure: "hPa",
    }
}

pub fn icon_set_name(icon_set: &IconSet) -> &'static str {
    match icon_set {
        IconSet::Nerd => "nerd",
        IconSet::Weather => "weather",
        IconSet::Emoji => "emoji",
        IconSet::Fontawesome => "fontawesome",
    }
}

/// What to render, and in which units/glyphs/language, for one structured build.
pub struct Request<'a> {
    pub days: u8,
    pub hours: u8,
    pub icon_set: &'a IconSet,
    pub imperial: bool,
    pub language: Language,
}

pub fn build(
    weather: &WeatherData,
    city: &str,
    // The bare place name, without province or country. Published beside the
    // full label so a narrow frontend can pick the short form instead of
    // eliding the long one — the split is done here, where the parts are still
    // separate, never by cutting the display string at a comma downstream.
    city_short: &str,
    request: &Request,
    cache: CacheInfo,
    colors: &ThemeColors,
) -> StructuredOutput {
    let Request {
        days,
        hours,
        icon_set,
        imperial,
        language,
    } = *request;
    let current = &weather.current;
    let icon_info = get_icon_plain(
        current.weather_code,
        current.is_day == 1,
        icon_set,
        language,
    );

    StructuredOutput {
        schema_version: SCHEMA_VERSION,
        error: None,
        location: Some(city.to_string()),
        location_short: Some(city_short.to_string()),
        units: unit_labels(imperial),
        icon_set: icon_set_name(icon_set),
        cache,
        palette: Palette::from_theme(colors),
        current: Some(Current {
            temperature: current.temperature_2m,
            feels_like: current.apparent_temperature,
            humidity_pct: current.relative_humidity_2m,
            wind_speed: current.wind_speed_10m,
            wind_direction_deg: current.wind_direction_10m,
            wind_direction: current.wind_direction_10m.map(degrees_to_cardinal),
            pressure: current.pressure_msl,
            precipitation: current.precipitation,
            weather_code: current.weather_code,
            is_day: current.is_day == 1,
            icon: icon_info.icon,
            description: icon_info.description,
            condition: icon_info.css_class,
        }),
        hourly: build_hourly(weather, hours, icon_set, language),
        daily: build_daily(weather, days, icon_set, language),
    }
}

pub fn error_output(
    message: &str,
    icon_set: &IconSet,
    imperial: bool,
    colors: &ThemeColors,
) -> StructuredOutput {
    StructuredOutput {
        schema_version: SCHEMA_VERSION,
        error: Some(ErrorInfo {
            message: message.to_string(),
            code: None,
        }),
        location: None,
        location_short: None,
        units: unit_labels(imperial),
        icon_set: icon_set_name(icon_set),
        cache: CacheInfo::empty(),
        palette: Palette::from_theme(colors),
        current: None,
        hourly: Vec::new(),
        daily: Vec::new(),
    }
}

/// Render the shared hourly selection. Which entries appear is decided once,
/// in `forecast`, so this matches the Waybar tooltip exactly.
fn build_hourly(
    weather: &WeatherData,
    hours: u8,
    icon_set: &IconSet,
    language: Language,
) -> Vec<HourlyEntry> {
    forecast::upcoming_hours(weather, hours)
        .into_iter()
        .map(|slot| {
            let icon_info = get_icon_plain(slot.weather_code, slot.is_day, icon_set, language);
            HourlyEntry {
                time: slot.time,
                temperature: slot.temperature,
                weather_code: slot.weather_code,
                is_day: slot.is_day,
                icon: icon_info.icon,
                description: icon_info.description,
                precip_pct: slot.precip_pct,
            }
        })
        .collect()
}

fn build_daily(
    weather: &WeatherData,
    days: u8,
    icon_set: &IconSet,
    language: Language,
) -> Vec<DailyEntry> {
    forecast::forecast_days(weather, days)
        .into_iter()
        .map(|slot| {
            let icon_info = get_icon_plain(slot.weather_code, true, icon_set, language);
            DailyEntry {
                date: slot.date,
                temperature_min: slot.temperature_min,
                temperature_max: slot.temperature_max,
                weather_code: slot.weather_code,
                icon: icon_info.icon,
                description: icon_info.description,
                precip_pct: slot.precip_pct,
                sunrise: slot.sunrise,
                sunset: slot.sunset,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{CurrentWeather, DailyForecast, HourlyForecast, WeatherData};

    fn fixture() -> WeatherData {
        WeatherData {
            current: CurrentWeather {
                time: Some("2026-08-20T15:15".into()),
                temperature_2m: 12.3,
                weather_code: 3,
                is_day: 1,
                relative_humidity_2m: Some(60.0),
                apparent_temperature: Some(10.1),
                wind_speed_10m: Some(9.0),
                wind_direction_10m: Some(40.0),
                pressure_msl: Some(1012.0),
                precipitation: Some(0.0),
            },
            daily: DailyForecast {
                time: vec!["2026-08-20".into(), "2026-08-20".into()],
                weather_code: vec![3, 61],
                temperature_2m_max: vec![15.0, 14.0],
                temperature_2m_min: vec![8.0, 7.0],
                sunrise: vec!["2026-08-20T07:30".into(), "2026-08-20T07:29".into()],
                sunset: vec!["2026-08-20T18:15".into(), "2026-08-20T18:16".into()],
                precipitation_probability_max: vec![20, 80],
                wind_speed_10m_max: vec![20.0, 25.0],
            },
            hourly: Some(HourlyForecast {
                time: vec![
                    "2026-08-20T13:00".into(),
                    "2026-08-20T14:00".into(),
                    "2026-08-20T15:00".into(),
                    "2026-08-20T16:00".into(),
                    "2026-08-20T17:00".into(),
                    "2026-08-20T18:00".into(),
                    "2026-08-20T19:00".into(),
                ],
                temperature_2m: vec![11.0, 11.5, 12.0, 12.5, 12.0, 11.0, 10.0],
                weather_code: vec![3, 3, 3, 2, 2, 1, 0],
                precipitation_probability: vec![0, 0, 10, 10, 20, 20, 30],
            }),
            timezone: "Europe/Berlin".into(),
            utc_offset_seconds: Some(7200),
        }
    }

    fn fresh_cache() -> CacheInfo {
        CacheInfo::empty()
    }

    /// Positional wrapper around `build`, so the tests stay readable. Always
    /// English — `structured_output_is_localized` below is the one test that
    /// calls `build` directly to check German threads through.
    fn build_test(
        weather: &WeatherData,
        city: &str,
        days: u8,
        hours: u8,
        icon_set: &IconSet,
        imperial: bool,
        cache: CacheInfo,
    ) -> StructuredOutput {
        build(
            weather,
            city,
            city,
            &Request {
                days,
                hours,
                icon_set,
                imperial,
                language: Language::En,
            },
            cache,
            &ThemeColors::default(),
        )
    }

    #[test]
    fn build_maps_current_conditions() {
        let out = build_test(
            &fixture(),
            "Berlin, Berlin, DE",
            7,
            4,
            &IconSet::Nerd,
            false,
            fresh_cache(),
        );
        assert_eq!(out.schema_version, SCHEMA_VERSION);
        assert!(out.error.is_none());
        assert_eq!(out.location.as_deref(), Some("Berlin, Berlin, DE"));
        assert_eq!(out.units.temperature, "°C");
        assert_eq!(out.units.wind_speed, "km/h");
        let current = out.current.expect("current present");
        assert_eq!(current.temperature, 12.3);
        assert_eq!(current.feels_like, Some(10.1));
        assert_eq!(current.humidity_pct, Some(60.0));
        assert_eq!(current.wind_direction, Some("NE"));
        assert_eq!(current.condition, "cloudy");
        assert_eq!(current.description, "Overcast");
        assert!(current.is_day);
        assert!(!current.icon.is_empty());
    }

    #[test]
    fn structured_output_is_localized() {
        // Unlike build_test, calls build() directly with language: De — this
        // is the one test confirming German actually threads through Request
        // into the current/hourly/daily condition text.
        let out = build(
            &fixture(),
            "Berlin",
            "Berlin",
            &Request {
                days: 2,
                hours: 1,
                icon_set: &IconSet::Nerd,
                imperial: false,
                language: Language::De,
            },
            fresh_cache(),
            &ThemeColors::default(),
        );
        let current = out.current.expect("current present");
        // current.weather_code is 3 ("Overcast" / "Bedeckt" in the fixture).
        assert_eq!(current.description, "Bedeckt");
        // condition (the CSS-class-style slug) never translates.
        assert_eq!(current.condition, "cloudy");
        // daily[0] shares the same code (weather_code: vec![3, 61]).
        assert_eq!(out.daily[0].description, "Bedeckt");
        assert_eq!(out.daily[1].description, "Leichter Regen");
    }

    #[test]
    fn hourly_starts_at_the_in_progress_hour_and_caps_count() {
        let out = build_test(&fixture(), "X", 7, 4, &IconSet::Nerd, false, fresh_cache());
        // current.time is 15:15 → the 15:00 slot is in progress and kept.
        let times: Vec<&str> = out.hourly.iter().map(|h| h.time.as_str()).collect();
        assert_eq!(
            times,
            vec![
                "2026-08-20T15:00",
                "2026-08-20T16:00",
                "2026-08-20T17:00",
                "2026-08-20T18:00"
            ]
        );
        assert_eq!(out.hourly[0].precip_pct, Some(10));
    }

    #[test]
    fn hourly_is_day_follows_sunrise_and_sunset() {
        let out = build_test(&fixture(), "X", 7, 24, &IconSet::Nerd, false, fresh_cache());
        let by_time = |t: &str| out.hourly.iter().find(|h| h.time == t).unwrap();
        assert!(by_time("2026-08-20T17:00").is_day);
        assert!(!by_time("2026-08-20T19:00").is_day); // after 18:15 sunset
    }

    #[test]
    fn daily_is_capped_to_requested_days() {
        let out = build_test(&fixture(), "X", 1, 0, &IconSet::Nerd, false, fresh_cache());
        assert_eq!(out.daily.len(), 1);
        assert_eq!(out.daily[0].date, "2026-08-20");
        assert_eq!(out.daily[0].temperature_min, 8.0);
        assert_eq!(out.daily[0].temperature_max, 15.0);
        assert_eq!(out.daily[0].precip_pct, Some(20));
    }

    #[test]
    fn mismatched_array_lengths_truncate_instead_of_panicking() {
        let mut weather = fixture();
        // Daily: only one weather_code for two days → one row survives.
        weather.daily.weather_code = vec![3];
        // Hourly: temperatures run out after 4 entries, precip after 2.
        if let Some(hourly) = weather.hourly.as_mut() {
            hourly.temperature_2m.truncate(4);
            hourly.precipitation_probability.truncate(2);
        }
        // Skip the now-filter so the truncation itself is what is observed.
        weather.current.time = Some("2026-08-20T00:00".into());

        let out = build_test(&weather, "X", 7, 24, &IconSet::Nerd, false, fresh_cache());
        assert_eq!(out.daily.len(), 1);
        assert_eq!(out.hourly.len(), 4);
        // Entries beyond the short optional array carry None, not synthesized 0.
        assert_eq!(out.hourly[1].precip_pct, Some(0));
        assert_eq!(out.hourly[2].precip_pct, None);

        // Fully empty arrays → valid empty JSON arrays.
        weather.daily.time.clear();
        weather.hourly = None;
        let out = build_test(&weather, "X", 7, 24, &IconSet::Nerd, false, fresh_cache());
        assert!(out.daily.is_empty());
        assert!(out.hourly.is_empty());
        let json = serde_json::to_string(&out).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["daily"].as_array().unwrap().is_empty());
        assert!(parsed["hourly"].as_array().unwrap().is_empty());
    }

    #[test]
    fn cache_info_reports_staleness() {
        let freshness = Freshness {
            fetched_at: Some(std::time::SystemTime::UNIX_EPOCH),
            stale: true,
            stale_reason: Some("fetch_error"),
        };
        let out = build_test(
            &fixture(),
            "X",
            1,
            0,
            &IconSet::Nerd,
            false,
            CacheInfo::from_freshness(&freshness),
        );
        let json = serde_json::to_string(&out).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["cache"]["stale"], serde_json::Value::Bool(true));
        assert_eq!(
            parsed["cache"]["stale_reason"].as_str(),
            Some("fetch_error")
        );
        assert!(parsed["cache"]["fetched_at"].is_string());

        let fresh = Freshness {
            fetched_at: Some(std::time::SystemTime::UNIX_EPOCH),
            stale: false,
            stale_reason: None,
        };
        let info = CacheInfo::from_freshness(&fresh);
        assert!(!info.stale);
        assert_eq!(info.stale_reason, None);
    }

    #[test]
    fn icons_carry_no_markup_even_for_fontawesome() {
        let out = build_test(
            &fixture(),
            "X",
            2,
            2,
            &IconSet::Fontawesome,
            false,
            fresh_cache(),
        );
        let current = out.current.unwrap();
        assert!(!current.icon.contains('<'));
        assert!(out.hourly.iter().all(|h| !h.icon.contains('<')));
        assert!(out.daily.iter().all(|d| !d.icon.contains('<')));
    }

    #[test]
    fn error_output_is_valid_json_with_error_object() {
        let out = error_output(
            "no results for location 'Nowhere'",
            &IconSet::Nerd,
            true,
            &ThemeColors::default(),
        );
        assert_eq!(out.schema_version, SCHEMA_VERSION);
        assert!(out.current.is_none());
        assert_eq!(out.units.temperature, "°F");
        assert_eq!(out.units.wind_speed, "mph");
        let json = serde_json::to_string(&out).expect("serializes");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(
            parsed["error"]["message"].as_str(),
            Some("no results for location 'Nowhere'")
        );
        assert!(parsed["location"].is_null());
        assert_eq!(parsed["cache"]["stale"], serde_json::Value::Bool(false));
    }
}
