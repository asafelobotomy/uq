import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import Quickshell
import Quickshell.Hyprland
import Quickshell.Widgets
import "../core"
import "../logic/weather.js" as Weather

// Clock, optional weather, and a popover with a month calendar and a "Set city" field.
// The weather icon and temperature stay visible while screen sharing; the city name does not
// (spec §2.10).
Island {
    id: root
    readonly property int iconSize: 18
    readonly property int cell: 26
    readonly property var wx: Bridge.weather
    readonly property bool hasWeather: wx !== null && wx !== undefined
    readonly property bool sharing: Bridge.sharing

    implicitWidth: row.implicitWidth + Theme.gapIn * 2

    SystemClock {
        id: clock
        precision: SystemClock.Minutes
    }

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
    function runQuery() {
        var gen = queryGen;
        var q = cityInput.text.trim();
        Bridge.call("GeocodeCity", [q]).then(function (r) {
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
        popup.visible = false;
    }
    function turnOff() {
        Bridge.call("SetSetting", ["weather_location", null]);
        popup.visible = false;
    }

    Timer {
        id: debounce
        interval: 400
        onTriggered: root.runQuery()
    }

    RowLayout {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        spacing: Theme.gapIn

        RowLayout {
            visible: root.hasWeather
            spacing: 4
            IconImage {
                implicitSize: root.iconSize
                Layout.preferredWidth: root.iconSize
                Layout.preferredHeight: root.iconSize
                source: root.hasWeather
                    ? Quickshell.iconPath(Weather.icon(root.wx.code, root.wx.is_day), true) : ""
            }
            UqText {
                text: root.hasWeather ? Weather.format(root.wx.temp_c) : ""
            }
            UqText {
                visible: !root.sharing && root.hasWeather && !!root.wx.location_name
                text: root.hasWeather && root.wx.location_name ? String(root.wx.location_name) : ""
                color: Theme.textMuted
                Layout.maximumWidth: 120
            }
        }
        UqText {
            text: Qt.formatDateTime(clock.date, "HH:mm")
        }
        UqText {
            text: Qt.formatDateTime(clock.date, "ddd d MMM")
            color: Theme.textMuted
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: {
            if (popup.visible) {
                popup.visible = false;
            } else {
                cityInput.text = "";
                root.results = [];
                root.error = "";
                root.queryGen++;
                popup.anchor.item = root;
                popup.visible = true;
            }
        }
    }

    PopupWindow {
        id: popup
        anchor.window: root.QsWindow.window
        anchor.edges: Edges.Bottom
        anchor.gravity: Edges.Bottom
        implicitWidth: 260
        implicitHeight: col.implicitHeight + Theme.gapIn * 2
        color: "transparent"
        visible: false

        Rectangle {
            anchors.fill: parent
            color: Theme.surface
            radius: Theme.radius
            border.color: Theme.border
            border.width: Theme.borderWidth

            ColumnLayout {
                id: col
                anchors.fill: parent
                anchors.margins: Theme.gapIn
                spacing: Theme.gapIn

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
                        opacity: model.month === clock.date.getMonth() ? 1 : 0.35
                        font.bold: model.day === clock.date.getDate()
                            && model.month === clock.date.getMonth()
                    }
                }

                UqText {
                    text: "Set city"
                    color: Theme.textMuted
                }
                UqInput {
                    id: cityInput
                    Layout.fillWidth: true
                    placeholder: "City"
                    onEdited: function (s) { root.onQueryEdited(s); }
                }
                UqText {
                    visible: root.error !== ""
                    text: root.error
                    color: Theme.textMuted
                }
                Repeater {
                    model: root.results
                    delegate: Rectangle {
                        id: item
                        required property var modelData
                        Layout.fillWidth: true
                        Layout.preferredHeight: root.cell
                        radius: Theme.radius - 2
                        color: itemMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                        UqText {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.leftMargin: Theme.gapIn
                            anchors.rightMargin: Theme.gapIn
                            text: String(item.modelData.name)
                                + (item.modelData.country ? ", " + item.modelData.country : "")
                        }
                        MouseArea {
                            id: itemMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            onClicked: root.choose(item.modelData)
                        }
                    }
                }
                Rectangle {
                    visible: root.hasWeather
                    Layout.fillWidth: true
                    Layout.preferredHeight: visible ? root.cell : 0
                    radius: Theme.radius - 2
                    color: offMouse.containsMouse ? Theme.surfaceAlt : "transparent"
                    UqText {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: Theme.gapIn
                        text: "Turn off weather"
                    }
                    MouseArea {
                        id: offMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: root.turnOff()
                    }
                }
            }
        }
    }
    HyprlandFocusGrab {
        windows: [popup]
        active: popup.visible
        onCleared: popup.visible = false
    }
}
