import QtQuick
import QtTest
import org.kde.kirigami as Kirigami
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "SettingRowSource"
    visible: true
    when: windowShown
    width: 600
    height: 300

    Component {
        id: comp
        SettingRow {
            width: 600
            settingId: "gaps-inner"
            label: "Gaps between windows"
        }
    }

    Component {
        id: plainLabel
        UqLabel {
            text: "Gaps between windows"
        }
    }

    function make(source) {
        // Never open a real file from a test.
        return createTemporaryObject(comp, tc, { source: source, opensFiles: false });
    }

    function text(row) {
        const t = findChild(row, "sourceText");
        return t && t.visible ? t.text : "";
    }

    function shown(row, name) {
        const item = findChild(row, name);
        return item !== null && item.visible;
    }

    function test_labels_data() {
        return [
            { tag: "theme", source: { source: "theme", detail: null }, text: "From the theme", muted: true },
            { tag: "default", source: { source: "default", detail: null }, text: "Default", muted: true },
            { tag: "user_lua", source: { source: "user_lua", detail: { line: 3, path: "/home/u/.config/hypr/user.lua" } },
              text: "Overridden in user.lua, line 3", muted: false },
            { tag: "runtime", source: { source: "runtime", detail: null }, text: "Changed while running (not by uQ)", muted: false },
            { tag: "error", source: { source: "error", detail: { message: "Hyprland rejected the new config: bad option" } },
              text: "Hyprland rejected the new config: bad option", muted: false },
            { tag: "uq", source: { source: "uq", detail: null }, text: "", muted: false },
            { tag: "null", source: null, text: "", muted: false },
        ];
    }

    function test_labels(data) {
        const row = make(data.source);
        compare(text(row), data.text);
        const t = findChild(row, "sourceText");
        compare(t.textFormat, Text.PlainText, "goes through UqLabel");
        if (data.text !== "")
            compare(t.opacity < 1, data.muted, "muted only for theme and default");
        compare(shown(row, "sourceWarning"), data.tag === "user_lua");
        compare(shown(row, "openUserLuaButton"), data.tag === "user_lua");
        compare(shown(row, "reapplyButton"), data.tag === "runtime");
    }

    function test_profile_source() {
        const row = make({ source: "profile", detail: { name: "Gaming" } });
        compare(text(row), "From the Gaming profile");
        const t = findChild(row, "sourceText");
        compare(t.textFormat, Text.PlainText, "a profile's name is never rich text");
        verify(t.opacity < 1, "muted like the theme");
        verify(!shown(row, "sourceWarning"));
        verify(!shown(row, "openUserLuaButton"));
        verify(!shown(row, "reapplyButton"));
        compare(text(make({ source: "profile", detail: null })), "From a profile");
        compare(text(make({ source: "profile", detail: { name: "" } })), "From a profile");
        const rich = make({ source: "profile", detail: { name: "<b>x</b>" } });
        compare(text(rich), "From the <b>x</b> profile");
        compare(findChild(rich, "sourceText").textFormat, Text.PlainText);
    }

    function test_error_uses_the_negative_colour() {
        const row = make({ source: "error", detail: { message: "nope" } });
        verify(Qt.colorEqual(findChild(row, "sourceText").color, Kirigami.Theme.negativeTextColor));
    }

    function test_open_user_lua_emits_line_and_opens_the_file() {
        const row = make({ source: "user_lua", detail: { line: 3, path: "/home/u/.config/hypr/user.lua" } });
        const lines = [];
        row.openUserLua.connect(line => lines.push(line));
        const button = findChild(row, "openUserLuaButton");
        verify(button.enabled);
        button.clicked();
        compare(lines, [3]);
        compare(row.userLuaUrl, "file:///home/u/.config/hypr/user.lua");
        compare(text(row), "Overridden in user.lua, line 3", "the label keeps the line number");
    }

    function test_user_lua_url_is_percent_encoded() {
        const row = make({ source: "user_lua", detail: { line: 1, path: "/home/a #b?c%d/user.lua" } });
        compare(row.userLuaUrl, "file:///home/a%20%23b%3Fc%25d/user.lua");
    }

    function test_runtime_reapply_emits() {
        const row = make({ source: "runtime", detail: null });
        const got = [];
        row.reapply.connect(() => got.push(true));
        findChild(row, "reapplyButton").clicked();
        compare(got.length, 1);
    }

    function test_no_source_keeps_the_row_height() {
        const bare = createTemporaryObject(comp, tc);
        const none = make(null);
        const uq = make({ source: "uq", detail: null });
        const label = createTemporaryObject(plainLabel, tc);
        // Today's row: one line of label plus the row's padding.
        compare(bare.implicitHeight, label.implicitHeight + Kirigami.Units.smallSpacing * 2);
        compare(none.implicitHeight, bare.implicitHeight);
        compare(uq.implicitHeight, bare.implicitHeight);
        const themed = make({ source: "theme", detail: null });
        verify(themed.implicitHeight > bare.implicitHeight, "a source line adds height");
    }

    function test_source_changes_follow() {
        const row = make({ source: "runtime", detail: null });
        compare(text(row), "Changed while running (not by uQ)");
        row.source = null;
        compare(text(row), "");
        verify(!shown(row, "reapplyButton"));
    }
}
