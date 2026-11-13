import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import "../../core"
import "../../logic/weather.js" as Weather
import ".."

// Weather (`weather`). Icon: the conditions. Small: and the temperature. Medium: and the city
// (never while the screen is shared). Large: and today's high and low. A click shows the
// forecast for the next hours. The city is chosen in edit mode (the island's settings); the
// bridge asks Open-Meteo, with that location only, while this island is on the bar.
Island {
    id: root

    readonly property int iconSize: Theme.fontSize + 7
    readonly property var wx: Bridge.weather
    readonly property bool hasWeather: wx !== null && wx !== undefined
    readonly property bool hasLocation: !!Settings.values.weather_location
    readonly property bool sharing: Bridge.sharing
    readonly property bool showCity: !sharing && (size === "medium" || size === "large")
        && hasWeather && !!wx.location_name

    Accessible.role: Accessible.Button
    Accessible.name: hasWeather ? "Weather: " + Weather.format(wx.temp_c)
        + (wx.location_name && !sharing ? " in " + wx.location_name : "")
        : (hasLocation ? "Weather: loading" : "Weather: no city chosen")

    RowLayout {
        anchors.centerIn: parent
        width: Math.min(implicitWidth, root.room)
        spacing: Theme.gapIn

        IconImage {
            implicitSize: root.iconSize
            Layout.preferredWidth: root.iconSize
            Layout.preferredHeight: root.iconSize
            opacity: root.hasWeather ? 1 : 0.6
            source: Quickshell.iconPath(root.hasWeather ? Weather.icon(root.wx.code, root.wx.is_day)
                : "weather-few-clouds", true)
        }
        UqText {
            visible: !root.isIcon && root.hasWeather
            text: root.hasWeather ? Weather.format(root.wx.temp_c) : ""
            Layout.minimumWidth: implicitWidth
        }
        UqText {
            visible: root.showCity
            text: root.showCity ? String(root.wx.location_name) : ""
            color: Theme.textMuted
            Layout.fillWidth: true
        }
        UqText {
            readonly property var today: root.hasWeather && root.wx.today ? root.wx.today : null
            visible: root.size === "large" && today !== null
            text: today ? Weather.format(today.max_c) + "/" + Weather.format(today.min_c) : ""
            color: Theme.textMuted
            Layout.minimumWidth: implicitWidth
        }
        UqText {
            visible: !root.isIcon && !root.hasWeather
            text: root.hasLocation ? "…" : "No city"
            color: Theme.textMuted
            Layout.fillWidth: true
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: popup.open && popup.anchorItem === root ? popup.hide() : popup.show(root, details)
    }

    LazyPopup {
        id: popup
    }

    Component {
        id: details
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 24

            RowLayout {
                visible: root.hasWeather
                spacing: Theme.gapIn
                IconImage {
                    implicitSize: root.iconSize * 2
                    Layout.preferredWidth: root.iconSize * 2
                    Layout.preferredHeight: root.iconSize * 2
                    source: root.hasWeather
                        ? Quickshell.iconPath(Weather.icon(root.wx.code, root.wx.is_day), true) : ""
                }
                ColumnLayout {
                    spacing: 0
                    UqText {
                        text: root.hasWeather ? Weather.format(root.wx.temp_c)
                            + (root.wx.location_name && !root.sharing ? " · " + root.wx.location_name : "")
                            : ""
                        baseSize: Math.round(Theme.fontSize * 1.4)
                        Layout.maximumWidth: Theme.fontSize * 20
                    }
                    UqText {
                        readonly property var today: root.hasWeather && root.wx.today ? root.wx.today : null
                        visible: today !== null
                        text: today ? "High " + Weather.format(today.max_c) + "  Low " + Weather.format(today.min_c) : ""
                        color: Theme.textMuted
                    }
                }
            }
            RowLayout {
                readonly property var hours: root.hasWeather && Array.isArray(root.wx.hourly)
                    ? root.wx.hourly.slice(0, 8) : []
                visible: hours.length > 0
                spacing: Theme.gapIn
                Layout.fillWidth: true
                Repeater {
                    model: parent.hours
                    delegate: ColumnLayout {
                        required property var modelData
                        spacing: 2
                        Layout.fillWidth: true
                        UqText {
                            Layout.alignment: Qt.AlignHCenter
                            text: typeof modelData.time === "number"
                                ? Qt.formatTime(new Date(modelData.time * 1000), "HH") : ""
                            color: Theme.textMuted
                        }
                        IconImage {
                            Layout.alignment: Qt.AlignHCenter
                            implicitSize: root.iconSize
                            source: Quickshell.iconPath(Weather.icon(modelData.code, true), true)
                        }
                        UqText {
                            Layout.alignment: Qt.AlignHCenter
                            text: Weather.format(modelData.temp_c)
                        }
                    }
                }
            }
            UqText {
                visible: !root.hasLocation || !root.hasWeather
                text: root.hasLocation ? "Fetching the weather…" : "No city chosen yet."
                color: Theme.textMuted
            }
            MenuRow {
                id: setCity
                Layout.fillWidth: true
                text: root.hasLocation ? "Change the city…" : "Choose a city…"
                onActivated: {
                    popup.hide();
                    root.editRequested();
                }
                Component.onCompleted: forceActiveFocus()
            }
        }
    }
}
