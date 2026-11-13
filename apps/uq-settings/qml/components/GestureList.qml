// The touchpad gestures on the Input page: one row per gesture (what you do, what it does, a
// switch, Change and Remove), a line under a row when it clashes with another, "Add a gesture",
// and "Back to uQ's gestures" once the list is your own. While the list is uQ's own, a line says
// so. A gesture in user.lua can stop uQ's from working, so its line is shown with Open user.lua
// (SourceLine). Labels come from uQ's own tables and the clash text from uQ, but all of it is
// shown through UqLabel (plain text) like every list here.
//
// `gestures` is `[{label, action_label, enabled, gesture}]`, `clashes` is `{"<index>": text}` and
// `userLua` is `{line, path}` or null: the page's snapshot.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

ColumnLayout {
    id: root

    property var gestures: []
    /// Whether the list is uQ's own two gestures (nothing of yours is saved).
    property bool defaults: true
    property var clashes: ({})
    property var userLua: null
    /// Whether a request can be sent now (the service is up and nothing is in flight).
    property bool idle: true
    /// Whether **Open user.lua** opens the file (tests turn it off).
    property bool opensFiles: true
    readonly property alias rows: list
    readonly property alias badge: defaultsLine
    readonly property alias luaLine: userLuaLine
    readonly property alias addButton: addBtn
    readonly property alias resetButton: resetBtn
    readonly property string defaultsText: "These are uQ's own gestures. Change one and the list becomes yours."

    /// A switch was moved: turn gesture `index` on or off.
    signal toggled(int index, bool enabled)
    signal changeRequested(int index)
    signal removed(int index)
    signal addRequested()
    /// "Back to uQ's gestures" was pressed.
    signal resetRequested()

    /// An arrow for a gesture's direction.
    function directionIcon(direction) {
        switch (String(direction || "")) {
        case "up":
            return "go-up";
        case "down":
            return "go-down";
        case "left":
            return "go-previous";
        case "right":
            return "go-next";
        case "pinch_in":
            return "zoom-out";
        case "pinch_out":
            return "zoom-in";
        case "horizontal":
            return "transform-move-horizontal";
        case "vertical":
            return "transform-move-vertical";
        default:
            return "transform-move";
        }
    }

    spacing: Kirigami.Units.smallSpacing
    Accessible.role: Accessible.Grouping
    Accessible.name: "Touchpad gestures"

    UqLabel {
        id: defaultsLine
        objectName: "defaultsBadge"
        visible: root.defaults
        text: root.defaultsText
        wrapMode: Text.Wrap
        opacity: 0.7
        Layout.fillWidth: true
    }
    SourceLine {
        id: userLuaLine
        objectName: "userLuaLine"
        textName: "userLuaText"
        opensFiles: root.opensFiles
        source: root.userLua ? ({ source: "user_lua", detail: root.userLua }) : null
        Layout.fillWidth: true
    }
    UqLabel {
        objectName: "noGestures"
        visible: root.gestures.length === 0
        text: "No gestures. Add one to use the touchpad for more than pointing."
        wrapMode: Text.Wrap
        opacity: 0.7
        Layout.fillWidth: true
    }
    Repeater {
        id: list
        model: root.gestures

        ColumnLayout {
            id: row

            required property var modelData
            required property int index
            readonly property alias toggle: onSwitch
            readonly property alias changeButton: changeBtn
            readonly property alias removeButton: removeBtn
            readonly property alias clashLine: clash
            readonly property alias nameLabel: what

            objectName: "gestureRow"
            spacing: 0
            Layout.fillWidth: true
            Accessible.role: Accessible.Grouping
            Accessible.name: String(row.modelData.label) + ": " + String(row.modelData.action_label)

            RowLayout {
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true

                // How many fingers, and which way: the gesture at a glance.
                Rectangle {
                    readonly property var g: row.modelData.gesture || ({})
                    implicitWidth: Kirigami.Units.gridUnit * 3
                    implicitHeight: Kirigami.Units.gridUnit * 2
                    radius: Kirigami.Units.cornerRadius
                    color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.15)
                    opacity: row.modelData.enabled === true ? 1 : 0.5
                    Accessible.ignored: true
                    RowLayout {
                        anchors.centerIn: parent
                        spacing: 2
                        UqLabel {
                            text: parent.parent.g.fingers ? String(parent.parent.g.fingers) : ""
                            font.bold: true
                        }
                        Kirigami.Icon {
                            source: root.directionIcon(parent.parent.g.direction)
                            implicitWidth: Kirigami.Units.iconSizes.small
                            implicitHeight: Kirigami.Units.iconSizes.small
                        }
                    }
                }
                ColumnLayout {
                    spacing: 0
                    Layout.fillWidth: true
                    UqLabel {
                        id: what
                        objectName: "gestureWhat"
                        text: String(row.modelData.label)
                        font.bold: true
                        elide: Text.ElideRight
                        opacity: row.modelData.enabled === true ? 1 : 0.6
                        Layout.fillWidth: true
                    }
                    UqLabel {
                        objectName: "gestureAction"
                        text: String(row.modelData.action_label)
                        elide: Text.ElideRight
                        opacity: row.modelData.enabled === true ? 0.8 : 0.5
                        Layout.fillWidth: true
                    }
                }
                QQC2.Switch {
                    id: onSwitch
                    objectName: "gestureSwitch"
                    checked: row.modelData.enabled === true
                    enabled: root.idle
                    Accessible.name: "Use this gesture"
                    Accessible.description: String(row.modelData.label)
                    onToggled: {
                        root.toggled(row.index, onSwitch.checked);
                        onSwitch.checked = Qt.binding(() => row.modelData.enabled === true);
                    }
                }
                QQC2.Button {
                    id: changeBtn
                    objectName: "changeButton"
                    text: "Change"
                    icon.name: "document-edit"
                    display: QQC2.AbstractButton.IconOnly
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "Change"
                    enabled: root.idle
                    Accessible.name: "Change " + String(row.modelData.label)
                    onClicked: root.changeRequested(row.index)
                }
                QQC2.Button {
                    id: removeBtn
                    objectName: "removeButton"
                    text: "Remove"
                    icon.name: "edit-delete"
                    display: QQC2.AbstractButton.IconOnly
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.text: "Remove"
                    enabled: root.idle
                    Accessible.name: "Remove " + String(row.modelData.label)
                    onClicked: root.removed(row.index)
                }
            }
            UqLabel {
                id: clash
                objectName: "clashLine"
                visible: text !== ""
                text: String((root.clashes || {})[String(row.index)] || "")
                wrapMode: Text.Wrap
                color: Kirigami.Theme.negativeTextColor
                Layout.fillWidth: true
            }
        }
    }
    RowLayout {
        spacing: Kirigami.Units.smallSpacing
        Layout.topMargin: Kirigami.Units.smallSpacing

        QQC2.Button {
            id: addBtn
            objectName: "addButton"
            text: "Add a gesture"
            icon.name: "list-add"
            enabled: root.idle
            onClicked: root.addRequested()
        }
        QQC2.Button {
            id: resetBtn
            objectName: "resetButton"
            text: "Back to uQ's gestures"
            icon.name: "edit-undo"
            visible: !root.defaults
            enabled: root.idle
            onClicked: root.resetRequested()
        }
    }
}
