use serde::Serialize;
use unicode_width::UnicodeWidthStr;

use crate::api::{DailyForecast, HourlyForecast, WeatherData};
use crate::format::degrees_to_cardinal;
use crate::icons::{get_icon, IconSet};

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

// One Dark theme colors (matching claude-usage tooltip)
const C_BORDER: &str = "#61afef";
const C_TEXT: &str = "#abb2bf";
const C_DIM: &str = "#5c6370";
const C_ACCENT: &str = "#61afef";
const C_GREEN: &str = "#98c379";
const C_YELLOW: &str = "#e5c07b";
const C_ORANGE: &str = "#d19a66";

const MIN_WIDTH: usize = 20;

pub fn pango_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn fg(color: &str, text: &str) -> String {
    format!("<span foreground='{color}'>{text}</span>")
}

fn bold_fg(color: &str, text: &str) -> String {
    format!("<span font_weight='bold' foreground='{color}'>{text}</span>")
}

fn border_line(content: &str, width: usize) -> String {
    let pad = width.saturating_sub(visible_len(content));
    let right_pad = " ".repeat(pad);
    format!(
        "{} {content}{right_pad} {}",
        fg(C_BORDER, "│"),
        fg(C_BORDER, "│")
    )
}

fn separator(width: usize) -> String {
    border_line(&fg(C_DIM, &"─".repeat(width)), width)
}

fn empty_line(width: usize) -> String {
    border_line(&" ".repeat(width), width)
}

fn top_border(width: usize) -> String {
    fg(C_BORDER, &format!("╭{}╮", "─".repeat(width + 2)))
}

fn bottom_border(width: usize) -> String {
    fg(C_BORDER, &format!("╰{}╯", "─".repeat(width + 2)))
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

fn rain_color(pct: u8) -> &'static str {
    if pct >= 60 {
        C_ACCENT
    } else if pct >= 30 {
        C_YELLOW
    } else {
        C_GREEN
    }
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
    _icon_set: &IconSet,
    tooltip_format: &TooltipFormat,
    days: u8,
    hours: u8,
    unit_label: &str,
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
    let icon_info = get_icon(current.weather_code, current.is_day == 1, tooltip_icons);
    let speed_unit = if unit_label == "°F" { "mph" } else { "km/h" };

    // Phase 1: Build all content strings (without borders)
    let title_raw = pango_escape(city);
    let title_vlen = visible_len(&title_raw);

    let temp_line = format!(
        "  {} {}  {}  {} {}",
        fg(C_TEXT, &icon_info.icon),
        bold_fg(C_ACCENT, &format!("{temp}{unit_label}")),
        fg(C_DIM, icon_info.description),
        fg(C_DIM, "feels"),
        fg(C_DIM, &format!("{feels}{unit_label}"))
    );

    let stats1 = format!(
        "  {}  {}{}   {}  {} {} {}",
        fg(C_ACCENT, "󰖎"),
        fg(C_TEXT, &humidity.to_string()),
        fg(C_DIM, "%"),
        fg(C_ACCENT, "󰖝"),
        fg(C_TEXT, &wind.to_string()),
        fg(C_DIM, speed_unit),
        fg(C_DIM, wind_dir),
    );

    let stats2 = format!(
        "  {}  {} {}",
        fg(C_ACCENT, "󰖏"),
        fg(C_TEXT, &pressure.to_string()),
        fg(C_DIM, "hPa"),
    );

    let show_days = matches!(tooltip_format, TooltipFormat::Days | TooltipFormat::Both);
    let show_hours = matches!(tooltip_format, TooltipFormat::Hours | TooltipFormat::Both);

    let hourly_lines = if show_hours && hours > 0 {
        data.hourly
            .as_ref()
            .map(|h| build_hourly_lines(h, hours, tooltip_icons, unit_label))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let daily_lines = if show_days {
        build_daily_lines(&data.daily, days, tooltip_icons, unit_label)
    } else {
        Vec::new()
    };

    // Phase 2: Calculate dynamic width from content
    let mut measurable: Vec<&str> = vec![&temp_line, &stats1, &stats2];
    for line in &hourly_lines {
        measurable.push(line);
    }
    for line in &daily_lines {
        measurable.push(line);
    }
    let width = content_width(&measurable).max(title_vlen);

    // Phase 3: Build bordered output
    let mut lines = Vec::new();
    lines.push(top_border(width));

    let title_pango = bold_fg(C_ACCENT, &title_raw);
    let left_pad = (width.saturating_sub(title_vlen)) / 2;
    let padded_title = format!("{}{}", " ".repeat(left_pad), title_pango);
    lines.push(border_line(&padded_title, width));

    lines.push(separator(width));
    lines.push(border_line(&temp_line, width));
    lines.push(empty_line(width));
    lines.push(border_line(&stats1, width));
    lines.push(border_line(&stats2, width));

    if !hourly_lines.is_empty() {
        lines.push(separator(width));
        lines.push(border_line(&bold_fg(C_TEXT, "  Hourly"), width));
        lines.push(empty_line(width));
        for line in &hourly_lines {
            lines.push(border_line(line, width));
        }
    }

    if !daily_lines.is_empty() {
        lines.push(separator(width));
        lines.push(border_line(&bold_fg(C_TEXT, "  Forecast"), width));
        lines.push(empty_line(width));
        for line in &daily_lines {
            lines.push(border_line(line, width));
        }
    }

    lines.push(bottom_border(width));
    lines.join("\n")
}

fn build_daily_lines(
    daily: &DailyForecast,
    days: u8,
    icon_set: &IconSet,
    unit_label: &str,
) -> Vec<String> {
    let count = (days as usize).min(daily.time.len());
    let mut lines = Vec::new();

    for i in 0..count {
        let day_name = short_day_name(&daily.time[i]);
        let icon_info = get_icon(daily.weather_code[i], true, icon_set);
        let min = daily.temperature_2m_min[i].round() as i32;
        let max = daily.temperature_2m_max[i].round() as i32;
        let rain = daily
            .precipitation_probability_max
            .get(i)
            .copied()
            .unwrap_or(0);

        let rain_str = if rain > 0 {
            format!(
                "  {}  {}",
                fg(rain_color(rain), rain_icon(icon_set)),
                fg(rain_color(rain), &format!("{rain}%"))
            )
        } else {
            String::new()
        };

        let row = format!(
            "  {} {}  {} {}/{}{}{}",
            fg(C_TEXT, &icon_info.icon),
            bold_fg(C_TEXT, &format!("{:<6}", day_name)),
            fg(C_DIM, ""),
            fg(C_GREEN, &min.to_string()),
            fg(C_ORANGE, &max.to_string()),
            fg(C_DIM, unit_label),
            rain_str,
        );
        lines.push(row);
    }
    lines
}

fn build_hourly_lines(
    hourly: &HourlyForecast,
    hours: u8,
    icon_set: &IconSet,
    unit_label: &str,
) -> Vec<String> {
    let count = (hours as usize).min(hourly.time.len());
    let mut lines = Vec::new();

    for i in 0..count {
        let time_str = hourly
            .time
            .get(i)
            .map(|t| {
                t.split('T')
                    .nth(1)
                    .unwrap_or("??:??")
                    .get(..5)
                    .unwrap_or("??:??")
            })
            .unwrap_or("??:??");
        let icon_info = get_icon(hourly.weather_code[i], true, icon_set);
        let temp = hourly.temperature_2m[i].round() as i32;
        let rain = hourly
            .precipitation_probability
            .get(i)
            .copied()
            .unwrap_or(0);

        let rain_str = if rain > 0 {
            format!("  {}", fg(rain_color(rain), &format!("{rain}%")))
        } else {
            String::new()
        };

        let row = format!(
            "  {} {}  {} {}{}{}",
            fg(C_DIM, time_str),
            fg(C_TEXT, &icon_info.icon),
            fg(C_DIM, ""),
            fg(C_TEXT, &temp.to_string()),
            fg(C_DIM, unit_label),
            rain_str,
        );
        lines.push(row);
    }
    lines
}

fn short_day_name(date_str: &str) -> String {
    if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        date.format("%a %d").to_string()
    } else {
        date_str.to_string()
    }
}

pub fn error_output(message: &str) -> WaybarOutput {
    let header = bold_fg("#e06c75", "  meteobar error");
    let body = fg(C_DIM, &format!("  {}", pango_escape(message)));

    let width = content_width(&[&header, &body]);

    let lines = [
        top_border(width),
        border_line(&header, width),
        separator(width),
        border_line(&body, width),
        bottom_border(width),
    ];

    WaybarOutput {
        text: "?".to_string(),
        tooltip: lines.join("\n"),
        class: vec!["error".to_string()],
        alt: "error".to_string(),
    }
}
