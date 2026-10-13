import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// The NERV MAGI popup: herdr's agents (blocked first), and the dashboard's
// controls: open it here, cast it to the TV (omarchy-cast), switch views.
// Everything goes through the scripts in this plugin folder, the same ones the
// herdr plugin actions use, so the bar, herdr and the terminal agree.
Panel {
  id: root

  moduleName: "anomius.nerv-magi"
  ipcTarget: "anomius.nerv-magi"
  manageIpc: false

  property var anchorItem: null
  property var hostWidget: null
  readonly property var barIdentity: hostWidget || root

  readonly property color contentForeground: bar ? bar.foreground : Color.foreground
  readonly property string contentFontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property color dimForeground: Qt.darker(contentForeground, 1.5)
  // The dashboard's own palette, for status only.
  readonly property color evaOrange: "#FF9900"
  readonly property color evaGreen: "#39FF14"
  readonly property color evaCyan: "#00FFFF"

  // This plugin's folder: the repo checkout `omarchy plugin add` made.
  readonly property string rootDir: decodeURIComponent(Qt.resolvedUrl(".").toString().replace(/^file:\/\//, "")).replace(/\/$/, "")
  readonly property int pollSec: Math.max(2, Number(root.settings && root.settings.pollIntervalSec) || 5)

  // ---- state from scripts/omarchy-status.sh
  property var status: ({ herdr: false, agents: [], tv: false, built: false, airplay: false, cast: null })
  property bool busy: false
  // Whether the poll in flight asked for the cast too (--full runs only while the panel is open).
  property bool pollFull: false
  property string busyLabel: ""
  property string lastView: "auto"

  readonly property var rank: ({ blocked: 0, working: 1, done: 2, idle: 3 })
  readonly property var agents: {
    var list = (status.agents || []).slice()
    list.sort(function(a, b) {
      var ra = rank[a.status] === undefined ? 4 : rank[a.status]
      var rb = rank[b.status] === undefined ? 4 : rank[b.status]
      return ra !== rb ? ra - rb : String(a.name).localeCompare(String(b.name))
    })
    return list
  }
  readonly property var counts: {
    var c = { active: 0, blocked: 0, done: 0, idle: 0 }
    var list = status.agents || []
    for (var i = 0; i < list.length; i++) {
      var s = list[i].status
      if (s === "working") c.active++
      else if (s === "blocked") c.blocked++
      else if (s === "done") c.done++
      else c.idle++
    }
    return c
  }
  readonly property bool casting: !!(status.cast && status.cast.kind === "extend")
  readonly property string tvLabel: !status.built ? "not built"
    : !status.tv ? "off"
    : casting ? "on the TV (" + (status.cast.receiver || "AirPlay") + ", workspace " + status.cast.workspace + ")"
    : "open on this computer"
  readonly property string summary: "NERV MAGI · "
    + (status.herdr ? counts.active + " active" + (counts.blocked > 0 ? " · " + counts.blocked + " BLOCKED" : "") : "herdr offline")
    + " · dashboard " + (status.tv ? (casting ? "on TV" : "open") : "off")

  function statusColor(s) {
    if (s === "blocked") return Color.urgent
    if (s === "working") return root.evaGreen
    if (s === "done") return root.evaCyan
    return root.dimForeground
  }
  function esc(s) {
    return String(s === undefined || s === null ? "" : s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
  }
  function statusWord(s) {
    return ({ blocked: "BLOCKED", working: "ACTIVE", done: "COMPLETE", idle: "STANDBY" })[s] || "UNKNOWN"
  }

  // ---- processes
  Process {
    id: statusProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var s = JSON.parse(String(text || "").trim() || "{}")
          if (!(s && typeof s === "object" && "agents" in s)) return
          // A light poll does not look at the cast: keep the last one known while the dashboard runs.
          if (!root.pollFull && s.tv && root.status.cast) s.cast = root.status.cast
          root.status = s
        } catch (e) {}
      }
    }
  }

  function poll() {
    if (statusProc.running) return
    root.pollFull = root.opened
    statusProc.command = ["bash", root.rootDir + "/scripts/omarchy-status.sh"].concat(root.pollFull ? ["--full"] : [])
    statusProc.running = true
  }

  Timer {
    interval: (root.opened ? 2 : root.pollSec) * 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.poll()
  }

  onOpenedChanged: if (opened) poll()

  // Fire a script from scripts/, then refresh. `label` shows while it runs.
  function run(script, args, label) {
    var p = Qt.createQmlObject('import Quickshell.Io; Process {}', root)
    p.command = ["bash", root.rootDir + "/scripts/" + script].concat(args || [])
    if (label) { root.busy = true; root.busyLabel = label }
    p.exited.connect(function() {
      if (label) { root.busy = false; root.busyLabel = "" }
      p.destroy()
      Qt.callLater(root.poll)
    })
    p.running = true
  }

  function tvCtl(word) {
    if (!status.tv) return
    if (["main", "load", "sync", "emergency", "auto"].indexOf(word) >= 0) root.lastView = word
    run("tv-ctl.sh", [word])
  }
  function openWindowed() { run("tv-start.sh", ["--windowed"], "Opening…") }
  function cast() { run("tv-cast.sh", [], "Casting to the TV…") }
  function stop() { casting ? run("tv-uncast.sh", [], "Stopping the cast…") : run("tv-stop.sh", [], "Stopping…") }
  function toggleDashboard() { status.tv ? stop() : openWindowed() }
  function focusAgent(pane) { Quickshell.execDetached(["herdr", "agent", "focus", pane]) }
  function openHub() { if (status.bin) Quickshell.execDetached(["xdg-terminal-exec", "--app-id=nerv-hub", status.bin, "hub"]) }
  function build() {
    // The path travels as "$1", never inside the command string.
    Quickshell.execDetached(["xdg-terminal-exec", "bash", "-c",
      "\"$1\"; echo; read -rp 'Press Enter to close'", "nerv-install", root.rootDir + "/scripts/install.sh"])
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    centerOnBar: true
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(380))
    contentHeight: panel.fittedContentHeight(bodyColumn.implicitHeight, Style.space(620))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()

      // 1-4 views · a auto · s scanlines · g grain · f fullscreen · o open/stop · c cast
      onTextKey: function(t) {
        var views = { "1": "main", "2": "load", "3": "sync", "4": "emergency", "a": "auto", "0": "auto",
                      "s": "scanlines", "g": "grain", "f": "fullscreen" }
        if (views[t] !== undefined) root.tvCtl(views[t])
        else if (t === "o") root.toggleDashboard()
        else if (t === "c" && !root.status.tv && root.status.airplay) root.cast()
      }

      Flickable {
        id: scroll
        anchors.fill: parent
        contentWidth: width
        contentHeight: bodyColumn.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height

        Column {
          id: bodyColumn
          width: scroll.width
          spacing: Style.spacing.md

          // ===================================================== HEADER
          Item {
            width: parent.width
            height: Math.max(title.implicitHeight, stateChip.implicitHeight)

            Text {
              textFormat: Text.PlainText
              id: title
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
              text: "NERV MAGI"
              color: root.evaOrange
              font.family: root.contentFontFamily
              font.pixelSize: Style.font.title
              font.bold: true
              font.letterSpacing: 2
            }

            Text {
              textFormat: Text.PlainText
              id: stateChip
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              text: !root.status.herdr ? "HERDR OFFLINE"
                : root.counts.blocked > 0 ? "EMERGENCY · " + root.counts.blocked + " BLOCKED"
                : "3/3 NOMINAL"
              color: !root.status.herdr ? root.dimForeground
                : root.counts.blocked > 0 ? Color.urgent : root.evaGreen
              font.family: root.contentFontFamily
              font.pixelSize: Style.font.caption
              font.bold: true
            }
          }

          Text {
            textFormat: Text.PlainText
            width: parent.width
            visible: root.status.herdr
            text: root.counts.active + " active · " + root.counts.done + " complete · "
              + root.counts.idle + " standby"
            color: root.dimForeground
            font.family: root.contentFontFamily
            font.pixelSize: Style.font.caption
          }

          PanelSeparator { width: parent.width; foreground: root.contentForeground }

          // ================================================== DASHBOARD
          Item {
            width: parent.width
            height: dashTitle.implicitHeight

            Text {
              textFormat: Text.PlainText
              id: dashTitle
              anchors.left: parent.left
              text: "Dashboard"
              color: root.contentForeground
              font.family: root.contentFontFamily
              font.pixelSize: Style.font.subtitle
              font.bold: true
            }

            Text {
              textFormat: Text.PlainText
              anchors.right: parent.right
              anchors.verticalCenter: dashTitle.verticalCenter
              text: root.busy ? root.busyLabel : root.tvLabel
              color: root.busy ? root.evaOrange : root.dimForeground
              font.family: root.contentFontFamily
              font.pixelSize: Style.font.caption
            }
          }

          Text {
            textFormat: Text.PlainText
            visible: !root.status.built
            width: parent.width
            wrapMode: Text.WordWrap
            text: "The dashboard isn't built yet. Build it once (Rust, about two minutes)."
            color: root.dimForeground
            font.family: root.contentFontFamily
            font.pixelSize: Style.font.body
          }

          Row {
            spacing: Style.spacing.md

            Button {
              visible: !root.status.built
              text: "Build"
              bordered: true
              foreground: root.contentForeground
              fontFamily: root.contentFontFamily
              onClicked: root.build()
            }
            Button {
              visible: root.status.built && !root.status.tv
              enabled: !root.busy
              text: "Open here"
              bordered: true
              foreground: root.contentForeground
              fontFamily: root.contentFontFamily
              tooltipText: "Open the dashboard in a window (o)"
              onClicked: root.openWindowed()
            }
            Button {
              visible: root.status.built && !root.status.tv && root.status.airplay
              enabled: !root.busy
              text: "Cast to TV"
              bordered: true
              foreground: root.contentForeground
              fontFamily: root.contentFontFamily
              tooltipText: "AirPlay second desktop via omarchy-cast, dashboard fullscreen on it (c)"
              onClicked: root.cast()
            }
            Button {
              visible: root.status.tv
              enabled: !root.busy
              text: root.casting ? "Stop casting" : "Close"
              bordered: true
              foreground: root.contentForeground
              fontFamily: root.contentFontFamily
              onClicked: root.stop()
            }
            Button {
              visible: root.status.tv
              text: "Fullscreen"
              foreground: root.contentForeground
              fontFamily: root.contentFontFamily
              tooltipText: "Toggle fullscreen (f)"
              onClicked: root.tvCtl("fullscreen")
            }
            Button {
              visible: root.status.built
              text: "Hub"
              foreground: root.contentForeground
              fontFamily: root.contentFontFamily
              tooltipText: "The terminal board (nerv hub) in a new terminal"
              onClicked: root.openHub()
            }
          }

          ButtonGroup {
            visible: root.status.tv
            options: [
              { value: "main", label: "MAIN" }, { value: "load", label: "LOAD" },
              { value: "sync", label: "SYNC" }, { value: "emergency", label: "EMERG" },
              { value: "auto", label: "AUTO" }
            ]
            value: root.lastView
            foreground: root.contentForeground
            fontFamily: root.contentFontFamily
            fontSize: Style.font.caption
            onChanged: function(v) { root.tvCtl(v) }
          }

          Row {
            visible: root.status.tv
            spacing: Style.spacing.md
            Button {
              text: "Scanlines"
              foreground: root.contentForeground
              fontFamily: root.contentFontFamily
              tooltipText: "off · subtle · heavy · crt (s)"
              onClicked: root.tvCtl("scanlines")
            }
            Button {
              text: "Grain"
              foreground: root.contentForeground
              fontFamily: root.contentFontFamily
              tooltipText: "Film grain on / off (g)"
              onClicked: root.tvCtl("grain")
            }
            Button {
              text: "Pin"
              foreground: root.contentForeground
              fontFamily: root.contentFontFamily
              tooltipText: "Pin / unpin the current view"
              onClicked: root.tvCtl("pin")
            }
          }

          PanelSeparator { width: parent.width; foreground: root.contentForeground }

          // ===================================================== PILOTS
          Item {
            width: parent.width
            height: pilotsTitle.implicitHeight

            Text {
              textFormat: Text.PlainText
              id: pilotsTitle
              anchors.left: parent.left
              text: "Pilots"
              color: root.contentForeground
              font.family: root.contentFontFamily
              font.pixelSize: Style.font.subtitle
              font.bold: true
            }

            Text {
              textFormat: Text.PlainText
              anchors.right: parent.right
              anchors.verticalCenter: pilotsTitle.verticalCenter
              text: root.status.herdr ? root.agents.length + " in herdr" : ""
              color: root.dimForeground
              font.family: root.contentFontFamily
              font.pixelSize: Style.font.caption
            }
          }

          Text {
            textFormat: Text.PlainText
            visible: !root.status.herdr || root.agents.length === 0
            width: parent.width
            wrapMode: Text.WordWrap
            text: !root.status.herdr ? "herdr isn't running, so there are no agents to show." : "No agents in herdr right now."
            color: root.dimForeground
            font.family: root.contentFontFamily
            font.pixelSize: Style.font.body
          }

          Repeater {
            model: root.agents

            delegate: Item {
              id: row
              required property var modelData

              width: bodyColumn.width
              height: Math.max(rowName.implicitHeight, rowWord.implicitHeight) + Style.spacing.md

              Rectangle {
                anchors.fill: parent
                radius: Style.cornerRadius
                color: rowHover.containsMouse ? Style.hoverFillFor(root.contentForeground, Color.accent) : "transparent"
              }

              MouseArea {
                id: rowHover
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: { root.focusAgent(row.modelData.pane); root.close() }
              }

              Rectangle {
                id: dot
                anchors.left: parent.left
                anchors.leftMargin: Style.spacing.md
                anchors.verticalCenter: parent.verticalCenter
                width: Style.space(8)
                height: width
                radius: row.modelData.status === "blocked" ? 0 : width / 2
                color: root.statusColor(row.modelData.status)
              }

              Text {
                id: rowName
                anchors.left: dot.right
                anchors.leftMargin: Style.spacing.md
                anchors.right: rowWord.left
                anchors.rightMargin: Style.spacing.md
                anchors.verticalCenter: parent.verticalCenter
                text: root.esc(row.modelData.name) + "  <font color='" + root.dimForeground + "'>"
                  + root.esc(row.modelData.kind) + " · " + root.esc(row.modelData.workspace) + "</font>"
                textFormat: Text.StyledText
                elide: Text.ElideRight
                color: root.contentForeground
                font.family: root.contentFontFamily
                font.pixelSize: Style.font.body
              }

              Text {
                textFormat: Text.PlainText
                id: rowWord
                anchors.right: parent.right
                anchors.rightMargin: Style.spacing.md
                anchors.verticalCenter: parent.verticalCenter
                text: root.statusWord(row.modelData.status)
                color: root.statusColor(row.modelData.status)
                font.family: root.contentFontFamily
                font.pixelSize: Style.font.caption
                font.bold: row.modelData.status === "blocked"
              }
            }
          }
        }
      }
    }
  }
}
