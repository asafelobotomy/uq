import QtQuick
import QtQuick.Layouts
import Quickshell
import "../core"
import "../logic/barlayout.js" as L
import "../logic/ticker.js" as T
import "../logic/weather.js" as Weather
import "../logic/work.js" as Work
import "islands"

// An island's settings, in edit mode (a left click on an island, or Enter on the selected one):
// its size (Icon, Small, Medium, Large, or Custom: wider than Large, one Icon size at a time),
// its text (font, bold / italic / all capitals, text size), what it shows (the clock's seconds and date, the
// weather's city, the stock symbols, the news feeds, the world clock's time zones), moving it,
// and removing it. Changes are saved at once (BarLayout). User text (symbols, addresses, city
// names) only ever reaches the screen through UqText.
IslandPopup {
    id: pop

    // {zone, index, id, size}: size is what the bar draws now (it may be smaller than chosen).
    property var target: null
    readonly property string island: target ? target.id : ""
    readonly property var item: target && BarLayout.layout[target.zone] ? BarLayout.layout[target.zone][target.index] : null
    readonly property string chosen: item ? L.sizeOf(item) : "medium"
    readonly property int chosenUnits: item ? L.unitsOf(item) : 0
    // The island's text style, as stored (every field present).
    readonly property var textStyle: L.textOf(item)
    function setText(t) {
        if (pop.target)
            BarLayout.setText(pop.target.zone, pop.target.index, t);
    }
    // Installed font families matching `fontQuery` (at most 6), for the font search.
    property string fontQuery: ""
    property var allFonts: null
    readonly property var fontMatches: {
        var q = fontQuery.trim().toLowerCase();
        if (q === "")
            return [];
        if (allFonts === null)
            allFonts = Qt.fontFamilies();
        var out = [];
        for (var i = 0; i < allFonts.length && out.length < 6; i++)
            if (allFonts[i].toLowerCase().indexOf(q) >= 0)
                out.push(allFonts[i]);
        return out;
    }
    function stepWidth(dir) {
        if (!pop.target || !pop.item)
            return;
        var s = L.stepSize(pop.chosen, dir, pop.item.units);
        if (s !== null)
            BarLayout.setSize(pop.target.zone, pop.target.index, s.size, s.units);
    }
    readonly property var opts: BarLayout.options[island] || ({})
    property string addError: ""

    function showFor(slot, t) {
        target = t;
        addError = "";
        fontQuery = "";
        results = [];
        geoError = "";
        show(slot);
        focusFirst.restart();
    }

    // Edits from here may move the island: follow it.
    function retarget(zone, index) {
        if (target)
            target = { zone: zone, index: index, id: target.id, size: target.size, units: target.units };
    }

    function setList(key, list) {
        if (list === null)
            return;
        var v = {};
        v[key] = list;
        BarLayout.setOptions(island, v);
    }
    function listMove(list, i, dir) {
        var j = i + dir;
        if (j < 0 || j >= list.length)
            return null;
        var out = list.slice();
        var x = out.splice(i, 1)[0];
        out.splice(j, 0, x);
        return out;
    }

    // --- weather: city search ---
    property var results: []
    property string geoError: ""
    property int queryGen: 0
    property string pendingQuery: ""
    function queryEdited(s) {
        queryGen++;
        results = [];
        geoError = "";
        pendingQuery = s;
        if (Weather.validQuery(s))
            debounce.restart();
        else
            debounce.stop();
    }
    function runQuery() {
        var gen = queryGen;
        Bridge.call("GeocodeCity", [pendingQuery.trim()]).then(function (r) {
            if (gen !== pop.queryGen)
                return;
            var list = Array.isArray(r) ? r.slice(0, 5) : [];
            pop.results = list;
            pop.geoError = list.length === 0 ? "Couldn't find that city" : "";
        }).catch(function () {
            if (gen === pop.queryGen)
                pop.geoError = "Couldn't find that city";
        });
    }
    Timer {
        id: debounce
        interval: 400
        onTriggered: pop.runQuery()
    }

    readonly property var suggestedZones: ["America/New_York", "America/Los_Angeles", "Europe/London",
        "Europe/Berlin", "Asia/Kolkata", "Asia/Shanghai", "Asia/Tokyo", "Australia/Sydney", "UTC"]

    ColumnLayout {
        id: col
        width: Theme.fontSize * 28
        spacing: 2

        UqText {
            text: L.name(pop.island)
            baseBold: true
            Layout.fillWidth: true
        }

        // --- size ---
        UqText {
            visible: pop.island !== L.SEPARATOR
            text: "Size"
            color: Theme.textMuted
            Layout.topMargin: Theme.gapIn
        }
        Flow {
            visible: pop.island !== L.SEPARATOR
            spacing: 2
            Layout.fillWidth: true
            Repeater {
                model: pop.island !== L.SEPARATOR ? L.SIZES : []
                delegate: MenuRow {
                    id: sizeRow
                    required property string modelData
                    required property int index
                    text: modelData === L.CUSTOM && pop.chosen === L.CUSTOM
                        ? L.sizeLabel(L.CUSTOM, pop.chosenUnits) : L.SIZE_NAMES[modelData]
                    detail: pop.chosen === modelData ? "✓" : ""
                    Accessible.checkable: true
                    Accessible.checked: pop.chosen === modelData
                    onActivated: if (pop.target) BarLayout.setSize(pop.target.zone, pop.target.index, modelData)
                    Component.onCompleted: if (index === 0) firstSize.row = sizeRow
                }
            }
        }
        // Custom: one Icon size narrower or wider (dragging the island's edge does the same).
        RowLayout {
            visible: pop.chosen === L.CUSTOM
            spacing: 2
            MenuRow {
                text: "Narrower"
                detail: "−"
                onActivated: pop.stepWidth(-1)
            }
            MenuRow {
                text: "Wider"
                detail: "+"
                enabled: pop.chosenUnits < L.MAX_CUSTOM
                onActivated: pop.stepWidth(1)
            }
        }
        QtObject {
            id: firstSize
            property Item row: null
        }
        UqText {
            // What the bar draws now: the slot's live size, else the one it had when opened.
            readonly property string shownSize: pop.anchorItem && pop.anchorItem.size !== undefined
                ? pop.anchorItem.size : (pop.target ? pop.target.size : "")
            readonly property int shownUnits: pop.anchorItem && pop.anchorItem.units !== undefined
                ? pop.anchorItem.units : (pop.target ? pop.target.units || 0 : 0)
            visible: pop.target !== null && pop.island !== L.SEPARATOR
                && (shownSize !== pop.chosen || (pop.chosen === L.CUSTOM && shownUnits !== pop.chosenUnits))
            text: "Shown " + L.sizeLabel(shownSize, shownUnits) + " on this screen, to fit."
            color: Theme.textMuted
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
        }

        // --- text: font, formatting, size (the island's face; its popup keeps the theme's) ---
        ColumnLayout {
            visible: pop.island !== L.SEPARATOR
            spacing: 2
            Layout.fillWidth: true
            Layout.topMargin: Theme.gapIn

            RowLayout {
                Layout.fillWidth: true
                UqText {
                    text: "Text"
                    color: Theme.textMuted
                    Layout.fillWidth: true
                }
                MenuRow {
                    visible: !L.isPlainText(pop.textStyle)
                    text: "Use the theme's"
                    onActivated: pop.setText(null)
                }
            }
            UqText {
                text: "Font"
                color: Theme.textMuted
            }
            Flow {
                spacing: 2
                Layout.fillWidth: true
                MenuRow {
                    text: "Theme"
                    detail: pop.textStyle.font === null ? "✓" : ""
                    Accessible.checkable: true
                    Accessible.checked: pop.textStyle.font === null
                    onActivated: pop.setText({ font: null })
                }
                MenuRow {
                    text: "Monospace"
                    detail: pop.textStyle.font === L.MONO ? "✓" : ""
                    Accessible.checkable: true
                    Accessible.checked: pop.textStyle.font === L.MONO
                    onActivated: pop.setText({ font: L.MONO })
                }
                // A font picked from the search below.
                MenuRow {
                    visible: pop.textStyle.font !== null && pop.textStyle.font !== L.MONO
                    text: pop.textStyle.font || ""
                    detail: "✓"
                    Accessible.checkable: true
                    Accessible.checked: true
                }
            }
            UqInput {
                id: fontInput
                Layout.fillWidth: true
                maximumLength: 64
                placeholder: "Find a font…"
                Accessible.name: "Find a font"
                onEdited: function (s) { pop.fontQuery = s; }
                Keys.onDownPressed: nextItemInFocusChain(true).forceActiveFocus()
            }
            Repeater {
                model: pop.fontMatches
                delegate: MenuRow {
                    required property string modelData
                    text: modelData
                    detail: pop.textStyle.font === modelData ? "✓" : ""
                    // Each name in its own font.
                    textItem.baseFamily: modelData
                    Layout.fillWidth: true
                    onActivated: pop.setText({ font: modelData })
                }
            }
            UqText {
                visible: pop.fontQuery.trim() !== "" && pop.fontMatches.length === 0
                text: "No installed font matches"
                color: Theme.textMuted
            }

            UqText {
                text: "Formatting"
                color: Theme.textMuted
                Layout.topMargin: Theme.gapIn
            }
            Flow {
                spacing: 2
                Layout.fillWidth: true
                MenuRow {
                    text: "Bold"
                    textItem.baseBold: true
                    detail: pop.textStyle.bold ? "✓" : ""
                    Accessible.checkable: true
                    Accessible.checked: pop.textStyle.bold
                    onActivated: pop.setText({ bold: !pop.textStyle.bold })
                }
                MenuRow {
                    text: "Italic"
                    textItem.font.italic: true
                    detail: pop.textStyle.italic ? "✓" : ""
                    Accessible.checkable: true
                    Accessible.checked: pop.textStyle.italic
                    onActivated: pop.setText({ italic: !pop.textStyle.italic })
                }
                MenuRow {
                    text: "All capitals"
                    detail: pop.textStyle.caps ? "✓" : ""
                    Accessible.checkable: true
                    Accessible.checked: pop.textStyle.caps
                    onActivated: pop.setText({ caps: !pop.textStyle.caps })
                }
            }

            UqText {
                text: "Text size: " + pop.textStyle.size + "%"
                color: Theme.textMuted
                Layout.topMargin: Theme.gapIn
            }
            RowLayout {
                spacing: 2
                MenuRow {
                    text: "Smaller"
                    detail: "−"
                    enabled: L.stepTextSize(pop.textStyle.size, -1) !== null
                    onActivated: {
                        var n = L.stepTextSize(pop.textStyle.size, -1);
                        if (n !== null)
                            pop.setText({ size: n });
                    }
                }
                MenuRow {
                    text: "Larger"
                    detail: "+"
                    enabled: L.stepTextSize(pop.textStyle.size, 1) !== null
                    onActivated: {
                        var n = L.stepTextSize(pop.textStyle.size, 1);
                        if (n !== null)
                            pop.setText({ size: n });
                    }
                }
            }
        }

        // --- clock ---
        ColumnLayout {
            visible: pop.island === "clock"
            spacing: 2
            Layout.fillWidth: true
            Layout.topMargin: Theme.gapIn
            MenuRow {
                Layout.fillWidth: true
                text: "Show seconds"
                detail: pop.opts.seconds === true ? "On" : "Off"
                Accessible.checkable: true
                Accessible.checked: pop.opts.seconds === true
                onActivated: BarLayout.setOptions("clock", { seconds: pop.opts.seconds !== true, date: pop.opts.date || "short" })
            }
            UqText {
                text: "Date (Medium and Large)"
                color: Theme.textMuted
            }
            Repeater {
                model: [{ id: "none", name: "No date" }, { id: "short", name: "Short: Fri 13 Nov" },
                        { id: "long", name: "Long: Friday 13 November" }]
                delegate: MenuRow {
                    required property var modelData
                    Layout.fillWidth: true
                    text: modelData.name
                    detail: (pop.opts.date || "short") === modelData.id ? "✓" : ""
                    onActivated: BarLayout.setOptions("clock", { seconds: pop.opts.seconds === true, date: modelData.id })
                }
            }
        }

        // --- open windows: their order ---
        ColumnLayout {
            readonly property string mode: Work.SORTS.indexOf(pop.opts.sort) >= 0 ? pop.opts.sort : "opened"
            id: sortBox
            visible: pop.island === "work"
            spacing: 2
            Layout.fillWidth: true
            Layout.topMargin: Theme.gapIn
            UqText {
                text: "Order windows by"
                color: Theme.textMuted
            }
            Repeater {
                model: pop.island === "work" ? Work.SORTS : []
                delegate: MenuRow {
                    required property string modelData
                    Layout.fillWidth: true
                    text: Work.SORT_NAMES[modelData]
                    detail: sortBox.mode === modelData ? "✓" : ""
                    Accessible.checkable: true
                    Accessible.checked: sortBox.mode === modelData
                    onActivated: BarLayout.setOptions("work", { sort: modelData, descending: pop.opts.descending === true })
                }
            }
            UqText {
                visible: Work.sortHasOrder(sortBox.mode)
                text: "Order"
                color: Theme.textMuted
            }
            Repeater {
                model: pop.island === "work" && Work.sortHasOrder(sortBox.mode) ? [false, true] : []
                delegate: MenuRow {
                    required property bool modelData
                    Layout.fillWidth: true
                    text: Work.ORDER_NAMES[sortBox.mode][modelData ? 1 : 0]
                    detail: (pop.opts.descending === true) === modelData ? "✓" : ""
                    onActivated: BarLayout.setOptions("work", { sort: sortBox.mode, descending: modelData })
                }
            }
            UqText {
                visible: sortBox.mode === "activity"
                text: "The app that last sent a notification comes first."
                color: Theme.textMuted
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }

        // --- weather ---
        ColumnLayout {
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

        // --- stocks, news, world clock: an ordered list ---
        ColumnLayout {
            id: listBox
            readonly property string listKey: pop.island === "stocks" ? "symbols"
                : (pop.island === "news" ? "feeds" : "zones")
            readonly property var list: pop.opts[listKey] || []
            readonly property int max: pop.island === "stocks" ? 12 : (pop.island === "news" ? 8 : 4)
            visible: pop.island === "stocks" || pop.island === "news" || pop.island === "worldclock"
            spacing: 2
            Layout.fillWidth: true
            Layout.topMargin: Theme.gapIn

            function label(v) {
                if (pop.island !== "news")
                    return pop.island === "worldclock" ? v.split("/").pop().replace(/_/g, " ") : v;
                var info = Bridge.news && Array.isArray(Bridge.news.feeds) ? Bridge.news.feeds : [];
                for (var i = 0; i < info.length; i++)
                    if (info[i] && info[i].url === v && typeof info[i].title === "string" && info[i].title !== "")
                        return info[i].title.slice(0, 80) + (info[i].ok === false ? " (can't be read)" : "");
                for (var j = 0; j < T.PRESET_FEEDS.length; j++)
                    if (T.PRESET_FEEDS[j].url === v)
                        return T.PRESET_FEEDS[j].name;
                return v;
            }
            function add(text) {
                var v = pop.island === "stocks" ? T.cleanSymbol(text)
                    : (pop.island === "news" ? (T.validFeed(text) ? text.trim() : null)
                    : T.cleanZone(text));
                if (v === null) {
                    pop.addError = pop.island === "stocks" ? "That isn't a ticker symbol"
                        : (pop.island === "news" ? "Use the feed's https:// address" : "Use a zone name like Europe/Paris");
                    return;
                }
                if (list.indexOf(v) >= 0 || list.length >= max) {
                    pop.addError = list.length >= max ? "The list is full" : "Already on the list";
                    return;
                }
                if (pop.island !== "news") {
                    pop.addError = "";
                    pop.setList(listKey, list.concat([v]));
                    addInput.clear();
                    return;
                }
                pop.addError = "Checking…";
                Bridge.call("PreviewFeed", [v]).then(function () {
                    pop.addError = "";
                    pop.setList("feeds", listBox.list.concat([v]));
                    addInput.clear();
                }).catch(function () {
                    pop.addError = "That address isn't a feed uQ can read";
                });
            }

            UqText {
                text: pop.island === "stocks" ? "Symbols" : (pop.island === "news" ? "Feeds" : "Time zones")
                color: Theme.textMuted
            }
            UqText {
                visible: listBox.list.length === 0
                text: "None yet."
                color: Theme.textMuted
            }
            Repeater {
                model: listBox.visible ? listBox.list : []
                delegate: RowLayout {
                    id: lrow
                    required property string modelData
                    required property int index
                    spacing: 2
                    Layout.fillWidth: true
                    UqText {
                        text: listBox.label(lrow.modelData)
                        Layout.fillWidth: true
                    }
                    MenuRow {
                        text: "Move up"
                        icon: "go-up"
                        enabled: lrow.index > 0
                        opacity: enabled ? 1 : 0.4
                        Accessible.name: "Move " + lrow.modelData + " up"
                        onActivated: pop.setList(listBox.listKey, pop.listMove(listBox.list, lrow.index, -1))
                    }
                    MenuRow {
                        text: "Move down"
                        icon: "go-down"
                        enabled: lrow.index < listBox.list.length - 1
                        opacity: enabled ? 1 : 0.4
                        Accessible.name: "Move " + lrow.modelData + " down"
                        onActivated: pop.setList(listBox.listKey, pop.listMove(listBox.list, lrow.index, 1))
                    }
                    MenuRow {
                        text: "Remove"
                        icon: "list-remove"
                        Accessible.name: "Remove " + lrow.modelData
                        onActivated: pop.setList(listBox.listKey, listBox.list.filter(function (x) { return x !== lrow.modelData; }))
                    }
                }
            }
            RowLayout {
                spacing: Theme.gapIn
                Layout.fillWidth: true
                UqInput {
                    id: addInput
                    Layout.fillWidth: true
                    maximumLength: pop.island === "news" ? 512 : 64
                    placeholder: pop.island === "stocks" ? "Add a symbol, e.g. AAPL, ^GSPC, BTC-USD"
                        : (pop.island === "news" ? "https://… feed address" : "Time zone, e.g. Europe/Paris")
                    Accessible.name: placeholder
                    onAccepted: listBox.add(text)
                }
                MenuRow {
                    text: "Add"
                    onActivated: listBox.add(addInput.text)
                }
            }
            UqText {
                visible: pop.addError !== ""
                text: pop.addError
                color: Theme.textMuted
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
            Flow {
                visible: (pop.island === "news" || pop.island === "worldclock") && listBox.list.length < listBox.max
                spacing: 2
                Layout.fillWidth: true
                Repeater {
                    model: pop.island === "news"
                        ? T.PRESET_FEEDS.filter(function (p) { return listBox.list.indexOf(p.url) < 0; })
                        : (pop.island === "worldclock"
                           ? pop.suggestedZones.filter(function (z) { return listBox.list.indexOf(z) < 0; })
                               .map(function (z) { return { name: z.split("/").pop().replace(/_/g, " "), url: z }; })
                           : [])
                    delegate: MenuRow {
                        required property var modelData
                        text: "+ " + modelData.name
                        Accessible.name: "Add " + modelData.name
                        onActivated: pop.setList(listBox.listKey, listBox.list.concat([modelData.url]))
                    }
                }
            }
            UqText {
                visible: pop.island === "stocks" || pop.island === "news"
                text: pop.island === "stocks" ? "Quotes may be delayed. Only the symbols are sent."
                    : "uQ asks only these sites for their headlines."
                color: Theme.textMuted
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }

        // --- place ---
        MenuRow {
            Layout.fillWidth: true
            Layout.topMargin: Theme.gapIn
            visible: pop.target !== null && L.canMove(BarLayout.layout, pop.target.zone, pop.target.index, -1)
            text: "Move left"
            detail: "Shift+←"
            onActivated: {
                BarLayout.move(pop.target.zone, pop.target.index, -1);
                pop.hide();
            }
        }
        MenuRow {
            Layout.fillWidth: true
            visible: pop.target !== null && L.canMove(BarLayout.layout, pop.target.zone, pop.target.index, 1)
            text: "Move right"
            detail: "Shift+→"
            onActivated: {
                BarLayout.move(pop.target.zone, pop.target.index, 1);
                pop.hide();
            }
        }
        MenuRow {
            Layout.fillWidth: true
            text: "Remove from the bar"
            detail: "Delete"
            onActivated: {
                if (pop.target)
                    BarLayout.remove(pop.target.zone, pop.target.index);
                pop.hide();
            }
        }
    }

    Timer {
        id: focusFirst
        interval: 0
        onTriggered: {
            if (pop.island === "weather")
                cityInput.forceActiveFocus();
            else if (firstSize.row)
                firstSize.row.forceActiveFocus();
        }
    }
}
