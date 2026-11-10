// The header's "Changed this session (N)" button and the sheet it opens: every change since the
// app started, from anywhere (this app, the terminal, the panel, ...), each with its summary,
// where it came from and an Undo button. All of it is uqd's text, shown through UqLabel.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

QQC2.ToolButton {
    id: root

    /// History entries `{id, time, origin, domain, summary}`, newest first, as uqd lists them.
    property var entries: []
    /// Unix seconds the app started at: entries from then on are this session's.
    property real sessionStart: 0
    readonly property var session: (root.entries || []).filter(e => e.time >= root.sessionStart)
    /// The sheet's rows, one per entry of `session` (for tests).
    readonly property alias rows: rowRepeater
    readonly property bool sheetOpen: sheet.visible
    /// The user asked to undo entry `id`.
    signal undoRequested(string id)

    function open() {
        sheet.open();
    }

    objectName: "sessionChangesButton"
    Accessible.name: countLabel.text
    contentItem: UqLabel {
        id: countLabel
        objectName: "sessionCountLabel"
        text: "Changed this session (" + root.session.length + ")"
        verticalAlignment: Text.AlignVCenter
    }
    onClicked: sheet.open()

    Kirigami.OverlaySheet {
        id: sheet

        title: "Changed this session"

        ColumnLayout {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 28
            spacing: Kirigami.Units.largeSpacing

            UqLabel {
                text: "Nothing has changed since uQ Settings opened."
                visible: root.session.length === 0
                opacity: 0.7
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            Repeater {
                id: rowRepeater

                model: root.session
                delegate: RowLayout {
                    id: row

                    required property var modelData

                    spacing: Kirigami.Units.largeSpacing
                    Layout.fillWidth: true

                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true

                        UqLabel {
                            objectName: "summaryText"
                            text: row.modelData.summary
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                        RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            UqLabel {
                                objectName: "timeText"
                                text: Qt.formatTime(new Date(row.modelData.time * 1000), "hh:mm")
                                opacity: 0.7
                            }
                            OriginLabel {
                                objectName: "originText"
                                origin: row.modelData.origin
                                opacity: 0.7
                            }
                        }
                    }
                    QQC2.Button {
                        objectName: "undoButton"
                        text: "Undo"
                        icon.name: "edit-undo"
                        Accessible.name: "Undo " + row.modelData.summary
                        onClicked: root.undoRequested(row.modelData.id)
                    }
                }
            }
        }
    }
}
