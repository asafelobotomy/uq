import QtQuick
import QtTest
import "../../apps/uq-settings/qml/components"

Item {
    id: root
    width: 400
    height: 200

    Component {
        id: comp
        ThemeChip {}
    }

    Component {
        id: spyComp
        SignalSpy {}
    }

    TestCase {
        name: "ThemeChip"
        when: windowShown

        function make(theme) {
            const c = createTemporaryObject(comp, root, { theme: theme });
            verify(c);
            return c;
        }

        function test_name_is_plain_text() {
            const c = make({ id: "user-x", name: "<b>x</b>", user: true, active: false, missing: false });
            const label = findChild(c, "themeName");
            verify(label);
            compare(label.text, "<b>x</b>");
            compare(label.textFormat, Text.PlainText);
        }

        function test_menu_only_for_user_themes() {
            const system = make({ id: "gruvbox", name: "Gruvbox", user: false, active: true, missing: false });
            verify(!findChild(system, "themeMenuButton").visible);
            verify(!findChild(system, "yoursTag").visible);
            const mine = make({ id: "user-ocean", name: "Ocean", user: true, active: false, missing: false });
            verify(findChild(mine, "themeMenuButton").visible);
            verify(findChild(mine, "yoursTag").visible);
            compare(findChild(mine, "yoursTag").text, "Yours");
        }

        function test_menu_items_send_the_theme_id() {
            const c = make({ id: "user-ocean", name: "Ocean", user: true, active: false, missing: false });
            const edit = createTemporaryObject(spyComp, root, { target: c, signalName: "edit" });
            const exp = createTemporaryObject(spyComp, root, { target: c, signalName: "exportTheme" });
            const del = createTemporaryObject(spyComp, root, { target: c, signalName: "remove" });
            findChild(c, "editItem").triggered();
            findChild(c, "exportItem").triggered();
            findChild(c, "deleteItem").triggered();
            compare(edit.count, 1);
            compare(edit.signalArguments[0][0], "user-ocean");
            compare(exp.signalArguments[0][0], "user-ocean");
            compare(del.signalArguments[0][0], "user-ocean");
            compare(findChild(c, "editItem").text, "Edit…");
            compare(findChild(c, "exportItem").text, "Export…");
            compare(findChild(c, "deleteItem").text, "Delete…");
        }

        function test_click_chooses_the_theme() {
            const c = make({ id: "dracula", name: "Dracula", user: false, active: false, missing: false });
            const chosen = createTemporaryObject(spyComp, root, { target: c, signalName: "chosen" });
            findChild(c, "themeButton").clicked();
            compare(chosen.count, 1);
            compare(chosen.signalArguments[0][0], "dracula");
        }

        function test_missing_line() {
            const gone = make({ id: "user-old", name: "Old", user: true, active: true, missing: true });
            verify(findChild(gone, "missingLine").visible);
            compare(findChild(gone, "missingLine").text, "Its files were removed");
            const fine = make({ id: "user-new", name: "New", user: true, active: false, missing: false });
            verify(!findChild(fine, "missingLine").visible);
        }

        function test_accessible_name() {
            const c = make({ id: "user-x", name: "<b>x</b>", user: true, active: false, missing: false });
            compare(c.Accessible.name, "Theme <b>x</b>");
            compare(findChild(c, "themeButton").Accessible.name, "Theme <b>x</b>");
        }
    }
}
