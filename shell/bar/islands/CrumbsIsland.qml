import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import "../../core"
import "../../services"
import "../../logic/ticker.js" as T
import "../../logic/crumbs.js" as C
import ".."

// Storage cleanup (`crumbs`), from crumBS when it is installed (`crumbs scan --json`: the space
// that can be reclaimed, by category). Icon: the bin. Small: and the amount. Medium and Large: and
// "to free". It turns amber at shell.bar.options.crumbs.warn_gb. A click lists the biggest
// categories, scans again, and opens crumBS's Storage page in Settings. The scan only reads; it
// runs at low priority, once for all screens (services/CrumbsStatus.qml): when the island is
// placed, then every half hour. Hidden while crumbs isn't installed or can't answer. The trash is left out of the amount: only naming it empties it.
Island {
    id: root

    readonly property var opts: BarLayout.options.crumbs || ({})
    readonly property real warnGb: typeof opts.warn_gb === "number" ? opts.warn_gb : 5

    // crumBS's reading, taken once for every screen (services/CrumbsStatus.qml).
    readonly property var rows: CrumbsStatus.rows
    readonly property bool failed: CrumbsStatus.failed

    readonly property real total: rows ? C.total(rows) : 0
    readonly property bool warn: warnGb > 0 && total >= warnGb * 1024 * 1024 * 1024
    readonly property color tint: warn ? Theme.warning : Theme.textMuted
    readonly property int iconSize: Theme.fontSize + 5

    shown: rows !== null && !failed
    Accessible.role: Accessible.Button
    Accessible.name: "Storage cleanup: " + T.bytes(total) + " can be freed. Show details"

    RowLayout {
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.room)
        spacing: Theme.gapIn

        IconImage {
            implicitSize: root.iconSize
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            source: Quickshell.iconPath("user-trash-full", "user-trash")
            opacity: root.warn ? 1 : 0.85
        }
        UqText {
            visible: !root.isIcon
            text: T.bytes(root.total)
            baseFamily: Theme.fontMono
            color: root.warn ? Theme.warning : Theme.text
            Layout.minimumWidth: implicitWidth
        }
        UqText {
            visible: root.size === "medium" || root.size === "large" || root.size === "custom"
            text: "to free"
            color: Theme.textMuted
            Layout.fillWidth: true
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: popup.open && popup.anchorItem === root ? popup.hide() : popup.show(root, details)
    }

    LazyPopup {
        id: popup
    }

    Component {
        id: details
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 24

            UqText {
                text: "Space you can free: " + T.bytes(root.total)
                baseBold: true
                color: root.warn ? Theme.warning : Theme.text
            }
            UqText {
                visible: !root.rows || root.rows.length === 0
                text: "Nothing to clean up right now."
                color: Theme.textMuted
                Layout.fillWidth: true
            }
            Repeater {
                model: root.rows ? root.rows.slice(0, 6) : []
                delegate: RowLayout {
                    required property var modelData
                    spacing: Theme.gapIn
                    Layout.fillWidth: true
                    UqText {
                        text: modelData.label
                        Layout.fillWidth: true
                    }
                    UqText {
                        text: T.bytes(modelData.size)
                        baseFamily: Theme.fontMono
                        color: Theme.textMuted
                    }
                }
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Open Storage settings…"
                onActivated: {
                    popup.hide();
                    Quickshell.execDetached(["uq-settings", "--page", "crumbs"]);
                }
                Component.onCompleted: forceActiveFocus()
            }
            MenuRow {
                Layout.fillWidth: true
                text: CrumbsStatus.scanning ? "Scanning…" : "Scan again"
                enabled: !CrumbsStatus.scanning
                onActivated: CrumbsStatus.refresh()
            }
        }
    }
}
