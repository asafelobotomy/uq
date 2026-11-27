//! Joining networks, saved connections and WireGuard.

use super::*;

#[tokio::test]
async fn joining_a_new_network_keeps_it_private() {
    let (_rig, nm, page, _root) = setup().await;
    let s = run(&page, Op::Refresh).await.state;
    let home = ap_of(&s, "Home");
    let psk = pw();
    let out = run(
        &page,
        Op::ConnectWifi {
            ap: home["ap"].as_str().unwrap().into(),
            device: home["device"].as_str().unwrap().into(),
            password: psk.clone(),
            all_users: false,
            eap: None,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let home = ap_of(&out.state, "Home");
    assert_eq!(home["state"], "on");
    let uuid = home["saved"].as_str().unwrap().to_string();
    {
        let st = nm.state.lock().unwrap();
        assert_eq!(st.secret(&uuid, "802-11-wireless-security", "psk"), psk);
        let c = st.conn(&uuid).unwrap();
        let perms = nm::get::<Vec<String>>(&c.settings["connection"], "permissions").unwrap();
        assert_eq!(perms, ["user:ana:"]);
    }
    assert_eq!(out.state["wifi"]["devices"][0]["state"], "Connected");

    // Too short a password never reaches NetworkManager.
    let cafe = ap_of(&out.state, "eduroam");
    let out = run(
        &page,
        Op::ConnectWifi {
            ap: cafe["ap"].as_str().unwrap().into(),
            device: cafe["device"].as_str().unwrap().into(),
            password: String::new(),
            all_users: false,
            eap: None,
        },
    )
    .await;
    assert!(out.error.unwrap().contains("user name and password"));
}

#[tokio::test]
async fn work_network_with_peap() {
    let (_rig, nm, page, _root) = setup().await;
    let s = run(&page, Op::Refresh).await.state;
    let edu = ap_of(&s, "eduroam");
    let out = run(
        &page,
        Op::ConnectWifi {
            ap: edu["ap"].as_str().unwrap().into(),
            device: edu["device"].as_str().unwrap().into(),
            password: String::new(),
            all_users: true,
            eap: Some(Eap {
                method: "peap".into(),
                identity: "ana@uni.example".into(),
                password: "s3cret".into(),
                ..Default::default()
            }),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let uuid = ap_of(&out.state, "eduroam")["saved"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(saved_of(&out.state, &uuid)["enterprise"], "peap");
    assert_eq!(saved_of(&out.state, &uuid)["everyone"], true);
    assert_eq!(
        nm.state.lock().unwrap().secret(&uuid, "802-1x", "password"),
        "s3cret"
    );
}

#[tokio::test]
async fn the_agent_asks_only_for_connections_started_here() {
    let (_rig, nm, page, _root) = setup().await;
    let page = Arc::new(page);
    nm.save(
        home_settings("u-home", Default::default()),
        Some("802-11-wireless-security"),
    )
    .await;
    let changed = Arc::new(Notify::new());
    let watcher = {
        let page = page.clone();
        let changed = changed.clone();
        tokio::spawn(async move { page.watch(changed).await })
    };
    until(&nm, |s| s.agent.is_some()).await;
    page_until(&page, |s| s["agent"] == true).await;

    // Started from the page: NetworkManager asks, the page shows the question.
    let out = run(
        &*page,
        Op::Activate {
            uuid: "u-home".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let s = page_until(&page, |s| !s["secret"].is_null()).await;
    let q = &s["secret"];
    assert_eq!(q["name"], "Home");
    assert_eq!(q["what"], "Wi-Fi network");
    assert_eq!(q["fields"][0]["key"], "psk");
    let id = q["id"].as_u64().unwrap();
    run(
        &*page,
        Op::AnswerSecret {
            id,
            values: HashMap::from([
                ("psk".into(), "typed-in-pw".into()),
                ("unasked".into(), "dropped".into()),
            ]),
        },
    )
    .await;
    until(&nm, |s| !s.agent_replies.is_empty()).await;
    {
        let st = nm.state.lock().unwrap();
        let reply = st.agent_replies[0].as_ref().unwrap();
        assert_eq!(
            nm::text(&reply["802-11-wireless-security"], "psk"),
            "typed-in-pw"
        );
        assert!(!reply["802-11-wireless-security"].contains_key("unasked"));
        assert_eq!(
            st.secret("u-home", "802-11-wireless-security", "psk"),
            "typed-in-pw"
        );
    }
    let s = page_until(&page, |s| s["secret"].is_null()).await;
    assert_eq!(ap_of(&s, "Home")["state"], "on");

    // Not started from the page: no question, NetworkManager hears "no secrets".
    nm.save(
        home_settings("u-other", Default::default()),
        Some("802-11-wireless-security"),
    )
    .await;
    let path = nm
        .state
        .lock()
        .unwrap()
        .conn("u-other")
        .unwrap()
        .path
        .clone();
    let other = zbus::connection::Builder::address(_rig.bus.address.as_str())
        .build()
        .await
        .unwrap();
    let mgr = NetworkManagerProxy::new(&other).await.unwrap();
    mgr.activate_connection(
        &OwnedObjectPath::try_from(path).unwrap(),
        &OwnedObjectPath::try_from(WIFI_DEV).unwrap(),
        &nm::root(),
    )
    .await
    .unwrap();
    until(&nm, |s| s.agent_replies.len() == 2).await;
    assert_eq!(
        nm.state.lock().unwrap().agent_replies[1]
            .as_ref()
            .unwrap_err(),
        "org.freedesktop.NetworkManager.SecretAgent.NoSecrets"
    );
    assert!(page.snapshot().await.unwrap()["secret"].is_null());

    // Cancelled: NetworkManager hears "cancelled" and the page says why it didn't connect.
    nm.state.lock().unwrap().conns[0].needs_secret = Some("802-11-wireless-security".into());
    run(
        &*page,
        Op::Activate {
            uuid: "u-home".into(),
        },
    )
    .await;
    let s = page_until(&page, |s| !s["secret"].is_null()).await;
    let id = s["secret"]["id"].as_u64().unwrap();
    run(&*page, Op::CancelSecret { id }).await;
    until(&nm, |s| s.agent_replies.len() == 3).await;
    assert_eq!(
        nm.state.lock().unwrap().agent_replies[2]
            .as_ref()
            .unwrap_err(),
        "org.freedesktop.NetworkManager.SecretAgent.UserCanceled"
    );
    let s = page_until(&page, |s| s["notice"]["text"] != "").await;
    assert_eq!(
        s["notice"]["text"],
        "Couldn't connect: a password is needed and none was given."
    );

    // Closing the page takes the agent off the bus.
    let agent = nm.state.lock().unwrap().agent.clone().unwrap();
    watcher.abort();
    let _ = watcher.await;
    drop(page);
    let dbus = zbus::fdo::DBusProxy::new(&other).await.unwrap();
    for _ in 0..100 {
        let name = zbus::names::BusName::try_from(agent.as_str()).unwrap();
        if !dbus.name_has_owner(name).await.unwrap() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the agent stayed on the bus after the page closed");
}
