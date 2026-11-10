// A banner on the Profiles page: what the last Turn on or Back to normal kept and couldn't do,
// or a short notice. Labels can come from an imported profile file (a workspace name, say), so
// every line is a UqLabel (plain text), never a Kirigami.InlineMessage's rich-text label.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Rectangle {
    id: card

    /// Plain labels of the settings kept because they were changed by hand.
    property var kept: []
    /// Notices, one per line.
    property var notices: []
    /// Closed by the user (or by a timer); a new message sets it back to false.
    property bool dismissed: false
    readonly property var lines: {
        const out = [];
        if ((card.kept || []).length > 0)
            out.push("Kept your own value: " + card.kept.join(", "));
        for (const n of card.notices || [])
            out.push(String(n));
        return out;
    }

    visible: lines.length > 0 && !dismissed
    implicitWidth: Kirigami.Units.gridUnit * 24
    implicitHeight: row.implicitHeight + Kirigami.Units.largeSpacing * 2
    radius: Kirigami.Units.cornerRadius
    color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g,
                   Kirigami.Theme.highlightColor.b, 0.12)
    border.width: 1
    border.color: Kirigami.Theme.highlightColor

    Accessible.role: Accessible.AlertMessage
    Accessible.name: lines.join(". ")

    RowLayout {
        id: row
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.smallSpacing

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true

            Repeater {
                model: card.lines
                delegate: UqLabel {
                    required property string modelData
                    required property int index
                    objectName: "messageLine" + index
                    text: modelData
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
            }
        }
        QQC2.ToolButton {
            objectName: "close"
            text: "Close"
            icon.name: "dialog-close"
            display: QQC2.AbstractButton.IconOnly
            Layout.alignment: Qt.AlignTop
            onClicked: card.dismissed = true
        }
    }
}
