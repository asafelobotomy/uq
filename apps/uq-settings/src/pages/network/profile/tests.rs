use super::*;

#[test]
fn security_from_flags() {
    assert_eq!(security(0, 0, 0), Security::Open);
    assert_eq!(security(1, 0, 0), Security::Wep);
    assert_eq!(
        security(1, 0, KEY_MGMT_PSK),
        Security::Password { sae_only: false }
    );
    assert_eq!(
        security(1, 0, KEY_MGMT_PSK | KEY_MGMT_SAE),
        Security::Password { sae_only: false },
        "WPA2/WPA3 mixed joins with a WPA2 password"
    );
    assert_eq!(
        security(1, 0, KEY_MGMT_SAE),
        Security::Password { sae_only: true }
    );
    assert_eq!(
        security(1, KEY_MGMT_8021X, KEY_MGMT_8021X),
        Security::Enterprise
    );
    assert_eq!(security(0, 0, KEY_MGMT_OWE), Security::Owe);
}

#[test]
fn uuids_are_version_4_and_differ() {
    let a = new_uuid();
    let b = new_uuid();
    assert_ne!(a, b);
    assert_eq!(a.len(), 36);
    assert_eq!(&a[14..15], "4");
    assert!(matches!(&a[19..20], "8" | "9" | "a" | "b"));
}

#[test]
fn a_new_wifi_network_is_private_by_default() {
    let w = WifiInput {
        ssid: b"Cafe".to_vec(),
        security: Some(Security::Password { sae_only: false }),
        password: "correct horse".into(),
        ..Default::default()
    };
    let s = wifi_settings("u-1", "ana", &w).unwrap();
    assert_eq!(kind(&s), "wifi");
    assert_eq!(
        get::<Vec<String>>(&s["connection"], "permissions").unwrap(),
        ["user:ana:"]
    );
    assert_eq!(text(&s["802-11-wireless-security"], "key-mgmt"), "wpa-psk");
    assert_eq!(text(&s["802-11-wireless-security"], "psk"), "correct horse");
    assert_eq!(ssid(&s), b"Cafe");

    let everyone = wifi_settings(
        "u-2",
        "ana",
        &WifiInput {
            all_users: true,
            ..w.clone()
        },
    )
    .unwrap();
    assert!(
        get::<Vec<String>>(&everyone["connection"], "permissions")
            .unwrap()
            .is_empty()
    );

    let short = WifiInput {
        password: "short".into(),
        ..w
    };
    assert!(
        wifi_settings("u", "ana", &short)
            .unwrap_err()
            .contains("8 to 63")
    );
}

#[test]
fn enterprise_networks_need_their_details() {
    let mut w = WifiInput {
        ssid: b"eduroam".to_vec(),
        security: Some(Security::Enterprise),
        ..Default::default()
    };
    assert!(wifi_settings("u", "ana", &w).is_err());
    w.eap = Some(Eap {
        method: "peap".into(),
        identity: "ana@uni.example".into(),
        password: "pw".into(),
        domain: "radius.uni.example".into(),
        ..Default::default()
    });
    let s = wifi_settings("u", "ana", &w).unwrap();
    let x = &s["802-1x"];
    assert_eq!(get::<Vec<String>>(x, "eap").unwrap(), ["peap"]);
    assert_eq!(text(x, "phase2-auth"), "mschapv2");
    assert_eq!(text(x, "password"), "pw");
    assert_eq!(get::<bool>(x, "system-ca-certs"), Some(true));
    assert_eq!(text(x, "domain-suffix-match"), "radius.uni.example");
    assert_eq!(text(&s["802-11-wireless-security"], "key-mgmt"), "wpa-eap");

    let dir = tempfile::tempdir().unwrap();
    let cert = dir.path().join("me.pem");
    std::fs::write(&cert, "x").unwrap();
    let cert = cert.display().to_string();
    w.eap = Some(Eap {
        method: "tls".into(),
        identity: "ana".into(),
        ca_cert: cert.clone(),
        client_cert: cert.clone(),
        private_key: cert.clone(),
        key_password: "kp".into(),
        ..Default::default()
    });
    let s = wifi_settings("u", "ana", &w).unwrap();
    let blob = get::<Vec<u8>>(&s["802-1x"], "client-cert").unwrap();
    assert_eq!(blob, format!("file://{cert}\0").into_bytes());
    assert_eq!(text(&s["802-1x"], "private-key-password"), "kp");
    w.eap.as_mut().unwrap().private_key = "relative.pem".into();
    assert!(
        wifi_settings("u", "ana", &w)
            .unwrap_err()
            .contains("full file path")
    );
}

#[test]
fn hotspot_checks_its_password() {
    let s = hotspot_settings("u", "ana", "uQ laptop", "longenough").unwrap();
    assert_eq!(kind(&s), "hotspot");
    assert_eq!(text(&s["ipv4"], "method"), "shared");
    assert!(hotspot_settings("u", "ana", "x", "short").is_err());
    assert!(hotspot_settings("u", "ana", " ", "longenough").is_err());
}

#[test]
fn ip_settings_round_trip_through_the_summary() {
    let mut s = wifi_settings(
        "u",
        "ana",
        &WifiInput {
            ssid: b"Home".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    s.get_mut("ipv4")
        .unwrap()
        .insert("addresses".into(), ov(vec![vec![1u32, 24, 0]]));
    apply_ip(
        &mut s,
        4,
        &IpInput {
            method: "manual".into(),
            addresses: "192.168.1.20/24, 10.0.0.2".into(),
            gateway: "192.168.1.1".into(),
            dns: "1.1.1.1 9.9.9.9".into(),
        },
    )
    .unwrap();
    assert!(
        !s["ipv4"].contains_key("addresses"),
        "the legacy key would win"
    );
    apply_ip(
        &mut s,
        6,
        &IpInput {
            method: "auto".into(),
            dns: "2606:4700:4700::1111".into(),
            ..Default::default()
        },
    )
    .unwrap();
    set_metered(&mut s, "yes").unwrap();
    let j = summary("/x", &s);
    assert_eq!(j["ipv4"]["method"], "manual");
    assert_eq!(j["ipv4"]["addresses"], "192.168.1.20/24, 10.0.0.2/24");
    assert_eq!(j["ipv4"]["gateway"], "192.168.1.1");
    assert_eq!(j["ipv4"]["dns"], "1.1.1.1, 9.9.9.9");
    assert_eq!(j["ipv6"]["method"], "auto");
    assert_eq!(j["ipv6"]["dns"], "2606:4700:4700::1111");
    assert_eq!(j["metered"], "yes");
    assert_eq!(j["everyone"], false);
    assert_eq!(get::<bool>(&s["ipv4"], "ignore-auto-dns"), Some(true));

    let bad = |m: &str, a: &str, d: &str| {
        apply_ip(
            &mut s.clone(),
            4,
            &IpInput {
                method: m.into(),
                addresses: a.into(),
                dns: d.into(),
                ..Default::default()
            },
        )
        .unwrap_err()
    };
    assert!(bad("manual", "", "").contains("at least one"));
    assert!(bad("manual", "300.1.1.1/24", "").contains("isn't an address"));
    assert!(bad("manual", "fe80::1/64", "").contains("IPv6 address"));
    assert!(bad("manual", "10.0.0.1/40", "").contains("network size"));
    assert!(bad("auto", "", "fe80::1").contains("DNS server"));
    assert!(bad("dhcp", "", "").contains("Choose"));
    assert!(set_metered(&mut s, "sometimes").is_err());
}
