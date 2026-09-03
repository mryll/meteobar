pragma ComponentBehavior: Bound
import QtQuick
import Quickshell.Io
import qs.Commons
import qs.Ui

// Forecast panel. Owns the meteobar process and the refresh timer, so the bar
// label stays current even while the panel is closed. All data comes from
// `meteobar --output json` (structured, no markup); this file only renders.
Panel {
  id: root
  moduleName: "mryll.meteobar"
  ipcTarget: "mryll.meteobar"
  manageIpc: false

  property var anchorItem: null

  // The bar tracks the widget mounted in its slot — BarWidget.qml — not this
  // nested panel, so popout coordination has to identify as that widget.
  property var hostWidget: null
  readonly property var barIdentity: hostWidget || root

  // ---- theme handles (guarded: `bar` is injected after load) --------------
  // The panel draws on the POPUP CARD, so it takes the popup surface's text
  // token — not the bar's. bar.foreground is chosen against the bar, which on a
  // transparent bar means "against the wallpaper"; that is the wrong contrast
  // reference for a card, and a theme that defines popups.text separately would
  // be ignored outright. (printbar already did this; the rest of the family now
  // agrees.)
  readonly property color fg: Color.popups.text

  // ---- Hero-data scale.
  //
  // meteobar is the family's one DATA hero: the condition glyph and the
  // temperature ARE the reading, not a brand mark, so they sit above the
  // identity scale every other panel uses (Style.font.display, 24). They were
  // hardcoded at 64 and 56 px, which ignored the user's font scale entirely —
  // a bar configured at 14pt kept a 64px hero. Deriving them from `display`
  // keeps the proportion and makes them scale with everything else.

  // ---- Freshness suffix tint, shared by the whole family. The timestamp is
  // ALWAYS dim ("when is this from" is information, not a warning); only the
  // "· stale (…)" suffix carries a muted warning tone, never full urgent.
  readonly property color freshnessWarn: panelColored ? lerpColor(Qt.darker(fg, 1.55), urgentColor, 0.4) : Qt.darker(fg, 1.55)

  readonly property int heroGlyphSize: Math.round(Style.font.display * 2.67)
  readonly property int heroValueSize: Math.round(Style.font.display * 2.33)
  readonly property color urgentColor: bar ? bar.urgent : Color.urgent
  readonly property string fontFam: bar ? bar.fontFamily : Style.font.family

  // ---- data ----------------------------------------------------------------
  // Last parsed `meteobar --output json` payload. Kept on failure so stale
  // data stays visible (meteobar itself also falls back to its cache).
  property var report: null
  property string errorMessage: ""

  readonly property bool hasData: !!(report && report.current)
  readonly property var current: hasData ? report.current : null
  readonly property var hourlyEntries: (report && report.hourly) ? report.hourly : []
  readonly property var dailyEntries: (report && report.daily) ? report.daily : []
  readonly property string locationName: (report && report.location) ? String(report.location) : ""
  readonly property string tempUnit: (report && report.units && report.units.temperature) ? report.units.temperature : "°C"
  readonly property string windUnit: (report && report.units && report.units.wind_speed) ? report.units.wind_speed : "km/h"
  readonly property var cacheInfo: (report && report.cache) ? report.cache : null
  readonly property string updatedText: (cacheInfo && cacheInfo.fetched_at) ? String(cacheInfo.fetched_at).slice(11, 16) : ""
  readonly property bool stale: pluginStale || !!(cacheInfo && cacheInfo.stale)
  readonly property string staleReason: pluginStale ? "refresh failed"
    : ((cacheInfo && cacheInfo.stale_reason) ? String(cacheInfo.stale_reason) : "unknown")

  // Bar label pieces, read by BarWidget.qml.
  readonly property string barIcon: current ? String(current.icon) : "󰖐"
  // The family's stale mark (nf-fa-pause), appended exactly the way the sibling
  // CLIs append theirs to the Waybar bar text: the reading keeps its own color
  // and staleness gets its own glyph, never a tint.
  readonly property string staleMark: stale ? "  " : ""
  readonly property string barText: (current ? barIcon + " " + Math.round(current.temperature) + "°" : barIcon) + staleMark

  // Overall min/max across the daily entries, for the per-day range bars.
  readonly property var tempRange: {
    var lo = Infinity, hi = -Infinity
    for (var i = 0; i < dailyEntries.length; i++) {
      lo = Math.min(lo, dailyEntries[i].temperature_min)
      hi = Math.max(hi, dailyEntries[i].temperature_max)
    }
    return { lo: lo, span: Math.max(1, hi - lo) }
  }

  // ---- settings ------------------------------------------------------------
  readonly property int refreshMinutes: Math.max(1, parseInt(setting("refreshMinutes", 15), 10) || 15)
  readonly property string unitsSetting: String(setting("units", "metric")) === "imperial" ? "imperial" : "metric"
  readonly property string locationSetting: String(setting("location", "")).trim()
  // Empty means "not set" — the CLI falls back to LC_MESSAGES/LANG, then
  // English, on its own. Not exposed in manifest.json: unlike units, this
  // has a sensible automatic default, so it is set (if at all) by editing
  // shell.json directly, the same path units already takes.
  readonly property string languageSetting: String(setting("language", "")).trim()
  readonly property string iconSetSetting: {
    var v = String(setting("iconSet", "nerd"))
    return ["nerd", "weather", "emoji", "fontawesome"].indexOf(v) >= 0 ? v : "nerd"
  }

  // Monochrome rendering: foreground / dimmed foreground only — no accent, no
  // urgent, no thermal ramp. Severity stays readable through text and glyphs,
  // and the structured JSON still carries it for anything scripting on top.
  readonly property string colorMode: {
    var v = String(setting("colorMode", "full"))
    return ["full", "none", "bar-only", "panel-only"].indexOf(v) >= 0 ? v : "full"
  }
  readonly property bool panelColored: colorMode === "full" || colorMode === "panel-only"
  // Exposed for the bar face, which in meteobar draws the condition glyph and
  // temperature in the plain bar foreground — there is no accent or ramp on it
  // to drop, so this currently gates nothing. It stays so the four states mean
  // the same thing here as in the sibling widgets, and so anything colored
  // added to the bar face later is gated from the start.
  readonly property bool barColored: colorMode === "full" || colorMode === "bar-only"

  // Refetch when any setting that changes the payload changes.
  readonly property string fetchKey: unitsSetting + "|" + locationSetting + "|" + iconSetSetting + "|" + languageSetting
  onFetchKeyChanged: Qt.callLater(refresh)

  // Open with fresh data. open/close/toggle shadow the Panel base so every
  // path — bar click, IPC, summon routing, outside-click dismiss — lands here.
  function open() {
    root.controller.show()
    refresh()
  }

  function close() {
    root.controller.hide()
  }

  function toggle() {
    if (root.opened) root.close()
    else root.open()
  }

  // The shell's base handler covers open/close/show/hide/toggle; this one adds
  // `refresh` so a keybind or a script can force a fetch without opening the
  // panel. Overriding means restating the five, so `manageIpc: false` above
  // turns the base one off and this is the only handler on the target.
  IpcHandler {
    target: root.ipcTarget

    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
    function refresh(): void { root.refresh() }
  }

  // Entrance sweep for the daily range bars: each segment grows from its min
  // (cold) edge to its full extent over 200ms when the panel opens. Driven by
  // a single openProgress value from the open path — data refreshes while the
  // panel is open never re-trigger it, and idle stays static at 1. The fill
  // binds to openProgress directly (no Behavior on the animated property, so
  // there is no retargeting while the sweep runs).
  property real openProgress: 1

  onOpenedChanged: if (opened) openAnim.restart()

  NumberAnimation {
    id: openAnim
    target: root
    property: "openProgress"
    from: 0
    to: 1
    duration: 200
    easing.type: Easing.OutCubic
  }

  // ---- process state machine -----------------------------------------------
  // Collector and process completion can race, and a failed start may never
  // fire the collector at all; finalize only once both are done (with a
  // fallback timer for the missing-collector case). A refresh requested while
  // a run is in flight is queued last-command-wins, so settings changed
  // mid-poll re-run with the NEW settings.
  property bool collectorDone: true
  property bool processDone: true

  // A fetch is in flight. BOTH halves matter: the exit code and the collected
  // stdout arrive in either order, which is exactly why maybeFinalize() waits
  // for the pair. The refresh button gates on this, not on collectorDone alone
  // — otherwise it re-enables in the gap between the two signals and a click
  // there queues a second run through pendingCmd, which is the one thing its
  // disabled state promises cannot happen.
  readonly property bool fetchBusy: !collectorDone || !processDone
  property string capturedText: ""
  property int exitCode: 0
  property var pendingCmd: null

  // True when this run's collector refused oversize output. Its message
  // must survive finalizeRun; a stale error from a previous run must not.
  property bool tripwireFired: false

  // True when onExited fired for the current run. A missing command emits
  // no exited. This separates "could not start" from "ran, no output".
  property bool sawExit: false

  // True only when the run could not START. Gates the copy button.
  // Operational errors never set it.
  property bool notInstalled: false

  // One constant, two users: the error message shows it and the copy
  // button copies it.
  readonly property string installCmd: "yay -S meteobar-bin"

  // The copy button shows a check for a moment.
  property bool installCopied: false
  Timer {
    id: copiedReset
    interval: 1500
    onTriggered: root.installCopied = false
  }

  function buildCmd() {
    var cmd = ["meteobar", "--output", "json", "--days", "6", "--hours", "12",
               "--units", unitsSetting, "--icons", iconSetSetting]
    if (locationSetting !== "") {
      cmd.push("--location")
      cmd.push(locationSetting)
    }
    if (languageSetting !== "") {
      cmd.push("--language")
      cmd.push(languageSetting)
    }
    return cmd
  }

  function refresh() {
    startRun(buildCmd())
  }

  function startRun(cmd) {
    if (meteoProc.running) {
      pendingCmd = cmd  // last-command-wins snapshot
      return
    }
    collectorDone = false
    processDone = false
    capturedText = ""
    sawExit = false
    tripwireFired = false
    exitCode = 0
    // Through sh, never direct: handing Quickshell 0.3.1 a nonexistent binary
    // can abort the whole shell inside the failed start (claudebar#6) — before
    // any QML signal fires. sh always starts; a failed exec is sh exiting 127
    // (not found) or 126 (not executable), which finalizeRun maps to the
    // failed-start path. "$0"/"$@" keep everything as argv elements.
    meteoProc.command = ["/bin/sh", "-c", 'exec "$0" "$@"'].concat(cmd)
    meteoProc.running = true
  }

  function maybeFinalize() {
    if (!collectorDone || !processDone) return
    exitFallback.stop()
    finalizeRun()
  }

  function finalizeRun() {
    notInstalled = false
    var text = capturedText.trim()
    if (text === "") {
      // Empty output has three causes. (1) The tripwire already set an
      // error: keep it. (2) Failed start: report not-installed. (3) The
      // process ran and printed nothing: an operational error, never
      // "not installed". A failed start is now sh exiting 126/127 (the
      // exec failed; sh's message goes to stderr, so stdout stays empty).
      // !sawExit stays as the belt for a start that emits no exited.
      if (tripwireFired) {
        // Already explained by this run's tripwire.
      } else if (!sawExit || exitCode === 126 || exitCode === 127) {
        notInstalled = true
        setError("meteobar could not start — not installed or not on PATH?\n\n"
                 + "Install it with:  " + installCmd + "\n"
                 + "Then open this panel again.")
      } else {
        setError("meteobar produced no output (exit " + exitCode + ")")
      }
    } else {
      handle(text)
    }
    if (pendingCmd) {
      var c = pendingCmd
      pendingCmd = null
      Qt.callLater(function() { root.startRun(c) })
    }
  }


  // Set when the plugin's own run fails, cleared by the next good parse. ORed
  // into the freshness state so a failure here reads like any other staleness.
  property bool pluginStale: false

  function setError(message) {
    errorMessage = String(message)
    // The last good payload stays on screen — deliberate — but it must stop
    // claiming to be current. Without this the footer keeps printing a plain
    // "Updated HH:MM" for data the CLI can no longer refresh.
    pluginStale = true
  }

  // Keeps the last known good report on ANY failure; nonempty-but-malformed
  // output becomes an explicit error. When the process exited nonzero but the
  // output is a valid structured error document, its message wins over a
  // generic exit-code error.
  function handle(out) {
    try {
      var d = JSON.parse(out)
      if (d && d.error && d.error.message) {
        setError(d.error.message)
        if (d.current) report = d
      } else if (d && d.current) {
        report = d
        errorMessage = ""
        pluginStale = false
      } else {
        setError(exitCode !== 0
          ? "meteobar exited with code " + exitCode
          : "malformed meteobar output")
      }
    } catch (e) {
      setError(exitCode !== 0
        ? "meteobar exited with code " + exitCode
        : "could not parse meteobar output")
    }
  }

  // ---- formatting helpers ----------------------------------------------------
  function dayLabel(dateString, index) {
    if (index === 0) return "Today"
    var p = String(dateString).split("-")
    if (p.length !== 3) return String(dateString)
    var d = new Date(Number(p[0]), Number(p[1]) - 1, Number(p[2]))
    return Qt.formatDate(d, "ddd d")
  }

  function hourLabel(timeString) {
    // Compact but unambiguous: "21h" reads as an hour where a bare "21" could
    // pass for a day of the month.
    return String(timeString).slice(11, 13) + "h"
  }

  function lerpColor(a, b, t) {
    return Qt.rgba(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t,
                   a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
  }

  // Colors the core resolved from the active theme. Reading them from the
  // payload — rather than re-deriving them here — is what keeps this panel and
  // the Waybar tooltip on the same values, including on a pywal-only machine.
  // The local derivations below remain as a fallback for an older binary whose
  // payload carries no palette.
  readonly property var corePalette: (report && report.palette) ? report.palette : null

  // Coerce a "#rrggbb" string from the payload into a color for arithmetic.
  function toColor(value) { return Qt.darker(String(value), 1.0) }

  // Same rule as ramp_color() in the core: interpolate between the published
  // stops, clamping outside them. The stops carry their positions, so moving a
  // threshold in the core moves this ramp too.
  function rampColor(stops, pct) {
    if (!stops || stops.length === 0) return Qt.darker(fg, 1.55)
    var v = Number(pct)
    if (!isFinite(v) || v <= stops[0].pct) return toColor(stops[0].color)
    for (var i = 0; i + 1 < stops.length; i++) {
      var lo = stops[i], hi = stops[i + 1]
      if (v <= hi.pct) {
        var span = hi.pct - lo.pct
        var ratio = span <= 0 ? 1 : (v - lo.pct) / span
        return lerpColor(toColor(lo.color), toColor(hi.color), ratio)
      }
    }
    return toColor(stops[stops.length - 1].color)
  }

  // Precipitation probability severity, straight off the core's ramp.
  function precipColor(p) {
    if (!panelColored) return Qt.darker(fg, 1.55)
    if (corePalette && corePalette.precip_ramp) return rampColor(corePalette.precip_ramp, p || 0)
    // Fallback: a payload without a palette.
    var t = Math.max(0, Math.min(1, (Number(p) - 20) / 60.0))
    return lerpColor(Qt.darker(fg, 1.55), Color.accent, t)
  }

  // Theme-aware cold/heat anchors: fixed blue/red hues, but lightness taken
  // from the theme foreground (clamped to stay saturated), so the colors read
  // as light on dark themes and dark on light themes — never a fixed hex.
  function thermalColor(hue) {
    var lightness = Math.max(0.32, Math.min(0.72, fg.hslLightness))
    return Qt.hsla(hue, 0.55, lightness, 1)
  }
  // Monochrome keeps the min/max hierarchy through tone instead of hue:
  // the low end dims, the high end sits at full foreground.
  readonly property color coldColor: !panelColored ? Qt.darker(fg, 1.55)
    : (corePalette && corePalette.temp_cold ? toColor(corePalette.temp_cold) : thermalColor(0.58))
  readonly property color heatColor: !panelColored ? fg
    : (corePalette && corePalette.temp_warm ? toColor(corePalette.temp_warm) : thermalColor(0.02))

  // Position on the cold→heat scale, 0..1 clamped.
  function warmthColor(t) {
    if (!panelColored) return fg
    return lerpColor(coldColor, heatColor, Math.max(0, Math.min(1, t)))
  }

  Process {
    id: meteoProc
    // A command that does not exist gives NEITHER `started` NOR `exited` —
    // Quickshell just drops `running` back to false. That is the only signal a
    // failed start emits, and without this handler the panel sits on its
    // loading text for ever: maybeFinalize() waits on processDone, which
    // nothing would ever set. This IS the first run of anyone who installed
    // the plugin from the marketplace and does not have the CLI yet.
    onRunningChanged: {
      if (running) return
      root.processDone = true
      exitFallback.restart()
      root.maybeFinalize()
    }
    onExited: function(code) {
      root.sawExit = true
      root.exitCode = code
      root.processDone = true
      exitFallback.restart()  // failed-start case: collector may never fire
      root.maybeFinalize()
    }
    stdout: StdioCollector {
      waitForEnd: true
      // A tripwire, not a limit, and it counts UTF-16 units rather than bytes —
      // QML's String.length has no byte view. A megabyte of units is up to
      // three megabytes of UTF-8, which is still far outside anything the CLI
      // can produce now that every file and every response it reads is capped.
      // The real bound is there; this only refuses to RETAIN an answer that
      // could not have come from a healthy run.
      readonly property int maxChars: 1024 * 1024
      onStreamFinished: {
        if (text.length > maxChars) {
          root.tripwireFired = true
          root.capturedText = ""
          root.setError("meteobar returned more than " + (maxChars / 1024) + "K characters — refusing it")
        } else {
          root.capturedText = text
        }
        root.collectorDone = true
        root.maybeFinalize()
      }
    }
  }

  Timer {
    id: exitFallback
    interval: 300
    repeat: false
    onTriggered: {
      root.collectorDone = true  // give up on the collector
      root.maybeFinalize()
    }
  }

  Timer {
    interval: root.refreshMinutes * 60 * 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(460))
    contentHeight: panel.fittedContentHeight(contentColumn.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(t) { if (t === "r") root.refresh() }

      Flickable {
        id: contentScroll
        anchors.fill: parent
        contentWidth: width
        contentHeight: contentColumn.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height

        Column {
          id: contentColumn
          width: contentScroll.width
          spacing: Style.space(14)

          // ---- Hero: big glyph + temperature left; location, condition, and
          //      stats stacked on the right.
          Item {
            width: parent.width
            height: Math.max(heroLeft.height, heroRight.height)
            visible: root.hasData

            Row {
              id: heroLeft
              anchors.left: parent.left
              anchors.leftMargin: Style.space(12)
              anchors.verticalCenter: parent.verticalCenter
              spacing: Style.space(16)

              Text {
                anchors.verticalCenter: parent.verticalCenter
                anchors.verticalCenterOffset: Style.space(5)
                textFormat: Text.PlainText
                text: root.current ? root.current.icon : ""
                color: root.fg
                font.family: root.fontFam
                // The condition glyph is the reading itself, so it sits above
                // the identity scale — but derived from it, not hardcoded.
                font.pixelSize: root.heroGlyphSize
              }

              Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: Style.space(2)

                Text {
                  textFormat: Text.PlainText
                  id: tempBig
                  text: root.current ? String(Math.round(root.current.temperature)) : ""
                  color: root.fg
                  font.family: root.fontFam
                  // Hero temperature read-out; deliberately oversized.
                  font.pixelSize: root.heroValueSize
                  font.bold: true
                }
                Text {
                  textFormat: Text.PlainText
                  text: root.tempUnit
                  color: root.fg
                  font.family: root.fontFam
                  font.pixelSize: Style.font.display
                  anchors.top: tempBig.top
                  anchors.topMargin: Style.space(10)
                }
              }
            }

            Column {
              id: heroRight
              anchors.right: parent.right
              anchors.rightMargin: Style.space(12)
              anchors.verticalCenter: parent.verticalCenter
              spacing: Style.space(10)
              // Bounded by what the hero glyph and temperature leave free: the
              // two blocks were anchored to opposite edges with nothing between
              // them, so a long resolved location ("Ciudad Autónoma de Buenos
              // Aires, AR") drew straight over the temperature.
              width: Math.max(0, parent.width - heroLeft.width - Style.space(16))

              // Left-aligned to the stats column below, like the first-party
              // weather panel: the pin then starts at the same x as FEELS
              // instead of floating with the text length. Anchoring to a
              // sibling is safe here — statsRow sizes itself from its own
              // children, so nothing depends back on this block.
              Item {
                anchors.left: statsRow.left
                anchors.right: parent.right
                height: Math.max(locationMark.implicitHeight, locationText.implicitHeight)
                visible: root.locationName !== ""

                Text {
                  textFormat: Text.PlainText
                  id: locationMark
                  text: ""  // nf-fa-map_marker
                  color: Qt.darker(root.fg, 1.4)
                  font.family: root.fontFam
                  font.pixelSize: Style.font.bodySmall
                  anchors.left: parent.left
                  anchors.verticalCenter: parent.verticalCenter
                }

                Text {
                  id: locationText
                  textFormat: Text.PlainText
                  text: root.locationName.toUpperCase()
                  color: Qt.darker(root.fg, 1.4)
                  font.family: root.fontFam
                  font.pixelSize: Style.font.bodySmall
                  font.letterSpacing: 1
                  // A long resolved name elides at the card edge instead of
                  // running under the temperature.
                  anchors.left: locationMark.right
                  anchors.leftMargin: Style.space(6)
                  anchors.right: parent.right
                  elide: Text.ElideRight
                  anchors.verticalCenter: parent.verticalCenter
                }
              }

              Text {
                anchors.left: statsRow.left
                visible: !!root.current
                textFormat: Text.PlainText
                text: root.current ? root.current.description : ""
                color: Qt.darker(root.fg, 1.4)
                font.family: root.fontFam
                font.pixelSize: Style.font.bodySmall
              }

              Row {
                id: statsRow
                anchors.right: parent.right
                visible: !!root.current
                spacing: Style.space(28)

                Column {
                  spacing: Style.space(5)
                  Text {
                    textFormat: Text.PlainText
                    text: "FEELS"
                    color: Qt.darker(root.fg, 1.55)
                    font.family: root.fontFam
                    font.pixelSize: Style.font.bodySmall
                    font.letterSpacing: 1
                  }
                  Text {
                    textFormat: Text.PlainText
                    text: (root.current && root.current.feels_like !== null && root.current.feels_like !== undefined)
                      ? Math.round(root.current.feels_like) + root.tempUnit : "—"
                    color: root.fg
                    font.family: root.fontFam
                    font.pixelSize: Style.font.title
                  }
                }

                Column {
                  spacing: Style.space(5)
                  Text {
                    textFormat: Text.PlainText
                    text: "WIND"
                    color: Qt.darker(root.fg, 1.55)
                    font.family: root.fontFam
                    font.pixelSize: Style.font.bodySmall
                    font.letterSpacing: 1
                  }
                  Text {
                    textFormat: Text.PlainText
                    text: (root.current && root.current.wind_speed !== null && root.current.wind_speed !== undefined)
                      ? Math.round(root.current.wind_speed) + " " + root.windUnit
                        + (root.current.wind_direction ? " " + root.current.wind_direction : "")
                      : "—"
                    color: root.fg
                    font.family: root.fontFam
                    font.pixelSize: Style.font.title
                  }
                }

                Column {
                  spacing: Style.space(5)
                  Text {
                    textFormat: Text.PlainText
                    text: "HUMID"
                    color: Qt.darker(root.fg, 1.55)
                    font.family: root.fontFam
                    font.pixelSize: Style.font.bodySmall
                    font.letterSpacing: 1
                  }
                  Text {
                    textFormat: Text.PlainText
                    text: (root.current && root.current.humidity_pct !== null && root.current.humidity_pct !== undefined)
                      ? Math.round(root.current.humidity_pct) + "%" : "—"
                    color: root.fg
                    font.family: root.fontFam
                    font.pixelSize: Style.font.title
                  }
                }
              }
            }
          }

          Text {
            textFormat: Text.PlainText
            visible: !root.hasData && root.errorMessage === ""
            text: "Fetching forecast…"
            color: Qt.darker(root.fg, 1.55)
            font.family: root.fontFam
            font.pixelSize: Style.font.bodySmall
            font.italic: true
          }

          // ---- Hourly strip: next ~12 hours, evenly distributed columns.
          PanelSeparator {
            visible: root.hourlyEntries.length > 0
            foreground: root.fg
          }

          PanelSectionHeader {
            visible: root.hourlyEntries.length > 0
            text: "NEXT " + root.hourlyEntries.length + " HOURS"
            foreground: root.fg
            fontFamily: root.fontFam
          }

          Item {
            visible: root.hourlyEntries.length > 0
            width: parent.width
            height: Style.space(66)

            Repeater {
              model: root.hourlyEntries

              Column {
                id: hourCell
                required property var modelData
                required property int index
                x: index * (parent.width / Math.max(1, root.hourlyEntries.length))
                width: parent.width / Math.max(1, root.hourlyEntries.length)
                anchors.verticalCenter: parent.verticalCenter
                spacing: Style.space(4)

                Text {
                  anchors.horizontalCenter: parent.horizontalCenter
                  textFormat: Text.PlainText
                  text: root.hourLabel(hourCell.modelData.time)
                  color: Qt.darker(root.fg, 1.55)
                  font.family: root.fontFam
                  font.pixelSize: Style.font.caption
                }
                Text {
                  anchors.horizontalCenter: parent.horizontalCenter
                  textFormat: Text.PlainText
                  text: hourCell.modelData.icon
                  color: root.fg
                  font.family: root.fontFam
                  font.pixelSize: Style.font.iconLarge
                }
                Text {
                  textFormat: Text.PlainText
                  anchors.horizontalCenter: parent.horizontalCenter
                  text: Math.round(hourCell.modelData.temperature) + "°"
                  color: root.fg
                  font.family: root.fontFam
                  font.pixelSize: Style.font.bodySmall
                }
                Text {
                  textFormat: Text.PlainText
                  anchors.horizontalCenter: parent.horizontalCenter
                  // Blank placeholder keeps every column the same height.
                  text: (hourCell.modelData.precip_pct !== null
                         && hourCell.modelData.precip_pct !== undefined
                         && hourCell.modelData.precip_pct >= 10)
                    ? hourCell.modelData.precip_pct + "%" : " "
                  color: root.precipColor(hourCell.modelData.precip_pct || 0)
                  font.family: root.fontFam
                  font.pixelSize: Style.font.caption
                }
              }
            }
          }

          // ---- Daily forecast: day, glyph, rain chance, min–max range bar.
          PanelSeparator {
            visible: root.dailyEntries.length > 0
            foreground: root.fg
          }

          PanelSectionHeader {
            visible: root.dailyEntries.length > 0
            text: "NEXT " + root.dailyEntries.length + " DAYS"
            foreground: root.fg
            fontFamily: root.fontFam
          }

          Column {
            visible: root.dailyEntries.length > 0
            width: parent.width
            spacing: Style.space(9)

            Repeater {
              model: root.dailyEntries

              Item {
                id: dayRow
                required property var modelData
                required property int index
                width: parent.width
                height: Style.space(20)

                readonly property int tempColW: Style.space(30)
                readonly property int barW: Style.space(104)
                readonly property int gap: Style.space(8)

                Text {
                  anchors.left: parent.left
                  anchors.leftMargin: Style.space(12)
                  anchors.verticalCenter: parent.verticalCenter
                  width: Style.space(58)
                  elide: Text.ElideRight
                  textFormat: Text.PlainText
                  text: root.dayLabel(dayRow.modelData.date, dayRow.index).toUpperCase()
                  color: Qt.darker(root.fg, 1.4)
                  font.family: root.fontFam
                  font.pixelSize: Style.font.caption
                  font.letterSpacing: 1
                }

                Text {
                  x: Style.space(76)
                  anchors.verticalCenter: parent.verticalCenter
                  textFormat: Text.PlainText
                  text: dayRow.modelData.icon
                  color: root.fg
                  font.family: root.fontFam
                  font.pixelSize: Style.font.title
                }

                Text {
                  textFormat: Text.PlainText
                  x: Style.space(104)
                  anchors.verticalCenter: parent.verticalCenter
                  text: (dayRow.modelData.precip_pct !== null
                         && dayRow.modelData.precip_pct !== undefined
                         && dayRow.modelData.precip_pct >= 10)
                    ? "󰖗 " + dayRow.modelData.precip_pct + "%" : ""
                  color: root.precipColor(dayRow.modelData.precip_pct || 0)
                  font.family: root.fontFam
                  font.pixelSize: Style.font.caption
                }

                Text {
                  textFormat: Text.PlainText
                  anchors.right: parent.right
                  anchors.rightMargin: Style.space(12)
                  anchors.verticalCenter: parent.verticalCenter
                  width: dayRow.tempColW
                  horizontalAlignment: Text.AlignRight
                  text: Math.round(dayRow.modelData.temperature_max) + "°"
                  color: root.heatColor
                  font.family: root.fontFam
                  font.pixelSize: Style.font.body
                }

                // Min–max span within the week's overall range.
                Item {
                  id: rangeBar
                  anchors.right: parent.right
                  anchors.rightMargin: Style.space(12) + dayRow.tempColW + dayRow.gap
                  anchors.verticalCenter: parent.verticalCenter
                  width: dayRow.barW
                  height: Style.space(4)

                  Rectangle {
                    anchors.fill: parent
                    radius: height / 2
                    color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.15)
                  }

                  Rectangle {
                    id: rangeFill
                    readonly property real loFrac: (dayRow.modelData.temperature_min - root.tempRange.lo) / root.tempRange.span
                    readonly property real hiFrac: (dayRow.modelData.temperature_max - root.tempRange.lo) / root.tempRange.span
                    readonly property real fullWidth: Math.max(Style.space(4), rangeBar.width * Math.max(0, Math.min(1, hiFrac) - Math.max(0, loFrac)))
                    // Anchored at the min edge; on open the segment grows from
                    // there toward its warm end (openProgress 0 → 1).
                    x: rangeBar.width * Math.max(0, Math.min(1, loFrac))
                    width: fullWidth * root.openProgress
                    height: parent.height
                    radius: height / 2

                    // Temperature currently reached by the painted edge,
                    // derived from the ACTUAL animated width rather than from
                    // openProgress. Without this the gradient always spans the
                    // final two colors, so a partial bar squeezes the whole
                    // ramp into it and the colors slide as it grows; deriving
                    // the end stop from the width keeps every point of the bar
                    // at its own temperature and merely uncovers the ramp.
                    readonly property real shownFrac: fullWidth > 0 ? width / fullWidth : 0
                    readonly property real shownHiFrac: loFrac + (hiFrac - loFrac) * Math.max(0, Math.min(1, shownFrac))

                    // Ends tinted to match the min/max read-outs: the bar runs
                    // from this day's cold end to its warm end on the shared
                    // week scale.
                    gradient: Gradient {
                      orientation: Gradient.Horizontal
                      GradientStop { position: 0; color: root.warmthColor(rangeFill.loFrac) }
                      GradientStop { position: 1; color: root.warmthColor(rangeFill.shownHiFrac) }
                    }
                  }
                }

                Text {
                  textFormat: Text.PlainText
                  anchors.right: parent.right
                  anchors.rightMargin: Style.space(12) + dayRow.tempColW + dayRow.gap + dayRow.barW + dayRow.gap
                  anchors.verticalCenter: parent.verticalCenter
                  width: dayRow.tempColW
                  horizontalAlignment: Text.AlignRight
                  text: Math.round(dayRow.modelData.temperature_min) + "°"
                  color: root.coldColor
                  font.family: root.fontFam
                  font.pixelSize: Style.font.bodySmall
                }
              }
            }
          }

          // ---- Footer: error (if any) and last fetch time.
          Item {
            visible: root.errorMessage !== ""
            width: parent.width
            implicitHeight: errorText.implicitHeight

            Text {
              id: errorText
              anchors.left: parent.left
              anchors.right: copyInstallButton.visible ? copyInstallButton.left : parent.right
              anchors.rightMargin: copyInstallButton.visible ? Style.space(8) : 0
              wrapMode: Text.Wrap
              textFormat: Text.PlainText
              text: root.errorMessage
              color: root.panelColored ? root.urgentColor : root.fg
              font.family: root.fontFam
              font.pixelSize: Style.font.bodySmall
            }

            // Copies installCmd as one argv element: no shell line, no
            // trailing newline. Gated on notInstalled, never on error text.
            PanelActionButton {
              id: copyInstallButton
              visible: root.notInstalled
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              iconText: root.installCopied ? "󰄬" : "󰆏"
              tooltipText: root.installCopied ? "Copied" : "Copy install command"
              foreground: Qt.darker(root.fg, 1.55)
              hoverColor: root.fg
              fontFamily: root.fontFam
              fontSize: Style.font.caption
              size: Style.space(20)
              onClicked: {
                Util.execArgv(["wl-copy", root.installCmd])
                root.installCopied = true
                copiedReset.restart()
              }
            }
          }

          // ---- Freshness footer: when the data is from, plus an inline
          //      refresh. The button re-runs the CLI right now — the same
          //      forced refresh the bar's middle-click does — so a stale panel
          //      can be corrected without closing it, and it is disabled while
          //      a fetch is already in flight so clicks cannot queue up. The
          //      rule and the row are always shown: the button has to stay
          //      reachable exactly when there is no timestamp to print yet.
          PanelSeparator {
            foreground: root.fg
          }

          Item {
            width: parent.width
            implicitHeight: Math.max(footerLabel.implicitHeight, refreshButton.implicitHeight)

            Row {
              id: footerLabel
              anchors.left: parent.left
              anchors.right: refreshButton.left
              anchors.rightMargin: Style.spacing.sm
              anchors.verticalCenter: parent.verticalCenter
              spacing: 0

              Text {
                textFormat: Text.PlainText
                text: root.updatedText !== "" ? "󰅐  Updated " + root.updatedText : ""
                color: Qt.darker(root.fg, 1.55)
                font.family: root.fontFam
                font.pixelSize: Style.font.caption
              }

              Text {
                visible: text !== ""
                textFormat: Text.PlainText
                text: root.stale ? " · stale (" + root.staleReason + ")" : ""
                color: root.freshnessWarn
                font.family: root.fontFam
                font.pixelSize: Style.font.caption
              }
            }

            PanelActionButton {
              id: refreshButton
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              // nf-md-refresh (U+F0450). Written literally: a JS "\\u" escape takes
              // exactly FOUR hex digits, so "\\uf0450" is U+F045 followed by a "0".
              iconText: "󰑐"
              tooltipText: "Refresh now"
              foreground: Qt.darker(root.fg, 1.55)
              hoverColor: root.fg
              fontFamily: root.fontFam
              fontSize: Style.font.caption
              size: Style.space(20)
              enabled: !root.fetchBusy
              onClicked: root.refresh()
            }
          }
        }
      }
    }
  }
}
