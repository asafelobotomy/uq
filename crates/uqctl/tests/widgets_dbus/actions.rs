//! App actions.

use super::*;

#[tokio::test]
async fn app_actions_are_listed_and_launched_by_the_bridge() {
    let bus = private_bus();
    let root = tempfile::tempdir().unwrap();
    let r = root.path();
    let actions = r.join("launcher/actions");
    put(
        &actions.join("check.appaction"),
        "[App Action]\nName=Check for update\nIcon=view-refresh\nMatch=org.test.*\n\
         Exec=/usr/bin/true check --id %d\n",
    );
    put(
        &actions.join("gone.appaction"),
        "[App Action]\nName=Gone\nExec=/usr/bin/uq-no-such-program %d\n",
    );
    let mut hypr = FakeHypr::start();
    let (_data, data) = data_dirs();
    let mut env = hypr.env();
    env.extend(data);
    let env_refs: Vec<(&str, &std::ffi::OsStr)> =
        env.iter().map(|(k, v)| (*k, v.as_os_str())).collect();
    let mut b = Bridge::start_with_root(&bus, &env_refs, r);
    b.event("Hello").await;
    assert_eq!(
        first(&mut b, "AppActions").await,
        json!([{"id": "check", "name": "Check for update", "icon": "view-refresh",
                "match": "org.test.*", "terminal": false}])
    );

    let ok = b
        .call(
            1,
            "Launch",
            json!([{"app_action": "check", "desktop_id": "org.test.App"}]),
        )
        .await;
    assert_eq!(ok["ok"], Value::Null, "{ok}");
    let sent = hypr.next(Duration::from_secs(5)).await.expect("a dispatch");
    assert!(sent.contains("systemd-run --user --scope"), "{sent}");
    assert!(
        sent.contains("-- '/usr/bin/true' 'check' '--id' 'org.test.App.desktop'"),
        "{sent}"
    );

    for (bad, why) in [
        (
            json!({"app_action": "check", "desktop_id": "-rf"}),
            "invalid desktop id",
        ),
        (
            json!({"app_action": "check", "desktop_id": "a;b"}),
            "invalid desktop id",
        ),
        (
            json!({"app_action": "check", "desktop_id": "term"}),
            "doesn't apply",
        ),
        (
            json!({"app_action": "check", "desktop_id": "org.test.Missing"}),
            "no app called",
        ),
        (
            json!({"app_action": "gone", "desktop_id": "org.test.App"}),
            "no app action",
        ),
        (
            json!({"app_action": "check", "desktop_id": "org.test.App", "argv": ["sh"]}),
            "only",
        ),
    ] {
        let r = b.call(2, "Launch", json!([bad])).await;
        assert!(r["err"].as_str().unwrap().contains(why), "{bad}: {r}");
    }
    assert!(hypr.next(Duration::from_millis(300)).await.is_none());

    // A terminal action, installed while running, uses the chosen terminal (none set: kitty).
    put(
        &actions.join("rm.appaction"),
        "[App Action]\nName=Uninstall…\nExec=/usr/bin/true rm %d\nTerminal=true\n",
    );
    wait_for(&mut b, "AppActions", Duration::from_secs(6), |l| {
        l.as_array().is_some_and(|a| a.len() == 2)
    })
    .await;
    let ok = b
        .call(
            3,
            "Launch",
            json!([{"app_action": "rm", "desktop_id": "org.test.App.desktop"}]),
        )
        .await;
    assert_eq!(ok["ok"], Value::Null, "{ok}");
    let sent = hypr.next(Duration::from_secs(5)).await.expect("a dispatch");
    assert!(
        sent.contains("-- 'kitty' '-e' '/usr/bin/true' 'rm' 'org.test.App.desktop'"),
        "{sent}"
    );
}
