import QtQuick
import QtQuick.Layouts
import Quickshell
import "../core"
import "../logic/barlayout.js" as L
import "../logic/ticker.js" as T
import "../logic/weather.js" as Weather
import "../logic/work.js" as Work
import "islands"
import "islandform"

// One island's settings (IslandSettings' popup and the Bar editor dialog show it): its size
// (Icon, Small, Medium, Large, or Custom: wider than Large, one Icon size at a time), whether it
// fills the bar's empty space beside its zone ("Fill the rest"), its text (font, bold / italic /
// all capitals, text size), what it shows (the clock's seconds and date, the weather's city, the
// stock symbols, the news feeds, the world clock's time zones), moving it, and removing it.
// Changes are saved at once (BarLayout). User text (symbols, addresses, city names) only ever
// reaches the screen through UqText.
Item {
    id: pop

    // Take the keyboard on the first control when pointed at an island (a popup does; the dialog keeps its own focus).
    property bool autoFocus: true
    property int formWidth: Theme.fontSize * 28
    // {zone, index, id, size}: size is what the bar draws now (it may be smaller than chosen).
    property var target: null
    // The island's slot in the bar, for the size it is drawn at now.
    property Item anchorItem: null
    // An action that removes or moves the island is done: the host may close.
    signal dismissed()
    implicitWidth: col.implicitWidth
    implicitHeight: col.implicitHeight
    readonly property string island: target ? target.id : ""
    readonly property var item: target && BarLayout.layout[target.zone] ? BarLayout.layout[target.zone][target.index] : null
    readonly property string chosen: item ? L.sizeOf(item) : "medium"
    readonly property int chosenUnits: item ? L.unitsOf(item) : 0
    // The island's text style, as stored (every field present).
    readonly property var textStyle: L.effectiveText(BarLayout.layout, item)
    // The island has a text of its own (else it takes the all-islands text).
    readonly property bool ownText: item !== null && item.text !== undefined && item.text !== null
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

    // Point the form at island `t`, from a fresh state.
    function setTarget(t) {
        target = t;
        reset();
        focusFirst.restart();
    }
    // Forget what was typed for the island shown before (searches, errors).
    function reset() {
        addError = "";
        fontQuery = "";
        results = [];
        geoError = "";
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
        width: pop.formWidth
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
        // Fill the rest: it grows into the empty space beside its zone (Open windows does unless
        // turned off).
        MenuRow {
            visible: pop.island !== L.SEPARATOR
            Layout.fillWidth: true
            text: "Fill the rest"
            detail: L.fillOf(pop.item) ? "✓" : ""
            Accessible.checkable: true
            Accessible.checked: L.fillOf(pop.item)
            onActivated: if (pop.target) BarLayout.setFill(pop.target.zone, pop.target.index, !L.fillOf(pop.item))
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
        TextOptions {
            pop: pop
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

        // --- system monitor: which readings the island shows ---
        ColumnLayout {
            visible: pop.island === "sysmon"
            spacing: 2
            Layout.fillWidth: true
            Layout.topMargin: Theme.gapIn
            UqText {
                text: "Show in the bar"
                color: Theme.textMuted
            }
            Repeater {
                model: pop.island === "sysmon" ? [{ id: "cpu", name: "Processor" }, { id: "mem", name: "Memory" },
                    { id: "gpu", name: "Graphics card" }, { id: "temp", name: "Temperature" }] : []
                delegate: MenuRow {
                    required property var modelData
                    readonly property bool on: pop.opts[modelData.id] !== false
                    Layout.fillWidth: true
                    text: modelData.name
                    detail: on ? "On" : "Off"
                    Accessible.checkable: true
                    Accessible.checked: on
                    onActivated: {
                        var o = { cpu: pop.opts.cpu !== false, mem: pop.opts.mem !== false,
                                  gpu: pop.opts.gpu !== false, temp: pop.opts.temp !== false };
                        o[modelData.id] = !on;
                        BarLayout.setOptions("sysmon", o);
                    }
                }
            }
        }

        // --- storage cleanup: when it turns amber ---
        ColumnLayout {
            visible: pop.island === "crumbs"
            spacing: 2
            Layout.fillWidth: true
            Layout.topMargin: Theme.gapIn
            UqText {
                text: "Turn amber when you can free"
                color: Theme.textMuted
            }
            Repeater {
                model: pop.island === "crumbs" ? [{ gb: 0, name: "Never" }, { gb: 2, name: "2 GB" }, { gb: 5, name: "5 GB" },
                    { gb: 10, name: "10 GB" }, { gb: 20, name: "20 GB" }, { gb: 50, name: "50 GB" }] : []
                delegate: MenuRow {
                    required property var modelData
                    readonly property bool on: (typeof pop.opts.warn_gb === "number" ? pop.opts.warn_gb : 5) === modelData.gb
                    Layout.fillWidth: true
                    text: modelData.name
                    detail: on ? "✓" : ""
                    Accessible.checkable: true
                    Accessible.checked: on
                    onActivated: BarLayout.setOptions("crumbs", { warn_gb: modelData.gb })
                }
            }
        }

        // --- security: notifications ---
        ColumnLayout {
            visible: pop.island === "security"
            spacing: 2
            Layout.fillWidth: true
            Layout.topMargin: Theme.gapIn
            MenuRow {
                Layout.fillWidth: true
                text: "Notify when it gets worse"
                detail: pop.opts.notify !== false ? "On" : "Off"
                Accessible.checkable: true
                Accessible.checked: pop.opts.notify !== false
                onActivated: BarLayout.setOptions("security", { notify: pop.opts.notify === false })
            }
        }

        // --- open windows: their order ---
        WorkSortOptions {
            id: sortBox
            pop: pop
        }

        // --- open windows: what a tab's X does ---
        ColumnLayout {
            readonly property string mode: Work.CLOSES.indexOf(pop.opts.close) >= 0 ? pop.opts.close : "ask"
            id: closeBox
            visible: pop.island === "work"
            spacing: 2
            Layout.fillWidth: true
            Layout.topMargin: Theme.gapIn
            UqText {
                text: "A tab's close button"
                color: Theme.textMuted
            }
            Repeater {
                model: pop.island === "work" ? Work.CLOSES : []
                delegate: MenuRow {
                    required property string modelData
                    Layout.fillWidth: true
                    text: Work.CLOSE_NAMES[modelData]
                    detail: closeBox.mode === modelData ? "✓" : ""
                    Accessible.checkable: true
                    Accessible.checked: closeBox.mode === modelData
                    onActivated: BarLayout.setOptions("work", Object.assign({}, pop.opts, { close: modelData }))
                }
            }
            UqText {
                text: "A minimised window's close button always closes it."
                color: Theme.textMuted
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }

        // --- weather ---
        WeatherOptions {
            id: weatherOptions
            pop: pop
        }

        // --- stocks, news, world clock: an ordered list ---
        ListOptions {
            id: listBox
            pop: pop
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
                pop.dismissed();
            }
        }
        MenuRow {
            Layout.fillWidth: true
            visible: pop.target !== null && L.canMove(BarLayout.layout, pop.target.zone, pop.target.index, 1)
            text: "Move right"
            detail: "Shift+→"
            onActivated: {
                BarLayout.move(pop.target.zone, pop.target.index, 1);
                pop.dismissed();
            }
        }
        MenuRow {
            Layout.fillWidth: true
            text: "Remove from the bar"
            detail: "Delete"
            onActivated: {
                if (pop.target)
                    BarLayout.remove(pop.target.zone, pop.target.index);
                pop.dismissed();
            }
        }
    }

    Timer {
        id: focusFirst
        interval: 0
        onTriggered: {
            if (!pop.autoFocus)
                return;
            if (pop.island === "weather")
                weatherOptions.cityField.forceActiveFocus();
            else if (firstSize.row)
                firstSize.row.forceActiveFocus();
        }
    }
}
