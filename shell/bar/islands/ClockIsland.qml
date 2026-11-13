import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import Quickshell
import "../../core"
import "../../logic/shellprefs.js" as ShellPrefs
import ".."

// The clock (`clock`): time and date, and a popup with this month's calendar. Its options
// (shell.bar.options.clock) add seconds and pick the date style; `compact` shows the time only.
// While the Weather island isn't on the bar, the popup offers to add it.
Island {
    id: root

    readonly property var opts: BarLayout.options.clock || ({})
    readonly property bool seconds: opts.seconds === true
    readonly property string dateStyle: size === "compact" ? "none"
        : (typeof opts.date === "string" ? opts.date : "short")
    readonly property string timeFormat: {
        var f = ShellPrefs.clockFormat(Settings.values.shell);
        return seconds ? f.replace("mm", "mm:ss") : f;
    }

    implicitWidth: row.implicitWidth + Theme.gapIn * 2
    Accessible.role: Accessible.Button
    Accessible.name: Qt.formatDateTime(clock.date, "dddd d MMMM, " + timeFormat)

    SystemClock {
        id: clock
        precision: root.seconds ? SystemClock.Seconds : SystemClock.Minutes
    }

    RowLayout {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        spacing: Theme.gapIn

        UqText {
            text: Qt.formatDateTime(clock.date, root.timeFormat)
            font.family: root.seconds ? Theme.fontMono : Theme.fontUi
        }
        UqText {
            visible: root.dateStyle !== "none"
            text: Qt.formatDateTime(clock.date, root.dateStyle === "long" ? "dddd d MMMM" : "ddd d MMM")
            color: Theme.textMuted
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

    IslandPopup {
        id: popup
    }

    Component {
        id: calendar
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 22

            UqText {
                Layout.alignment: Qt.AlignHCenter
                text: Qt.formatDate(clock.date, "MMMM yyyy")
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
                    font.bold: model.day === clock.date.getDate()
                        && model.month === clock.date.getMonth()
                }
            }
            MenuRow {
                id: addWeather
                visible: !BarLayout.placed("weather")
                Layout.fillWidth: true
                text: "Show weather in the bar"
                onActivated: {
                    BarLayout.addBefore("weather", "clock");
                    popup.hide();
                }
            }
            Component.onCompleted: if (addWeather.visible) addWeather.forceActiveFocus()
        }
    }
}
