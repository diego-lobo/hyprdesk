// hyprdesk desk switcher (~/Projects/hyprdesk).
//
// Replaces omarchy.workspaces while hyprdesk is running. A desk spans
// every monitor, so the stock widget cannot represent it: it filters to
// workspace ids 1-10 and highlights the focused workspace, which leaves
// the external monitor's desk workspaces (11+) invisible and nothing
// highlighted whenever focus sits on that monitor.
//
// Rendering is derived purely from compositor state that Quickshell
// already tracks reactively; the hyprdesk daemon is only invoked to act
// on a click or scroll. That keeps the bar correct even if the daemon is
// restarted underneath it.

import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import qs.Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "hyprdesk.desks"

  // Mirrors hyprdesk's desk model (src/model.rs): desk d on the monitor
  // in slot m owns Hyprland workspace d + deskCount*m, so a workspace's
  // desk identity is its id modulo the desk count. Ids outside the
  // mapped range are not ours (special workspaces are negative).
  readonly property int deskCount: 10
  readonly property int maxMonitors: 8

  // Desks shown even when empty, mirroring the stock persistent 1-5.
  readonly property int persistentCount: 5

  // The glyph the stock bar uses for the active workspace (U+F14FB).
  readonly property string activeIcon: "󱓻"

  readonly property string hyprdesk: (Quickshell.env("HOME") || "") + "/.cargo/bin/hyprdesk"

  function deskOf(workspaceId) {
    if (workspaceId < 1 || workspaceId > root.deskCount * root.maxMonitors) return 0

    return ((workspaceId - 1) % root.deskCount) + 1
  }

  // A desk is occupied when ANY of its per-monitor workspaces holds a
  // window - the cross-monitor aggregation a workspace widget cannot do.
  function occupied(desk) {
    var values = Hyprland.workspaces.values
    for (var i = 0; i < values.length; i++) {
      if (root.deskOf(values[i].id) === desk && values[i].toplevels.values.length > 0) return true
    }

    return false
  }

  readonly property int activeDesk: Hyprland.focusedWorkspace ? root.deskOf(Hyprland.focusedWorkspace.id) : 0

  // The active desk and every occupied desk always show; the rest of
  // 1..persistentCount fill in dimmed. Higher desks stay hidden until
  // something lives on them.
  function deskIds() {
    var ids = []
    for (var desk = 1; desk <= root.deskCount; desk++) {
      if (desk === root.activeDesk || root.occupied(desk) || desk <= root.persistentCount) ids.push(desk)
    }

    return ids
  }

  // Desk 10 renders as "0", matching its key on the number row.
  function label(desk) {
    return desk === root.deskCount ? "0" : String(desk)
  }

  function switchTo(desk) {
    if (root.bar) root.bar.run(Util.shellQuote(root.hyprdesk) + " vdesk " + desk)
  }

  function step(delta) {
    if (!root.bar) return

    var subcommand = delta > 0 ? "nextdesk" : "prevdesk"
    root.bar.run(Util.shellQuote(root.hyprdesk) + " " + subcommand + " --cycle")
  }

  readonly property real trailingGap: root.vertical ? 0 : Style.spaceReal(1.5)

  implicitWidth: grid.implicitWidth + trailingGap
  implicitHeight: grid.implicitHeight

  GridLayout {
    id: grid
    anchors.fill: parent
    anchors.rightMargin: root.trailingGap
    columns: root.vertical ? 1 : root.deskIds().length
    columnSpacing: root.vertical ? 0 : Style.space(1)
    rowSpacing: root.vertical ? Style.space(2) : 0

    Repeater {
      model: root.deskIds()

      WidgetButton {
        required property int modelData

        readonly property bool isActive: modelData === root.activeDesk

        bar: root.bar
        text: isActive ? root.activeIcon : root.label(modelData)
        tooltipText: "Desk " + modelData
        opacity: isActive || root.occupied(modelData) ? 1 : 0.5
        horizontalMargin: 6
        verticalPadding: 6
        fixedWidth: root.vertical ? root.barSize : Style.space(20)
        fixedHeight: root.barSize
        onPressed: function() { root.switchTo(modelData) }
        onWheelMoved: function(delta) { root.step(delta) }
      }
    }
  }
}
