import QtQuick
import QtTest
import "../../shell/logic/tokens.js" as Tokens

TestCase {
    name: "tokens"

    readonly property string valid: JSON.stringify({
        id: "x", name: "X", variant: "light",
        colors: { accent: "#123456", bg: "#ffffff" },
        ui: { radius: 3, gap_in: 2, gap_out: 9, border_width: 1 },
        fonts: { ui: "Foo", mono: "Bar Mono", size: 14 }
    })

    function test_valid_maps_accent() {
        var t = Tokens.toTheme(Tokens.parse(valid));
        compare(t.accent, "#123456");
        compare(t.bg, "#ffffff");
        compare(t.radius, 3);
        compare(t.gapIn, 2);
        compare(t.gapOut, 9);
        compare(t.borderWidth, 1);
        compare(t.fontUi, "Foo");
        compare(t.fontMono, "Bar Mono");
        compare(t.fontSize, 14);
        compare(t.variant, "light");
    }

    function test_garbage_is_null() {
        compare(Tokens.parse("not json {"), null);
        compare(Tokens.parse(""), null);
        compare(Tokens.parse(null), null);
        compare(Tokens.parse("[1,2]"), null);
        compare(Tokens.parse("42"), null);
    }

    function test_missing_key_falls_back_to_default() {
        var t = Tokens.toTheme(Tokens.parse(valid));
        compare(t.surface, Tokens.defaults().surface);
        compare(t.text, "#e4e4e7");
        var e = Tokens.toTheme({});
        compare(e.accent, "#8ab4f8");
        compare(e.radius, 10);
        compare(e.fontUi, "Inter");
        compare(e.variant, "dark");
    }

    function test_wrong_types_fall_back() {
        var t = Tokens.toTheme({ colors: { accent: 5, bg: "nothex" }, ui: { radius: "big" },
                                 fonts: { size: -3 } });
        compare(t.accent, "#8ab4f8");
        compare(t.bg, "#1f1f22");
        compare(t.radius, 10);
        compare(t.fontSize, 11);
    }
}
