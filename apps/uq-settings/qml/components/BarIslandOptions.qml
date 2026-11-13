// The options of one bar island, under its row in the bar editor: the clock's seconds and date,
// the stock ticker's symbols, the news island's feeds, the world clock's time zones. It never
// changes anything itself: it asks (`changed(island, value)`, the whole new options object for
// that island) and shows `options` again once the page has uqd's answer. Feed addresses and
// symbols are the user's own text; they only reach the screen through UqLabel.
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import "barrows.js" as R
import "."

ColumnLayout {
    id: root

    property string island: ""
    /// shell.bar.options.<island>
    property var options: ({})
    signal changed(string island, var value)
    /// The last thing typed that couldn't be added, said plainly; "" when none.
    property string addError: ""

    readonly property var list: island === "stocks" ? (options.symbols || [])
        : (island === "news" ? (options.feeds || []) : (island === "worldclock" ? (options.zones || []) : []))
    readonly property string listKey: island === "stocks" ? "symbols" : (island === "news" ? "feeds" : "zones")
    readonly property int max: island === "stocks" ? 12 : (island === "news" ? 8 : 4)
    readonly property alias addField: field
    readonly property alias listRepeater: rep

    function send(list) {
        if (list === null)
            return;
        const v = {};
        v[listKey] = list;
        changed(island, v);
    }
    function clean(text) {
        return island === "stocks" ? R.cleanSymbol(text) : (island === "news" ? R.cleanFeed(text) : R.cleanZone(text));
    }
    function add(text) {
        const v = clean(text);
        if (v === null) {
            addError = island === "stocks" ? "That isn't a ticker symbol."
                : (island === "news" ? "Use the feed's address, starting with https://." : "Use a time zone name like Europe/Paris.");
            return false;
        }
        const next = R.listAdd(list, v, max);
        if (next === null) {
            addError = list.indexOf(v) >= 0 ? "That one is already on the list." : "The list is full.";
            return false;
        }
        addError = "";
        send(next);
        return true;
    }
    function feedName(url) {
        for (const p of R.PRESET_FEEDS)
            if (p.url === url)
                return p.name;
        return url;
    }

    spacing: Kirigami.Units.smallSpacing

    // --- clock ---
    RowLayout {
        visible: root.island === "clock"
        spacing: Kirigami.Units.largeSpacing
        QQC2.CheckBox {
            id: seconds
            objectName: "clockSeconds"
            text: "Show seconds"
            checked: root.options.seconds === true
            onToggled: {
                root.changed("clock", { seconds: seconds.checked, date: root.options.date || "short" });
                seconds.checked = Qt.binding(() => root.options.seconds === true);
            }
        }
        UqLabel {
            text: "Date"
        }
        UqComboBox {
            id: dateCombo
            objectName: "clockDate"
            readonly property var ids: ["none", "short", "long"]
            model: ["None", "Short (Fri 13 Nov)", "Long (Friday 13 November)"]
            currentIndex: Math.max(0, ids.indexOf(root.options.date || "short"))
            Accessible.name: "Date in the clock"
            onActivated: index => {
                root.changed("clock", { seconds: root.options.seconds === true, date: ids[index] });
                dateCombo.currentIndex = Qt.binding(() => Math.max(0, ids.indexOf(root.options.date || "short")));
            }
        }
    }

    // --- stocks, news, world clock: an ordered list ---
    UqLabel {
        visible: root.island !== "clock" && root.list.length === 0
        text: root.island === "stocks" ? "No symbols yet." : (root.island === "news" ? "No feeds yet." : "No time zones yet.")
        opacity: 0.7
    }
    Repeater {
        id: rep
        model: root.island === "clock" ? [] : root.list
        RowLayout {
            id: lrow
            required property int index
            required property string modelData
            readonly property alias removeButton: removeBtn
            spacing: Kirigami.Units.smallSpacing
            Layout.fillWidth: true
            UqLabel {
                text: root.island === "news" ? root.feedName(lrow.modelData) : lrow.modelData
                elide: Text.ElideMiddle
                Layout.fillWidth: true
            }
            QQC2.ToolButton {
                icon.name: "go-up"
                enabled: root.enabled && lrow.index > 0
                Accessible.name: "Move " + lrow.modelData + " up"
                onClicked: root.send(R.listMove(root.list, lrow.index, -1))
            }
            QQC2.ToolButton {
                icon.name: "go-down"
                enabled: root.enabled && lrow.index < root.list.length - 1
                Accessible.name: "Move " + lrow.modelData + " down"
                onClicked: root.send(R.listMove(root.list, lrow.index, 1))
            }
            QQC2.ToolButton {
                id: removeBtn
                icon.name: "list-remove"
                enabled: root.enabled
                Accessible.name: "Remove " + lrow.modelData
                onClicked: root.send(R.listRemove(root.list, lrow.index))
            }
        }
    }
    RowLayout {
        visible: root.island !== "clock"
        spacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true
        QQC2.TextField {
            id: field
            objectName: "optionField"
            Layout.fillWidth: true
            maximumLength: root.island === "news" ? 512 : 64
            placeholderText: root.island === "stocks" ? "Symbol, like AAPL, ^GSPC or BTC-USD"
                : (root.island === "news" ? "Feed address (https://…)" : "Time zone, like Europe/Paris")
            Accessible.name: root.island === "stocks" ? "Add a symbol" : (root.island === "news" ? "Add a feed" : "Add a time zone")
            onAccepted: if (root.add(field.text)) field.clear()
        }
        QQC2.Button {
            text: "Add"
            enabled: root.enabled && field.text.trim() !== "" && root.list.length < root.max
            onClicked: if (root.add(field.text)) field.clear()
        }
    }
    UqLabel {
        visible: root.addError !== "" && root.island !== "clock"
        text: root.addError
        color: Kirigami.Theme.negativeTextColor
    }
    // Suggested feeds not added yet.
    Flow {
        visible: root.island === "news" && root.list.length < root.max
        spacing: Kirigami.Units.smallSpacing
        Layout.fillWidth: true
        Repeater {
            model: root.island === "news" ? R.PRESET_FEEDS.filter(p => root.list.indexOf(p.url) < 0) : []
            QQC2.Button {
                required property var modelData
                icon.name: "list-add"
                enabled: root.enabled
                Accessible.name: "Add " + modelData.name
                onClicked: root.send(R.listAdd(root.list, modelData.url, root.max))
                contentItem: UqLabel {
                    text: modelData.name
                }
            }
        }
    }
    UqLabel {
        visible: root.island === "stocks" || root.island === "news"
        text: root.island === "stocks"
            ? "Quotes come from a free, delayed source. Only the symbols are sent."
            : "uQ asks only these sites for their headlines, every 15 minutes."
        opacity: 0.7
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
    }
}
