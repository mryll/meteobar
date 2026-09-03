# AGENTS.md — meteobar

Weather widget for Waybar (Rust). Shared family contract: the binary always
exits 0 with valid Waybar JSON, on every path, errors included.

- Build: `make build`; install: `make install PREFIX=~/.local`. Lint: `cargo clippy`; format: `cargo fmt`.
- Tests: `cargo test` (unit tests in `src/structured.rs` and `src/cache.rs`).

## Non-Obvious Rules

- **The CLI always runs through `/bin/sh -c 'exec "$0" "$@"'`, never direct.** A nonexistent binary handed to Quickshell 0.3.1 can abort the whole shell inside the failed start (claudebar#6), before any QML signal fires. The failed-start discriminator is `!sawExit || exitCode === 126 || exitCode === 127` on empty output; any other exited-empty run is an operational failure, never "not installed".
- **`installCmd` is the one constant** — the message shows it and the button copies it (`Util.execArgv(["wl-copy", ...])`, no shell line, no trailing newline). The button gates on `notInstalled`, never on error text. Pinned in `tests/plugin_qml.rs`.

- Output must be valid Waybar JSON (`{"text": ..., "tooltip": ..., "class": ..., "alt": ...}`)
- `--output json` is a second, structured output mode (raw data, no Pango; consumed by the Omarchy shell plugin in `omarchy/`) — it must always exit 0 with valid JSON, errors go in the `error: {message}` field
- Forecast selection lives ONLY in `forecast.rs` (`upcoming_hours`, `forecast_days`); both the Waybar tooltip and the structured JSON render those slots, so "next N hours" and day/night are identical on both surfaces. Never index the API's parallel arrays — they can disagree in length in a cached payload and that used to panic the Waybar path; zip them instead
- `--language` (`en`/`de`, resolved from the flag, then `LC_MESSAGES`, then `LANG`, else English — see `i18n::Language::resolve`) is applied once, in `icons::get_icon`/`get_icon_plain`, through the fixed match table in `i18n.rs`; never re-translate per output surface — this keeps the Waybar tooltip and `--output json` (and therefore the Omarchy panel) in sync the same way `icon_set` already does. `language` is deliberately absent from `manifest.json`; the panel threads it through `setting("language", "")` → `buildCmd()`, the same path `units` takes. `i18n::every_condition_string_is_translated` walks `icons::all_descriptions()`, so a WMO code added without a German entry fails the test suite instead of shipping untranslated
- The core publishes `palette` in the structured JSON (text/dim/accent, temp_cold/temp_warm, and `precip_ramp` as `{pct, color}` stops). QML consumes it via `rampColor()`, which mirrors `theme.rs::ramp_color`. Thresholds and colors live in the core; the panel must not re-derive them
- The response cache is request-keyed (`weather-<hash>.json`, hash over location input + units + days + hours), so different flag sets never cross-serve payloads
- Tooltip uses Pango markup for colors and formatting — escape user-facing strings
- `--no-color[=all|bar|tooltip]` (plus `NO_COLOR`, which the explicit flag overrides) resolves to a `ColorChoice` in `waybar.rs`; all color markup goes through `Paint`, which emits nothing when disabled. Monochrome drops color ONLY — glyphs, box drawing, bold, alignment and the `class`/`alt` fields all stay, and `--output json` is byte-identical either way
- Argument errors go through `report_cli_error` (not clap's default exit 2): they emit a waybar error object, or a structured error when the raw argv asked for `--output json`, always exit 0. `--help`/`--version` print normally
- Tooltip always uses Nerd Font icons regardless of `--icons` setting (for monospace alignment)
- Response cache uses flock-based file locking (`cache.rs`) with 60s TTL
- Theme resolution chain (`theme.rs::load_from`): Omarchy theme at `$XDG_STATE_HOME/omarchy/current/theme/colors.toml` (default `~/.local/state/...`, legacy `~/.config/omarchy/...` as fallback) → pywal cache at `$XDG_CACHE_HOME/wal/colors.json` (default `~/.cache/...`) → built-in One Dark defaults. pywal is consulted only when no Omarchy theme file was found
- Omarchy theme keys: prefer the semantic names current themes ship (`accent`, `foreground`, `background`, `red`, `green`, `yellow`, `orange`); `color1/2/3` are the legacy fallback
- pywal mapping: `special.foreground`/`special.background` → text/dim blend, `color4` (fallback `special.cursor`) → accent, `color2` → green, `color3` → yellow, `color1` → error. pywal has no orange slot — synthesize it as the yellow⊕red midpoint, never alias it to red (that flattens gauges across the widget family)
- A missing or non-hex value must only affect its own field — never sink the whole theme load. Values are validated with the strict `is_hex_color` (`#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa`) so malformed colors can't reach Pango markup
- Theme loading must never panic or error: absent/unreadable/invalid files degrade silently to the next tier, preserving exit 0
- Font Awesome icons are wrapped in Pango markup (`<span>`) for correct rendering in Waybar

