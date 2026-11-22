import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import Quickshell
import "../../core"
import "../../logic/shellprefs.js" as ShellPrefs
import ".."

// The clock (`clock`). Icon: the hour over the minutes. Small: the time. Medium: the time and
// the short date. Large: the time and the date as chosen (shell.bar.options.clock: seconds, and
// no, short or long date). The 12/24-hour clock (shell.clock_24h) and UK/US dates
// (shell.date_format) are set in Settings > Date and time. A click shows this month's calendar,
// today framed, its weeks starting on Monday (UK) or Sunday (US).
Island {
    id: root

    readonly property var opts: BarLayout.options.clock || ({})
    readonly property bool seconds: opts.seconds === true && !isIcon
    readonly property string dateStyle: (size === "icon" || size === "small") ? "none"
        : (opts.date === "none" ? "none" : (size === "medium" ? "short" : (opts.date === "long" ? "long" : "short")))
    readonly property var dates: ShellPrefs.datePatterns(Settings.values.shell)
    readonly property var weekLocale: Qt.locale(dates.locale)
    readonly property string timeFormat: {
        var f = ShellPrefs.clockFormat(Settings.values.shell);
        return seconds ? f.replace("mm", "mm:ss") : f;
    }

    Accessible.role: Accessible.Button
    Accessible.name: Qt.formatDateTime(clock.date, dates.long) + ", " + Qt.formatDateTime(clock.date, timeFormat)

    SystemClock {
        id: clock
        precision: root.seconds ? SystemClock.Seconds : SystemClock.Minutes
    }

    // Icon: two short lines, so they stay readable in a square.
    Column {
        visible: root.isIcon
        anchors.centerIn: parent
        spacing: -2
        UqText {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDateTime(clock.date, ShellPrefs.clockFormat(Settings.values.shell) === "HH:mm" ? "HH" : "h")
            baseBold: true
        }
        UqText {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDateTime(clock.date, "mm")
            color: Theme.textMuted
        }
    }

    RowLayout {
        visible: !root.isIcon
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.room)
        spacing: Theme.gapIn

        UqText {
            text: Qt.formatDateTime(clock.date, root.timeFormat)
            baseFamily: root.seconds ? Theme.fontMono : Theme.fontUi
            elide: Text.ElideNone
            Layout.minimumWidth: implicitWidth
        }
        UqText {
            visible: root.dateStyle !== "none"
            text: Qt.formatDateTime(clock.date, root.dateStyle === "long" ? root.dates.long : root.dates.short)
            color: Theme.textMuted
            Layout.fillWidth: true
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: {
            if (popup.open && popup.anchorItem === root)
                popup.hide();
            else
                popup.show(root, calendar);
        }
    }

    LazyPopup {
        id: popup
    }

    Component {
        id: calendar
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 22

            UqText {
                Layout.alignment: Qt.AlignHCenter
                text: Qt.formatDate(clock.date, root.dates.full)
            }
            DayOfWeekRow {
                Layout.fillWidth: true
                locale: root.weekLocale
                delegate: UqText {
                    required property string shortName
                    horizontalAlignment: Text.AlignHCenter
                    text: shortName
                    color: Theme.textMuted
                }
            }
            MonthGrid {
                Layout.fillWidth: true
                month: clock.date.getMonth()
                year: clock.date.getFullYear()
                locale: root.weekLocale
                // Today sits in a frame in the accent colour; days of the months either side
                // are muted.
                delegate: Item {
                    id: day
                    required property var model
                    // From the clock, not the model's `today`, so it moves on at midnight.
                    readonly property bool today: model.day === clock.date.getDate()
                        && model.month === clock.date.getMonth() && model.year === clock.date.getFullYear()
                    implicitWidth: dayText.implicitWidth + 8
                    implicitHeight: dayText.implicitHeight + 6
                    Accessible.role: Accessible.StaticText
                    Accessible.name: today ? "Today, " + Qt.formatDate(model.date, root.dates.long) : String(model.day)
                    Rectangle {
                        anchors.centerIn: parent
                        visible: day.today
                        width: Math.max(parent.height, dayText.implicitWidth + 8)
                        height: parent.height
                        radius: Math.min(Theme.radius, height / 2)
                        color: Qt.rgba(Theme.accent.r, Theme.accent.g, Theme.accent.b, 0.18)
                        border.color: Theme.accent
                        border.width: 2
                    }
                    UqText {
                        id: dayText
                        anchors.centerIn: parent
                        horizontalAlignment: Text.AlignHCenter
                        text: day.model.day
                        color: day.today ? Theme.accent : (day.model.month === clock.date.getMonth() ? Theme.text : Theme.textMuted)
                        baseBold: day.today
                    }
                }
            }
        }
    }
}
