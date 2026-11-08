import QtQuick
import QtTest
import "../../shell/core"

// Quickshell.Io cannot load under qmltestrunner (its plugin is built into the qs binary), so
// the real directory swap is simulated by an injected reader over an in-memory tree where a
// rename moves a whole directory. Reload logic re-resolves the path on every read; the real
// FileView wiring is exercised by tests/shell/theme_dirswap.sh under a headless qs.
TestCase {
    name: "theme-reload"

    property var tree: ({})

    function tokensJson(accent) {
        return JSON.stringify({ id: "x", variant: "dark", colors: { accent: accent } });
    }
    function fakeRead(path) {
        var parts = path.split("/").filter(function (s) { return s.length; });
        var n = tree;
        for (var i = 0; i < parts.length; i++) {
            if (n === undefined || typeof n === "string") return null;
            n = n[parts[i]];
        }
        return typeof n === "string" ? n : null;
    }

    function init() {
        Theme.statePathOverride = "/state";
        Theme.readText = fakeRead;
    }

    function test_reload_after_dir_swap() {
        tree = { state: { uq: { theme: { "tokens.json": tokensJson("#111111") } } } };
        Theme.reload();
        compare(Theme.accent.toString(), "#111111");
        // Phase 1 swaps the whole theme/ directory.
        tree.state.uq["theme.old"] = tree.state.uq.theme;
        tree.state.uq.theme = { "tokens.json": tokensJson("#222222") };
        Theme.dirChanged();
        tryVerify(function () { return Theme.accent.toString() === "#222222"; }, 3000);
    }

    function test_parse_failure_keeps_previous() {
        tree = { state: { uq: { theme: { "tokens.json": tokensJson("#333333") } } } };
        Theme.reload();
        compare(Theme.accent.toString(), "#333333");
        tree.state.uq.theme["tokens.json"] = "{ torn write";
        Theme.reload();
        compare(Theme.accent.toString(), "#333333");
        tree.state.uq.theme = undefined;
        Theme.reload();
        compare(Theme.accent.toString(), "#333333");
    }
}
