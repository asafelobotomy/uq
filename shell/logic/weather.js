.pragma library

// WMO weather code -> freedesktop icon name.
function icon(code, isDay) {
    if (typeof code !== "number")
        return "weather-severe-alert";
    var day = isDay !== false;
    if (code === 0)
        return day ? "weather-clear" : "weather-clear-night";
    if (code === 1 || code === 2)
        return day ? "weather-few-clouds" : "weather-few-clouds-night";
    if (code === 3)
        return "weather-overcast";
    if (code === 45 || code === 48)
        return "weather-fog";
    if ((code >= 51 && code <= 57) || (code >= 80 && code <= 82))
        return "weather-showers-scattered";
    if (code >= 61 && code <= 67)
        return "weather-showers";
    if ((code >= 71 && code <= 77) || code === 85 || code === 86)
        return "weather-snow";
    if (code >= 95 && code <= 99)
        return "weather-storm";
    return "weather-severe-alert";
}

// "9°"; never "-0°".
function format(tempC) {
    if (typeof tempC !== "number" || !isFinite(tempC))
        return "";
    var r = Math.round(tempC);
    if (r === 0)
        r = 0;   // folds -0
    return r + "°";
}

// GeocodeCity accepts names of at most 100 chars; shorter than 2 is not worth a request.
function validQuery(s) {
    var t = String(s).trim();
    return t.length >= 2 && t.length <= 100;
}

// The SetSetting("weather_location", ...) value for a geocode result, or null if malformed.
function locationArg(r) {
    if (!r || typeof r.name !== "string" || typeof r.lat !== "number" || typeof r.lon !== "number")
        return null;
    return { name: r.name, lat: r.lat, lon: r.lon };
}

// "Turn off weather" is offered whenever a location is set, even before any Weather event
// arrives (e.g. offline at login).
function canTurnOff(location, weather) {
    return location !== null && location !== undefined;
}
