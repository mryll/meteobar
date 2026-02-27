use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct ThemeColors {
    pub border: String,
    pub text: String,
    pub dim: String,
    pub accent: String,
    pub green: String,
    pub yellow: String,
    pub orange: String,
    pub error: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            border: "#61afef".into(),
            text: "#abb2bf".into(),
            dim: "#5c6370".into(),
            accent: "#61afef".into(),
            green: "#98c379".into(),
            yellow: "#e5c07b".into(),
            orange: "#d19a66".into(),
            error: "#e06c75".into(),
        }
    }
}

impl ThemeColors {
    pub fn load() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("omarchy/current/theme/colors.toml");

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        let map = parse_toml_flat(&content);
        Self::from_map(&map).unwrap_or_default()
    }

    fn from_map(map: &HashMap<String, String>) -> Option<Self> {
        let foreground = map.get("foreground")?;
        let background = map.get("background")?;
        let accent = map.get("accent")?;
        let color1 = map.get("color1")?;
        let color2 = map.get("color2")?;
        let color3 = map.get("color3")?;

        Some(Self {
            border: accent.clone(),
            accent: accent.clone(),
            text: foreground.clone(),
            dim: blend_hex(foreground, background, 0.5),
            green: color2.clone(),
            yellow: color3.clone(),
            orange: color1.clone(),
            error: color1.clone(),
        })
    }
}

fn parse_toml_flat(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_string();
            let value = value.trim().trim_matches('"').to_string();
            map.insert(key, value);
        }
    }
    map
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

fn blend_hex(c1: &str, c2: &str, ratio: f32) -> String {
    let (r1, g1, b1) = parse_hex(c1).unwrap_or((171, 178, 191));
    let (r2, g2, b2) = parse_hex(c2).unwrap_or((40, 44, 52));
    let blend = |a: u8, b: u8| -> u8 {
        (a as f32 * (1.0 - ratio) + b as f32 * ratio).round() as u8
    };
    format!("#{:02x}{:02x}{:02x}", blend(r1, r2), blend(g1, g2), blend(b1, b2))
}
