use std::time::Duration;

use super::*;
use crate::pages::fake_system::SysRig;
use crate::pages::fake_system::nm::{FakeNm, NmState, WIFI_DEV};
use crate::pages::runner::run;

async fn setup() -> (SysRig, FakeNm, NetworkState, tempfile::TempDir) {
    let mut rig = SysRig::new();
    let nm = FakeNm::start(&mut rig).await;
    let root = tempfile::tempdir().unwrap();
    let page = NetworkState::with(rig.system(), root.path().to_path_buf(), "ana".into());
    (rig, nm, page, root)
}

fn home_settings(uuid: &str, password: &str) -> Settings {
    let mut s = profile::wifi_settings(
        uuid,
        "ana",
        &WifiInput {
            ssid: b"Home".to_vec(),
            security: Some(Security::Password { sae_only: false }),
            password: if password.is_empty() {
                "placeholder"
            } else {
                password
            }
            .into(),
            ..Default::default()
        },
    )
    .unwrap();
    if password.is_empty() {
        s.get_mut("802-11-wireless-security").unwrap().remove("psk");
    }
    s
}

fn ap_of(state: &Value, ssid: &str) -> Value {
    state["wifi"]["networks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["ssid"] == ssid)
        .cloned()
        .unwrap_or_else(|| panic!("{ssid} not listed: {state}"))
}

fn saved_of(state: &Value, uuid: &str) -> Value {
    state["saved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["uuid"] == uuid)
        .cloned()
        .unwrap_or(Value::Null)
}

/// Wait until `f` holds for the fake's state (an agent call runs in the background).
async fn until(nm: &FakeNm, f: impl Fn(&NmState) -> bool) {
    for _ in 0..200 {
        if f(&nm.state.lock().unwrap()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out");
}

/// Wait until the page's state satisfies `f`, and return it.
async fn page_until(page: &NetworkState, f: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..200 {
        let s = page.snapshot().await.unwrap();
        if f(&s) {
            return s;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out: {}", page.snapshot().await.unwrap());
}

#[tokio::test]
async fn reads_wifi_wired_and_saved_connections() {
    let (_rig, nm, page, _root) = setup().await;
    nm.save(home_settings("u-home", "hunter22!"), None).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = out.state;
    assert_eq!(s["available"], true);
    assert_eq!(s["wifi"]["enabled"], true);
    let names: Vec<_> = s["wifi"]["networks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["ssid"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        names,
        ["Home", "eduroam", "Cafe"],
        "saved first; hidden left out"
    );
    let home = ap_of(&s, "Home");
    assert_eq!(home["strength"], 80, "the strongest of its access points");
    assert_eq!(home["band"], "5 GHz");
    assert_eq!(home["security"], "password");
    assert_eq!(home["saved"], "u-home");
    assert_eq!(home["state"], "off");
    assert_eq!(ap_of(&s, "eduroam")["security"], "enterprise");
    assert_eq!(ap_of(&s, "Cafe")["security"], "open");
    assert_eq!(s["wired"][0]["name"], "enp3s0");
    assert_eq!(s["wired"][0]["state"], "Not connected");
    assert_eq!(s["wired"][0]["speed"], "1000 Mb/s");
    let saved = saved_of(&s, "u-home");
    assert_eq!(saved["kind"], "wifi");
    assert_eq!(saved["ssid"], "Home");
    assert_eq!(saved["everyone"], false);
    assert!(
        !s.to_string().contains("hunter22"),
        "secrets never reach the page"
    );
    assert_eq!(s["hotspot"]["can"], true);
    assert_eq!(s["openvpn"], false);
    assert!(s["secret"].is_null());

    nm.state.lock().unwrap().carrier = false;
    let s = run(&page, Op::Refresh).await.state;
    assert_eq!(s["wired"][0]["state"], "Cable unplugged");
}

#[tokio::test]
async fn joining_a_new_network_keeps_it_private() {
    let (_rig, nm, page, _root) = setup().await;
    let s = run(&page, Op::Refresh).await.state;
    let home = ap_of(&s, "Home");
    let out = run(
        &page,
        Op::ConnectWifi {
            ap: home["ap"].as_str().unwrap().into(),
            device: home["device"].as_str().unwrap().into(),
            password: "correct horse".into(),
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
        assert_eq!(
            st.secret(&uuid, "802-11-wireless-security", "psk"),
            "correct horse"
        );
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
        home_settings("u-home", ""),
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
        home_settings("u-other", ""),
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
        .unwrap()
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

#[tokio::test]
async fn saved_connection_settings_round_trip_and_keep_secrets() {
    let (rig, nm, page, root) = setup().await;
    nm.save(home_settings("u-home", "keep-me-1"), None).await;
    let set = |op| run(&page, op);
    assert_eq!(
        set(Op::SetMetered {
            uuid: "u-home".into(),
            metered: "yes".into()
        })
        .await
        .error,
        None
    );
    set(Op::SetAutoconnect {
        uuid: "u-home".into(),
        on: false,
    })
    .await;
    set(Op::SetEveryone {
        uuid: "u-home".into(),
        on: true,
    })
    .await;
    let out = set(Op::SetIp {
        uuid: "u-home".into(),
        family: 4,
        ip: IpInput {
            method: "manual".into(),
            addresses: "192.168.1.20/24".into(),
            gateway: "192.168.1.1".into(),
            dns: "9.9.9.9".into(),
        },
    })
    .await;
    assert_eq!(out.error, None);
    let bad = set(Op::SetIp {
        uuid: "u-home".into(),
        family: 6,
        ip: IpInput {
            method: "manual".into(),
            addresses: "10.0.0.1/8".into(),
            ..Default::default()
        },
    })
    .await;
    assert!(bad.error.unwrap().contains("IPv4 address"));

    // A page opened afresh reads the same values: nothing resets itself.
    let again = NetworkState::with(rig.system(), root.path().to_path_buf(), "ana".into());
    let s = run(&again, Op::Refresh).await.state;
    let home = saved_of(&s, "u-home");
    assert_eq!(home["metered"], "yes");
    assert_eq!(home["autoconnect"], false);
    assert_eq!(home["everyone"], true);
    assert_eq!(home["ipv4"]["method"], "manual");
    assert_eq!(home["ipv4"]["addresses"], "192.168.1.20/24");
    assert_eq!(home["ipv4"]["gateway"], "192.168.1.1");
    assert_eq!(home["ipv4"]["dns"], "9.9.9.9");
    assert_eq!(
        nm.state
            .lock()
            .unwrap()
            .secret("u-home", "802-11-wireless-security", "psk"),
        "keep-me-1",
        "an edit keeps the saved password"
    );

    // Start, stop, forget.
    let s = run(
        &page,
        Op::Activate {
            uuid: "u-home".into(),
        },
    )
    .await
    .state;
    assert_eq!(saved_of(&s, "u-home")["state"], "on");
    let out = run(
        &page,
        Op::SetMetered {
            uuid: "u-home".into(),
            metered: "no".into(),
        },
    )
    .await;
    assert_eq!(out.state["notice"]["text"], SAVED_NOTE);
    let s = run(
        &page,
        Op::Deactivate {
            uuid: "u-home".into(),
        },
    )
    .await
    .state;
    assert_eq!(saved_of(&s, "u-home")["state"], "off");
    let s = run(
        &page,
        Op::Forget {
            uuid: "u-home".into(),
        },
    )
    .await
    .state;
    assert!(saved_of(&s, "u-home").is_null());
    assert!(ap_of(&s, "Home")["saved"].is_null());
    let out = run(
        &page,
        Op::Forget {
            uuid: "u-home".into(),
        },
    )
    .await;
    assert!(out.error.unwrap().contains("gone"));
}

#[tokio::test]
async fn wireguard_import_and_edit() {
    let (_rig, nm, page, root) = setup().await;
    let file = root.path().join("office.conf");
    std::fs::write(
        &file,
        "[Interface]\nPrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\nAddress = 10.2.0.2/32\nDNS = 10.2.0.1\nPostUp = true\n\n[Peer]\nPublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=\nAllowedIPs = 10.2.0.0/16\nEndpoint = vpn.example.com:51820\n",
    )
    .unwrap();
    let out = run(
        &page,
        Op::ImportWireGuard {
            path: file.clone(),
            all_users: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert!(
        out.state["notice"]["text"]
            .as_str()
            .unwrap()
            .contains("left out because only the command-line tool uses them: postup")
    );
    let wg = out.state["saved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["kind"] == "wireguard")
        .cloned()
        .unwrap();
    assert_eq!(wg["name"], "office");
    assert_eq!(wg["wireguard"]["endpoint"], "vpn.example.com:51820");
    assert_eq!(wg["ipv4"]["addresses"], "10.2.0.2/32");
    assert!(
        !out.state.to_string().contains("yAnz5TF"),
        "the key stays hidden"
    );
    let uuid = wg["uuid"].as_str().unwrap().to_string();
    assert_eq!(
        nm.state
            .lock()
            .unwrap()
            .secret(&uuid, "wireguard", "private-key"),
        "yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk="
    );

    let out = run(
        &page,
        Op::EditWireGuard {
            uuid: uuid.clone(),
            peer: wireguard::PeerEdit {
                endpoint: "203.0.113.9:51820".into(),
                allowed_ips: "0.0.0.0/0".into(),
                keepalive: 25,
            },
        },
    )
    .await;
    assert_eq!(out.error, None);
    let wg = saved_of(&out.state, &uuid);
    assert_eq!(wg["wireguard"]["endpoint"], "203.0.113.9:51820");
    assert_eq!(wg["wireguard"]["keepalive"], 25);
    assert_eq!(
        nm.state
            .lock()
            .unwrap()
            .secret(&uuid, "wireguard", "private-key"),
        "yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=",
        "editing keeps the private key"
    );
    let s = run(&page, Op::Activate { uuid: uuid.clone() }).await.state;
    assert_eq!(s["vpn_on"], true);

    std::fs::write(&file, "[Interface]\n").unwrap();
    let out = run(
        &page,
        Op::ImportWireGuard {
            path: file,
            all_users: false,
        },
    )
    .await;
    assert!(out.error.unwrap().contains("PrivateKey"));
    let out = run(
        &page,
        Op::ImportWireGuard {
            path: root.path().join("missing.conf"),
            all_users: false,
        },
    )
    .await;
    assert!(out.error.unwrap().starts_with("Can't read"));

    std::fs::create_dir_all(root.path().join(VPN_PLUGINS)).unwrap();
    std::fs::write(
        root.path()
            .join(VPN_PLUGINS)
            .join("nm-openvpn-service.name"),
        "",
    )
    .unwrap();
    assert_eq!(run(&page, Op::Refresh).await.state["openvpn"], true);
}

#[tokio::test]
async fn hotspot_on_and_off() {
    let (_rig, nm, page, _root) = setup().await;
    let out = run(
        &page,
        Op::Hotspot {
            on: true,
            name: "ana's laptop".into(),
            password: "short".into(),
        },
    )
    .await;
    assert!(out.error.unwrap().contains("8 to 63"));
    let out = run(
        &page,
        Op::Hotspot {
            on: true,
            name: "ana's laptop".into(),
            password: "longenough".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["hotspot"]["state"], "on");
    assert_eq!(out.state["hotspot"]["name"], "ana's laptop");
    let out = run(
        &page,
        Op::Hotspot {
            on: false,
            name: String::new(),
            password: String::new(),
        },
    )
    .await;
    assert_eq!(out.state["hotspot"]["state"], "off");
    // On again with the saved one, then with a new password.
    let out = run(
        &page,
        Op::Hotspot {
            on: true,
            name: String::new(),
            password: String::new(),
        },
    )
    .await;
    assert_eq!(out.state["hotspot"]["state"], "on");
    let uuid = out.state["hotspot"]["uuid"].as_str().unwrap().to_string();
    run(
        &page,
        Op::Hotspot {
            on: true,
            name: String::new(),
            password: "another-one".into(),
        },
    )
    .await;
    assert_eq!(
        nm.state
            .lock()
            .unwrap()
            .secret(&uuid, "802-11-wireless-security", "psk"),
        "another-one"
    );
    assert_eq!(
        nm.state.lock().unwrap().conns.len(),
        1,
        "one hotspot, reused"
    );
}

#[tokio::test]
async fn wifi_switch_scan_and_quiet_refusal() {
    let (_rig, nm, page, _root) = setup().await;
    let out = run(&page, Op::SetWifi(false)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["wifi"]["enabled"], false);
    nm.deny.set(true);
    let out = run(&page, Op::SetWifi(true)).await;
    assert_eq!(out.error, None, "a refused prompt is quiet");
    assert_eq!(out.state["wifi"]["enabled"], false);
    let out = run(
        &page,
        Op::Activate {
            uuid: "nope".into(),
        },
    )
    .await;
    assert!(out.error.is_some());
    nm.deny.set(false);
    run(&page, Op::Scan).await;
    assert_eq!(nm.state.lock().unwrap().scans, 1);
    let out = run(
        &page,
        Op::ConnectHidden {
            ssid: "Secret Lab".into(),
            security: "password".into(),
            password: "labpassword".into(),
            all_users: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    let lab = out.state["saved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Secret Lab")
        .cloned()
        .unwrap();
    assert_eq!(lab["state"], "on");
}

#[tokio::test]
async fn without_network_manager_the_page_names_the_package() {
    let rig = SysRig::new();
    let page = NetworkState::with(rig.system(), PathBuf::from("/nonexistent"), "ana".into());
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["available"], false);
    assert_eq!(out.state["package"], "networkmanager");
    let out = run(&page, Op::SetWifi(true)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Network settings isn't available. Install the networkmanager package.")
    );
}

#[test]
fn words() {
    assert_eq!(state_text(100, true), "Connected");
    assert_eq!(state_text(20, false), "Cable unplugged");
    assert_eq!(state_text(70, true), "Connecting…");
    assert_eq!(band(2412), "2.4 GHz");
    assert_eq!(band(5955), "6 GHz");
    assert!(reason_text(8).contains("password"));
    assert!(reason_text(999).contains("999"));
}
