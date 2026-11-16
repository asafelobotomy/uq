import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "UqAppIcon"
    visible: true
    when: windowShown
    width: 200
    height: 100

    Component {
        id: comp
        UqAppIcon {}
    }

    function test_plain_theme_names_are_looked_up() {
        const c = createTemporaryObject(comp, tc, { name: "org.gimp.GIMP", label: "GIMP" });
        verify(c);
        verify(c.plainName);
        compare(c.Accessible.name, "GIMP");
    }

    // A name from an app's output must never reach the image loader as a path or URL.
    function test_paths_and_urls_show_the_letter_tile() {
        for (const bad of ["/etc/passwd", "file:///x.png", "https://example.com/i.png", "../x", ".hidden", "a b", ""]) {
            const c = createTemporaryObject(comp, tc, { name: bad, label: "Steam" });
            verify(!c.plainName, bad);
            verify(!c.found, bad);
        }
    }

    // No dashes: Qt looks up "a-b-c" as "a-b", then "a", so a dashed name can find a generic icon.
    function test_letter_tile_uses_the_first_letter() {
        const c = createTemporaryObject(comp, tc, { name: "nosuchiconanywherexyz", label: "steam", size: 48 });
        verify(c);
        compare(c.width, 48);
        tryCompare(c, "found", false);
    }
}
