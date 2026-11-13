import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import "../../core"
import "../../logic/weather.js" as Weather
import ".."

// Weather (`weather`): icon and temperature, and the city unless the screen is shared or the
// island is compact. The popup has today's high and low, the next hours, a city search and
// "Turn off weather". Open-Meteo is asked by the bridge, with the chosen location only.
Island {
    id: root

    readonly property int iconSize: Theme.fontSize + 7
    readonly property var wx: Bridge.weather
    readonly property bool hasWeather: wx !== null && wx !== undefined
    readonly property bool hasLocation: !!Settings.values.weather_location
    readonly property bool sharing: Bridge.sharing

    implicitWidth: row.implicitWidth + Theme.gapIn * 2
    Accessible.role: Accessible.Button
    Accessible.name: hasWeather ? "Weather: " + Weather.format(wx.temp_c)
        + (wx.location_name && !sharing ? " in " + wx.location_name : "")
        : (hasLocation ? "Weather: loading" : "Weather: choose a city")

    // --- city search ---
    property var results: []
    property string error: ""
    property int queryGen: 0
    function onQueryEdited(s) {
        queryGen++;
        results = [];
        error = "";
        if (Weather.validQuery(s))
            debounce.restart();
        else
            debounce.stop();
    }
    function runQuery(q) {
        var gen = queryGen;
        Bridge.call("GeocodeCity", [q.trim()]).then(function (r) {
            if (gen !== root.queryGen)
                return;
            var list = Array.isArray(r) ? r.slice(0, 5) : [];
            root.results = list;
            root.error = list.length === 0 ? "Couldn't find that city" : "";
        }).catch(function () {
            if (gen !== root.queryGen)
                return;
            root.results = [];
            root.error = "Couldn't find that city";
        });
    }
    function choose(r) {
        var loc = Weather.locationArg(r);
        if (loc === null)
            return;
        Bridge.call("SetSetting", ["weather_location", loc]);
        popup.hide();
    }
    // Off means both: no more lookups and no island.
    function turnOff() {
        Bridge.call("SetSetting", ["weather_location", null]);
        BarLayout.removeId("weather");
        popup.hide();
    }
    property string pendingQuery: ""
    Timer {
        id: debounce
        interval: 400
        onTriggered: root.runQuery(root.pendingQuery)
    }

    RowLayout {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
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
            visible: root.hasWeather
            text: root.hasWeather ? Weather.format(root.wx.temp_c) : ""
        }
        UqText {
            visible: !root.sharing && root.size !== "compact" && root.hasWeather && !!root.wx.location_name
            text: root.hasWeather && root.wx.location_name ? String(root.wx.location_name) : ""
            color: Theme.textMuted
            Layout.maximumWidth: Theme.fontSize * (root.size === "wide" ? 16 : 10)
        }
        UqText {
            visible: !root.hasWeather && root.size !== "compact"
            text: root.hasLocation ? "…" : "Set city"
            color: Theme.textMuted
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: {
            if (popup.open && popup.anchorItem === root) {
                popup.hide();
            } else {
                root.results = [];
                root.error = "";
                root.queryGen++;
                popup.show(root, details);
            }
        }
    }

    IslandPopup {
        id: popup
    }

    Component {
        id: details
        ColumnLayout {
            spacing: Theme.gapIn
            width: Theme.fontSize * 24

            // Now and today
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
                        font.pixelSize: Math.round(Theme.fontSize * 1.4)
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
            // Next hours
            RowLayout {
                readonly property var hours: root.hasWeather && Array.isArray(root.wx.hourly)
                    ? root.wx.hourly.slice(0, root.size === "wide" ? 8 : 6) : []
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
                text: root.hasLocation ? "Change city" : "Choose a city for the weather"
                color: Theme.textMuted
            }
            UqInput {
                id: cityInput
                Layout.fillWidth: true
                placeholder: "City"
                Accessible.name: "City"
                onEdited: function (s) {
                    root.pendingQuery = s;
                    root.onQueryEdited(s);
                }
                Keys.onDownPressed: nextItemInFocusChain(true).forceActiveFocus()
                Component.onCompleted: forceActiveFocus()
            }
            UqText {
                visible: root.error !== ""
                text: root.error
                color: Theme.textMuted
            }
            Repeater {
                model: root.results
                delegate: MenuRow {
                    required property var modelData
                    Layout.fillWidth: true
                    text: String(modelData.name)
                    detail: modelData.country ? String(modelData.country) : ""
                    onActivated: root.choose(modelData)
                }
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Turn off weather"
                onActivated: root.turnOff()
            }
        }
    }
}
