//! Saved connections and WireGuard.

use super::*;

#[tokio::test]
async fn reads_wifi_wired_and_saved_connections() {
    let (_rig, nm, page, _root) = setup().await;
    let psk = pw();
    nm.save(home_settings("u-home", &psk), None).await;
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
        !s.to_string().contains(psk.as_str()),
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
async fn saved_connection_settings_round_trip_and_keep_secrets() {
    let (rig, nm, page, root) = setup().await;
    let psk = pw();
    nm.save(home_settings("u-home", &psk), None).await;
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
        psk,
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
