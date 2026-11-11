// One shortcut in a list: its name, the keys in words, **Change**, an on/off switch and **Reset**
// (only for one of uQ's that was changed). One of your own also has **Delete**. Under it, where
// the shortcut stands out: "Overridden in user.lua, line N" (SettingRow words it, with Open
// user.lua) and "From the X profile" under the switch. The lock
// recovery shortcut's switch is disabled and says why. Names, keys, action text and profile
// names are plain text (UqLabel). The row only reports what was asked; the page does it.
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
        objectName: "comboText"
        text: row.hasKeys ? String(row.bind.combo_text) : "No keys"
        opacity: row.hasKeys && row.bind.enabled !== false ? 1 : 0.6
        font.bold: row.hasKeys
        Layout.alignment: Qt.AlignVCenter
    }
    UqLabel {
        objectName: "actionText"
        visible: row.custom
        text: String(row.bind.action_text || "")
        opacity: 0.7
        Layout.alignment: Qt.AlignVCenter
    }
    QQC2.Button {
        objectName: "change"
        text: "Change"
        icon.name: "document-edit"
        enabled: row.idle
        Accessible.description: row.title
        onClicked: row.changeRequested()
    }
    QQC2.ToolButton {
        objectName: "reset"
        visible: !row.custom && row.bind.changed === true
        text: "Reset"
        icon.name: "edit-undo"
        display: QQC2.AbstractButton.IconOnly
        enabled: row.idle
        Accessible.description: row.title + ". Back to " + (row.bind.default_combo_text ? String(row.bind.default_combo_text) : "no keys")
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
