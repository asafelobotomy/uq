//! Reading devices, the hotspot and Wi-Fi switch.

use super::*;

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
