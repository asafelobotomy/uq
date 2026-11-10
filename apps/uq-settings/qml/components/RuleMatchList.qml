// "Matches now": the open windows a draft window rule matches, from RulesModel.preview(). Titles
// and app IDs come from other programs, so each is a UqLabel (plain text). A rule with no
// matches can still be saved: the window may not be open yet.
import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: list

    /// `[{title, class, verdict}]`; `verdict` is "yes", "if_dialog" or "unknown".
    property var matches: []
    readonly property var rows: list.matches || []

    /// The note beside a row that may or may not match; "" for a sure match.
    function verdictText(verdict) {
        switch (verdict) {
        case "if_dialog":
            return "Matches if it's a dialog";
        case "unknown":
            return "Can't tell from here";
        default:
            return "";
        }
    }

    spacing: Kirigami.Units.smallSpacing

    UqLabel {
        text: "Matches now"
        font.bold: true
        Accessible.role: Accessible.Heading
    }
    UqLabel {
        objectName: "noMatches"
        visible: list.rows.length === 0
        text: "No open window matches. You can still save the rule."
        wrapMode: Text.Wrap
        opacity: 0.7
        Layout.fillWidth: true
    }
    Repeater {
        model: list.rows
        delegate: RowLayout {
            id: matchRow

            required property var modelData
            readonly property string note: list.verdictText(modelData.verdict)

            spacing: Kirigami.Units.largeSpacing
            Layout.fillWidth: true
            Accessible.role: Accessible.ListItem
            Accessible.name: String(modelData.title)
            Accessible.description: String(modelData["class"]) + (note !== "" ? ". " + note : "")

            UqLabel {
                objectName: "matchTitle"
                text: String(matchRow.modelData.title)
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            UqLabel {
                objectName: "matchClass"
                text: String(matchRow.modelData["class"])
                elide: Text.ElideMiddle
                opacity: 0.7
                Layout.maximumWidth: Kirigami.Units.gridUnit * 12
            }
            UqLabel {
                objectName: "matchVerdict"
                visible: matchRow.note !== ""
                text: matchRow.note
                font.italic: true
            }
        }
    }
}
