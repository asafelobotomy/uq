pragma ComponentBehavior: Bound
// Under the pill: the results, the pinned squares, the terminal's drop-down and strip, and what went wrong.
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Widgets
import "../../core"
import "../../logic/launcher.js" as L
import "../../logic/rank.js" as Rank
import "../../logic/commands.js" as Commands
import "../../logic/quickui.js" as QuickUi
import "../../logic/shellprefs.js" as ShellPrefs
import "../../logic/failure.js" as F
import "../../logic/lookhome.js" as H
import ".."

Rectangle {
    id: body
    // Passed in from LookSeeDo.qml.
    property var field
    property var pill
    property var root
    // The app menu, for LookSeeDo.qml's wipe() to close.
    property alias appActionMenu: actionMenu

    y: pill.height + Theme.gapIn
    width: parent.width
    height: col.implicitHeight + Theme.gapOut * 2
    visible: root.sections.length > 0 || root.homeShown || root.runMode || root.pinSlot >= 0
             || root.termState !== "" || root.dockStrip || root.error !== "" || root.searchError !== ""
             || root.emojiHint || copied.running
             || (root.searched !== "" && root.deadlinePassed)
    radius: Theme.radius
    color: Theme.surface
    border.color: Theme.border
    border.width: Theme.borderWidth

    ColumnLayout {
        id: col
        anchors.fill: parent
        anchors.margins: Theme.gapOut
        spacing: Theme.gapIn

        ResultsList {
            id: flick
            actionMenu: actionMenu
            root: body.root
        }

        // The pinned squares: open an app, or fill an empty square.
        PinnedSquares {
            id: pinned
            root: body.root
        }

        // The default terminal, dropped down from the pill with the command being typed.
        // Enter opens the real one in its place.
        TerminalDrop {
            id: termDrop
            root: body.root
        }

        // The terminal is docked under the pill: what Enter does next, or what is running.
        Rectangle {
            objectName: "lookseedoSent"
            Layout.fillWidth: true
            Layout.preferredHeight: root.sentHeight
            visible: root.dockStrip || (root.termState === "sent" && !root.docked)
            radius: Theme.radiusControl
            color: Theme.surfaceAlt
            UqText {
                anchors.fill: parent
                anchors.leftMargin: Theme.gapOut
                anchors.rightMargin: Theme.gapOut
                verticalAlignment: Text.AlignVCenter
                elide: Text.ElideRight
                text: L.dockText(root.termName, root.dockTabs !== "", root.termState, root.runMode)
                color: Theme.accent
            }
        }

        UqText {
            Layout.fillWidth: true
            Layout.leftMargin: Theme.gapIn
            // Nothing found, once the providers have had their time.
            visible: root.sections.length === 0 && root.searched !== "" && root.deadlinePassed
                     && !root.emojiHint && root.query.trim() === root.searched
                     && !root.runMode && root.pinSlot < 0
            text: "No matches"
            color: Theme.textMuted
        }
        UqText {
            Layout.fillWidth: true
            Layout.leftMargin: Theme.gapIn
            visible: root.pinSlot >= 0 && root.searched !== "" && root.sections.length === 0
            text: "No apps match"
            color: Theme.textMuted
        }
        UqText {
            Layout.fillWidth: true
            visible: root.emojiHint
            text: "No emoji provider installed"
            color: Theme.textMuted
        }
        UqText {
            Layout.fillWidth: true
            visible: copied.running
            text: "Copied"
            color: Theme.success
        }
        UqText {
            objectName: "lookseedoError"
            Layout.fillWidth: true
            visible: text !== ""
            text: root.error !== "" ? root.error : root.searchError
            color: Theme.error
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
        }
        UqText {
            objectName: "lookseedoHint"
            Layout.fillWidth: true
            Layout.topMargin: Theme.gapIn
            visible: root.termState !== "sent" && !root.dockStrip
            text: root.pinSlot >= 0 ? "Enter pins the app   Esc goes back"
                : (root.runMode ? "Enter runs it in " + root.termName + "   Shift+Enter runs it in the background"
                : (root.homeShown ? "Alt+1 to 5 open a pinned app   = maths   > terminal   : emoji"
                : "= maths   > terminal   : emoji   Tab next group"))
            color: Theme.textMuted
            font.pixelSize: Theme.fontSize - 1
        }
    }

    // More rows below: the list fades out instead of ending on a row cut in half.
    Rectangle {
        x: col.x + flick.x
        y: col.y + flick.y + flick.height - height
        width: flick.width
        height: Math.min(28, flick.height)
        visible: flick.visible && flick.contentY + flick.height < flick.contentHeight - 1
        gradient: Gradient {
            GradientStop { position: 0; color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0) }
            GradientStop { position: 1; color: Theme.surface }
        }
    }

    AppActionMenu {
        id: actionMenu
        onDone: Surfaces.close()
        onOpenChanged: if (!open && root.shown) field.forceActiveFocus()
    }
}
