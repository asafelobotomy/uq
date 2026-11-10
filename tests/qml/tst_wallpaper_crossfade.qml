import QtQuick
import QtTest
import "../../shell/logic/wallpaper.js" as W
import "../../shell/wallpaper"

Item {
    width: 200
    height: 100

    Crossfade {
        id: cf

        anchors.fill: parent
        fadeMs: 20
    }

    TestCase {
        id: tc

        name: "wallpaper_crossfade"
        when: windowShown

        readonly property string dir: decodeURIComponent(Qt.resolvedUrl("fixtures/").toString().substring(7))

        function u(name) { return W.fileUrl(dir + name); }

        function showsNow(url) {
            return cf.shownTarget === url && cf.shown.status === Image.Ready && cf.shown.opacity === 1;
        }

        function init() {
            cf.target = "";
            cf.clearOnFail = true;
            cf.reduceMotion = false;
        }

        function test_space_and_non_ascii_path_shows() {
            var url = u("odd name #1 é.png");
            cf.target = url;
            tryVerify(function () { return showsNow(url); });
        }

        function test_a_b_a_shows_a_again() {
            cf.target = u("red.png");
            tryVerify(function () { return showsNow(u("red.png")); });
            cf.target = u("blue.png");
            tryVerify(function () { return showsNow(u("blue.png")); });
            cf.target = u("red.png");
            tryVerify(function () { return showsNow(u("red.png")); });
            cf.target = u("blue.png");
            tryVerify(function () { return showsNow(u("blue.png")); });
        }

        function test_change_mid_fade_settles_on_latest() {
            cf.fadeMs = 200;
            cf.target = u("red.png");
            tryVerify(function () { return cf.shown !== null; });
            cf.target = u("blue.png");
            wait(50);
            cf.target = u("red.png");
            tryVerify(function () { return showsNow(u("red.png")); }, 3000);
            cf.fadeMs = 20;
        }

        function test_unreadable_path_shows_fallback() {
            var n = 0;
            cf.failed.connect(function () { n++; });
            cf.target = u("red.png");
            tryVerify(function () { return showsNow(u("red.png")); });
            cf.target = u("bad.png");
            tryVerify(function () { return n >= 1; });
            compare(cf.shown, null);
            cf.target = u("missing.png");
            tryVerify(function () { return n >= 2; });
            compare(cf.shown, null);
        }

        function test_keeps_old_image_when_not_clearing() {
            cf.clearOnFail = false;
            cf.target = u("red.png");
            tryVerify(function () { return showsNow(u("red.png")); });
            cf.target = u("bad.png");
            wait(200);
            verify(cf.shown !== null);
            compare(cf.shownTarget, u("red.png"));
        }

        function test_reduce_motion_is_immediate() {
            cf.reduceMotion = true;
            cf.target = u("red.png");
            tryVerify(function () { return showsNow(u("red.png")); });
            cf.target = u("blue.png");
            tryVerify(function () { return showsNow(u("blue.png")); });
        }
    }
}
