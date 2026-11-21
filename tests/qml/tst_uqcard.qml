import QtQuick
import QtTest
import "../../shell/core"

// UqCard: every shell popup's card. Theme surface, border and corners; fades and grows in when
// shown, fades out when hidden, hides itself once gone; instant under ui.reduce_motion.
TestCase {
    name: "uqcard"

    Component {
        id: c
        UqCard { width: 200; height: 100 }
    }

    function cleanup() {
        Settings.values = ({});
    }

    function test_theme_look() {
        var card = createTemporaryObject(c, this);
        compare(card.color, Theme.surface);
        compare(card.border.color, Theme.border);
        compare(card.border.width, Theme.borderWidth);
        compare(card.radius, Theme.radius);
    }

    function test_shown_from_the_start_is_not_animated() {
        var card = createTemporaryObject(c, this);
        compare(card.reveal, 1);
        compare(card.opacity, 1);
        compare(card.scale, 1);
    }

    function test_comes_in_and_goes() {
        var card = createTemporaryObject(c, this, { shown: false });
        compare(card.reveal, 0);
        compare(card.opacity, 0);
        card.shown = true;
        // Part-way in on the way: smaller and see-through, then whole.
        tryVerify(function () { return card.reveal > 0 && card.reveal < 1; }, 1000);
        verify(card.scale < 1);
        tryCompare(card, "reveal", 1);
        compare(card.scale, 1);
        card.shown = false;
        tryCompare(card, "reveal", 0);
        compare(card.opacity, 0);
    }

    function test_reduce_motion_is_instant() {
        Settings.values = ({ ui: { reduce_motion: true } });
        var card = createTemporaryObject(c, this, { shown: false });
        card.shown = true;
        tryCompare(card, "reveal", 1, 50);
        compare(card.scale, 1);
    }

    function test_control_radii_follow_the_theme() {
        var old = Theme.radius;
        Theme.radius = 10;
        compare(Theme.radiusControl, 8);
        compare(Theme.radiusInner, 6);
        Theme.radius = 0;
        compare(Theme.radiusControl, 0);
        compare(Theme.radiusInner, 0);
        Theme.radius = old;
    }
}
