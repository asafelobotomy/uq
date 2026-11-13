import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"
import "../../apps/uq-settings/qml/components/barrows.js" as R

TestCase {
    id: tc
    name: "BarLayoutEditor"
    visible: true
    when: windowShown
    width: 800
    height: 700

    readonly property var catalog: [
        { id: "system", name: "System" }, { id: "work", name: "Workspaces" },
        { id: "clock", name: "Clock" }, { id: "stocks", name: "Stock ticker" },
        { id: "news", name: "News headlines" }, { id: "separator", name: "Separator" }
    ]
    readonly property var bar: ({
        start: [{ id: "system" }, { id: "work" }],
        center: [],
        end: [{ id: "clock", size: "wide" }, { id: "stocks" }],
        options: { stocks: { symbols: ["AAPL", "MSFT"] }, clock: { seconds: false, date: "short" } }
    })

    Component {
        id: comp
        BarLayoutEditor {
            width: 780
            catalog: tc.catalog
            bar: tc.bar
        }
    }

    // --- rows logic ---

    function test_rows_have_zone_headings() {
        const rows = R.rows(bar);
        compare(rows.map(r => r.kind === "zone" ? "#" + r.zone : r.id),
                ["#start", "system", "work", "#center", "#end", "clock", "stocks"]);
        compare(rows[5].size, "wide");
        compare(rows[6].index, 1);
    }

    function test_drag_moves_across_zones() {
        const rows = R.rows(bar);
        // "clock" (row 5) dropped just before the Centre heading's next row: into Centre.
        compare(R.moveEdit(rows, 5, 4), { op: "move", id: "clock", zone: "end", index: 0, to_zone: "center", to_index: 0 });
        // "system" (row 1) dropped at the very end.
        compare(R.moveEdit(rows, 1, 7), { op: "move", id: "system", zone: "start", index: 0, to_zone: "end", to_index: 2 });
        // Never above the first heading: it stays first in Left.
        compare(R.moveEdit(rows, 2, 0), { op: "move", id: "work", zone: "start", index: 1, to_zone: "start", to_index: 0 });
        compare(R.moveEdit(rows, 1, 1), null, "no move");
        compare(R.moveEdit(rows, 1, 2), null, "dropped where it was");
        compare(R.moveEdit(rows, 0, 3), null, "headings don't move");
    }

    function test_steps_cross_headings() {
        const rows = R.rows(bar);
        // "work" down: past the Centre heading, first in Centre.
        compare(R.stepEdit(rows, 2, 1), { op: "move", id: "work", zone: "start", index: 1, to_zone: "center", to_index: 0 });
        // "clock" up: past Right and the empty Centre? No, one step: last in Centre.
        compare(R.stepEdit(rows, 5, -1), { op: "move", id: "clock", zone: "end", index: 0, to_zone: "center", to_index: 0 });
        compare(R.stepEdit(rows, 1, -1), null, "first island can't go up");
        compare(R.stepEdit(rows, 6, 1), null, "last island can't go down");
    }

    function test_list_helpers_and_checks() {
        compare(R.listAdd(["A"], "B", 2), ["A", "B"]);
        compare(R.listAdd(["A", "B"], "C", 2), null);
        compare(R.listAdd(["A"], "A", 5), null);
        compare(R.listMove(["A", "B", "C"], 2, -1), ["A", "C", "B"]);
        compare(R.listMove(["A"], 0, -1), null);
        compare(R.listRemove(["A", "B"], 0), ["B"]);
        compare(R.cleanSymbol(" btc-usd "), "BTC-USD");
        compare(R.cleanSymbol("<b>"), null);
        compare(R.cleanFeed("http://x.org/rss"), null);
        compare(R.cleanFeed(" https://x.org/rss "), "https://x.org/rss");
        compare(R.cleanZone("America/New York"), "America/New_York");
        compare(R.cleanZone("../etc"), null);
        compare(R.addable(bar, catalog).map(c => c.id), ["news", "separator"]);
    }

    // --- the component ---

    function make(props) {
        const e = createTemporaryObject(comp, tc, props || {});
        tryVerify(() => e.list.itemAtIndex(e.list.count - 1) !== null, 2000, "every row is drawn");
        return e;
    }

    function test_buttons_send_intents() {
        const e = make();
        const got = [];
        e.edit.connect(x => { got.push(JSON.parse(JSON.stringify(x))); });
        e.list.itemAtIndex(2).downButton.clicked();
        e.list.itemAtIndex(6).removeButton.clicked();
        compare(got, [
            { op: "move", id: "work", zone: "start", index: 1, to_zone: "center", to_index: 0 },
            { op: "remove", id: "stocks", zone: "end", index: 1 }
        ]);
        verify(!e.list.itemAtIndex(1).upButton.enabled);
        verify(!e.list.itemAtIndex(6).downButton.enabled);
    }

    function test_size_and_options() {
        const e = make();
        const got = [];
        e.edit.connect(x => { got.push(JSON.parse(JSON.stringify(x))); });
        const clock = e.list.itemAtIndex(5);
        compare(clock.sizeBox.currentIndex, 2, "wide");
        clock.sizeBox.activated(0);
        compare(got[0], { op: "size", id: "clock", zone: "end", index: 0, size: "compact" });
        verify(clock.optionsButton.visible);
        verify(!e.list.itemAtIndex(1).optionsButton.visible, "System has no options");
        const stocks = e.list.itemAtIndex(6);
        stocks.optionsButton.clicked();
        tryVerify(() => stocks.optionsOpen);
        const opts = findChild(stocks, "optionField").parent.parent;
        opts.add("tsla");
        compare(got[1], { op: "options", island: "stocks", value: { symbols: ["AAPL", "MSFT", "TSLA"] } });
        verify(!opts.add("not a symbol"));
        verify(opts.addError !== "");
        opts.listRepeater.itemAt(0).removeButton.clicked();
        compare(got[2], { op: "options", island: "stocks", value: { symbols: ["MSFT"] } });
    }

    function test_add_and_reset() {
        const e = make();
        const got = [];
        e.edit.connect(x => { got.push(JSON.parse(JSON.stringify(x))); });
        compare(e.addCombo.count, 2);
        findChild(e, "addButton").clicked();
        compare(got[0], { op: "add", id: "news", zone: "end" });
        findChild(e, "resetBar").clicked();
        compare(got[1], { op: "reset" });
    }

    function test_new_bar_redraws_rows() {
        const e = make();
        compare(e.list.count, 7);
        e.bar = { start: [], center: [{ id: "clock" }], end: [] };
        compare(e.list.count, 4);
    }

    // Keyboard users keep their place: after a move, once uqd's answer redraws the rows, focus is
    // on the moved island's button again (not on whatever row now sits where it was).
    function test_focus_follows_the_moved_island() {
        const e = make();
        e.edit.connect(x => {
            const b = JSON.parse(JSON.stringify(tc.bar));
            const item = b[x.zone].splice(x.index, 1)[0];
            b[x.to_zone].splice(x.to_index, 0, item);
            e.bar = b;
        });
        e.list.itemAtIndex(2).downButton.forceActiveFocus();
        e.list.itemAtIndex(2).downButton.clicked();
        tryVerify(() => {
            const it = e.list.itemAtIndex(3);
            return it && it.islandId === "work" && it.downButton.activeFocus;
        }, 2000, "Workspaces, now first in Centre, has its down button focused");
    }

    // Options edits don't rebuild the rows, so an open options panel stays.
    function test_options_edit_keeps_rows() {
        const e = make();
        const stocks = e.list.itemAtIndex(6);
        stocks.optionsButton.clicked();
        tryVerify(() => stocks.optionsOpen);
        const b = JSON.parse(JSON.stringify(tc.bar));
        b.options.stocks.symbols = ["AAPL"];
        e.bar = b;
        compare(e.list.itemAtIndex(6), stocks, "the same row, not a new one");
    }

    function test_disabled_editor_sends_nothing() {
        const e = make({ enabled: false });
        verify(!e.list.itemAtIndex(2).downButton.enabled);
        verify(!e.list.itemAtIndex(2).removeButton.enabled);
    }
}
