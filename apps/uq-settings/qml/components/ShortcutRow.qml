// One shortcut in a list: its name, the keys drawn as keycaps (pressing them is **Change**), an
// on/off switch and **Reset** (only for one of uQ's that was changed). One of your own also says
// what it does and has **Delete**. Under it, where the shortcut stands out: "Overridden in
// user.lua, line N" (SettingRow words it, with Open user.lua) and "From the X profile" under the
// switch. The lock recovery shortcut's switch is disabled and says why. Names, keys, action text
// and profile names are plain text (UqLabel). The row only reports what was asked; the page does
// it.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "."

SettingRow {
    id: row

    /// A row of the page's snapshot: `{id, label, combo_text, default_combo_text, enabled,
    /// changed, can_disable, action_text, source, line, path, profile, name?}`.
    property var bind: ({})
    /// Whether a request can be sent now (the page isn't busy).
    property bool idle: true
    /// One of your own shortcuts (it has a name and can be deleted).
    readonly property bool custom: bind.name !== undefined && bind.name !== null
    readonly property string title: custom ? String(bind.name) : String(bind.label)
    readonly property bool hasKeys: bind.combo_text !== undefined && bind.combo_text !== null && String(bind.combo_text) !== ""
    /// "From the <name> profile", worded as SettingRow words it; "" when no profile holds it.
    readonly property string profileText: bind.profile ? "From the " + String(bind.profile) + " profile" : ""
    readonly property alias toggle: onSwitch
    readonly property string alwaysOnText: "Always on: it gets you back in if the lock screen stops working."

    /// **Change** was pressed.
    signal changeRequested()
    /// The switch was flipped to `on`.
    signal switched(bool on)
    /// **Reset** was pressed (one of uQ's, changed).
    signal resetRequested()
    /// **Delete** was pressed (one of your own).
    signal deleteRequested()

    label: row.title
    // Only user.lua is said under the name: "Default" and a profile's switch aren't, and a
    // refused change is one banner on the page, not a line on every row.
    source: bind.source === "user_lua" ? ({ source: "user_lua", detail: { line: bind.line, path: bind.path } }) : null
    Accessible.name: title
    Accessible.description: hasKeys ? String(bind.combo_text) : "No keys"

    UqLabel {
        objectName: "actionText"
        visible: row.custom
        text: String(row.bind.action_text || "")
        opacity: 0.7
        elide: Text.ElideRight
        Layout.maximumWidth: Kirigami.Units.gridUnit * 12
        Layout.alignment: Qt.AlignVCenter
    }
    // The keys, drawn as keycaps; pressing them opens the editor. A plain AbstractButton, so the
    // style draws nothing but the keycaps and a hover or focus wash.
    QQC2.AbstractButton {
        id: changeButton
        objectName: "change"
        text: "Change"
        enabled: row.idle
        focusPolicy: Qt.StrongFocus
        padding: Kirigami.Units.smallSpacing
        hoverEnabled: true
        Accessible.role: Accessible.Button
        Accessible.name: "Change"
        Accessible.description: row.title + ". " + (row.hasKeys ? String(row.bind.combo_text) : "No keys")
        QQC2.ToolTip.text: "Change"
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        onClicked: row.changeRequested()
        background: Rectangle {
            radius: Kirigami.Units.cornerRadius
            color: changeButton.down || changeButton.hovered ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.2) : "transparent"
            border.width: changeButton.visualFocus ? 1 : 0
            border.color: Kirigami.Theme.highlightColor
        }
        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing
            opacity: changeButton.enabled ? 1 : 0.6
            KeyCaps {
                objectName: "keycaps"
                visible: row.hasKeys
                combo: row.hasKeys ? String(row.bind.combo_text) : ""
                opacity: row.bind.enabled !== false ? 1 : 0.6
            }
            UqLabel {
                objectName: "comboText"
                visible: !row.hasKeys
                text: row.hasKeys ? String(row.bind.combo_text) : "No keys"
                opacity: row.hasKeys && row.bind.enabled !== false ? 1 : 0.6
            }
        }
    }
    QQC2.ToolButton {
        objectName: "reset"
        visible: !row.custom && row.bind.changed === true
        text: "Reset"
        icon.name: "edit-undo"
        display: QQC2.AbstractButton.IconOnly
        enabled: row.idle
        Accessible.description: row.title + ". Back to " + (row.bind.default_combo_text ? String(row.bind.default_combo_text) : "no keys")
        QQC2.ToolTip.text: "Back to " + (row.bind.default_combo_text ? String(row.bind.default_combo_text) : "no keys")
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        onClicked: row.resetRequested()
    }
    QQC2.ToolButton {
        objectName: "delete"
        visible: row.custom
        text: "Delete"
        icon.name: "edit-delete"
        display: QQC2.AbstractButton.IconOnly
        enabled: row.idle
        Accessible.description: row.title
        QQC2.ToolTip.text: "Delete"
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        onClicked: row.deleteRequested()
    }
    ColumnLayout {
        spacing: 0

        QQC2.Switch {
            id: onSwitch
            objectName: "enabled"
            checked: row.bind.enabled !== false
            enabled: row.idle && row.bind.can_disable !== false
            Layout.alignment: Qt.AlignRight
            Accessible.name: "Shortcut on"
            Accessible.description: row.bind.can_disable === false ? row.title + ". " + row.alwaysOnText : (row.profileText !== "" ? row.title + ". " + row.profileText : row.title)
            onToggled: {
                row.switched(onSwitch.checked);
                onSwitch.checked = Qt.binding(() => row.bind.enabled !== false);
            }
        }
        UqLabel {
            objectName: "alwaysOn"
            visible: row.bind.can_disable === false
            text: row.alwaysOnText
            wrapMode: Text.Wrap
            opacity: 0.7
            Layout.maximumWidth: Kirigami.Units.gridUnit * 14
            Layout.alignment: Qt.AlignRight
        }
        UqLabel {
            objectName: "shortcutProfile"
            visible: row.profileText !== ""
            text: row.profileText
            opacity: 0.7
            Layout.alignment: Qt.AlignRight
        }
    }
}
