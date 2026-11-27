// Weather.
import QtQuick
import QtQuick.Layouts
import Quickshell
import "../../core"
import "../../logic/barlayout.js" as L
import "../../logic/ticker.js" as T
import "../../logic/weather.js" as Weather
import "../../logic/work.js" as Work
import "../islands"

ColumnLayout {
    // Passed in from IslandForm.qml.
    property var pop
    // The city field, for IslandForm.qml to focus first.
    property alias cityField: cityInput

    visible: pop.island === "weather"
    spacing: Theme.gapIn
    Layout.fillWidth: true
    Layout.topMargin: Theme.gapIn
    UqText {
        text: Settings.values.weather_location
            ? "City: " + String(Settings.values.weather_location.name) : "Choose a city for the weather"
        color: Theme.textMuted
        Layout.fillWidth: true
    }
    UqInput {
        id: cityInput
        Layout.fillWidth: true
        placeholder: "City"
        Accessible.name: "City"
        onEdited: function (s) { pop.queryEdited(s); }
        Keys.onDownPressed: nextItemInFocusChain(true).forceActiveFocus()
    }
    UqText {
        visible: pop.geoError !== ""
        text: pop.geoError
        color: Theme.textMuted
    }
    Repeater {
        model: pop.island === "weather" ? pop.results : []
        delegate: MenuRow {
            required property var modelData
            Layout.fillWidth: true
            text: String(modelData.name)
            detail: modelData.country ? String(modelData.country) : ""
            onActivated: {
                var loc = Weather.locationArg(modelData);
                if (loc !== null)
                    Bridge.call("SetSetting", ["weather_location", loc]);
                pop.results = [];
                cityInput.clear();
            }
        }
    }
}
