import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "SessionChanges"
    visible: true
    when: windowShown
    width: 600
    height: 500

    readonly property var entries: [
        { id: "1000-0004", time: 1000, origin: "settings", domain: "look", summary: "Corner rounding: 4 → 8" },
        { id: "1000-0003", time: 1000, origin: "uqctl", domain: "theme", summary: "Theme: gruvbox → dracula" },
        { id: "990-0002", time: 990, origin: "shell", domain: "shell", summary: "24-hour clock: off → on" },
        { id: "500-0001", time: 500, origin: "settings", domain: "look", summary: "Before this session" },
    ]

    Component {
        id: comp
        SessionChanges {
            entries: tc.entries
            sessionStart: 900
        }
    }

    function make() {
        return createTemporaryObject(comp, tc);
    }

    function test_count_label() {
        const s = make();
        const label = findChild(s, "sessionCountLabel");
        compare(label.text, "Changed this session (3)", "only entries from sessionStart on");
        compare(label.textFormat, Text.PlainText, "goes through UqLabel");
        s.entries = [];
        compare(label.text, "Changed this session (0)");
    }

    function test_origin_words_data() {
        return [
            { tag: "settings", origin: "settings", words: "here" },
            { tag: "uqctl", origin: "uqctl", words: "from the terminal" },
            { tag: "shell", origin: "shell", words: "from the panel" },
            { tag: "restore", origin: "restore", words: "by an undo or a restore" },
            { tag: "import", origin: "import", words: "from an import" },
            { tag: "profile", origin: "profile:gaming", words: "from the profile gaming" },
            { tag: "unknown", origin: "unknown", words: "from somewhere else" },
            { tag: "empty", origin: "", words: "from somewhere else" },
        ];
    }

    Component {
        id: originComp
        OriginLabel {}
    }

    function test_origin_words(data) {
        const label = createTemporaryObject(originComp, tc, { origin: data.origin });
        compare(label.text, data.words);
        compare(label.textFormat, Text.PlainText);
    }

    function test_rows_show_summary_and_origin() {
        const s = make();
        s.open();
        compare(s.rows.count, 3);
        const first = s.rows.itemAt(0);
        compare(findChild(first, "summaryText").text, "Corner rounding: 4 → 8");
        compare(findChild(first, "summaryText").textFormat, Text.PlainText);
        compare(findChild(first, "originText").text, "here");
        compare(findChild(s.rows.itemAt(1), "originText").text, "from the terminal");
        compare(findChild(s.rows.itemAt(2), "originText").text, "from the panel");
    }

    function test_undo_click_emits_the_id() {
        const s = make();
        const ids = [];
        s.undoRequested.connect(id => ids.push(id));
        s.open();
        const button = findChild(s.rows.itemAt(1), "undoButton");
        verify(button);
        tryVerify(() => button.visible && button.width > 0);
        // The sheet lives in the window's overlay and animates in, so a synthetic mouse click can
        // land before it is in place. Press the button directly: this still checks the wiring
        // from the row's button to undoRequested with that row's id.
        button.clicked();
        compare(ids, ["1000-0003"]);
    }

    function test_button_opens_the_sheet() {
        const s = make();
        verify(!s.sheetOpen);
        mouseClick(s);
        tryVerify(() => s.sheetOpen);
    }
}
