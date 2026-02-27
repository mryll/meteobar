mod api;
mod cache;
mod format;
mod icons;
mod waybar;

use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

use api::ResolvedLocation;
use cache::CacheEntry;
use format::FormatData;
use icons::IconSet;
use waybar::{TooltipFormat, WaybarOutput};

#[derive(Parser)]
#[command(name = "meteobar", version, about = "Weather widget for Waybar using Open-Meteo")]
struct Cli {
    #[arg(long)]
    location: Option<String>,

    #[arg(long, requires = "lon", allow_hyphen_values = true)]
    lat: Option<f64>,

    #[arg(long, requires = "lat", allow_hyphen_values = true)]
    lon: Option<f64>,

    #[arg(long, help = "Display name for the location (used with --lat/--lon)")]
    city_name: Option<String>,

    #[arg(long, default_value = "{icon} {temp}°")]
    format: String,

    #[arg(long, value_enum, default_value_t = TooltipFormat::Days)]
    tooltip_format: TooltipFormat,

    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u8).range(1..=7))]
    days: u8,

    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=24))]
    hours: u8,

    #[arg(long, value_enum, default_value_t = CliUnits::Metric)]
    units: CliUnits,

    #[arg(long, value_enum, default_value_t = IconSet::Nerd)]
    icons: IconSet,

    #[arg(long)]
    cache_dir: Option<PathBuf>,

    #[arg(long)]
    no_cache: bool,

    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
    timeout: u64,
}

#[derive(Clone, clap::ValueEnum)]
enum CliUnits {
    Metric,
    Imperial,
}

fn main() {
    let cli = Cli::parse();

    let cache_dir = cli
        .cache_dir
        .clone()
        .unwrap_or_else(|| {
            dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("meteobar")
        });

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(cli.timeout))
        .user_agent(format!("meteobar/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("failed to build HTTP client");

    let units = match cli.units {
        CliUnits::Metric => api::Units::Metric,
        CliUnits::Imperial => api::Units::Imperial,
    };
    let unit_label = match cli.units {
        CliUnits::Metric => "°C",
        CliUnits::Imperial => "°F",
    };

    let result = run_pipeline(&cli, &client, &units, &cache_dir);

    match result {
        PipelineResult::Fresh {
            weather,
            city,
            lat,
            lon,
        } => {
            if !cli.no_cache {
                let entry = CacheEntry {
                    weather: weather.clone(),
                    city: city.clone(),
                    location_query: cli.location.clone(),
                    lat,
                    lon,
                    timestamp: chrono::Utc::now().timestamp(),
                };
                let _ = cache::save(&entry, &cache_dir);
            }
            let output = build_output(&weather, &city, &cli, unit_label, false);
            print_and_exit(output);
        }
        PipelineResult::Stale { weather, city } => {
            let output = build_output(&weather, &city, &cli, unit_label, true);
            print_and_exit(output);
        }
        PipelineResult::Error(msg) => {
            let output = waybar::error_output(&msg);
            print_and_exit(output);
        }
    }
}

enum PipelineResult {
    Fresh {
        weather: api::WeatherData,
        city: String,
        lat: f64,
        lon: f64,
    },
    Stale {
        weather: api::WeatherData,
        city: String,
    },
    Error(String),
}

fn run_pipeline(
    cli: &Cli,
    client: &reqwest::blocking::Client,
    units: &api::Units,
    cache_dir: &std::path::Path,
) -> PipelineResult {
    let fresh = try_fresh(cli, client, units, cache_dir);

    match fresh {
        Ok(r) => PipelineResult::Fresh {
            weather: r.weather,
            city: r.city,
            lat: r.lat,
            lon: r.lon,
        },
        Err(_) if !cli.no_cache => match cache::load(cache_dir) {
            Ok(entry) => PipelineResult::Stale {
                weather: entry.weather,
                city: entry.city,
            },
            Err(cache_err) => PipelineResult::Error(cache_err),
        },
        Err(e) => PipelineResult::Error(e),
    }
}

struct FreshResult {
    weather: api::WeatherData,
    city: String,
    lat: f64,
    lon: f64,
}

fn try_fresh(
    cli: &Cli,
    client: &reqwest::blocking::Client,
    units: &api::Units,
    cache_dir: &std::path::Path,
) -> Result<FreshResult, String> {
    let location = resolve_location(cli, client, cache_dir)?;
    let weather = api::fetch_weather(
        client,
        location.lat,
        location.lon,
        cli.days,
        cli.hours,
        units,
    )?;
    Ok(FreshResult {
        weather,
        city: location.city,
        lat: location.lat,
        lon: location.lon,
    })
}

fn resolve_location(
    cli: &Cli,
    client: &reqwest::blocking::Client,
    cache_dir: &std::path::Path,
) -> Result<ResolvedLocation, String> {
    if let (Some(lat), Some(lon)) = (cli.lat, cli.lon) {
        let city = cli
            .city_name
            .clone()
            .unwrap_or_else(|| format!("{:.2},{:.2}", lat, lon));
        return Ok(ResolvedLocation { lat, lon, city });
    }

    if let Some(ref location) = cli.location {
        if !cli.no_cache {
            if let Some((lat, lon)) = cache::get_cached_location(cache_dir, location) {
                return Ok(ResolvedLocation {
                    lat,
                    lon,
                    city: location.clone(),
                });
            }
        }
        return api::geocode(client, location);
    }

    api::geolocate_ip(client)
}

fn build_output(
    weather: &api::WeatherData,
    city: &str,
    cli: &Cli,
    unit_label: &str,
    stale: bool,
) -> WaybarOutput {
    let icon_info = icons::get_icon(
        weather.current.weather_code,
        weather.current.is_day == 1,
        &cli.icons,
    );

    let current = &weather.current;
    let today_rain = weather
        .daily
        .precipitation_probability_max
        .first()
        .copied()
        .unwrap_or(0);

    let data = FormatData {
        icon: icon_info.icon.to_string(),
        temp: format!("{}", current.temperature_2m.round() as i32),
        feels_like: format!(
            "{}",
            current
                .apparent_temperature
                .unwrap_or(current.temperature_2m)
                .round() as i32
        ),
        humidity: format!(
            "{}",
            current.relative_humidity_2m.unwrap_or(0.0).round() as i32
        ),
        wind: format!("{}", current.wind_speed_10m.unwrap_or(0.0).round() as i32),
        wind_dir: format::degrees_to_cardinal(current.wind_direction_10m.unwrap_or(0.0))
            .to_string(),
        pressure: format!("{}", current.pressure_msl.unwrap_or(0.0).round() as i32),
        city: city.to_string(),
        min: format!(
            "{}",
            weather
                .daily
                .temperature_2m_min
                .first()
                .unwrap_or(&0.0)
                .round() as i32
        ),
        max: format!(
            "{}",
            weather
                .daily
                .temperature_2m_max
                .first()
                .unwrap_or(&0.0)
                .round() as i32
        ),
        rain_chance: format!("{}", today_rain),
        description: icon_info.description.to_string(),
    };

    let text = format::render(&cli.format, &data);
    let tooltip = waybar::build_tooltip(
        city,
        weather,
        &cli.icons,
        &cli.tooltip_format,
        cli.days,
        cli.hours,
        unit_label,
    );

    let mut class = vec![icon_info.css_class.to_string()];
    if stale {
        class.push("stale".to_string());
    }

    WaybarOutput {
        text,
        tooltip,
        class,
        alt: icon_info.css_class.to_string(),
    }
}

fn print_and_exit(output: WaybarOutput) {
    match serde_json::to_string(&output) {
        Ok(json) => println!("{json}"),
        Err(_) => println!(r#"{{"text":"?","tooltip":"serialization error","class":["error"],"alt":"error"}}"#),
    }
}
