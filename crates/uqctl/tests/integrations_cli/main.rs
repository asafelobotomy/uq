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
    put(
        &uq.join("integrations/demo/rules.toml"),
        "[[window_rule]]\nid = \"demo.games\"\nclass = [\"^(demo-game)$\"]\n\
         workspace = \"name:game\"\nfullscreen = true\n",
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

mod check;
mod list_run;
