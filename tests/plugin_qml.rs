// Static contract of the not-installed discrimination and the copy-install
// button in the Omarchy panel. The QML has no test runner; these substring
// checks pin the load-bearing lines. Expectations are written out by hand.
static PANEL: &str = include_str!("../omarchy/Panel.qml");
static MANIFEST: &str = include_str!("../manifest.json");

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
