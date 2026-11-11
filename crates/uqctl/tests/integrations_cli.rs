//! `uqctl integrations check` and `list` on temporary `uq/` folders (Phase 4 spec §2.7).

use std::path::Path;
use std::process::Command;

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A clean `uq/` folder with one of everything. `/usr/bin/env` stands in for the app's program.
fn good_folder(uq: &Path) {
    put(&uq.join("README.md"), "What this folder adds.\n");
    put(
        &uq.join("widgets/demo/widget.toml"),
        "uq_api = 1\nid = \"demo\"\nname = \"Demo\"\nslot = \"quick\"\nqml = \"Widget.qml\"\n\
         [[command]]\nid = \"status\"\nargv = [\"/usr/bin/env\"]\noutput = \"text\"\n",
    );
    put(
        &uq.join("widgets/demo/Widget.qml"),
        "import QtQuick\nItem {\n    property var ctx\n    UqText { text: \"ok\" }\n}\n",
    );
    put(
        &uq.join("pages/demo/page.toml"),
        "uq_api = 1\nid = \"demo\"\nname = \"Demo\"\nqml = \"Page.qml\"\n\
         [[command]]\nid = \"list\"\nargv = [\"/usr/bin/env\"]\noutput = \"text\"\n\
         args = [{ name = \"app\", pattern = \"^[a-z]+$\" }]\nconfirm = \"Remove %s?\"\n",
    );
    put(
        &uq.join("pages/demo/Page.qml"),
        "import QtQuick\nItem {\n    property var ctx\n}\n",
    );
    put(
        &uq.join("launcher/demo.provider"),
        "[Launcher Provider]\nBusName=org.demo.Provider\nName=Demo\nPrefix=:\nActivatable=true\n",
    );
    put(
        &uq.join("launcher/actions/demo-check.appaction"),
        "[App Action]\nName=Check\nMatch=demo-*.desktop\nExec=/usr/bin/env %d\n",
    );
    put(
        &uq.join("dbus/org.demo.Provider.service"),
        "[D-BUS Service]\nName=org.demo.Provider\nExec=/usr/bin/demo-provider\n",
    );
}

fn uqctl(args: &[&str]) -> (bool, String) {
    uqctl_on("unix:path=/nonexistent/uq-test-bus", args)
}

fn uqctl_on(bus: &str, args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_uqctl"))
        .args(args)
        .env("DBUS_SESSION_BUS_ADDRESS", bus)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn a_clean_folder_passes() {
    let d = tempfile::tempdir().unwrap();
    let uq = d.path().join("uq");
    good_folder(&uq);
    let (ok, out) = uqctl(&["integrations", "check", uq.to_str().unwrap()]);
    assert!(ok, "{out}");
    assert_eq!(
        out.trim(),
        "ok: 1 widget(s), 1 page(s), 1 provider(s), 1 app action(s), 1 service file(s)"
    );
}

#[test]
fn problems_are_listed_and_fail() {
    let d = tempfile::tempdir().unwrap();
    let uq = d.path().join("uq");
    good_folder(&uq);
    // A bare Text in a page, a widget whose id isn't its folder, a command that doesn't exist,
    // a bad bus name, an activatable provider without a service file, a stray file, an app
    // action whose program is missing, and an unknown key.
    put(
        &uq.join("pages/demo/Page.qml"),
        "import QtQuick\nItem {\n    Text { text: \"x\" }\n}\n",
    );
    put(
        &uq.join("widgets/other/widget.toml"),
        "uq_api = 1\nid = \"wrong\"\nname = \"W\"\nslot = \"bar\"\nqml = \"W.qml\"\n",
    );
    put(&uq.join("widgets/other/W.qml"), "Item {}\n");
    put(
        &uq.join("widgets/cmd/widget.toml"),
        "uq_api = 1\nid = \"cmd\"\nname = \"C\"\nslot = \"bar\"\nqml = \"W.qml\"\n\
         [[command]]\nid = \"x\"\nargv = [\"/usr/bin/uq-test-no-such-program\"]\noutput = \"none\"\n",
    );
    put(&uq.join("widgets/cmd/W.qml"), "Item {}\n");
    put(
        &uq.join("launcher/bad.provider"),
        "[Launcher Provider]\nBusName=nodots\nName=Bad\n",
    );
    put(
        &uq.join("launcher/cold.provider"),
        "[Launcher Provider]\nBusName=org.cold.Provider\nName=Cold\nActivatable=true\n",
    );
    put(&uq.join("launcher/notes.txt"), "stray");
    put(
        &uq.join("launcher/actions/gone.appaction"),
        "[App Action]\nName=Gone\nExec=/usr/bin/uq-test-no-such-program %d\n",
    );
    put(
        &uq.join("pages/extra/page.toml"),
        "id = \"extra\"\nname = \"E\"\nqml = \"Page.qml\"\nsurprise = 1\n",
    );
    put(&uq.join("pages/extra/Page.qml"), "Item {}\n");
    put(&uq.join("icons/x.svg"), "<svg/>");

    let (ok, out) = uqctl(&["integrations", "check", uq.to_str().unwrap()]);
    assert!(!ok, "{out}");
    for needle in [
        "Page.qml:3: Text, Label",
        "must equal the folder name",
        "uq-test-no-such-program does not exist",
        "invalid BusName",
        "Activatable=true needs dbus/org.cold.Provider.service",
        "notes.txt: only <id>.provider files",
        "Exec program /usr/bin/uq-test-no-such-program does not exist",
        "unknown field",
        "icons: not part of a uq/ folder",
        "9 problem(s)",
    ] {
        assert!(out.contains(needle), "{needle:?} missing from:\n{out}");
    }
}

#[test]
fn a_missing_folder_fails() {
    let (ok, out) = uqctl(&["integrations", "check", "/nonexistent/uq"]);
    assert!(!ok);
    assert!(out.contains("1 problem(s)"), "{out}");
}

#[test]
fn list_reads_a_tree_laid_out_like_usr_share_uq() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    let uq = d.path().join("src");
    good_folder(&uq);
    std::fs::create_dir_all(root.join("shell")).unwrap();
    std::fs::create_dir_all(root.join("settings")).unwrap();
    std::fs::rename(uq.join("widgets"), root.join("shell/widgets")).unwrap();
    std::fs::rename(uq.join("pages"), root.join("settings/pages")).unwrap();
    std::fs::rename(uq.join("launcher"), root.join("launcher")).unwrap();
    put(
        &root.join("settings/pages/waits/page.toml"),
        "id = \"waits\"\nname = \"W\"\nqml = \"Page.qml\"\n[requires]\nbus_name = \"org.uq.TestNobody\"\n",
    );
    put(&root.join("settings/pages/waits/Page.qml"), "Item {}\n");

    let (ok, out) = uqctl(&["integrations", "list", "--root", root.to_str().unwrap()]);
    assert!(ok, "{out}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 5, "{out}");
    assert!(lines[0].starts_with("widget\tdemo\tshown\t"), "{out}");
    assert!(lines[1].starts_with("page\tdemo\tshown\t"), "{out}");
    assert!(
        lines[2].starts_with("page\twaits\tshown\t") && lines[2].contains("not checked"),
        "no session bus here, so the name is not checked: {out}"
    );
    assert!(
        lines[3].starts_with("provider\torg.demo.Provider\tshown\t"),
        "{out}"
    );
    assert!(
        lines[4].starts_with("app_action\tdemo-check\tshown\t"),
        "{out}"
    );

    let (ok, json) = uqctl(&[
        "integrations",
        "list",
        "--json",
        "--root",
        root.to_str().unwrap(),
    ]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 5);
    assert_eq!(v[2]["waits_for"], "org.uq.TestNobody");
    assert_eq!(v[3]["kind"], "provider");

    // On a bus where nobody owns the name, the page is hidden.
    let bus = uq_core::test_bus::private_bus();
    let (ok, out) = uqctl_on(
        &bus.address,
        &["integrations", "list", "--root", root.to_str().unwrap()],
    );
    assert!(ok);
    assert!(
        out.contains("page\twaits\thidden\t") && out.contains("hidden until org.uq.TestNobody"),
        "{out}"
    );

    let empty = tempfile::tempdir().unwrap();
    let (ok, out) = uqctl(&[
        "integrations",
        "list",
        "--root",
        empty.path().to_str().unwrap(),
    ]);
    assert!(ok);
    assert_eq!(out.trim(), "no integrations installed");
}
