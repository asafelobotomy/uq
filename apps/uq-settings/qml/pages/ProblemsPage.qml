// Problems: what went wrong lately, in plain words. Crashes first, each with Copy report (the
// report, with window titles and names taken out, ready to paste into an issue), Open and
// Dismiss; then one card per warning or error uQ logged in the chosen time, with what it means,
// what to do and how often it happened. Messages, file names and app names come from logs and
// other programs, so every one is shown through UqLabel (plain text). docs/diagnostics.md.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.uq.settings
import "../components"

SettingsPage {
    id: page

    readonly property var snap: {
        try {
            return JSON.parse(pm.stateJson);
        } catch (e) {
            return {};
        }
    }
    readonly property bool loaded: snap.hours !== undefined
    readonly property var crashes: snap.crashes || []
    readonly property var problems: snap.problems || []
    readonly property int hours: snap.hours || 24
    property int noticeSeen: 0

    function refresh() {
        pm.refresh();
    }
    function showNews() {
        const notice = page.snap.notice;
        if (notice && notice.seq > page.noticeSeen) {
            page.noticeSeen = notice.seq;
            noticeMessage.dismissed = false;
            noticeMessage.lines = notice.text !== "" ? [notice.text] : [];
        }
    }
    /// "Today, 11:02", "Yesterday, 23:10" or the date and time of unix time `t`.
    function when(t) {
        const d = new Date(t * 1000);
        const today = new Date();
        const yesterday = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1);
        const time = Qt.formatTime(d, "hh:mm");
        if (d.toDateString() === today.toDateString())
            return "Today, " + time;
        if (d.toDateString() === yesterday.toDateString())
            return "Yesterday, " + time;
        return d.toLocaleDateString(Qt.locale(), Locale.ShortFormat) + ", " + time;
    }
    function severityIcon(s) {
        return s === "error" ? "dialog-error" : (s === "warn" ? "dialog-warning" : "dialog-information");
    }
    function sourceName(s) {
        switch (s) {
        case "uqd": // jargon-ok: a source id from the log, mapped to plain words
            return "uQ's settings service";
        case "shell":
            return "the shell";
        case "lock":
            return "the lock screen";
        case "uqlip":
            return "clipboard history";
        }
        return String(s);
    }
    /// The code lists' text without its Markdown backticks.
    function plain(s) {
        return String(s || "").replace(/`/g, "");
    }
    function timesText(p) {
        if (p.count <= 1)
            return page.when(p.time);
        return p.count + " times, last " + page.when(p.time) + " (first " + page.when(p.first) + ")";
    }

    ProblemsModel {
        id: pm
        onStateJsonChanged: page.showNews()
    }

    banners: [
        RuleMessage {
            objectName: "problemsError"
            negative: true
            closable: true
            lines: pm.error !== "" ? ["That wasn't done: " + pm.error] : []
            Layout.fillWidth: true
        },
        RuleMessage {
            id: noticeMessage
            objectName: "problemsNotice"
            Layout.fillWidth: true
        }
    ]

    SettingsTab {
        title: "Problems"
        iconName: "dialog-warning"

        SettingsCard {
            objectName: "crashCard"
            title: "Crashes"
            iconName: "tools-report-bug"
            help: "When the shell, the lock screen, one of uQ's programs or an app built on uQ crashes, a report is saved here. It has the versions, the screens and the last things logged, with window titles and names taken out."
            wide: true
            actions: [
                QQC2.Button {
                    objectName: "copyNow"
                    text: "Copy a report"
                    icon.name: "edit-copy"
                    flat: true
                    enabled: !pm.busy
                    Accessible.description: "A report on how uQ is doing now, for a problem that isn't a crash"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    QQC2.ToolTip.text: "A report on how uQ is doing now, for a problem that isn't a crash"
                    onClicked: pm.copyReport("")
                }
            ]

            SettingRow {
                settingId: "crashes"
                label: "Crash reports"
                highlight: App.highlight
                visible: page.crashes.length === 0
                Layout.fillWidth: true

                UqLabel {
                    objectName: "noCrashes"
                    text: page.loaded ? "No crashes. Nice." : "Looking…"
                    opacity: 0.7
                }
            }

            Repeater {
                model: page.crashes
                delegate: RowLayout {
                    id: crash

                    required property var modelData

                    objectName: "crash"
                    spacing: Kirigami.Units.largeSpacing
                    Layout.fillWidth: true
                    Layout.leftMargin: Kirigami.Units.smallSpacing

                    Kirigami.Icon {
                        source: "tools-report-bug"
                        implicitWidth: Kirigami.Units.iconSizes.medium
                        implicitHeight: Kirigami.Units.iconSizes.medium
                        color: Kirigami.Theme.negativeTextColor
                        Layout.alignment: Qt.AlignTop
                        Accessible.ignored: true
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true

                        UqLabel {
                            text: crash.modelData.title
                            font.bold: true
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                        UqLabel {
                            text: page.when(crash.modelData.time) + " · " + crash.modelData.reason
                                  + ((crash.modelData.place || []).length > 0 ? " in " + crash.modelData.place[0] : "")
                            opacity: 0.7
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                        UqLabel {
                            text: crash.modelData.code + " · " + crash.modelData.id
                            opacity: 0.5
                            font: Kirigami.Theme.smallFont
                        }
                    }
                    QQC2.Button {
                        objectName: "copyCrash"
                        text: "Copy report"
                        icon.name: "edit-copy"
                        Accessible.name: "Copy the report on " + crash.modelData.title
                        enabled: !pm.busy
                        Layout.alignment: Qt.AlignTop
                        onClicked: pm.copyReport(crash.modelData.id)
                    }
                    QQC2.ToolButton {
                        objectName: "openCrash"
                        text: "Open"
                        icon.name: "document-open"
                        display: QQC2.AbstractButton.IconOnly
                        Accessible.name: "Open the report on " + crash.modelData.title
                        enabled: !pm.busy
                        Layout.alignment: Qt.AlignTop
                        QQC2.ToolTip.text: "Open the report"
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                        onClicked: pm.openReport(crash.modelData.id)
                    }
                    QQC2.ToolButton {
                        objectName: "dismissCrash"
                        text: "Dismiss"
                        icon.name: "window-close"
                        display: QQC2.AbstractButton.IconOnly
                        Accessible.name: "Dismiss " + crash.modelData.title
                        enabled: !pm.busy
                        Layout.alignment: Qt.AlignTop
                        QQC2.ToolTip.text: "Dismiss (the report stays on disk)"
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                        onClicked: pm.dismiss(crash.modelData.id)
                    }
                }
            }
        }

        SettingsCard {
            objectName: "problemsCard"
            title: "Warnings and errors"
            iconName: "dialog-warning"
            help: "What uQ and apps with their own codes logged. Each has a code, so it can be looked up and reported."
            wide: true

            SettingRow {
                settingId: "problems"
                iconName: "view-calendar-day"
                label: "Show the last"
                highlight: App.highlight
                Layout.fillWidth: true

                QQC2.ButtonGroup {
                    id: spanGroup
                }
                QQC2.ToolButton {
                    objectName: "span-1"
                    text: "Hour"
                    checkable: true
                    checked: page.hours === 1
                    QQC2.ButtonGroup.group: spanGroup
                    onClicked: pm.setHours(1)
                }
                QQC2.ToolButton {
                    objectName: "span-24"
                    text: "Day"
                    checkable: true
                    checked: page.hours === 24
                    QQC2.ButtonGroup.group: spanGroup
                    onClicked: pm.setHours(24)
                }
                QQC2.ToolButton {
                    objectName: "span-168"
                    text: "Week"
                    checkable: true
                    checked: page.hours === 168
                    QQC2.ButtonGroup.group: spanGroup
                    onClicked: pm.setHours(168)
                }
            }
            UqLabel {
                objectName: "noProblems"
                text: page.loaded ? "Nothing went wrong in this time." : "Looking…"
                visible: page.problems.length === 0
                opacity: 0.7
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
            UqLabel {
                objectName: "otherCount"
                text: page.snap.other > 0
                      ? (page.snap.other === 1 ? "1 other message" : page.snap.other + " other messages")
                        + " from Qt and other libraries aren't listed. The full log is in a terminal: uqctl log"
                      : ""
                visible: text !== ""
                opacity: 0.6
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
            }
        }

        // One card per problem, newest first.
        Repeater {
            model: page.problems
            delegate: SettingsCard {
                id: problem

                required property var modelData

                objectName: "problem"
                title: problem.modelData.title !== "" ? problem.modelData.title : String(problem.modelData.code)
                iconName: page.severityIcon(problem.modelData.severity)

                UqLabel {
                    text: problem.modelData.code + " · " + page.sourceName(problem.modelData.source)
                          + (problem.modelData.owner !== "" && problem.modelData.owner !== "uQ" ? " (" + problem.modelData.owner + ")" : "")
                          + " · " + page.timesText(problem.modelData)
                    opacity: 0.7
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: page.plain(problem.modelData.means)
                    visible: text !== ""
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                UqLabel {
                    text: problem.modelData.fix !== "" ? "What to do: " + page.plain(problem.modelData.fix) : ""
                    visible: text !== ""
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                UqLabel {
                    objectName: "problemMessage"
                    text: problem.modelData.message
                    font.family: Kirigami.Theme.fixedWidthFont.family
                    font.pointSize: Math.max(7, Kirigami.Theme.defaultFont.pointSize - 1)
                    opacity: 0.6
                    wrapMode: Text.WrapAnywhere
                    maximumLineCount: 4
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
        }
    }
}
