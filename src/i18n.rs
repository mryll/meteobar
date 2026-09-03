//! Condition-text and weekday translations. A plain `match` over a fixed,
//! small string set — no i18n crate, no locale data. New languages are added
//! by extending `Language`, `condition_text`, and `short_weekday`; the
//! `every_condition_string_is_translated` test below fails loudly if a
//! variant is missing a case for any of the 28 WMO codes `icons.rs` knows.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    En,
    De,
}

impl Language {
    /// `--language` wins outright. Otherwise `LC_MESSAGES`, then `LANG`, are
    /// read left to right and the first one that resolves to a known,
    /// non-English language wins — an explicit `en*` value short-circuits
    /// the search so it is not shadowed by a later variable. Nothing
    /// recognized falls back to English, same as an unset environment.
    pub fn resolve(flag: Option<&str>, lc_messages: Option<&str>, lang: Option<&str>) -> Language {
        if let Some(code) = flag {
            return Self::from_code(code);
        }
        for candidate in [lc_messages, lang].into_iter().flatten() {
            let primary = Self::primary_subtag(candidate);
            if primary.eq_ignore_ascii_case("en") {
                return Language::En;
            }
            let resolved = Self::from_code(candidate);
            if resolved != Language::En {
                return resolved;
            }
        }
        Language::En
    }

    /// POSIX locale strings look like `de_DE.UTF-8` or `de_DE@euro`; only the
    /// language subtag before the first `_`, `.`, or `@` is meaningful here.
    fn primary_subtag(code: &str) -> &str {
        code.split(['_', '.', '@', '-']).next().unwrap_or(code)
    }

    fn from_code(code: &str) -> Language {
        match Self::primary_subtag(code).to_ascii_lowercase().as_str() {
            "de" => Language::De,
            _ => Language::En,
        }
    }
}

/// Translate one of the 28 fixed WMO condition strings `icons.rs` owns.
/// `english` must be one of those literals; anything else (there is no other
/// caller) is returned unchanged, same as an untranslated language.
pub fn condition_text(english: &'static str, language: Language) -> &'static str {
    match language {
        Language::En => english,
        Language::De => de_condition_text(english),
    }
}

fn de_condition_text(english: &'static str) -> &'static str {
    match english {
        "Clear sky" => "Klarer Himmel",
        "Mainly clear" => "Überwiegend klar",
        "Partly cloudy" => "Teilweise bewölkt",
        "Overcast" => "Bedeckt",
        "Fog" => "Nebel",
        "Rime fog" => "Raureifnebel",
        "Light drizzle" => "Leichter Nieselregen",
        "Moderate drizzle" => "Mäßiger Nieselregen",
        "Dense drizzle" => "Starker Nieselregen",
        "Freezing drizzle" => "Gefrierender Nieselregen",
        "Dense freezing drizzle" => "Starker gefrierender Nieselregen",
        "Slight rain" => "Leichter Regen",
        "Moderate rain" => "Mäßiger Regen",
        "Heavy rain" => "Starker Regen",
        "Freezing rain" => "Gefrierender Regen",
        "Heavy freezing rain" => "Starker gefrierender Regen",
        "Slight snow" => "Leichter Schneefall",
        "Moderate snow" => "Mäßiger Schneefall",
        "Heavy snow" => "Starker Schneefall",
        "Snow grains" => "Schneegriesel",
        "Slight rain showers" => "Leichte Regenschauer",
        "Moderate rain showers" => "Mäßige Regenschauer",
        "Violent rain showers" => "Heftige Regenschauer",
        "Slight snow showers" => "Leichte Schneeschauer",
        "Heavy snow showers" => "Starke Schneeschauer",
        "Thunderstorm" => "Gewitter",
        "Thunderstorm with hail" => "Gewitter mit Hagel",
        "Thunderstorm with heavy hail" => "Gewitter mit starkem Hagel",
        other => other,
    }
}

/// Abbreviated weekday name for the daily forecast (`waybar.rs::short_day_name`).
/// Not chrono's `%a`: that formats in the process locale regardless of
/// `--language`/`LANG` without the (unstable) `chrono/unstable-locales`
/// feature, which is more machinery than 7 fixed abbreviations need.
pub fn short_weekday(weekday: chrono::Weekday, language: Language) -> &'static str {
    use chrono::Weekday::*;
    match language {
        Language::En => match weekday {
            Mon => "Mon",
            Tue => "Tue",
            Wed => "Wed",
            Thu => "Thu",
            Fri => "Fri",
            Sat => "Sat",
            Sun => "Sun",
        },
        Language::De => match weekday {
            Mon => "Mo",
            Tue => "Di",
            Wed => "Mi",
            Thu => "Do",
            Fri => "Fr",
            Sat => "Sa",
            Sun => "So",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_condition_string_is_translated() {
        for english in crate::icons::all_descriptions() {
            let de = de_condition_text(english);
            assert_ne!(
                de, english,
                "no German translation for condition string {english:?}"
            );
        }
    }

    #[test]
    fn english_is_the_identity_translation() {
        for english in crate::icons::all_descriptions() {
            assert_eq!(condition_text(english, Language::En), english);
        }
    }

    #[test]
    fn spot_check_a_few_german_conditions() {
        assert_eq!(condition_text("Clear sky", Language::De), "Klarer Himmel");
        assert_eq!(condition_text("Overcast", Language::De), "Bedeckt");
        assert_eq!(
            condition_text("Thunderstorm with heavy hail", Language::De),
            "Gewitter mit starkem Hagel"
        );
    }

    #[test]
    fn explicit_flag_wins_over_environment() {
        assert_eq!(
            Language::resolve(Some("de"), Some("en_US"), None),
            Language::De
        );
        assert_eq!(
            Language::resolve(Some("en"), Some("de_DE"), None),
            Language::En
        );
    }

    #[test]
    fn lc_messages_beats_lang() {
        assert_eq!(
            Language::resolve(None, Some("de_DE.UTF-8"), Some("en_US.UTF-8")),
            Language::De
        );
    }

    #[test]
    fn lang_is_the_fallback_when_lc_messages_is_unset() {
        assert_eq!(Language::resolve(None, None, Some("de_AT")), Language::De);
    }

    #[test]
    fn an_explicit_english_locale_short_circuits_a_later_german_one() {
        // LC_MESSAGES=en_US must win outright, not be treated as "unresolved"
        // and fall through to LANG=de_DE.
        assert_eq!(
            Language::resolve(None, Some("en_US.UTF-8"), Some("de_DE.UTF-8")),
            Language::En
        );
    }

    #[test]
    fn unknown_or_missing_locale_defaults_to_english() {
        assert_eq!(Language::resolve(None, None, None), Language::En);
        assert_eq!(
            Language::resolve(None, Some("fr_FR.UTF-8"), None),
            Language::En
        );
        assert_eq!(Language::resolve(None, Some(""), None), Language::En);
    }

    #[test]
    fn short_weekday_covers_every_day_in_both_languages() {
        use chrono::Weekday::*;
        for day in [Mon, Tue, Wed, Thu, Fri, Sat, Sun] {
            assert!(!short_weekday(day, Language::En).is_empty());
            assert!(!short_weekday(day, Language::De).is_empty());
        }
        assert_eq!(short_weekday(Mon, Language::De), "Mo");
    }
}
