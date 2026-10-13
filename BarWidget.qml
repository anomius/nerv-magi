import QtQuick
import Quickshell.Io
import qs.Commons
import qs.Ui

// Bar button for NERV MAGI. A hexagon: dim when no agent works, lit with the
// number of active agents, red and pulsing while any agent is blocked. The data
// and the popup live in Panel.qml (loaded eagerly so the icon is live at once);
// this file owns the icon and the plugin's IPC target.
BarWidget {
  id: root

  moduleName: "anomius.nerv-magi"

  // A moved widget briefly overlaps its retiring copy; registering the IPC
  // target at once could collide with it, so wait a tick.
  property bool ipcRegistrationReady: false

  readonly property var panel: panelLoader.item
  readonly property bool opened: panel ? panel.opened === true : false
  readonly property bool popoutSwitchClosing: panel ? panel.popoutSwitchClosing === true : false
  readonly property int active: panel ? panel.counts.active : 0
  readonly property int blocked: panel ? panel.counts.blocked : 0
  readonly property bool showCount: !root.settings || root.settings.showCount !== false

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    if ("bar" in target) target.bar = root.bar
    if ("settings" in target) target.settings = root.settings
    if ("anchorItem" in target) target.anchorItem = button
    if ("hostWidget" in target) target.hostWidget = root
  }

  function open() { if (panel) panel.open() }
  function close() { if (panel) panel.close() }
  function togglePanel() { if (panel) panel.toggle() }
  function closeForPopoutSwitch() { if (panel) panel.closeForPopoutSwitch() }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onBarChanged: injectPanel()
  onSettingsChanged: injectPanel()
  Component.onCompleted: ipcRegistrationTimer.start()

  Timer {
    id: ipcRegistrationTimer
    interval: 100
    onTriggered: root.ipcRegistrationReady = true
  }

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

  IpcHandler {
    enabled: root.ipcRegistrationReady
    target: root.moduleName

    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.togglePanel() }

    // `omarchy-shell anomius.nerv-magi status`
    function status(): string {
      return root.panel ? root.panel.summary : "loading"
    }

    // The panel's buttons, for keybindings: `omarchy-shell anomius.nerv-magi cast`.
    function window(): string { if (!root.panel) return "unavailable"; root.panel.openWindowed(); return "ok" }
    function cast(): string { if (!root.panel) return "unavailable"; root.panel.cast(); return "ok" }
    function stop(): string { if (!root.panel) return "unavailable"; root.panel.stop(); return "ok" }
    // main | load | sync | emergency | auto | next | prev | pin | fullscreen | scanlines | grain | help
    function view(word: string): string {
      if (!root.panel) return "unavailable"
      if (!root.panel.status.tv) return "dashboard not running"
      root.panel.tvCtl(word)
      return "ok"
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    // nf-md-hexagon (filled) while agents work, nf-md-hexagon_outline when idle
    text: (root.active > 0 || root.blocked > 0 ? "\u{F02D8}" : "\u{F02D9}")
      + (root.showCount && !root.vertical && (root.blocked > 0 || root.active > 0)
          ? " " + (root.blocked > 0 ? root.blocked : root.active) : "")
    fontSize: Style.bar.iconFont
    fixedHeight: root.vertical ? Style.bar.iconSlot : -1
    active: root.blocked > 0
    dimmed: root.active === 0 && root.blocked === 0
    tooltipText: root.panel ? root.panel.summary : "NERV MAGI"

    onPressed: function(pressedButton) {
      if (pressedButton === Qt.LeftButton) root.togglePanel()
      else if (pressedButton === Qt.MiddleButton && root.panel) root.panel.tvCtl("next")
      else if (pressedButton === Qt.RightButton && root.panel) root.panel.toggleDashboard()
    }

    SequentialAnimation on opacity {
      running: root.blocked > 0
      loops: Animation.Infinite
      alwaysRunToEnd: true
      NumberAnimation { to: 0.35; duration: 520; easing.type: Easing.InOutSine }
      NumberAnimation { to: 1.0; duration: 520; easing.type: Easing.InOutSine }
    }
  }
}
