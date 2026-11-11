// A page's offscreen test with the stub org.uq.settings module and a fake ctx:
//   QT_QPA_PLATFORM=offscreen qmltestrunner -import tests/uq/stub -input tests/uq
// Point `pageUrl` at your page (from tests/uq/: "../../uq/pages/<id>/Page.qml").
import QtQuick
import QtTest

Item {
    id: root
    width: 600
    height: 400

    readonly property url pageUrl: Qt.resolvedUrl("../settings-page/Page.qml")

    FakeContext {
        id: fake
        commands: ({
                status: { code: 0, output: { state: "running", last: "notes" } },
                forget: { code: 0, output: null }
            })
        confirmed: ["forget"]
    }

    Loader {
        id: loader
        anchors.fill: parent
    }

    function findLabel(item, text) {
        if (item.text === text)
            return item;
        for (let i = 0; i < item.children.length; ++i) {
            const hit = findLabel(item.children[i], text);
            if (hit)
                return hit;
        }
        return null;
    }

    TestCase {
        name: "SettingsPage"
        when: windowShown

        function init() {
            fake.calls = [];
            loader.source = "";
            loader.source = root.pageUrl;
            verify(loader.status === Loader.Ready, "the page loads");
            loader.item.ctx = fake.ctx;
        }

        function test_reads_status_on_open() {
            tryVerify(() => findLabel(loader.item, "Running: running") !== null);
            compare(fake.calls[0], ["watch", "status"]);
            compare(fake.calls[1], ["run", "status", []]);
        }

        function test_a_change_reads_again() {
            tryVerify(() => findLabel(loader.item, "Running: running") !== null);
            fake.commands = ({ status: { code: 0, output: { state: "paused", last: "" } } });
            fake.change("status");
            tryVerify(() => findLabel(loader.item, "Running: paused") !== null);
            fake.commands = ({ status: { code: 0, output: { state: "running", last: "notes" } }, forget: { code: 0, output: null } });
        }

        function test_a_failed_read_shows_the_error() {
            fake.commands = ({ status: { error: "timed out" } });
            loader.source = "";
            loader.source = root.pageUrl;
            loader.item.ctx = fake.ctx;
            tryVerify(() => findLabel(loader.item, "timed out") !== null);
            fake.commands = ({ status: { code: 0, output: { state: "running", last: "notes" } }, forget: { code: 0, output: null } });
        }
    }
}
