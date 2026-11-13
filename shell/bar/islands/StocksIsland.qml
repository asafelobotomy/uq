import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets
import "../../core"
import "../../logic/ticker.js" as T
import ".."

// Stock ticker (`stocks`). Icon: the first symbol's day change as ▲/▼ (colour and glyph).
// Small: the symbols and their change, scrolling. Medium and Large: with prices too. A click
// lists every quote (price, change, day range); the symbols are chosen in edit mode. Quotes come
// from the bridge (`Stocks` events, a free delayed source); when it can't be reached the last
// quotes stay, marked as old.
Island {
    id: root

    readonly property var ev: Bridge.stocks
    readonly property var quotes: T.quotes(ev)
    readonly property var symbols: (BarLayout.options.stocks || {}).symbols || []
    readonly property string note: T.state(ev, now)
    readonly property bool withPrice: size === "medium" || size === "large"
    readonly property var first: quotes.length > 0 ? quotes[0] : null
    property double now: Date.now()

    Accessible.role: Accessible.Button
    Accessible.name: "Stocks: " + quotes.map(T.quoteText).join(", ")

    Timer {
        interval: 60000
        repeat: true
        running: root.note !== "" || popup.open
        onTriggered: root.now = Date.now()
    }

    // Icon: the first symbol's direction.
    UqText {
        visible: root.isIcon
        anchors.centerIn: parent
        text: root.first && root.first.changePct !== null ? T.glyph(root.first.changePct) : "–"
        font.pixelSize: Math.round(Theme.fontSize * 1.4)
        color: root.first && T.direction(root.first.changePct) > 0 ? Theme.success
            : (root.first && T.direction(root.first.changePct) < 0 ? Theme.error : Theme.textMuted)
    }

    UqText {
        visible: !root.isIcon && root.symbols.length === 0
        anchors.fill: parent
        verticalAlignment: Text.AlignVCenter
        text: "No symbols"
        color: Theme.textMuted
    }

    Marquee {
        visible: !root.isIcon && root.symbols.length > 0
        anchors.fill: parent
        model: root.quotes.length > 0 ? root.quotes : [{ symbol: root.note || "Loading…", price: null, ok: false, changePct: null }]
        delegate: Row {
            required property var modelData
            spacing: Theme.gapIn
            UqText {
                text: modelData.symbol
                font.bold: true
                elide: Text.ElideNone
            }
            UqText {
                // Every real quote row (not the "Loading…" stand-in); "–" when it has no price.
                visible: root.withPrice && modelData.name !== undefined
                text: T.price(modelData.price)
                font.family: Theme.fontMono
                elide: Text.ElideNone
                color: root.ev && root.ev.status === "error" ? Theme.textMuted : Theme.text
            }
            UqText {
                visible: modelData.changePct !== null
                text: T.pct(modelData.changePct)
                font.family: Theme.fontMono
                elide: Text.ElideNone
                color: T.direction(modelData.changePct) > 0 ? Theme.success
                    : (T.direction(modelData.changePct) < 0 ? Theme.error : Theme.textMuted)
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        onClicked: popup.open && popup.anchorItem === root ? popup.hide() : popup.show(root, details)
    }

    LazyPopup {
        id: popup
    }

    Component {
        id: details
        ColumnLayout {
            id: col
            spacing: Theme.gapIn
            width: Theme.fontSize * 28

            UqText {
                text: "Stocks"
                font.bold: true
            }
            UqText {
                visible: root.note !== ""
                text: root.note
                color: Theme.textMuted
            }
            UqText {
                visible: root.symbols.length === 0
                text: "No symbols chosen yet."
                color: Theme.textMuted
            }
            Repeater {
                model: root.symbols
                delegate: RowLayout {
                    id: qrow
                    required property string modelData
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
                        Layout.preferredWidth: Theme.fontSize * 12
                        UqText {
                            text: qrow.modelData
                            font.bold: true
                        }
                        UqText {
                            text: qrow.q === null ? "No quote yet"
                                : (!qrow.q.ok && qrow.q.price === null ? "Not found" : qrow.q.name)
                            color: Theme.textMuted
                            Layout.fillWidth: true
                        }
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.preferredWidth: Theme.fontSize * 14
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
                }
            }
            MenuRow {
                id: refresh
                Layout.fillWidth: true
                text: "Refresh"
                onActivated: Bridge.call("RefreshStocks", []).catch(function () {})
                Component.onCompleted: forceActiveFocus()
            }
            MenuRow {
                Layout.fillWidth: true
                text: "Change the symbols…"
                onActivated: {
                    popup.hide();
                    root.editRequested();
                }
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
