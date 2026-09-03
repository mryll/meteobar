use std::io::Read;

use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Cap on a single API response body. Both endpoints answer in a few KiB —
/// clap caps a request at `--days 7 --hours 24` — so this is three orders of
/// magnitude above anything legitimate, and still bounded.
const RESPONSE_LIMIT: u64 = 2 * 1024 * 1024;

/// Deserialize a response body without letting it size itself.
///
/// `Response::json` buffers the whole body first, and the only bound on that
/// is whatever the server chooses to send. Nothing between here and the
/// network is trusted enough for that: DNS, the TLS endpoint, or a proxy can
/// all answer with a stream that never ends. meteobar runs inside the
/// long-lived, unsandboxed omarchy-shell process, so an unbounded body is the
/// shell's memory, not meteobar's.
///
/// Reading one byte past the cap is what separates a body that fits from one
/// that was cut at it. `take` hides the real length, so the message names the
/// cap rather than a size it cannot know.
fn read_json_bounded<T: DeserializeOwned>(resp: reqwest::blocking::Response) -> Result<T, String> {
    let mut buf: Vec<u8> = Vec::new();
    resp.take(RESPONSE_LIMIT + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("read failed: {e}"))?;
    if buf.len() as u64 > RESPONSE_LIMIT {
        return Err(format!("response is larger than {} bytes", RESPONSE_LIMIT));
    }
    serde_json::from_slice(&buf).map_err(|e| e.to_string())
}

#[derive(Debug)]
pub struct ResolvedLocation {
    pub lat: f64,
    pub lon: f64,
    /// Disambiguated display name: "Avellaneda, Buenos Aires, AR". This is what
    /// proves the geocoder found the place you meant — there is more than one
    /// Avellaneda — so it stays the default label.
    pub city: String,
    /// Just the place, with no province or country: "Avellaneda". Published
    /// alongside the full name so a narrow surface can pick the short form
    /// instead of eliding the long one. The SPLIT happens here, once, where the
    /// parts are still separate; a frontend must never re-derive it by cutting
    /// the display string at a comma.
    pub short: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WeatherData {
    pub current: CurrentWeather,
    pub daily: DailyForecast,
    #[serde(default)]
    pub hourly: Option<HourlyForecast>,
    pub timezone: String,
    /// UTC offset of the location's timezone, used to derive "now" in local
    /// time when a cache entry predates `current.time`. Optional so caches
    /// written by older versions still deserialize.
    #[serde(default)]
    pub utc_offset_seconds: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CurrentWeather {
    /// Observation time in the location's timezone ("YYYY-MM-DDTHH:MM").
    /// Optional so caches written by older versions still deserialize.
    #[serde(default)]
    pub time: Option<String>,
    pub temperature_2m: f64,
    pub weather_code: u8,
    pub is_day: u8,
    #[serde(default)]
    pub relative_humidity_2m: Option<f64>,
    #[serde(default)]
    pub apparent_temperature: Option<f64>,
    #[serde(default)]
    pub wind_speed_10m: Option<f64>,
    #[serde(default)]
    pub wind_direction_10m: Option<f64>,
    #[serde(default)]
    pub pressure_msl: Option<f64>,
    #[serde(default)]
    pub precipitation: Option<f64>,
    #[serde(default)]
    pub uv_index: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DailyForecast {
    pub time: Vec<String>,
    pub weather_code: Vec<u8>,
    pub temperature_2m_max: Vec<f64>,
    pub temperature_2m_min: Vec<f64>,
    pub sunrise: Vec<String>,
    pub sunset: Vec<String>,
    #[serde(default)]
    pub precipitation_probability_max: Vec<u8>,
    #[serde(default)]
    pub wind_speed_10m_max: Vec<f64>,
    #[serde(default)]
    pub uv_index_max: Vec<f64>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HourlyForecast {
    pub time: Vec<String>,
    pub temperature_2m: Vec<f64>,
    pub weather_code: Vec<u8>,
    #[serde(default)]
    pub precipitation_probability: Vec<u8>,
}

#[derive(Deserialize)]
struct GeocodingResponse {
    #[serde(default)]
    results: Vec<GeocodingResult>,
}

#[derive(Deserialize)]
struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    admin1: Option<String>,
    #[serde(default)]
    admin2: Option<String>,
    #[serde(default)]
    admin3: Option<String>,
    #[serde(default)]
    admin4: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    country_code: Option<String>,
}

#[derive(Deserialize)]
struct IpGeoResponse {
    success: bool,
    latitude: f64,
    longitude: f64,
    city: String,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    country_code: Option<String>,
}

pub fn geocode(client: &Client, location: &str) -> Result<ResolvedLocation, String> {
    let (search_name, qualifiers) = parse_location(location);

    let count = if qualifiers.is_empty() { 1 } else { 5 };
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count={count}",
        urlencoding(search_name)
    );

    let raw = client
        .get(&url)
        .send()
        .map_err(|e| format!("geocoding request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("geocoding HTTP error: {e}"))?;
    let resp: GeocodingResponse =
        read_json_bounded(raw).map_err(|e| format!("geocoding parse failed: {e}"))?;

    if resp.results.is_empty() {
        return Err(format!("no results for location '{location}'"));
    }

    let result = if qualifiers.is_empty() {
        &resp.results[0]
    } else {
        resp.results
            .iter()
            .find(|r| matches_qualifier(r, &qualifiers))
            .unwrap_or(&resp.results[0])
    };

    Ok(ResolvedLocation {
        lat: result.latitude,
        lon: result.longitude,
        city: build_display_name(
            &result.name,
            result.admin1.as_deref(),
            result.country_code.as_deref(),
        ),
        short: result.name.clone(),
    })
}

pub fn geolocate_ip(client: &Client) -> Result<ResolvedLocation, String> {
    let geo_client = Client::builder()
        // Same reason as the main client in main.rs: no legitimate redirect
        // exists here, so following one only ever hands the destination to
        // whoever answered. The `unwrap_or_else` below falls back to the main
        // client, which carries the same policy.
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| client.clone());

    let raw = geo_client
        .get("https://ipwho.is/")
        .send()
        .map_err(|e| format!("IP geolocation failed: {e}"))?;
    let resp: IpGeoResponse =
        read_json_bounded(raw).map_err(|e| format!("IP geolocation parse failed: {e}"))?;

    if !resp.success {
        return Err("IP geolocation lookup failed".into());
    }

    Ok(ResolvedLocation {
        lat: resp.latitude,
        lon: resp.longitude,
        city: build_display_name(
            &resp.city,
            resp.region.as_deref(),
            resp.country_code.as_deref(),
        ),
        short: resp.city.clone(),
    })
}

fn parse_location(input: &str) -> (&str, Vec<&str>) {
    match input.split_once(',') {
        Some((city, rest)) => {
            let city = city.trim();
            if city.is_empty() {
                return (input, Vec::new());
            }
            let qualifiers: Vec<&str> = rest
                .split(',')
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect();
            if qualifiers.is_empty() {
                (city, Vec::new())
            } else {
                (city, qualifiers)
            }
        }
        None => (input, Vec::new()),
    }
}

fn matches_qualifier(result: &GeocodingResult, qualifiers: &[&str]) -> bool {
    qualifiers.iter().all(|token| {
        if token.len() == 2 {
            result
                .country_code
                .as_ref()
                .is_some_and(|cc| cc.eq_ignore_ascii_case(token))
        } else {
            let t = token.to_lowercase();
            [
                &result.admin1,
                &result.admin2,
                &result.admin3,
                &result.admin4,
                &result.country,
            ]
            .iter()
            .any(|field| {
                field
                    .as_ref()
                    .is_some_and(|v| v.to_lowercase().contains(&t))
            })
        }
    })
}

fn build_display_name(city: &str, admin1: Option<&str>, country_code: Option<&str>) -> String {
    let mut parts = vec![city.to_string()];
    if let Some(a) = admin1 {
        if !a.is_empty() {
            parts.push(a.to_string());
        }
    }
    if let Some(cc) = country_code {
        if !cc.is_empty() {
            parts.push(cc.to_string());
        }
    }
    parts.join(", ")
}

pub enum Units {
    Metric,
    Imperial,
}

pub fn fetch_weather(
    client: &Client,
    lat: f64,
    lon: f64,
    days: u8,
    hours: u8,
    units: &Units,
) -> Result<WeatherData, String> {
    let current_params = "temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,is_day,wind_speed_10m,wind_direction_10m,pressure_msl,precipitation,uv_index";
    let daily_params = "weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,precipitation_probability_max,wind_speed_10m_max,uv_index_max";

    let mut url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current={current_params}&daily={daily_params}&timezone=auto&forecast_days={days}"
    );

    if hours > 0 {
        url.push_str("&hourly=temperature_2m,weather_code,precipitation_probability");
    }

    match units {
        Units::Imperial => {
            url.push_str("&temperature_unit=fahrenheit&wind_speed_unit=mph");
        }
        Units::Metric => {}
    }

    let raw = client
        .get(&url)
        .send()
        .map_err(|e| format!("weather fetch failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("weather HTTP error: {e}"))?;
    let data: WeatherData =
        read_json_bounded(raw).map_err(|e| format!("weather parse failed: {e}"))?;

    validate_daily(&data.daily)?;
    if let Some(ref hourly) = data.hourly {
        validate_hourly(hourly)?;
    }

    Ok(data)
}

fn validate_daily(d: &DailyForecast) -> Result<(), String> {
    let len = d.time.len();
    if d.weather_code.len() != len
        || d.temperature_2m_max.len() != len
        || d.temperature_2m_min.len() != len
        || d.sunrise.len() != len
        || d.sunset.len() != len
    {
        return Err("daily forecast vectors have mismatched lengths".into());
    }
    Ok(())
}

fn validate_hourly(h: &HourlyForecast) -> Result<(), String> {
    let len = h.time.len();
    if h.temperature_2m.len() != len || h.weather_code.len() != len {
        return Err("hourly forecast vectors have mismatched lengths".into());
    }
    Ok(())
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b',' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
