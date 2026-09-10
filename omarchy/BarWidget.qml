import QtQuick
import qs.Ui

// Bar entry point: compact condition glyph + temperature. All data fetching
// and the forecast panel live in Panel.qml (loaded once, panel opens on
// click), mirroring the structure of the first-party weather plugin.
BarWidget {
  id: root
  moduleName: "mryll.meteobar"

  // Injected by the shell when a plugin declares it. Carries
  // updateEntryInline(), which is how the panel persists a location override
  // back into this widget's shell.json entry.
  property var shell: null

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    if ("bar" in target) target.bar = root.bar
    if ("settings" in target) target.settings = root.settings
    if ("shell" in target) target.shell = root.shell
    if ("anchorItem" in target) target.anchorItem = button
    if ("hostWidget" in target) target.hostWidget = root
  }

  function refresh() {
    if (panelLoader.item && panelLoader.item.refresh) panelLoader.item.refresh()
  }

  function togglePanel() {
    if (panelLoader.item && panelLoader.item.toggle) panelLoader.item.toggle()
  }

  // Shape contract for shell.summon/hide/toggle routing (Bar.findPanelWidget
  // requires open/close/opened on the bar-widget root).
  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false

  function open() {
    if (panelLoader.item && panelLoader.item.open) panelLoader.item.open()
  }

  function close() {
    if (panelLoader.item && panelLoader.item.close) panelLoader.item.close()
  }

  // Forwarded so this widget can stand in for the panel as the bar's popout
  // identity (Bar.requestPopout prefers closeForPopoutSwitch over close).
  readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false

  function closeForPopoutSwitch() {
    if (panelLoader.item) panelLoader.item.closeForPopoutSwitch()
  }

  // How wide the bar's open-panel underline should be. Without this hint the bar
  // falls back to 55% of the SLOT, which reads as a dot under a narrow widget
  // but as a bar that visibly stops short under a wide one. The painted content
  // is the honest extent, so the mark tracks what the widget draws instead of a
  // fraction of the box it happens to sit in. (Same hint the first-party clock
  // gives; it passes its label width.)
  readonly property real openPanelIndicatorWidth: button.labelWidth

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onBarChanged: injectPanel()
  onSettingsChanged: injectPanel()

  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    visible: false
    onLoaded: {
      root.injectPanel()
      Qt.callLater(root.injectPanel)
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    // Icon-only on vertical bars; glyph + temperature on horizontal ones.
    text: panelLoader.item
      ? (root.vertical ? panelLoader.item.barIcon : panelLoader.item.barText)
      : ""
    dimmed: panelLoader.item ? !panelLoader.item.hasData : true
    // Tooltip suppressed: the panel is the detail view.
    tooltipText: ""

    onPressed: function(b) {
      if (b === Qt.MiddleButton) root.refresh()
      else if (b !== Qt.RightButton) root.togglePanel()
    }
  }
}
