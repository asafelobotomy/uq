import QtQuick
import QtTest
import "../../shell/logic/screenshot.js" as S

TestCase {
    name: "screenshot"

    function test_valid_paths() {
        verify(S.validPath("/home/me/Pictures/Screenshots/Screenshot_20261117_101500.png"));
        verify(S.validPath("/tmp/a b #1 é.PNG"));
        var bad = ["", "x.png", "relative/a.png", "/a/b.jpg", "/a/../etc/x.png", "/a/\nb.png", "/", null, undefined, 5, {}];
        for (var i = 0; i < bad.length; i++)
            verify(!S.validPath(bad[i]), String(bad[i]));
    }

    function test_file_url_escapes() {
        compare(S.fileUrl("/a/b c#d?e.png"), "file:///a/b%20c%23d%3Fe.png");
    }

    function test_show() {
        compare(S.show("/a/b.png", "copied"), { path: "/a/b.png", copied: true });
        compare(S.show("/a/b.png", ""), { path: "/a/b.png", copied: false });
        compare(S.show("/etc/passwd", "copied"), null);
    }

    function test_labels() {
        compare(S.baseName("/a/b/c.png"), "c.png");
        compare(S.folderLabel("/home/me/Pictures/Screenshots/c.png", "/home/me"), "~/Pictures/Screenshots");
        compare(S.folderLabel("/home/meow/c.png", "/home/me"), "/home/meow");
        compare(S.folderLabel("/c.png", "/home/me"), "/");
        compare(S.status({ copied: true }, false, false), "Copied to clipboard");
        compare(S.status({ copied: false }, false, false), "Saved");
        compare(S.status({ copied: true }, true, false), "Discarded");
        verify(S.status({ copied: false }, false, true).indexOf("Couldn't copy") === 0);
    }

    function test_viewer() {
        compare(S.where("/home/me/Pictures/Screenshots/c.png", "/home/me"), "c.png in ~/Pictures/Screenshots");
        compare(S.viewerCommand("/a/-b.png", false), ["qview", "--", "/a/-b.png"]);
        compare(S.viewerCommand("/a/b c.png", true), ["qview", "--markup", "/a/b c.png"]);
    }
}
