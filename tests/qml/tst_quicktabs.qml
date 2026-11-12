import QtQuick
import QtTest
import "../../shell/core"
import "../../shell/quick"
import "../../shell/logic/quickui.js" as U

// The quick-settings tab strip: roving focus, arrow keys wrap, Home / End, click, and sizes and
// colours from Theme.
TestCase {
    name: "quicktabs"
    width: 400; height: 100
    visible: true
    when: windowShown

    Component {
        id: c
        QuickTabs {
            width: 360
            tabs: U.shownTabs({ apps: true })
            current: "sound"
            onChosen: function (key) { current = key; }
        }
    }

    function tabItem(bar, key) {
        var found = null;
        function walk(it) {
            for (var i = 0; i < it.children.length; i++) {
                var ch = it.children[i];
                if (ch.key === key)
                    found = ch;
                else
                    walk(ch);
            }
        }
        walk(bar);
        return found;
    }

    function test_roving_focus_and_arrows() {
        var bar = createTemporaryObject(c, this);
        bar.focusCurrent();
        verify(tabItem(bar, "sound").activeFocus);
        // Only the selected tab is in the Tab chain.
        verify(tabItem(bar, "sound").activeFocusOnTab);
        verify(!tabItem(bar, "network").activeFocusOnTab);

        keyClick(Qt.Key_Right);
        compare(bar.current, "network");
        verify(tabItem(bar, "network").activeFocus);
        keyClick(Qt.Key_Down);
        compare(bar.current, "display");
        keyClick(Qt.Key_Left);
        compare(bar.current, "network");
        keyClick(Qt.Key_Up);
        keyClick(Qt.Key_Up);
        compare(bar.current, "system");     // wraps
        verify(tabItem(bar, "system").activeFocus);
        verify(!tabItem(bar, "sound").activeFocusOnTab);
        keyClick(Qt.Key_Home);
        compare(bar.current, "sound");
        keyClick(Qt.Key_End);
        compare(bar.current, "system");
        // Ctrl+arrows are left to the panel.
        keyClick(Qt.Key_Right, Qt.ControlModifier);
        compare(bar.current, "system");
    }

    function test_click_selects_and_focuses() {
        var bar = createTemporaryObject(c, this);
        waitForItemPolished(bar);
        var t = tabItem(bar, "display");
        verify(t.width > 0 && t.height > 0);
        mouseClick(t);
        compare(bar.current, "display");
        verify(t.activeFocus);
        verify(t.activeFocusOnTab);
        verify(!tabItem(bar, "sound").activeFocusOnTab);
    }

    function test_theme_tokens() {
        var bar = createTemporaryObject(c, this);
        var sel = tabItem(bar, "sound"), other = tabItem(bar, "network");
        compare(bar.color, Theme.surfaceAlt);
        compare(bar.radius, Theme.radius);
        compare(sel.color, Theme.accent);
        compare(sel.radius, Math.max(0, Theme.radius - Theme.gapIn));
        bar.focusCurrent();
        compare(sel.border.width, Theme.borderWidth);
        compare(other.border.width, 0);
        // The strip follows a theme change.
        var old = Theme.accent;
        Theme.accent = "#ff0000";
        compare(sel.color, Theme.accent);
        Theme.accent = old;
    }
}
