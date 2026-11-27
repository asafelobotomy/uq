// Stocks, news, world clock: an ordered list.
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
    id: listBox
    // Passed in from IslandForm.qml.
    property var pop

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
