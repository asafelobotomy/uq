// "Are you sure?" before a plugin page's command runs (Phase 4 spec §2.3 `confirm`). The question
// is the page's fixed text from page.toml, shown as plain text. The command runs only on an
// explicit Confirm; Cancel, Escape or closing the dialog cancels it. Neither button is the
// default, so Enter does nothing. Plain controls only, so it works without Kirigami.
//
// With a `preview` (uq_api 3) the question is followed by the plan: what would change, grouped,
// with sizes and whether each can be put back. A plan that says it would be refused shows why and
// offers no Confirm. The plan is cleaned plain text (uq_core::integration::plan).
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts

QQC2.Dialog {
    id: dialog

    /// The request waiting for an answer; -1 when none.
    property int request: -1
    /// The question, from page.toml (plain text).
    property string question
    /// The previewed plan ({summary, changes, more, would_refuse, reason}), or null for none.
    property var plan: null
    readonly property bool refused: dialog.plan !== null && dialog.plan.would_refuse === true
    /// The changes of the plan grouped in order of first appearance:
    /// [{name, bytes, items: [change…]}]; a change without a group goes under "Changes".
    readonly property var groups: dialog.groupsOf(dialog.plan)
    readonly property alias confirmButton: confirmBtn
    readonly property alias cancelButton: cancelBtn
    readonly property alias closeButton: closeBtn
    readonly property alias questionLabel: questionText

    /// The answer for `request`: true only for Confirm.
    signal answered(int request, bool confirmed)

    function groupsOf(plan) {
        if (!plan || !Array.isArray(plan.changes))
            return [];
        const order = [];
        const byName = {};
        for (const c of plan.changes) {
            const name = c.group || "Changes";
            if (!(name in byName)) {
                byName[name] = { name: name, bytes: 0, known: false, items: [] };
                order.push(name);
            }
            const g = byName[name];
            g.items.push(c);
            if (typeof c.bytes === "number") {
                g.bytes += c.bytes;
                g.known = true;
            }
        }
        return order.map(n => byName[n]);
    }

    function sizeText(bytes) {
        if (typeof bytes !== "number")
            return "";
        const units = ["B", "kB", "MB", "GB", "TB"];
        let v = bytes;
        let i = 0;
        while (v >= 1000 && i < units.length - 1) {
            v /= 1000;
            i++;
        }
        return (i === 0 ? String(v) : v.toFixed(v < 10 ? 1 : 0)) + " " + units[i];
    }

    function undoText(change) {
        if (change.reversible === true)
            return "can be put back";
        if (change.reversible === false)
            return "can't be undone";
        return "";
    }

    /// Ask about `request`, with its previewed `plan` if it has one. A question still open is
    /// cancelled first.
    function ask(req, text, plan) {
        if (dialog.request >= 0)
            dialog.reply(false);
        dialog.request = req;
        dialog.question = text;
        dialog.plan = plan || null;
        dialog.open();
    }

    function reply(confirmed) {
        const req = dialog.request;
        if (req < 0)
            return;
        dialog.request = -1;
        // A refused plan can't be confirmed, however the button was reached.
        dialog.answered(req, confirmed && !dialog.refused);
        if (dialog.opened)
            dialog.close();
    }

    title: "Are you sure?"
    modal: true
    closePolicy: QQC2.Popup.CloseOnEscape
    anchors.centerIn: parent
    standardButtons: QQC2.Dialog.NoButton
    width: Math.min(dialog.plan ? 560 : 420, parent ? parent.width : 420)

    // Escape, or the window closing it, is "no".
    onClosed: dialog.reply(false)

    contentItem: ColumnLayout {
        spacing: 12

        UqLabel {
            id: questionText
            objectName: "confirmQuestion"
            text: dialog.question
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "confirmSummary"
            visible: dialog.plan !== null && text !== ""
            text: dialog.plan ? dialog.plan.summary : ""
            font.bold: true
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "confirmRefusal"
            visible: dialog.refused
            text: dialog.refused ? (dialog.plan.reason !== "" ? dialog.plan.reason : "This can't be done right now.") : ""
            wrapMode: Text.Wrap
            Layout.fillWidth: true
        }
        UqLabel {
            objectName: "confirmNothing"
            visible: dialog.plan !== null && !dialog.refused && dialog.groups.length === 0
            text: "Nothing would change."
            opacity: 0.7
            Layout.fillWidth: true
        }
        QQC2.ScrollView {
            objectName: "confirmPlan"
            visible: dialog.groups.length > 0
            clip: true
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(planColumn.implicitHeight, 280)

            ColumnLayout {
                id: planColumn
                width: dialog.width - 48
                spacing: 8

                Repeater {
                    model: dialog.groups
                    delegate: ColumnLayout {
                        required property var modelData
                        spacing: 2
                        Layout.fillWidth: true

                        RowLayout {
                            Layout.fillWidth: true
                            UqLabel {
                                objectName: "planGroupName"
                                text: modelData.name
                                font.bold: true
                                Layout.fillWidth: true
                            }
                            UqLabel {
                                text: modelData.known ? dialog.sizeText(modelData.bytes) : ""
                                opacity: 0.7
                            }
                        }
                        Repeater {
                            model: modelData.items
                            delegate: RowLayout {
                                required property var modelData
                                Layout.fillWidth: true
                                Layout.leftMargin: 12
                                spacing: 8

                                ColumnLayout {
                                    spacing: 0
                                    Layout.fillWidth: true
                                    UqLabel {
                                        objectName: "planItem"
                                        text: modelData.label
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                    UqLabel {
                                        visible: text !== ""
                                        text: modelData.detail || ""
                                        opacity: 0.6
                                        elide: Text.ElideMiddle
                                        Layout.fillWidth: true
                                    }
                                }
                                ColumnLayout {
                                    spacing: 0
                                    UqLabel {
                                        text: dialog.sizeText(modelData.bytes)
                                        horizontalAlignment: Text.AlignRight
                                        Layout.alignment: Qt.AlignRight
                                    }
                                    UqLabel {
                                        visible: text !== ""
                                        text: dialog.undoText(modelData)
                                        opacity: 0.6
                                        horizontalAlignment: Text.AlignRight
                                        Layout.alignment: Qt.AlignRight
                                    }
                                }
                            }
                        }
                    }
                }
                UqLabel {
                    visible: dialog.plan !== null && dialog.plan.more > 0
                    text: dialog.plan ? "And " + dialog.plan.more + " more." : ""
                    opacity: 0.7
                }
            }
        }
        RowLayout {
            Layout.alignment: Qt.AlignRight
            QQC2.Button {
                id: cancelBtn
                objectName: "confirmCancel"
                text: "Cancel"
                visible: !dialog.refused
                onClicked: dialog.reply(false)
            }
            QQC2.Button {
                id: closeBtn
                objectName: "confirmClose"
                text: "Close"
                visible: dialog.refused
                onClicked: dialog.reply(false)
            }
            QQC2.Button {
                id: confirmBtn
                objectName: "confirmRun"
                text: "Confirm"
                visible: !dialog.refused
                onClicked: dialog.reply(true)
            }
        }
    }
}
