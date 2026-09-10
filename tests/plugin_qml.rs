// Static contract of the not-installed discrimination and the copy-install
// button in the Omarchy panel. The QML has no test runner; these substring
// checks pin the load-bearing lines. Expectations are written out by hand.
static PANEL: &str = include_str!("../omarchy/Panel.qml");

#[test]
fn a_run_is_marked_not_installed_only_without_an_exit_signal() {
    assert!(PANEL.contains("sawExit = false"), "startRun must reset sawExit");
    assert!(PANEL.contains("root.sawExit = true"), "onExited must set sawExit");
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
