import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 600
    height: 300

    Component {
        id: comp
        DisplayArrangement {
            width: 600
            height: 300
            fixedScale: 0.1
            outputs: [
                { name: "eDP-1", label: "Laptop screen", enabled: true, mirror_of: null, rect: { x: 0, y: 0, w: 1600, h: 1000 } },
                { name: "DP-1", label: "Desk monitor", enabled: true, mirror_of: null, rect: { x: 1600, y: 0, w: 2560, h: 1440 } },
                { name: "HDMI-A-1", label: "Off", enabled: false, mirror_of: null, rect: { x: 0, y: 0, w: 1920, h: 1080 } }
            ]
        }
    }

    TestCase {
        name: "DisplayArrangement"
        when: windowShown

        function find(a, name) {
            return findChild(a, "display-" + name);
        }

        function test_only_displays_that_are_on() {
            const a = createTemporaryObject(comp, root);
            verify(find(a, "DP-1"));
            verify(!find(a, "HDMI-A-1"));
        }

        function test_drag_emits_output_coordinates() {
            const a = createTemporaryObject(comp, root);
            const got = [];
            a.moved.connect((n, x, y) => got.push([n, x, y]));
            const box = find(a, "DP-1");
            compare(box.width, 256);
            const startX = box.x;
            // Drag 50 preview px right and 20 down: up to 500 and 200 display px at scale 0.1
            // (the drag threshold eats a few preview px).
            mouseDrag(a, box.x + 10, box.y + 10, 50, 20);
            compare(got.length, 1);
            compare(got[0][0], "DP-1");
            verify(got[0][1] > 1900 && got[0][1] <= 2110, JSON.stringify(got));
            verify(got[0][2] > 50 && got[0][2] <= 210, JSON.stringify(got));
            compare(box.x, startX, "waits for the model before moving");
        }

        function test_click_picks() {
            const a = createTemporaryObject(comp, root);
            const got = [];
            a.picked.connect(n => got.push(n));
            mouseClick(find(a, "eDP-1"));
            compare(got, ["eDP-1"]);
        }
    }
}
