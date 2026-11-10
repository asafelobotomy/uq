// One setting: a label on the left, its control(s) on the right. When `highlight` (bound to
// App.highlight by the page) becomes this row's `settingId`, the row scrolls into view and
// flashes for 2 s. Under the label, `source` (uqd's `{source, detail}` for the row) says where the
// live value comes from; `uq` or none shows nothing, so the row keeps its height.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Rectangle {
    id: row

    property string settingId
    property string label
    property string highlight
    readonly property bool flashing: flash.running
    default property alias content: slot.data
    /// Where the live value comes from: `{source, detail}` from the page's `sources`, or null.
    property var source: null
    /// Whether **Open user.lua** opens the file (tests turn it off).
    property bool opensFiles: true
    readonly property string sourceKind: source && source.source ? source.source : ""
    readonly property var sourceDetail: source && source.detail ? source.detail : ({})
    readonly property string sourceText: {
        switch (sourceKind) {
        case "theme":
            return "From the theme";
        case "profile":
            return sourceDetail.name ? "From the " + sourceDetail.name + " profile" : "From a profile";
        case "default":
            return "Default";
        case "user_lua":
            return sourceDetail.line ? "Overridden in user.lua, line " + sourceDetail.line : "Overridden in user.lua";
        case "runtime":
            return "Changed while running (not by uQ)";
        case "error":
            return sourceDetail.message ? String(sourceDetail.message) : "";
        default:
            return "";
        }
    }
    readonly property string userLuaUrl: sourceDetail.path ? "file://" + sourceDetail.path : ""

    /// **Open user.lua** was pressed; `line` is the line that sets this value.
    signal openUserLua(int line)
    /// **Re-apply** was pressed: put uQ's value back over the one changed while running.
    signal reapply()

    function flashNow() {
        flash.restart();
        revealer.reveal(row, Kirigami.Units.largeSpacing);
    }
    function check() {
        if (highlight !== "" && highlight === settingId)
            flashNow();
    }

    implicitWidth: layout.implicitWidth + Kirigami.Units.largeSpacing * 2
    implicitHeight: layout.implicitHeight + Kirigami.Units.smallSpacing * 2
    radius: Kirigami.Units.cornerRadius
    color: flashing ? Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g,
                              Kirigami.Theme.highlightColor.b, 0.25) : "transparent"
    Behavior on color {
        ColorAnimation { duration: Kirigami.Units.shortDuration }
    }

    Accessible.role: Accessible.Grouping
    Accessible.name: label
    Accessible.description: sourceText

    onHighlightChanged: check()
    Component.onCompleted: check()
    // Layout may place the row after the highlight arrives (a page that just loaded); follow it.
    onYChanged: if (flashing) revealer.reveal(row, Kirigami.Units.largeSpacing)

    ScrollReveal {
        id: revealer
    }

    Timer {
        id: flash
        interval: 2000
    }

    RowLayout {
        id: layout
        anchors.fill: parent
        anchors.leftMargin: Kirigami.Units.largeSpacing
        anchors.rightMargin: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.largeSpacing

        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true

            UqLabel {
                text: row.label
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            RowLayout {
                objectName: "sourceLine"
                visible: row.sourceText !== ""
                spacing: Kirigami.Units.smallSpacing
                Layout.fillWidth: true

                Kirigami.Icon {
                    objectName: "sourceWarning"
                    visible: row.sourceKind === "user_lua"
                    source: "dialog-warning"
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: Kirigami.Units.iconSizes.small
                    Accessible.ignored: true
                }
                UqLabel {
                    objectName: "sourceText"
                    text: row.sourceText
                    wrapMode: Text.Wrap
                    opacity: row.sourceKind === "theme" || row.sourceKind === "default" || row.sourceKind === "profile" ? 0.7 : 1
                    color: row.sourceKind === "error" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                    Layout.fillWidth: true
                }
                QQC2.Button {
                    objectName: "openUserLuaButton"
                    visible: row.sourceKind === "user_lua"
                    enabled: row.userLuaUrl !== ""
                    text: "Open user.lua"
                    icon.name: "document-open"
                    onClicked: {
                        row.openUserLua(row.sourceDetail.line || 0);
                        if (row.opensFiles)
                            Qt.openUrlExternally(row.userLuaUrl);
                    }
                }
                QQC2.Button {
                    objectName: "reapplyButton"
                    visible: row.sourceKind === "runtime"
                    text: "Re-apply"
                    icon.name: "view-refresh"
                    onClicked: row.reapply()
                }
            }
        }
        RowLayout {
            id: slot
            spacing: Kirigami.Units.smallSpacing
        }
    }
}
