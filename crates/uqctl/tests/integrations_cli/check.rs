//! Checking an integration folder.

use super::*;

#[test]
fn a_clean_folder_passes() {
    let d = tempfile::tempdir().unwrap();
    let uq = d.path().join("uq");
    good_folder(&uq);
    let (ok, out) = uqctl(&["integrations", "check", uq.to_str().unwrap()]);
    assert!(ok, "{out}");
    assert_eq!(
        out.trim(),
        "ok: 1 widget(s), 1 page(s), 1 provider(s), 1 app action(s), 1 service file(s), \
         1 rule file(s)"
    );
}

#[test]
fn a_missing_folder_fails() {
    let (ok, out) = uqctl(&["integrations", "check", "/nonexistent/uq"]);
    assert!(!ok);
    assert!(out.contains("1 problem(s)"), "{out}");
}

/// A page may ship sibling files: `logic.js` (imported from its QML) and further component
/// files. They pass the check, and the text rule covers them too.
#[test]
fn pages_may_ship_logic_js_and_components() {
    let d = tempfile::tempdir().unwrap();
    let uq = d.path().join("uq");
    good_folder(&uq);
    put(
        &uq.join("pages/demo/Page.qml"),
        "import QtQuick\nimport \"logic.js\" as Logic\nItem {\n    property var ctx\n    \
         StatusRow {}\n}\n",
    );
    put(
        &uq.join("pages/demo/logic.js"),
        ".pragma library\nfunction describe(s) { return s ? \"Running\" : \"Stopped\"; }\n",
    );
    put(
        &uq.join("pages/demo/StatusRow.qml"),
        "import QtQuick\nItem {}\n",
    );
    let dir = uq.to_str().unwrap();
    let (ok, out) = uqctl(&["integrations", "check", dir]);
    assert!(ok, "{out}");

    put(
        &uq.join("pages/demo/StatusRow.qml"),
        "import QtQuick\nItem {\n    Text { text: \"x\" }\n}\n",
    );
    put(
        &uq.join("pages/demo/logic.js"),
        ".pragma library\nfunction make(p) { return Qt.createQmlObject(\"\", p); }\n",
    );
    let (ok, out) = uqctl(&["integrations", "check", dir]);
    assert!(!ok, "{out}");
    assert!(out.contains("StatusRow.qml:3: Text"), "{out}");
    assert!(out.contains("logic.js:2: Text"), "{out}");
}

/// `uq_api = 3` pages may stream and preview; a page that says less is refused, and so is a
/// `ctx.command.stream` call in a page that says less.
#[test]
fn api3_features_need_uq_api_3() {
    let d = tempfile::tempdir().unwrap();
    let uq = d.path().join("uq");
    good_folder(&uq);
    let page = |api: u32| {
        put(
            &uq.join("pages/demo/page.toml"),
            &format!(
                "uq_api = {api}\nid = \"demo\"\nname = \"Demo\"\nqml = \"Page.qml\"\n\
                 [[command]]\nid = \"run\"\nargv = [\"/usr/bin/env\", \"run\"]\noutput = \"ndjson\"\n\
                 confirm = \"Run it?\"\npreview = \"plan\"\n\
                 [[command]]\nid = \"plan\"\nargv = [\"/usr/bin/env\", \"plan\"]\noutput = \"json\"\n"
            ),
        );
    };
    put(
        &uq.join("pages/demo/Page.qml"),
        "import QtQuick\nItem {\n    property var ctx\n    Component.onCompleted: ctx.command.stream(\"run\", [], e => {}, r => {})\n}\n",
    );
    page(3);
    let (ok, out) = uqctl(&[
        "integrations",
        "check",
        "--no-programs",
        uq.to_str().unwrap(),
    ]);
    assert!(ok, "{out}");
    page(2);
    let (ok, out) = uqctl(&[
        "integrations",
        "check",
        "--no-programs",
        uq.to_str().unwrap(),
    ]);
    assert!(!ok, "{out}");
    assert!(out.contains("needs uq_api = 3"), "{out}");
}
