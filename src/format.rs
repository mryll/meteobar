pub struct FormatData {
    pub icon: String,
    pub temp: String,
    pub feels_like: String,
    pub humidity: String,
    pub wind: String,
    pub wind_dir: String,
    pub pressure: String,
    pub city: String,
    pub min: String,
    pub max: String,
    pub rain_chance: String,
    pub description: String,
}

pub fn render(template: &str, data: &FormatData) -> String {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut key = String::new();
            let mut found_close = false;
            for inner in chars.by_ref() {
                if inner == '}' {
                    found_close = true;
                    break;
                }
                key.push(inner);
            }
            if found_close {
                match resolve_placeholder(&key, data) {
                    Some(val) => result.push_str(val),
                    None => {
                        result.push('{');
                        result.push_str(&key);
                        result.push('}');
                    }
                }
            } else {
                result.push('{');
                result.push_str(&key);
            }
        } else {
            result.push(ch);
        }
    }

    result
}

fn resolve_placeholder<'a>(key: &str, data: &'a FormatData) -> Option<&'a str> {
    match key {
        "icon" => Some(&data.icon),
        "temp" => Some(&data.temp),
        "feels_like" => Some(&data.feels_like),
        "humidity" => Some(&data.humidity),
        "wind" => Some(&data.wind),
        "wind_dir" => Some(&data.wind_dir),
        "pressure" => Some(&data.pressure),
        "city" => Some(&data.city),
        "min" => Some(&data.min),
        "max" => Some(&data.max),
        "rain_chance" => Some(&data.rain_chance),
        "description" => Some(&data.description),
        _ => None,
    }
}

pub fn degrees_to_cardinal(degrees: f64) -> &'static str {
    let dirs = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let index = ((degrees + 22.5) / 45.0) as usize % 8;
    dirs[index]
}
