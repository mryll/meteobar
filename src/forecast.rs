//! Forecast selection — the single source of truth for *which* entries the
//! frontends show.
//!
//! Both the Waybar tooltip and the structured JSON (and through it the Omarchy
//! panel) render these slots, so "the next N hours" means exactly the same
//! thing on both surfaces, day/night included. Selection zips the API's
//! parallel arrays instead of indexing them, so a payload whose arrays
//! disagree in length truncates to the shortest rather than panicking.

use crate::api::WeatherData;

/// One hourly forecast entry, already filtered and with daylight resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct HourSlot {
    /// "YYYY-MM-DDTHH:MM" in the location's timezone.
    pub time: String,
    pub temperature: f64,
    pub weather_code: u8,
    pub is_day: bool,
    pub precip_pct: Option<u8>,
}

/// One daily forecast entry.
#[derive(Debug, Clone, PartialEq)]
pub struct DaySlot {
    /// "YYYY-MM-DD".
    pub date: String,
    pub temperature_min: f64,
    pub temperature_max: f64,
    pub weather_code: u8,
    pub precip_pct: Option<u8>,
    /// The day's peak UV index. `None` when the payload carries no value for
    /// this day, which a cache written before UV was requested will not.
    pub uv_index_max: Option<f64>,
    pub sunrise: String,
    pub sunset: String,
}

/// "Now" in the location's timezone, for filtering the hourly series. Prefers
/// the observation time reported by the API; for cache entries that predate
/// that field, derives it from the response's own UTC offset. Returns None
/// when the response carries no timezone information at all — the machine
/// clock is NOT a substitute (the configured location may be anywhere).
fn reference_now(weather: &WeatherData) -> Option<String> {
    if let Some(time) = &weather.current.time {
        return Some(time.clone());
    }
    let offset = weather
        .utc_offset_seconds
        .and_then(|s| i32::try_from(s).ok())
        .and_then(chrono::FixedOffset::east_opt)?;
    Some(
        chrono::Utc::now()
            .with_timezone(&offset)
            .format("%Y-%m-%dT%H:%M")
            .to_string(),
    )
}

/// Keep the in-progress hour: an entry covers [HH:00, HH+1:00), so it is
/// current while `now` shares its "YYYY-MM-DDTHH" prefix. ISO-8601 strings of
/// equal shape compare chronologically as plain strings. Without a reference
/// time the series is emitted from its start.
fn is_current_or_future(time: &str, now: Option<&str>) -> bool {
    let Some(now) = now else { return true };
    time >= now || time.get(..13) == now.get(..13)
}

/// Day/night for an hourly slot, from the matching day's sunrise/sunset.
/// All three timestamps share the "YYYY-MM-DDTHH:MM" shape, so string
/// comparison is chronological. Defaults to day when the date is missing.
fn is_daylight(time: &str, weather: &WeatherData) -> bool {
    let Some(date) = time.get(..10) else {
        return true;
    };
    let daily = &weather.daily;
    daily
        .time
        .iter()
        .zip(daily.sunrise.iter())
        .zip(daily.sunset.iter())
        .find(|((day, _), _)| day.as_str() == date)
        .map(|((_, sunrise), sunset)| time >= sunrise.as_str() && time < sunset.as_str())
        .unwrap_or(true)
}

/// The next `hours` hourly entries, starting at the in-progress hour.
pub fn upcoming_hours(weather: &WeatherData, hours: u8) -> Vec<HourSlot> {
    let Some(hourly) = weather.hourly.as_ref() else {
        return Vec::new();
    };
    let now = reference_now(weather);
    let precip = hourly
        .precipitation_probability
        .iter()
        .map(|p| Some(*p))
        .chain(std::iter::repeat(None));

    hourly
        .time
        .iter()
        .zip(hourly.temperature_2m.iter())
        .zip(hourly.weather_code.iter())
        .zip(precip)
        .filter(|(((time, _), _), _)| is_current_or_future(time, now.as_deref()))
        .take(hours as usize)
        .map(
            |(((time, temperature), weather_code), precip_pct)| HourSlot {
                time: time.clone(),
                temperature: *temperature,
                weather_code: *weather_code,
                is_day: is_daylight(time, weather),
                precip_pct,
            },
        )
        .collect()
}

/// The first `days` daily entries.
pub fn forecast_days(weather: &WeatherData, days: u8) -> Vec<DaySlot> {
    let daily = &weather.daily;
    let precip = daily
        .precipitation_probability_max
        .iter()
        .map(|p| Some(*p))
        .chain(std::iter::repeat(None));
    let uv = daily
        .uv_index_max
        .iter()
        .map(|v| Some(*v))
        .chain(std::iter::repeat(None));

    daily
        .time
        .iter()
        .zip(daily.weather_code.iter())
        .zip(daily.temperature_2m_min.iter())
        .zip(daily.temperature_2m_max.iter())
        .zip(daily.sunrise.iter())
        .zip(daily.sunset.iter())
        .zip(precip)
        .zip(uv)
        .take(days as usize)
        .map(
            |(
                ((((((date, weather_code), tmin), tmax), sunrise), sunset), precip_pct),
                uv_index_max,
            )| {
                DaySlot {
                    date: date.clone(),
                    temperature_min: *tmin,
                    temperature_max: *tmax,
                    weather_code: *weather_code,
                    precip_pct,
                    uv_index_max,
                    sunrise: sunrise.clone(),
                    sunset: sunset.clone(),
                }
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{CurrentWeather, DailyForecast, HourlyForecast, WeatherData};

    pub fn fixture() -> WeatherData {
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
                uv_index: Some(5.2),
            },
            daily: DailyForecast {
                time: vec!["2026-08-20".into(), "2026-08-21".into()],
                weather_code: vec![3, 61],
                temperature_2m_max: vec![15.0, 14.0],
                temperature_2m_min: vec![8.0, 7.0],
                sunrise: vec!["2026-08-20T07:30".into(), "2026-08-21T07:29".into()],
                sunset: vec!["2026-08-20T18:15".into(), "2026-08-21T18:16".into()],
                precipitation_probability_max: vec![20, 80],
                wind_speed_10m_max: vec![20.0, 25.0],
                uv_index_max: vec![8.9, 6.4],
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

    #[test]
    fn hours_start_at_the_in_progress_hour_and_cap() {
        let slots = upcoming_hours(&fixture(), 4);
        let times: Vec<&str> = slots.iter().map(|s| s.time.as_str()).collect();
        assert_eq!(
            times,
            vec![
                "2026-08-20T15:00",
                "2026-08-20T16:00",
                "2026-08-20T17:00",
                "2026-08-20T18:00"
            ]
        );
        assert_eq!(slots[0].precip_pct, Some(10));
    }

    #[test]
    fn daylight_follows_sunrise_and_sunset() {
        let slots = upcoming_hours(&fixture(), 24);
        let at = |t: &str| slots.iter().find(|s| s.time == t).unwrap();
        assert!(at("2026-08-20T17:00").is_day);
        assert!(!at("2026-08-20T19:00").is_day); // after 18:15 sunset
    }

    #[test]
    fn mismatched_arrays_truncate_instead_of_panicking() {
        let mut weather = fixture();
        weather.daily.weather_code = vec![3];
        if let Some(hourly) = weather.hourly.as_mut() {
            hourly.temperature_2m.truncate(4);
            hourly.precipitation_probability.truncate(2);
        }
        weather.current.time = Some("2026-08-20T00:00".into());

        let hours = upcoming_hours(&weather, 24);
        assert_eq!(hours.len(), 4);
        assert_eq!(hours[1].precip_pct, Some(0));
        assert_eq!(hours[2].precip_pct, None);
        assert_eq!(forecast_days(&weather, 7).len(), 1);
    }

    #[test]
    fn empty_series_select_nothing() {
        let mut weather = fixture();
        weather.hourly = None;
        weather.daily.time.clear();
        assert!(upcoming_hours(&weather, 12).is_empty());
        assert!(forecast_days(&weather, 6).is_empty());
    }

    #[test]
    fn without_an_observation_time_the_offset_drives_the_filter() {
        let mut weather = fixture();
        weather.current.time = None;
        // A far-future series is entirely upcoming whatever the wall clock says.
        if let Some(hourly) = weather.hourly.as_mut() {
            for time in hourly.time.iter_mut() {
                *time = time.replace("2026-08-20", "2099-01-01");
            }
        }
        assert_eq!(upcoming_hours(&weather, 3).len(), 3);
    }
}
