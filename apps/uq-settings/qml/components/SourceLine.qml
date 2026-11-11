// Where a list row's value comes from, worded as SettingRow words a source: "Overridden in
// user.lua, line N" (with Open user.lua), an error's message, or "From the X profile". Rows of
// a list (a startup app, a variable) use it where a whole SettingRow would be too much. `uq`,
// `default` or none shows nothing. Profile names and messages are plain text.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

RowLayout {
    id: line

    /// uqd's `{source, detail}` for the row, or null.
    property var source: null
    /// The objectName of the text, so tests and pages can find it.
    property string textName: "sourceText"
    /// Whether **Open user.lua** opens the file (tests turn it off).
    property bool opensFiles: true
    readonly property string kind: source && source.source ? String(source.source) : ""
    readonly property var detail: source && source.detail ? source.detail : ({})
    readonly property string text: {
        switch (kind) {
        case "profile":
            return detail.name ? "From the " + String(detail.name) + " profile" : "From a profile";
        case "user_lua":
            return detail.line ? "Overridden in user.lua, line " + detail.line : "Overridden in user.lua";
        case "error":
            return detail.message ? String(detail.message) : "";
        default:
            return "";
        }
    }
    readonly property string userLuaUrl: detail.path ? "file://" + detail.path : ""

    visible: text !== ""
    spacing: Kirigami.Units.smallSpacing

    Kirigami.Icon {
        visible: line.kind === "user_lua"
        source: "dialog-warning"
        implicitWidth: Kirigami.Units.iconSizes.small
        implicitHeight: Kirigami.Units.iconSizes.small
        Accessible.ignored: true
    }
    UqLabel {
        objectName: line.textName
        text: line.text
        wrapMode: Text.Wrap
        opacity: line.kind === "profile" ? 0.7 : 1
        color: line.kind === "error" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
        Layout.fillWidth: true
    }
    QQC2.Button {
        visible: line.kind === "user_lua"
        enabled: line.userLuaUrl !== ""
        text: "Open user.lua"
        icon.name: "document-open"
        onClicked: {
            if (line.opensFiles)
                Qt.openUrlExternally(line.userLuaUrl);
        }
    }
}
