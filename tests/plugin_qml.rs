// Static contract of the not-installed discrimination and the copy-install
// button in the Omarchy panel. The QML has no test runner; these substring
// checks pin the load-bearing lines. Expectations are written out by hand.
static PANEL: &str = include_str!("../omarchy/Panel.qml");
static MANIFEST: &str = include_str!("../manifest.json");
static BAR_WIDGET: &str = include_str!("../omarchy/BarWidget.qml");

#[test]
fn a_run_is_marked_not_installed_only_without_an_exit_signal() {
    assert!(
        PANEL.contains("sawExit = false"),
        "startRun must reset sawExit"
    );
    assert!(
        PANEL.contains("root.sawExit = true"),
        "onExited must set sawExit"
    );
    assert!(
        PANEL.contains("} else if (!sawExit || exitCode === 126 || exitCode === 127) {"),
        "gate on !sawExit or sh's exec-failure codes"
    );
    assert!(
        PANEL.contains(r#"["/bin/sh", "-c", 'exec "$0" "$@"'].concat(cmd)"#),
        "the command must be wrapped in sh (claudebar#6: a missing binary can abort the shell)"
    );
    assert!(
        !PANEL.contains("command = cmd"),
        "the direct (unwrapped) command assignment is banned"
    );
    assert!(
        PANEL.contains("tripwireFired = false") && PANEL.contains("if (tripwireFired) {"),
        "the empty branch gates on the per-run tripwire flag, not stale text"
    );
    assert!(
        PANEL.contains("produced no output (exit "),
        "a run that exited empty is an operational error"
    );
}

#[test]
fn the_install_command_is_one_constant_copied_as_argv() {
    assert_eq!(
        PANEL.matches("yay -S meteobar-bin").count(),
        1,
        "installCmd literal must appear exactly once"
    );
    assert!(
        PANEL.contains(r#"Util.execArgv(["wl-copy", root.installCmd])"#),
        "copy must go through execArgv argv-style"
    );
    assert!(!PANEL.contains("bash -c"), "no shell line around wl-copy");
    assert!(
        PANEL.contains("visible: root.notInstalled"),
        "the button gates on notInstalled, not on error text"
    );
}

#[test]
fn exact_coordinates_are_exposed_and_forwarded_to_meteobar() {
    for key in ["lat", "lon", "cityName"] {
        assert!(
            MANIFEST.contains(&format!(r#""key": "{key}""#)),
            "manifest must expose the {key} setting"
        );
    }

    assert!(
        PANEL.contains(
            r#"readonly property bool hasExactCoordinates: latSetting !== "" && lonSetting !== """#
        ),
        "exact coordinates require both latitude and longitude"
    );
    // The pairs are pinned, not just the flag names: a test that only looks
    // for `--lat` would still pass with the two values swapped.
    assert!(
        PANEL.contains("cmd.push(\"--lat\")\n      cmd.push(latSetting)"),
        "adapter must forward --lat followed by latSetting"
    );
    assert!(
        PANEL.contains("cmd.push(\"--lon\")\n      cmd.push(lonSetting)"),
        "adapter must forward --lon followed by lonSetting"
    );
    // Joined with `=`: as a separate argv element a name that starts with a
    // hyphen reads as an option and clap rejects it.
    assert!(
        PANEL.contains(r#"cmd.push("--city-name=" + cityNameSetting)"#),
        "adapter must forward --city-name joined with = to its value"
    );
    for setting in ["latSetting", "lonSetting", "cityNameSetting"] {
        assert!(
            fetch_key_line().contains(setting),
            "{setting} must be part of fetchKey so a change refetches"
        );
    }
    assert!(
        PANEL.contains(r#"} else if (locationSetting !== "") {"#),
        "the location setting remains the fallback when coordinates are incomplete"
    );
}

fn fetch_key_line() -> &'static str {
    PANEL
        .lines()
        .find(|l| l.contains("readonly property string fetchKey:"))
        .expect("Panel.qml declares fetchKey")
}

#[test]
fn a_picked_suggestion_commits_every_qualifier_it_carries() {
    // The stored value is re-geocoded on every fetch, so a bare name can
    // resolve somewhere other than the row that was clicked: "Bally" alone
    // matches towns in both Pennsylvania and California.
    assert!(
        PANEL.contains("commitName"),
        "a suggestion must carry a qualified commit form, not just its name"
    );
    assert!(
        PANEL.contains("r.admin1") && PANEL.contains("r.country_code"),
        "the commit form must carry the region and country-code qualifiers"
    );
}

#[test]
fn saving_a_location_carries_the_other_settings_across() {
    // updateEntryInline REPLACES this widget's shell.json entry rather than
    // merging into it, so anything not copied first is silently dropped --
    // units, iconSet and colorMode would reset on every location edit.
    assert!(
        PANEL
            .contains(r#"for (var k in root.settings) if (k !== "id") next[k] = root.settings[k]"#),
        "every existing setting must be copied before location is written back"
    );
    // Coordinates take precedence over `location` in buildCmd, so a place
    // picked in the panel must drop them or the pick would change nothing.
    for key in ["lat", "lon", "cityName"] {
        assert!(
            PANEL.contains(&format!("delete next.{key}")),
            "a location edit must drop the stored {key}"
        );
    }
    assert!(
        PANEL.contains("if (v === root.locationSetting && !root.hasExactCoordinates) {"),
        "the unchanged-text shortcut must not skip the write while coordinates exist"
    );
}

#[test]
fn the_location_is_persisted_through_the_bar_facade() {
    // The host injects `bar`, `moduleName` and `settings` into a widget and
    // nothing else (Bar.qml injectProps); the plugin shell API rides on
    // bar.shell, which is how the first-party clock persists its format. A
    // `shell` property of our own is never filled, so a panel that reads it
    // would silently take the CLI fallback on every edit.
    assert!(
        PANEL.contains("var api = root.bar ? root.bar.shell : null"),
        "the persist path must read the facade from bar.shell"
    );
    assert!(
        PANEL.contains(r#"api.updateEntryInline("mryll.meteobar", next)"#),
        "the facade must be asked to update this widget's own entry"
    );
    assert!(
        !PANEL.contains("property var shell") && !BAR_WIDGET.contains("property var shell"),
        "no self-declared shell property: the host never fills one"
    );
}

#[test]
fn the_geocoder_curl_runs_through_sh_and_there_is_no_cli_write_fallback() {
    // claudebar#6: a nonexistent binary handed to Quickshell can abort the
    // whole shell inside the failed start. sh always starts.
    assert!(
        PANEL.contains(r#"geocodeProc.command = ["/bin/sh", "-c", 'exec "$0" "$@"',"#),
        "the geocoder curl must be wrapped in sh"
    );
    assert!(
        !PANEL.contains(r#"command = ["curl""#),
        "no direct (unwrapped) curl command assignment"
    );
    // `omarchy bar set` rides the same shell IPC as the facade, so it is not
    // an independent path; a fire-and-forget process would lose the edit
    // silently. A refused write keeps the field open instead.
    assert!(
        !PANEL.contains(r#""omarchy", "bar", "set""#),
        "no CLI fallback for the location write"
    );
    // One contiguous block: the refusal returns BEFORE the field closes. Two
    // independent fragments would still match with the early return gone.
    assert!(
        PANEL.contains(
            "    if (!wrote) {\n      console.warn(\"meteobar: the shell did not accept the location write; the field stays open\")\n      return\n    }\n\n    root.editingLocation = false\n"
        ),
        "the field closes only after the facade accepted the write"
    );
}

#[test]
fn only_one_geocode_request_runs_at_a_time() {
    // The debounce and the deferred re-ask can both fire while a run is
    // active; Quickshell would queue a repeat run and the old answer could
    // be tagged with the new query, which Enter then trusts.
    assert!(
        PANEL.contains(
            "    if (geocodeProc.running || !root.geocodeStreamDone) return\n    root.geocodeStreamDone = false\n    root.geocodeActiveQuery = root.geocodePendingQuery\n"
        ),
        "startGeocode must refuse to move the active query under a running request"
    );
    assert!(
        PANEL.contains("onRunningChanged: if (!running) root.geocodeMaybeContinue()")
            && PANEL.contains("root.geocodeStreamDone = true"),
        "the re-ask waits for both the exit and the end of the stream"
    );
    assert!(
        PANEL.contains(
            "if (!root.editingLocation) return\n      locationField.text = root.locationSetting"
        ),
        "the deferred open must stand down after a cancel"
    );
}

#[test]
fn enter_never_takes_a_row_for_text_the_user_no_longer_has() {
    // Clear the field and press Enter inside the debounce: the old rows are
    // still listed, and the result must be automatic detection, not the
    // first row of the previous query.
    assert!(
        PANEL.contains(
            r#"if (typed !== "" && root.locationSuggestions.length > 0 && root.suggestionsQuery === typed)"#
        ),
        "Enter must check the rows answer the current text"
    );
    assert!(
        PANEL.contains("root.suggestionsQuery = root.geocodeActiveQuery"),
        "a finished request must tag its rows with the query they answer"
    );
    assert!(
        PANEL.contains("if (v === root.locationSetting) {"),
        "an unchanged commit must not be written back"
    );
}

#[test]
fn a_closed_panel_ends_the_edit_and_the_parser_trusts_no_length() {
    assert!(
        PANEL.contains("else if (editingLocation) cancelEditingLocation()"),
        "a close from anywhere must end the edit"
    );
    assert!(
        PANEL.contains("if (!Array.isArray(results)) return []"),
        "the geocoder body must be a real array"
    );
    assert!(
        PANEL.contains("i < Math.min(results.length, 5)"),
        "no more rows than the URL asked for"
    );
    assert!(
        PANEL.contains("if (!root.editingLocation) return")
            && PANEL.contains("if (!root.hasData && root.errorMessage === \"\") return"),
        "an answer after the field closed is dropped; no edit while loading"
    );
}

#[test]
fn the_panel_shortcuts_stand_down_while_the_location_is_edited() {
    // The panel binds bare single-key shortcuts, so an unblocked key catcher
    // fires "r" (refresh) in the middle of typing "Derry".
    assert!(
        PANEL.contains("blocked: root.editingLocation"),
        "the key catcher must stand down while the field holds the keys"
    );
}
