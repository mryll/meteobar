use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct ResolvedLocation {
    pub lat: f64,
    pub lon: f64,
    pub city: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WeatherData {
    pub current: CurrentWeather,
    pub daily: DailyForecast,
    #[serde(default)]
    pub hourly: Option<HourlyForecast>,
    pub timezone: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CurrentWeather {
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
}

#[derive(Deserialize)]
struct IpApiResponse {
    latitude: f64,
    longitude: f64,
    city: String,
}

pub fn geocode(client: &Client, city: &str) -> Result<ResolvedLocation, String> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1",
        urlencoding(city)
    );
    let resp: GeocodingResponse = client
        .get(&url)
        .send()
        .map_err(|e| format!("geocoding request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("geocoding HTTP error: {e}"))?
        .json()
        .map_err(|e| format!("geocoding parse failed: {e}"))?;

    let result = resp
        .results
        .into_iter()
        .next()
        .ok_or_else(|| format!("no results for location '{city}'"))?;

    Ok(ResolvedLocation {
        lat: result.latitude,
        lon: result.longitude,
        city: result.name,
    })
}

pub fn geolocate_ip(client: &Client) -> Result<ResolvedLocation, String> {
    let geo_client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| client.clone());

    let resp: IpApiResponse = geo_client
        .get("https://ipapi.co/json/")
        .send()
        .map_err(|e| format!("IP geolocation failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("IP geolocation HTTP error: {e}"))?
        .json()
        .map_err(|e| format!("IP geolocation parse failed: {e}"))?;

    Ok(ResolvedLocation {
        lat: resp.latitude,
        lon: resp.longitude,
        city: resp.city,
    })
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
    let current_params = "temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,is_day,wind_speed_10m,wind_direction_10m,pressure_msl,precipitation";
    let daily_params = "weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,precipitation_probability_max,wind_speed_10m_max";

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

    let data: WeatherData = client
        .get(&url)
        .send()
        .map_err(|e| format!("weather fetch failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("weather HTTP error: {e}"))?
        .json()
        .map_err(|e| format!("weather parse failed: {e}"))?;

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
