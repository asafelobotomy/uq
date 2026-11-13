import QtQuick
import QtQuick.Layouts
import "../../core"
import "../../logic/ticker.js" as T
import "../../logic/barlayout.js" as L
import ".."

// Stock ticker (`stocks`): the chosen symbols scroll by with price and day change. The popup
// lists each quote (price, change, day range) and edits the list: add a symbol, remove one,
// move one up or down. Quotes come from the bridge (`Stocks` events, a free delayed source);
// when it can't be reached the last quotes stay, marked as old.
Island {
    id: root

    readonly property var ev: Bridge.stocks
    readonly property var quotes: T.quotes(ev)
    readonly property var symbols: (BarLayout.options.stocks || {}).symbols || []
    readonly property string note: T.state(ev, now)
    property double now: Date.now()

    implicitWidth: (symbols.length === 0 ? empty.implicitWidth : L.textWidth(size, Theme.fontSize))
        + Theme.gapIn * 2
    Accessible.role: Accessible.Button
    Accessible.name: "Stocks: " + quotes.map(T.quoteText).join(", ")

    Timer {
        interval: 60000
        repeat: true
        running: root.note !== ""
        onTriggered: root.now = Date.now()
    }

    UqText {
        id: empty
        visible: root.symbols.length === 0
        anchors.verticalCenter: parent.verticalCenter
        text: "Add stocks"
        color: Theme.textMuted
    }

    Marquee {
        visible: root.symbols.length > 0
        anchors.fill: parent
        model: root.quotes.length > 0 ? root.quotes : [{ symbol: root.note || "Loading…", price: null, ok: false, changePct: null }]
        delegate: Row {
            required property var modelData
            spacing: Theme.gapIn
            UqText {
                text: modelData.symbol
                font.bold: true
            }
            UqText {
                // Every real quote row (not the "Loading…" stand-in); "–" when it has no price.
                visible: modelData.name !== undefined
                text: T.price(modelData.price)
                font.family: Theme.fontMono
                color: root.ev && root.ev.status === "error" ? Theme.textMuted : Theme.text
            }
            UqText {
                visible: modelData.changePct !== null
                text: T.pct(modelData.changePct)
                font.family: Theme.fontMono
                color: T.direction(modelData.changePct) > 0 ? Theme.success
                    : (T.direction(modelData.changePct) < 0 ? Theme.error : Theme.textMuted)
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: {
            if (popup.open && popup.anchorItem === root)
                popup.hide();
            else
                popup.show(root, details);
        }
    }

    IslandPopup {
        id: popup
    }

    function setSymbols(list) {
        BarLayout.setOptions("stocks", { symbols: list });
    }
    function moveSymbol(i, dir) {
        var l = symbols.slice();
        var j = i + dir;
        if (j < 0 || j >= l.length)
            return;
        var s = l.splice(i, 1)[0];
        l.splice(j, 0, s);
        setSymbols(l);
    }

    Component {
        id: details
        ColumnLayout {
            id: col
            spacing: Theme.gapIn
            width: Theme.fontSize * 30
            property string addError: ""

            UqText {
                text: "Stocks"
                font.bold: true
            }
            UqText {
                visible: root.note !== ""
                text: root.note
                color: Theme.textMuted
            }
            Repeater {
                model: root.symbols
                delegate: RowLayout {
                    id: qrow
                    required property string modelData
                    required property int index
                    readonly property var q: {
                        for (var i = 0; i < root.quotes.length; i++)
                            if (root.quotes[i].symbol === modelData)
                                return root.quotes[i];
                        return null;
                    }
                    spacing: Theme.gapIn
                    Layout.preferredWidth: col.width

                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Layout.preferredWidth: Theme.fontSize * 10
                        UqText {
                            text: qrow.modelData
                            font.bold: true
                        }
                        UqText {
                            text: qrow.q === null ? "No quote yet"
                                : (!qrow.q.ok && qrow.q.price === null ? "Not found"
                                : (qrow.q.name !== "" ? qrow.q.name : ""))
                            color: Theme.textMuted
                            Layout.fillWidth: true
                        }
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: Theme.fontSize * 12
                        UqText {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignRight
                            text: qrow.q && qrow.q.price !== null
                                ? T.price(qrow.q.price) + (qrow.q.currency ? " " + qrow.q.currency : "") : ""
                            font.family: Theme.fontMono
                        }
                        UqText {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignRight
                            visible: qrow.q !== null && qrow.q.changePct !== null
                            text: qrow.q ? T.pct(qrow.q.changePct) : ""
                            font.family: Theme.fontMono
                            color: qrow.q && T.direction(qrow.q.changePct) > 0 ? Theme.success
                                : (qrow.q && T.direction(qrow.q.changePct) < 0 ? Theme.error : Theme.textMuted)
                        }
                        UqText {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignRight
                            visible: qrow.q !== null && qrow.q.low !== null && qrow.q.high !== null
                            text: qrow.q ? "Day " + T.price(qrow.q.low) + "–" + T.price(qrow.q.high) : ""
                            color: Theme.textMuted
                        }
                    }
                    MenuRow {
                        Layout.preferredWidth: Theme.fontSize * 2 + Theme.gapIn * 2
                        text: "Move up"
                        icon: "go-up"
                        enabled: qrow.index > 0
                        opacity: enabled ? 1 : 0.4
                        Accessible.name: "Move " + qrow.modelData + " up"
                        onActivated: root.moveSymbol(qrow.index, -1)
                    }
                    MenuRow {
                        Layout.preferredWidth: Theme.fontSize * 2 + Theme.gapIn * 2
                        text: "Move down"
                        icon: "go-down"
                        enabled: qrow.index < root.symbols.length - 1
                        opacity: enabled ? 1 : 0.4
                        Accessible.name: "Move " + qrow.modelData + " down"
                        onActivated: root.moveSymbol(qrow.index, 1)
                    }
                    MenuRow {
                        Layout.preferredWidth: Theme.fontSize * 2 + Theme.gapIn * 2
                        text: "Remove"
                        icon: "list-remove"
                        Accessible.name: "Remove " + qrow.modelData
                        onActivated: root.setSymbols(root.symbols.filter(function (s) { return s !== qrow.modelData; }))
                    }
                }
            }

            RowLayout {
                spacing: Theme.gapIn
                Layout.fillWidth: true
                UqInput {
                    id: symbolInput
                    Layout.fillWidth: true
                    placeholder: "Add a symbol, e.g. AAPL, ^GSPC, BTC-USD"
                    maximumLength: 16
                    Accessible.name: "Add a stock symbol"
                    onAccepted: addRow.activated()
                    Component.onCompleted: forceActiveFocus()
                }
                MenuRow {
                    id: addRow
                    text: "Add"
                    onActivated: {
                        var s = T.cleanSymbol(symbolInput.text);
                        if (s === null) {
                            col.addError = "That isn't a ticker symbol";
                        } else if (root.symbols.indexOf(s) >= 0) {
                            col.addError = s + " is already on the list";
                        } else if (root.symbols.length >= 12) {
                            col.addError = "At most 12 symbols";
                        } else {
                            col.addError = "";
                            root.setSymbols(root.symbols.concat([s]));
                            symbolInput.clear();
                        }
                    }
                }
            }
            UqText {
                visible: col.addError !== ""
                text: col.addError
                color: Theme.textMuted
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Refresh"
                onActivated: Bridge.call("RefreshStocks", []).catch(function () {})
            }
            UqText {
                text: "Quotes may be delayed. Only the symbols are sent."
                color: Theme.textMuted
                wrapMode: Text.WordWrap
                Layout.fillWidth: true
            }
        }
    }
}
