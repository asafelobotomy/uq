import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import Quickshell
import "../../core"
import "../../logic/shellprefs.js" as ShellPrefs
import ".."

// The clock (`clock`). Icon: the hour over the minutes. Small: the time. Medium: the time and
// the short date. Large: the time and the date as chosen (shell.bar.options.clock: seconds, and
// no, short or long date). A click shows this month's calendar.
Island {
    id: root

    readonly property var opts: BarLayout.options.clock || ({})
    readonly property bool seconds: opts.seconds === true && !isIcon
    readonly property string dateStyle: (size === "icon" || size === "small") ? "none"
        : (opts.date === "none" ? "none" : (size === "medium" ? "short" : (opts.date === "long" ? "long" : "short")))
    readonly property string timeFormat: {
        var f = ShellPrefs.clockFormat(Settings.values.shell);
        return seconds ? f.replace("mm", "mm:ss") : f;
    }

    Accessible.role: Accessible.Button
    Accessible.name: Qt.formatDateTime(clock.date, "dddd d MMMM, " + timeFormat)

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
            text: Qt.formatDateTime(clock.date, root.dateStyle === "long" ? "dddd d MMMM" : "ddd d MMM")
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
                text: Qt.formatDate(clock.date, "dddd d MMMM yyyy")
            }
            DayOfWeekRow {
                Layout.fillWidth: true
                locale: Qt.locale()
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
                locale: Qt.locale()
                delegate: UqText {
                    required property var model
                    horizontalAlignment: Text.AlignHCenter
                    text: model.day
                    color: model.month === clock.date.getMonth() ? Theme.text : Theme.textMuted
                    baseBold: model.day === clock.date.getDate()
                        && model.month === clock.date.getMonth()
                }
            }
        }
    }
}
