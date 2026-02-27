pub struct IconInfo {
    pub icon: &'static str,
    pub css_class: &'static str,
    pub description: &'static str,
}

#[derive(Clone, clap::ValueEnum)]
pub enum IconSet {
    /// Material Design weather icons (outline style)
    Nerd,
    /// Erik Flowers Weather Icons — artistic, filled, with day/night variants
    Weather,
    /// Unicode emoji
    Emoji,
}

struct IconEntry {
    day_nerd: &'static str,
    night_nerd: &'static str,
    day_weather: &'static str,
    night_weather: &'static str,
    day_emoji: &'static str,
    night_emoji: &'static str,
    css_class: &'static str,
    description: &'static str,
}

// Weather Icons (nf-weather-*) codepoints from Erik Flowers, included in Nerd Fonts:
//   day_sunny        \u{e30d}    night_clear              \u{e32b}
//   day_cloudy       \u{e302}    night_alt_cloudy         \u{e37e}
//   day_fog          \u{e303}    night_fog                \u{e346}
//   day_sprinkle     \u{e30b}    night_alt_sprinkle       \u{e328}
//   day_showers      \u{e309}    night_alt_showers        \u{e326}
//   day_rain         \u{e308}    night_alt_rain           \u{e325}
//   day_rain_mix     \u{e306}    night_alt_rain_mix       \u{e323}
//   day_snow         \u{e30a}    night_alt_snow           \u{e327}
//   day_thunderstorm \u{e30f}    night_alt_thunderstorm   \u{e32a}
//   day_hail         \u{e304}    night_alt_hail           \u{e321}
//   day_storm_showers\u{e30e}    night_alt_storm_showers  \u{e329}
//   cloudy           \u{e312}    rain                     \u{e318}
//   fog              \u{e313}    showers                  \u{e319}
//   snow             \u{e31a}    rain_mix                 \u{e316}
//   thunderstorm     \u{e31d}    snowflake_cold           \u{e36f}
//   storm_showers    \u{e31c}

const ICONS: &[(u8, IconEntry)] = &[
    (0,  IconEntry { day_nerd: "󰖙", night_nerd: "󰖔", day_weather: "\u{e30d}", night_weather: "\u{e32b}", day_emoji: "☀️",  night_emoji: "🌙",  css_class: "clear",  description: "Clear sky" }),
    (1,  IconEntry { day_nerd: "󰖙", night_nerd: "󰖔", day_weather: "\u{e30d}", night_weather: "\u{e32b}", day_emoji: "🌤️", night_emoji: "🌙",  css_class: "clear",  description: "Mainly clear" }),
    (2,  IconEntry { day_nerd: "󰖕", night_nerd: "󰼱", day_weather: "\u{e302}", night_weather: "\u{e37e}", day_emoji: "⛅",  night_emoji: "☁️",  css_class: "cloudy", description: "Partly cloudy" }),
    (3,  IconEntry { day_nerd: "󰖐", night_nerd: "󰖐", day_weather: "\u{e312}", night_weather: "\u{e312}", day_emoji: "☁️",  night_emoji: "☁️",  css_class: "cloudy", description: "Overcast" }),
    (45, IconEntry { day_nerd: "󰖑", night_nerd: "󰖑", day_weather: "\u{e303}", night_weather: "\u{e346}", day_emoji: "🌫️", night_emoji: "🌫️", css_class: "foggy",  description: "Fog" }),
    (48, IconEntry { day_nerd: "󰖑", night_nerd: "󰖑", day_weather: "\u{e303}", night_weather: "\u{e346}", day_emoji: "🌫️", night_emoji: "🌫️", css_class: "foggy",  description: "Rime fog" }),
    (51, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e30b}", night_weather: "\u{e328}", day_emoji: "🌦️", night_emoji: "🌧️", css_class: "rainy",  description: "Light drizzle" }),
    (53, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e30b}", night_weather: "\u{e328}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Moderate drizzle" }),
    (55, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e319}", night_weather: "\u{e319}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Dense drizzle" }),
    (56, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e306}", night_weather: "\u{e323}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Freezing drizzle" }),
    (57, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e316}", night_weather: "\u{e316}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Dense freezing drizzle" }),
    (61, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e308}", night_weather: "\u{e325}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Slight rain" }),
    (63, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e318}", night_weather: "\u{e318}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Moderate rain" }),
    (65, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e318}", night_weather: "\u{e318}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Heavy rain" }),
    (66, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e306}", night_weather: "\u{e323}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Freezing rain" }),
    (67, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e316}", night_weather: "\u{e316}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Heavy freezing rain" }),
    (71, IconEntry { day_nerd: "󰖘", night_nerd: "󰖘", day_weather: "\u{e30a}", night_weather: "\u{e327}", day_emoji: "🌨️", night_emoji: "🌨️", css_class: "snowy",  description: "Slight snow" }),
    (73, IconEntry { day_nerd: "󰖘", night_nerd: "󰖘", day_weather: "\u{e31a}", night_weather: "\u{e31a}", day_emoji: "🌨️", night_emoji: "🌨️", css_class: "snowy",  description: "Moderate snow" }),
    (75, IconEntry { day_nerd: "󰖘", night_nerd: "󰖘", day_weather: "\u{e31a}", night_weather: "\u{e31a}", day_emoji: "🌨️", night_emoji: "🌨️", css_class: "snowy",  description: "Heavy snow" }),
    (77, IconEntry { day_nerd: "󰖘", night_nerd: "󰖘", day_weather: "\u{e36f}", night_weather: "\u{e36f}", day_emoji: "🌨️", night_emoji: "🌨️", css_class: "snowy",  description: "Snow grains" }),
    (80, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e309}", night_weather: "\u{e326}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Slight rain showers" }),
    (81, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e319}", night_weather: "\u{e319}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Moderate rain showers" }),
    (82, IconEntry { day_nerd: "󰖗", night_nerd: "󰖗", day_weather: "\u{e31c}", night_weather: "\u{e329}", day_emoji: "🌧️", night_emoji: "🌧️", css_class: "rainy",  description: "Violent rain showers" }),
    (85, IconEntry { day_nerd: "󰖘", night_nerd: "󰖘", day_weather: "\u{e30a}", night_weather: "\u{e327}", day_emoji: "🌨️", night_emoji: "🌨️", css_class: "snowy",  description: "Slight snow showers" }),
    (86, IconEntry { day_nerd: "󰖘", night_nerd: "󰖘", day_weather: "\u{e31a}", night_weather: "\u{e31a}", day_emoji: "🌨️", night_emoji: "🌨️", css_class: "snowy",  description: "Heavy snow showers" }),
    (95, IconEntry { day_nerd: "󰖓", night_nerd: "󰖓", day_weather: "\u{e30f}", night_weather: "\u{e32a}", day_emoji: "⛈️",  night_emoji: "⛈️",  css_class: "stormy", description: "Thunderstorm" }),
    (96, IconEntry { day_nerd: "󰖓", night_nerd: "󰖓", day_weather: "\u{e304}", night_weather: "\u{e321}", day_emoji: "⛈️",  night_emoji: "⛈️",  css_class: "stormy", description: "Thunderstorm with hail" }),
    (99, IconEntry { day_nerd: "󰖓", night_nerd: "󰖓", day_weather: "\u{e31d}", night_weather: "\u{e31d}", day_emoji: "⛈️",  night_emoji: "⛈️",  css_class: "stormy", description: "Thunderstorm with heavy hail" }),
];

fn find_entry(code: u8) -> &'static IconEntry {
    ICONS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, e)| e)
        .unwrap_or(&ICONS[0].1)
}

pub fn get_icon(code: u8, is_day: bool, icon_set: &IconSet) -> IconInfo {
    let entry = find_entry(code);
    let icon = match (icon_set, is_day) {
        (IconSet::Nerd, true) => entry.day_nerd,
        (IconSet::Nerd, false) => entry.night_nerd,
        (IconSet::Weather, true) => entry.day_weather,
        (IconSet::Weather, false) => entry.night_weather,
        (IconSet::Emoji, true) => entry.day_emoji,
        (IconSet::Emoji, false) => entry.night_emoji,
    };
    IconInfo {
        icon,
        css_class: entry.css_class,
        description: entry.description,
    }
}
