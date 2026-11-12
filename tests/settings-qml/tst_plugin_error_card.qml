import QtQuick
import QtTest
import "../../apps/uq-settings/qml"
import "../../apps/uq-settings/qml/components"

TestCase {
    id: tc
    name: "PluginPage"
    visible: true
    when: windowShown
    width: 400
    height: 300

    Component {
        id: pageComp
        PluginPage {
            width: 400
            height: 300
        }
    }

    function findByName(item, name) {
        if (item.objectName === name)
            return item;
        for (let i = 0; i < item.children.length; ++i) {
            const hit = findByName(item.children[i], name);
            if (hit)
                return hit;
        }
        return null;
    }

    function test_broken_page_shows_error_card() {
        const page = createTemporaryObject(pageComp, tc, {
            source: Qt.resolvedUrl("fixtures/broken/Page.qml")
        });
        verify(page);
        verify(page.failed);
        const card = findByName(page, "errorDetail").parent.parent;
        verify(card.visible);
        compare(findByName(page, "errorTitle").text, "This page couldn't load");
        const detail = findByName(page, "errorDetail").text;
        verify(detail.length > 0, "detail is empty");
        verify(detail.indexOf("Page.qml") >= 0, "detail names the file: " + detail);
    }

    function test_good_page_loads_with_theme_and_no_card() {
        const page = createTemporaryObject(pageComp, tc, {
            source: Qt.resolvedUrl("fixtures/good/Page.qml"),
            theme: { bg: "#112233" }
        });
        verify(page);
        verify(!page.failed);
        const loaded = page.children[0].item;
        compare(loaded.marker, "loaded");
        compare(loaded.theme.bg, "#112233");
        verify(page.visible);
        verify(!page.children[1].visible);
    }

    // A theme change while the page is open reaches its `theme` and its ctx.
    function test_theme_change_reaches_an_open_page() {
        const page = createTemporaryObject(pageComp, tc, {
            source: Qt.resolvedUrl("fixtures/ctx/Page.qml"),
            pageId: "ctx",
            theme: { accent: "#00ff00" }
        });
        verify(!page.failed);
        const loaded = page.children[0].item;
        page.theme = { accent: "#ff0000" };
        compare(loaded.ctx.theme.accent, "#ff0000");
        const good = createTemporaryObject(pageComp, tc, {
            source: Qt.resolvedUrl("fixtures/good/Page.qml"),
            theme: { bg: "#112233" }
        });
        good.theme = { bg: "#445566" };
        compare(good.children[0].item.theme.bg, "#445566");
    }

    function test_page_gets_a_context() {
        const page = createTemporaryObject(pageComp, tc, {
            source: Qt.resolvedUrl("fixtures/ctx/Page.qml"),
            pageId: "ctx",
            theme: { accent: "#00ff00" }
        });
        verify(!page.failed);
        const loaded = page.children[0].item;
        compare(loaded.ctx.theme.accent, "#00ff00");
        // No host here: the request fails with an error instead of throwing.
        tryVerify(() => loaded.result !== null);
        verify(loaded.result.error.length > 0);
        compare(page.context.pageId, "ctx");
    }

    function test_page_may_use_sibling_files() {
        // logic.js imported as `import "logic.js" as Logic`, and a component file beside it.
        const page = createTemporaryObject(pageComp, tc, {
            source: Qt.resolvedUrl("fixtures/logic/Page.qml")
        });
        verify(!page.failed, page.errorText);
        const loaded = page.children[0].item;
        compare(loaded.marker, "Running");
        compare(loaded.row.summary, "Checking…");
    }

    function test_missing_file_shows_error_card() {
        const page = createTemporaryObject(pageComp, tc, {
            source: Qt.resolvedUrl("fixtures/nope/Page.qml")
        });
        verify(page.failed);
        verify(page.errorText.length > 0);
    }
}
