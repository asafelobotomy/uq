import QtQuick
import QtTest
import "../../shell/logic/weather.js" as Weather

TestCase {
    name: "weather"
    function test_icon() {
        compare(Weather.icon(0, true), "weather-clear");
        compare(Weather.icon(0, false), "weather-clear-night");
        compare(Weather.icon(1, true), "weather-few-clouds");
        compare(Weather.icon(2, false), "weather-few-clouds-night");
        compare(Weather.icon(3, true), "weather-overcast");
        compare(Weather.icon(45, true), "weather-fog");
        compare(Weather.icon(48, false), "weather-fog");
        compare(Weather.icon(51, true), "weather-showers-scattered");
        compare(Weather.icon(57, true), "weather-showers-scattered");
        compare(Weather.icon(81, true), "weather-showers-scattered");
        compare(Weather.icon(63, true), "weather-showers");
        compare(Weather.icon(67, true), "weather-showers");
        compare(Weather.icon(75, true), "weather-snow");
        compare(Weather.icon(86, true), "weather-snow");
        compare(Weather.icon(95, true), "weather-storm");
        compare(Weather.icon(99, false), "weather-storm");
    }
    function test_icon_unknown() {
        compare(Weather.icon(42, true), "weather-severe-alert");
        compare(Weather.icon(-1, true), "weather-severe-alert");
        compare(Weather.icon(null, true), "weather-severe-alert");
        compare(Weather.icon("x", true), "weather-severe-alert");
    }
    function test_format() {
        compare(Weather.format(8.6), "9°");
        compare(Weather.format(-0.4), "0°");
        compare(Weather.format(-3.6), "-4°");
        compare(Weather.format(null), "");
    }
    function test_validQuery() {
        verify(!Weather.validQuery("a"));
        verify(!Weather.validQuery(" a "));
        verify(Weather.validQuery("Oslo"));
        verify(!Weather.validQuery(new Array(102).join("x")));
    }
    function test_locationArg() {
        compare(Weather.locationArg({ name: "Oslo", country: "NO", lat: 59.9, lon: 10.7 }),
                { name: "Oslo", lat: 59.9, lon: 10.7 });
        compare(Weather.locationArg({ name: "Oslo" }), null);
        compare(Weather.locationArg(null), null);
    }
    function test_canTurnOff() {
        var loc = { name: "Oslo", lat: 59.9, lon: 10.7 };
        verify(Weather.canTurnOff(loc, null));          // offline at login: no Weather event yet
        verify(Weather.canTurnOff(loc, undefined));
        verify(Weather.canTurnOff(loc, { temp_c: 1 }));
        verify(!Weather.canTurnOff(null, null));
        verify(!Weather.canTurnOff(undefined, { temp_c: 1 }));
    }
}
