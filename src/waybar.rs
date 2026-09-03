use chrono::Datelike;
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

use crate::api::WeatherData;
use crate::forecast::{self, DaySlot, HourSlot};
use crate::format::degrees_to_cardinal;
use crate::i18n::{self, Language};
use crate::icons::{get_icon, IconSet};
use crate::theme::{ramp_color, ThemeColors};

#[derive(Serialize)]
pub struct WaybarOutput {
    pub text: String,
    pub tooltip: String,
    pub class: Vec<String>,
    pub alt: String,
}

#[derive(Clone, clap::ValueEnum)]
pub enum TooltipFormat {
    Days,
    Hours,
    Both,
}

const MIN_WIDTH: usize = 20;

pub fn pango_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Which surface(s) `--no-color` applies to. `All` is what a bare
/// `--no-color` means, and what a non-empty `NO_COLOR` env var requests.
#[derive(Clone, Copy, PartialEq, Debug, clap::ValueEnum)]
pub enum NoColorScope {
    /// Both the bar text and the tooltip (default when no value is given)
    All,
    /// Only the bar text
    Bar,
    /// Only the tooltip
    Tooltip,
}

/// Resolved per-surface decision: `true` means that surface keeps its colors.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorChoice {
    pub bar: bool,
    pub tooltip: bool,
}

/// Combine the optional `--no-color[=WHAT]` flag with the `NO_COLOR` env var.
/// An explicit flag always wins — including `--no-color=bar`, which keeps the
/// tooltip colored even when NO_COLOR is set, because the flag is the more
/// specific instruction. NO_COLOR counts only when set to a non-empty value
/// (<https://no-color.org>).
pub fn resolve_color_choice(flag: Option<NoColorScope>, no_color_env: Option<&str>) -> ColorChoice {
    match flag {
        Some(NoColorScope::All) => ColorChoice {
            bar: false,
            tooltip: false,
        },
        Some(NoColorScope::Bar) => ColorChoice {
            bar: false,
            tooltip: true,
        },
        Some(NoColorScope::Tooltip) => ColorChoice {
            bar: true,
            tooltip: false,
        },
        None => {
            let disabled = no_color_env.is_some_and(|value| !value.is_empty());
            ColorChoice {
                bar: !disabled,
                tooltip: !disabled,
            }
        }
    }
}

/// Color emission for one surface. Monochrome drops color markup only —
/// glyphs, box drawing, alignment, and bold are all structural and stay.
#[derive(Clone, Copy)]
pub struct Paint {
    enabled: bool,
}

impl Paint {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    fn fg(&self, color: &str, text: &str) -> String {
        if self.enabled {
            format!("<span foreground='{color}'>{text}</span>")
        } else {
            text.to_string()
        }
    }

    fn bold_fg(&self, color: &str, text: &str) -> String {
        if self.enabled {
            format!("<span font_weight='bold' foreground='{color}'>{text}</span>")
        } else {
            // Weight is structure, not color: keep it in monochrome.
            format!("<span font_weight='bold'>{text}</span>")
        }
    }
}

fn visible_len(s: &str) -> usize {
    let mut plain = String::with_capacity(s.len());
    let mut in_tag = false;
    let mut in_entity = false;

    for ch in s.chars() {
        if in_tag {
            if ch == '>' {
                in_tag = false;
            }
            continue;
        }
        if in_entity {
            if ch == ';' {
                in_entity = false;
                plain.push('x'); // entity counts as 1 visible cell
            }
            continue;
        }
        match ch {
            '<' => in_tag = true,
            '&' => in_entity = true,
            _ => plain.push(ch),
        }
    }

    plain.width()
}

fn rain_icon(icon_set: &IconSet) -> &'static str {
    match icon_set {
        IconSet::Nerd => "󰖗",
        IconSet::Weather => "\u{e318}",
        IconSet::Emoji => "💧",
        IconSet::Fontawesome => "\u{f73d}",
    }
}

fn content_width(items: &[&str]) -> usize {
    items
        .iter()
        .map(|c| visible_len(c))
        .max()
        .unwrap_or(MIN_WIDTH)
        .max(MIN_WIDTH)
}

pub fn build_tooltip(
    city: &str,
    data: &WeatherData,
    tooltip_format: &TooltipFormat,
    days: u8,
    hours: u8,
    unit_label: &str,
    colors: &ThemeColors,
    last_fetched: Option<chrono::DateTime<chrono::Local>>,
    // Some(reason) when the payload is a stale fallback. The freshness footer
    // names it, exactly as the Omarchy shell panel does.
    stale_reason: Option<&str>,
    tooltip_font: &str,
    p: Paint,
    language: Language,
) -> String {
    let current = &data.current;
    let temp = current.temperature_2m.round() as i32;
    let feels = current
        .apparent_temperature
        .map(|v| v.round() as i32)
        .unwrap_or(temp);
    let humidity = current
        .relative_humidity_2m
        .map(|v| v.round() as i32)
        .unwrap_or(0);
    let wind = current.wind_speed_10m.unwrap_or(0.0).round() as i32;
    let wind_dir = degrees_to_cardinal(current.wind_direction_10m.unwrap_or(0.0));
    let pressure = current.pressure_msl.unwrap_or(0.0).round() as i32;
    // Tooltip always uses Nerd Font icons for consistent monospace alignment.
    // Pango renders emoji from a separate font with different glyph metrics,
    // breaking box-drawing border alignment. Nerd icons are part of the
    // monospace font and have consistent width. The --icons flag still
    // controls the bar text via the `text` field.
    let tooltip_icons = &IconSet::Nerd;
    let icon_info = get_icon(
        current.weather_code,
        current.is_day == 1,
        tooltip_icons,
        language,
    );
    let speed_unit = if unit_label == "°F" { "mph" } else { "km/h" };

    let (c_text, c_dim, c_accent) = (&colors.text, &colors.dim, &colors.accent);

    // Phase 1: Build all content strings (without borders)
    let title_raw = pango_escape(city);
    let title_vlen = visible_len(&title_raw);

    let temp_line = format!(
        "  {} {}  {}  {} {}",
        p.fg(c_text, &icon_info.icon),
        p.bold_fg(c_accent, &format!("{temp}{unit_label}")),
        p.fg(c_dim, icon_info.description),
        p.fg(c_dim, "feels"),
        p.fg(c_dim, &format!("{feels}{unit_label}"))
    );

    let stats1 = format!(
        "  {}  {}{}   {}  {} {} {}",
        p.fg(c_accent, "󰖎"),
        p.fg(c_text, &humidity.to_string()),
        p.fg(c_dim, "%"),
        p.fg(c_accent, "󰖝"),
        p.fg(c_text, &wind.to_string()),
        p.fg(c_dim, speed_unit),
        p.fg(c_dim, wind_dir),
    );

    let stats2 = format!(
        "  {}  {} {}",
        p.fg(c_accent, "󰖏"),
        p.fg(c_text, &pressure.to_string()),
        p.fg(c_dim, "hPa"),
    );

    let show_days = matches!(tooltip_format, TooltipFormat::Days | TooltipFormat::Both);
    let show_hours = matches!(tooltip_format, TooltipFormat::Hours | TooltipFormat::Both);

    // Selection is shared with the structured output (and so with the panel):
    // same entries, same daylight, on both surfaces.
    let hourly_lines = if show_hours && hours > 0 {
        build_hourly_lines(
            &forecast::upcoming_hours(data, hours),
            tooltip_icons,
            unit_label,
            colors,
            p,
            language,
        )
    } else {
        Vec::new()
    };

    let daily_lines = if show_days {
        build_daily_lines(
            &forecast::forecast_days(data, days),
            tooltip_icons,
            unit_label,
            colors,
            p,
            language,
        )
    } else {
        Vec::new()
    };

    let updated_time = last_fetched.unwrap_or_else(chrono::Local::now);
    let stale_suffix = match stale_reason {
        Some(reason) => format!(" · stale ({reason})"),
        None => String::new(),
    };
    let updated_line = format!(
        "  {}",
        p.fg(
            c_dim,
            &format!("󰅐  Updated {}{stale_suffix}", updated_time.format("%H:%M"))
        ),
    );

    // Phase 2: Calculate dynamic width from content
    let mut measurable: Vec<&str> = vec![&temp_line, &stats1, &stats2, &updated_line];
    for line in &hourly_lines {
        measurable.push(line);
    }
    for line in &daily_lines {
        measurable.push(line);
    }
    let width = content_width(&measurable).max(title_vlen);

    // Phase 3: Build output.
    //
    // One tooltip shape, pinned to a monospace font. The pin is not decoration:
    // the rules are made of ─, and in a proportional font that character is
    // nearly twice as wide as a letter — the tooltip then sizes itself to the
    // rules and grows a dead margin to the right of the text. Waybar draws the
    // tooltip in a GTK window that IGNORES font-family from CSS, so the markup
    // is the only place this can be said.
    let rule = || p.fg(c_dim, &"─".repeat(width));

    let mut lines = vec![
        p.bold_fg(c_accent, &title_raw),
        rule(),
        temp_line.clone(),
        stats1.clone(),
        stats2.clone(),
    ];

    if !hourly_lines.is_empty() {
        lines.push(rule());
        lines.push(p.bold_fg(c_text, "  Hourly"));
        for hl in &hourly_lines {
            lines.push(hl.clone());
        }
    }

    if !daily_lines.is_empty() {
        lines.push(rule());
        lines.push(p.bold_fg(c_text, "  Forecast"));
        for dl in &daily_lines {
            lines.push(dl.clone());
        }
    }

    lines.push(rule());
    lines.push(updated_line.clone());

    let body = lines.join("\n");
    format!(
        "<span font_family='{}'>{body}</span>",
        pango_escape(tooltip_font).replace('\'', "&apos;")
    )
}

fn build_daily_lines(
    days: &[DaySlot],
    icon_set: &IconSet,
    unit_label: &str,
    colors: &ThemeColors,
    p: Paint,
    language: Language,
) -> Vec<String> {
    let ramp = colors.precip_ramp();
    days.iter()
        .map(|slot| {
            let day_name = short_day_name(&slot.date, language);
            // The description is discarded below (only the glyph is used in
            // this line), but get_icon always needs a language now.
            let icon_info = get_icon(slot.weather_code, true, icon_set, language);
            let min = slot.temperature_min.round() as i32;
            let max = slot.temperature_max.round() as i32;
            let rain = slot.precip_pct.unwrap_or(0);

            let rain_str = if rain > 0 {
                let rc = ramp_color(&ramp, rain);
                format!(
                    "  {}  {}",
                    p.fg(&rc, rain_icon(icon_set)),
                    p.fg(&rc, &format!("{rain}%"))
                )
            } else {
                String::new()
            };

            format!(
                "  {} {}  {} {}/{}{}{}",
                p.fg(&colors.text, &icon_info.icon),
                p.bold_fg(&colors.text, &format!("{:<6}", day_name)),
                p.fg(&colors.dim, ""),
                p.fg(&colors.temp_cold(), &format!("{:>2}", min)),
                p.fg(&colors.temp_warm(), &format!("{:>2}", max)),
                p.fg(&colors.dim, unit_label),
                rain_str,
            )
        })
        .collect()
}

fn build_hourly_lines(
    hours: &[HourSlot],
    icon_set: &IconSet,
    unit_label: &str,
    colors: &ThemeColors,
    p: Paint,
    language: Language,
) -> Vec<String> {
    let ramp = colors.precip_ramp();
    hours
        .iter()
        .map(|slot| {
            let time_str = slot
                .time
                .split('T')
                .nth(1)
                .unwrap_or("??:??")
                .get(..5)
                .unwrap_or("??:??");
            // Daylight comes from the shared selection, so a night hour gets
            // its night glyph here exactly as it does in the panel. The
            // description is discarded below (only the glyph is used in this
            // line), but get_icon always needs a language now.
            let icon_info = get_icon(slot.weather_code, slot.is_day, icon_set, language);
            let temp = slot.temperature.round() as i32;
            let rain = slot.precip_pct.unwrap_or(0);

            let rain_str = if rain > 0 {
                format!("  {}", p.fg(&ramp_color(&ramp, rain), &format!("{rain}%")))
            } else {
                String::new()
            };

            format!(
                "  {} {}  {} {}{}{}",
                p.fg(&colors.dim, time_str),
                p.fg(&colors.text, &icon_info.icon),
                p.fg(&colors.dim, ""),
                p.fg(&colors.text, &temp.to_string()),
                p.fg(&colors.dim, unit_label),
                rain_str,
            )
        })
        .collect()
}

fn short_day_name(date_str: &str, language: Language) -> String {
    if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        // chrono's %a formats in the process locale, not --language/LANG
        // (that needs the unstable-locales feature); the weekday name is
        // built from the fixed table in i18n.rs instead.
        format!(
            "{} {}",
            i18n::short_weekday(date.weekday(), language),
            date.format("%d")
        )
    } else {
        date_str.to_string()
    }
}

pub fn error_output(message: &str, colors: &ThemeColors, p: Paint) -> WaybarOutput {
    let header = p.bold_fg(&colors.error, "  meteobar error");
    let body = p.fg(&colors.dim, &format!("  {}", pango_escape(message)));

    let width = content_width(&[&header, &body]);

    // The error fallback stays unpinned: it is one header plus one line, so
    // there is no column to keep and no rule long enough to overshoot.
    let lines = [header, p.fg(&colors.dim, &"─".repeat(width)), body];

    WaybarOutput {
        text: "?".to_string(),
        tooltip: lines.join("\n"),
        class: vec!["error".to_string()],
        alt: "error".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{CurrentWeather, DailyForecast, HourlyForecast, WeatherData};

    fn colored() -> ColorChoice {
        ColorChoice {
            bar: true,
            tooltip: true,
        }
    }

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
                time: vec!["2026-08-20".into()],
                weather_code: vec![61],
                temperature_2m_max: vec![15.0],
                temperature_2m_min: vec![8.0],
                sunrise: vec!["2026-08-20T07:30".into()],
                sunset: vec!["2026-08-20T18:15".into()],
                precipitation_probability_max: vec![80],
                wind_speed_10m_max: vec![20.0],
            },
            hourly: Some(HourlyForecast {
                time: vec!["2026-08-20T15:00".into()],
                temperature_2m: vec![12.0],
                weather_code: vec![61],
                precipitation_probability: vec![70],
            }),
            timezone: "UTC".into(),
            utc_offset_seconds: Some(0),
        }
    }

    fn tooltip_with(paint: Paint) -> String {
        build_tooltip(
            "Berlin",
            &fixture(),
            &TooltipFormat::Both,
            1,
            1,
            "°C",
            &ThemeColors::default(),
            None,
            None,
            "JetBrainsMono Nerd Font, JetBrainsMono Nerd Font Mono, monospace",
            paint,
            Language::En,
        )
    }

    // ---- flag / env resolution ------------------------------------------

    #[test]
    fn no_flag_and_no_env_keeps_every_surface_colored() {
        assert_eq!(resolve_color_choice(None, None), colored());
        // NO_COLOR set but empty does not count (no-color.org).
        assert_eq!(resolve_color_choice(None, Some("")), colored());
    }

    #[test]
    fn the_four_states_map_to_the_right_surfaces() {
        let cases = [
            (NoColorScope::All, false, false),
            (NoColorScope::Bar, false, true),
            (NoColorScope::Tooltip, true, false),
        ];
        for (scope, bar, tooltip) in cases {
            assert_eq!(
                resolve_color_choice(Some(scope), None),
                ColorChoice { bar, tooltip },
                "scope: {scope:?}"
            );
        }
    }

    #[test]
    fn no_color_env_behaves_like_no_color_all() {
        assert_eq!(
            resolve_color_choice(None, Some("1")),
            ColorChoice {
                bar: false,
                tooltip: false
            }
        );
        // Any non-empty value counts, not just "1".
        assert_eq!(
            resolve_color_choice(None, Some("anything")),
            ColorChoice {
                bar: false,
                tooltip: false
            }
        );
    }

    #[test]
    fn an_explicit_flag_beats_no_color_env() {
        // --no-color=bar with NO_COLOR set still colors the tooltip: the flag
        // is the more specific instruction.
        assert_eq!(
            resolve_color_choice(Some(NoColorScope::Bar), Some("1")),
            ColorChoice {
                bar: false,
                tooltip: true
            }
        );
        assert_eq!(
            resolve_color_choice(Some(NoColorScope::Tooltip), Some("1")),
            ColorChoice {
                bar: true,
                tooltip: false
            }
        );
    }

    // ---- what "plain" means ----------------------------------------------

    #[test]
    fn colored_tooltip_emits_color_markup() {
        let tooltip = tooltip_with(Paint::new(true));
        assert!(tooltip.contains("foreground='#"));
        assert!(tooltip.contains("font_weight='bold'"));
    }

    #[test]
    fn monochrome_tooltip_has_no_color_markup_but_keeps_structure() {
        let tooltip = tooltip_with(Paint::new(false));
        assert!(!tooltip.contains("foreground="), "color span leaked");
        assert!(!tooltip.contains('#'), "inline hex leaked");

        // Structure survives: glyphs, rules, bold, and the section labels.
        assert!(tooltip.contains('󰖗'), "weather glyph lost");
        assert!(tooltip.contains("font_weight='bold'"), "bold lost");
        assert!(tooltip.contains("Berlin"));
        assert!(tooltip.contains("Hourly") && tooltip.contains("Forecast"));
        assert!(tooltip.contains('─'), "rule lost");
        assert!(tooltip.contains("70%") && tooltip.contains("80%"));

        // The font pin is a font family, not a color, so monochrome keeps it —
        // and it is what keeps the rules the same width as the text.
        assert!(tooltip.contains("font_family="), "font pin lost");
    }

    #[test]
    fn monochrome_keeps_the_tooltip_aligned() {
        // Widths are measured on markup-stripped text, so dropping color must
        // not change the box geometry.
        let colored = tooltip_with(Paint::new(true));
        let mono = tooltip_with(Paint::new(false));
        let widths = |s: &str| -> Vec<usize> { s.lines().map(visible_len).collect() };
        assert_eq!(widths(&colored), widths(&mono));
    }

    #[test]
    fn german_language_localizes_the_tooltip() {
        let weather = fixture();
        let tooltip = build_tooltip(
            "Berlin",
            &weather,
            &TooltipFormat::Both,
            1,
            1,
            "°C",
            &ThemeColors::default(),
            None,
            None,
            "JetBrainsMono Nerd Font, JetBrainsMono Nerd Font Mono, monospace",
            Paint::new(false),
            Language::De,
        );
        // current.weather_code is 3 ("Overcast" / "Bedeckt" in the fixture).
        assert!(
            tooltip.contains("Bedeckt"),
            "expected the German condition text: {tooltip:?}"
        );
        assert!(
            !tooltip.contains("Overcast"),
            "English condition text leaked: {tooltip:?}"
        );
        // The daily line's weekday abbreviation is German too.
        let date = chrono::NaiveDate::parse_from_str(&weather.daily.time[0], "%Y-%m-%d").unwrap();
        let de_weekday = i18n::short_weekday(date.weekday(), Language::De);
        assert!(
            tooltip.contains(de_weekday),
            "expected the German weekday abbreviation {de_weekday:?}: {tooltip:?}"
        );
    }

    // ---- one selection, two frontends -------------------------------------

    #[test]
    fn both_frontends_select_the_same_hours_with_the_same_daylight() {
        let weather = fixture();
        // What the panel is given.
        let structured = crate::structured::build(
            &weather,
            "Berlin",
            "Berlin",
            &crate::structured::Request {
                days: 1,
                hours: 3,
                icon_set: &IconSet::Nerd,
                imperial: false,
                language: Language::En,
            },
            crate::structured::CacheInfo::empty(),
            &ThemeColors::default(),
        );
        // What the tooltip renders.
        let slots = forecast::upcoming_hours(&weather, 3);
        let lines = build_hourly_lines(
            &slots,
            &IconSet::Nerd,
            "°C",
            &ThemeColors::default(),
            Paint::new(false),
            Language::En,
        );

        assert_eq!(structured.hourly.len(), lines.len());
        for (entry, line) in structured.hourly.iter().zip(lines.iter()) {
            // Same hour...
            assert!(
                line.contains(&entry.time[11..16]),
                "tooltip line {line:?} is missing hour {}",
                &entry.time[11..16]
            );
            // ...and the same glyph, which is where day/night used to diverge.
            assert!(
                line.contains(&entry.icon),
                "tooltip line {line:?} is missing glyph {}",
                entry.icon
            );
        }
    }

    #[test]
    fn a_night_hour_gets_its_night_glyph_in_the_tooltip() {
        let mut weather = fixture();
        // Push "now" past sunset so the first upcoming hour is a night hour.
        weather.current.time = Some("2026-08-20T19:00".into());
        if let Some(hourly) = weather.hourly.as_mut() {
            hourly.time = vec!["2026-08-20T19:00".into()];
            hourly.temperature_2m = vec![11.0];
            hourly.weather_code = vec![0]; // clear: day 󰖙 vs night 󰖔
            hourly.precipitation_probability = vec![0];
        }
        let slots = forecast::upcoming_hours(&weather, 1);
        assert!(!slots[0].is_day);
        let lines = build_hourly_lines(
            &slots,
            &IconSet::Nerd,
            "°C",
            &ThemeColors::default(),
            Paint::new(false),
            Language::En,
        );
        assert!(
            lines[0].contains('󰖔'),
            "expected the night glyph: {lines:?}"
        );
        assert!(!lines[0].contains('󰖙'));
    }

    #[test]
    fn an_inconsistent_cached_payload_renders_instead_of_panicking() {
        // Unequal parallel arrays deserialise fine from a cache written by an
        // older version or a truncated response; the tooltip must survive them.
        let mut weather = fixture();
        weather.daily.temperature_2m_min.clear();
        weather.daily.sunset.clear();
        if let Some(hourly) = weather.hourly.as_mut() {
            hourly.temperature_2m.clear();
        }
        let tooltip = tooltip_with(Paint::new(true));
        assert!(!tooltip.is_empty());

        let rendered = build_tooltip(
            "Berlin",
            &weather,
            &TooltipFormat::Both,
            7,
            24,
            "°C",
            &ThemeColors::default(),
            None,
            None,
            "JetBrainsMono Nerd Font, JetBrainsMono Nerd Font Mono, monospace",
            Paint::new(true),
            Language::En,
        );
        assert!(rendered.contains("Berlin"));
    }

    #[test]
    fn precipitation_uses_the_published_ramp_on_both_surfaces() {
        let colors = ThemeColors::default();
        let ramp = colors.precip_ramp();
        // The tooltip paints exactly what the ramp resolves for that value.
        let slots = vec![HourSlot {
            time: "2026-08-20T15:00".into(),
            temperature: 12.0,
            weather_code: 61,
            is_day: true,
            precip_pct: Some(45),
        }];
        let line = &build_hourly_lines(
            &slots,
            &IconSet::Nerd,
            "°C",
            &colors,
            Paint::new(true),
            Language::En,
        )[0];
        assert!(line.contains(&ramp_color(&ramp, 45)));
    }

    #[test]
    fn error_output_honors_monochrome_and_keeps_its_class() {
        let colors = ThemeColors::default();
        let colored = error_output("boom", &colors, Paint::new(true));
        let mono = error_output("boom", &colors, Paint::new(false));

        assert!(colored.tooltip.contains("foreground='#"));
        assert!(!mono.tooltip.contains("foreground="));
        assert!(!mono.tooltip.contains('#'));
        assert!(mono.tooltip.contains("meteobar error") && mono.tooltip.contains("boom"));

        // The machine contract is untouched in both cases.
        assert_eq!(colored.class, vec!["error".to_string()]);
        assert_eq!(mono.class, colored.class);
        assert_eq!(mono.alt, colored.alt);
        assert_eq!(mono.text, colored.text);
    }
}
