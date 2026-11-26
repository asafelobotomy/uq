use super::*;

#[test]
fn lint_matches_the_shell_script() {
    let qml = "import QtQuick\n\
               Item {\n\
               \x20   Text { text: \"a\" }\n\
               \x20   UqText { text: \"ok\" }\n\
               \x20   QQC2.Label {\n\
               \x20   }\n\
               \x20   property var x: Qt.createQmlObject(\"\", this)\n\
               \x20   TextInput\n\
               \n\
               \x20   {\n\
               \x20   }\n\
               \x20   Label\n\
               \x20   Item {}\n\
               \x20   // Text is fine in prose without a brace\n\
               \x20   Item { Text {} }\n\
               \x20   ctx.UqText {}\n\
               \x20   Controls.TextEdit{}\n\
               }\n";
    let lines: Vec<usize> = qml_lint_hits(qml).into_iter().map(|(n, _)| n).collect();
    assert_eq!(lines, [3, 5, 7, 8, 15, 17]);
}

#[test]
fn api2_components_are_found() {
    let qml = "import org.uq.settings\n\
               SettingsPage {\n\
               \x20   SettingsTab { title: \"Apps\" }\n\
               \x20   // SettingsCard in a comment without a brace\n\
               \x20   MySettingsCard {}\n\
               \x20   Item { UqAppIcon{} }\n\
               }\n";
    let hits: Vec<(usize, String)> = api2_hits(qml);
    assert_eq!(
        hits,
        [
            (2, "SettingsPage".to_string()),
            (3, "SettingsTab".to_string()),
            (6, "UqAppIcon".to_string()),
        ]
    );
}

#[test]
fn api3_calls_are_found() {
    let text = "ctx.command.run(\"a\", [], cb);\n\
                // ctx.command.stream(\"b\") is a comment\n\
                host.ctx.command.stream(\"c\", [], ev, done);\n\
                const s = ctx.command\n  .stream(x);\n";
    assert_eq!(
        api3_hits(text),
        [(3, "ctx.command.stream".to_string())],
        "a call split over two lines is not found: keep it on one"
    );
}

#[test]
fn list_lines_show_state_and_reason() {
    use uq_core::integration::scan::Kind;
    let e = Entry {
        kind: Kind::AppAction,
        id: "x".into(),
        name: Some("X".into()),
        source: "/s/x.appaction".into(),
        valid: true,
        visible: false,
        error: Some("hidden: /usr/bin/x\nis missing".into()),
        waits_for: None,
        bus: Bus::Session,
    };
    assert_eq!(
        list_lines(std::slice::from_ref(&e)),
        ["app_action\tx\thidden\t/s/x.appaction\thidden: /usr/bin/x is missing"]
    );
    let j = list_json(&[e]);
    assert_eq!(j[0]["kind"], "app_action");
    assert_eq!(j[0]["visible"], false);
}
