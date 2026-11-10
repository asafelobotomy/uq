// A banner on the Window rules page: "Close and reopen … to apply.", the rules section's error,
// what a pick ended with, the "Why does this window…?" answer and refusals. Window titles and app
// IDs come from other programs, rule names and patterns from files, and Hyprland's message from
// Hyprland, so every line is a UqLabel (plain text), never a Kirigami.InlineMessage's rich-text
// label. Built like ProfileMessage.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Rectangle {
    id: card

    /// The lines to show, in order; anything that isn't a string is shown as its text.
    property var lines: []
    /// Something went wrong (drawn in the negative colour) rather than a notice.
    property bool negative: false
    /// Whether the close button is offered.
    property bool closable: true
    /// Closed by the user; a new message sets it back to false.
    property bool dismissed: false
    readonly property var shown: (card.lines || []).map(l => String(l))

    /// "Close and reopen {names} to apply." for up to three names and `more` others.
    function reopenText(names, more) {
        const list = (names || []).map(n => String(n));
        const extra = more > 0 ? more + " more" : "";
        const all = extra === "" ? list : list.concat([extra]);
        if (all.length === 0)
            return "";
        const joined = all.length === 1 ? all[0] : all.slice(0, -1).join(", ") + " and " + all[all.length - 1];
        return "Close and reopen " + joined + " to apply.";
    }
    /// What a pick that didn't give a window ended with; "" for a window.
    function pickText(status) {
        switch (status) {
        case "picked":
            return "";
        case "none":
            return "No window there.";
        case "cancelled":
            return "Choosing a window was cancelled.";
        case "no_shell":
            return "The desktop shell didn't answer. Try again.";
        case "timeout":
            return "Took too long; nothing was chosen.";
        default:
            return "Couldn't start choosing a window.";
        }
    }
    /// Show `lines` again, even if the user closed the last message.
    function show(lines) {
        card.lines = lines;
        card.dismissed = false;
    }

    visible: shown.length > 0 && !dismissed
    implicitWidth: Kirigami.Units.gridUnit * 24
    implicitHeight: row.implicitHeight + Kirigami.Units.largeSpacing * 2
    radius: Kirigami.Units.cornerRadius
    readonly property color tone: card.negative ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.highlightColor
    color: Qt.rgba(tone.r, tone.g, tone.b, 0.12)
    border.width: 1
    border.color: tone

    Accessible.role: Accessible.AlertMessage
    Accessible.name: shown.join(". ")

    RowLayout {
        id: row
        anchors.fill: parent
        anchors.margins: Kirigami.Units.largeSpacing
        spacing: Kirigami.Units.smallSpacing

        ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true

            Repeater {
                model: card.shown
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
            visible: card.closable
            text: "Close"
            icon.name: "dialog-close"
            display: QQC2.AbstractButton.IconOnly
            Layout.alignment: Qt.AlignTop
            onClicked: card.dismissed = true
        }
    }
}
