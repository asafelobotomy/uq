use super::*;

pub const SAMPLE: &str = "\
# from the VPN provider
[Interface]
PrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=
Address = 10.2.0.2/32, fd00:2::2/128
DNS = 10.2.0.1, vpn.example
PostUp = iptables -A FORWARD -i %i -j ACCEPT

[Peer]
PublicKey = xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=
PresharedKey = /UwcSPg38hW/D9Y3tcS1FOV0K1wuURMbS0sesJEP5ak=
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = vpn.example.com:51820
PersistentKeepalive = 25
";

#[test]
fn parses_a_provider_file() {
    let c = parse(SAMPLE).unwrap();
    assert_eq!(c.addresses, ["10.2.0.2/32", "fd00:2::2/128"]);
    assert_eq!(c.dns, ["10.2.0.1", "vpn.example"]);
    assert_eq!(c.left_out, ["postup"]);
    let p = &c.peers[0];
    assert_eq!(p.allowed_ips, ["0.0.0.0/0", "::/0"]);
    assert_eq!(p.endpoint, "vpn.example.com:51820");
    assert_eq!(p.keepalive, 25);
    assert!(!p.preshared_key.is_empty());
}

#[test]
fn bad_files_name_the_line() {
    assert!(
        parse("[Peer]\nPublicKey = x\n")
            .unwrap_err()
            .contains("Line 2")
    );
    assert!(parse("PrivateKey = a\n").unwrap_err().contains("follow"));
    assert!(
        parse("[Interface]\nAddress = 10.0.0.1/24\n")
            .unwrap_err()
            .contains("PrivateKey")
    );
    let no_peer = "[Interface]\nPrivateKey = yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=\n";
    assert!(parse(no_peer).unwrap_err().contains("[Peer]"));
    assert!(
        parse(&format!("{no_peer}Colour = red\n"))
            .unwrap_err()
            .contains("Line 3")
    );
    assert!(
        parse(&format!("{no_peer}just words\n"))
            .unwrap_err()
            .contains("name = value")
    );
}

#[test]
fn settings_hold_the_keys_and_addresses() {
    let c = parse(SAMPLE).unwrap();
    let s = settings(&c, "work vpn!", "u-wg", "ana", false).unwrap();
    assert_eq!(text(&s["connection"], "type"), "wireguard");
    assert_eq!(text(&s["connection"], "interface-name"), "workvpn");
    assert_eq!(
        get::<Vec<String>>(&s["connection"], "permissions").unwrap(),
        ["user:ana:"]
    );
    assert_eq!(text(&s["wireguard"], "private-key"), c.private_key);
    let j = super::super::profile::summary("/c", &s);
    assert_eq!(j["kind"], "wireguard");
    assert_eq!(j["ipv4"]["addresses"], "10.2.0.2/32");
    assert_eq!(j["ipv4"]["dns"], "10.2.0.1");
    assert_eq!(j["ipv6"]["addresses"], "fd00:2::2/128");
    assert_eq!(j["wireguard"]["endpoint"], "vpn.example.com:51820");
    assert_eq!(j["wireguard"]["allowed_ips"], "0.0.0.0/0, ::/0");
    assert_eq!(j["wireguard"]["keepalive"], 25);
    assert_eq!(
        get::<Vec<String>>(&s["ipv4"], "dns-search").unwrap(),
        ["vpn.example"]
    );
}

#[test]
fn edit_changes_the_first_peer() {
    let c = parse(SAMPLE).unwrap();
    let mut s = settings(&c, "wg", "u", "ana", true).unwrap();
    edit_peer(
        &mut s,
        &PeerEdit {
            endpoint: "203.0.113.5:51820".into(),
            allowed_ips: "10.2.0.0/16".into(),
            keepalive: 0,
        },
    )
    .unwrap();
    let j = super::super::profile::summary("/c", &s);
    assert_eq!(j["wireguard"]["endpoint"], "203.0.113.5:51820");
    assert_eq!(j["wireguard"]["allowed_ips"], "10.2.0.0/16");
    assert_eq!(j["wireguard"]["keepalive"], 0);
    assert!(
        edit_peer(
            &mut s,
            &PeerEdit {
                allowed_ips: "nope".into(),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(edit_peer(&mut s, &PeerEdit::default()).is_err());
}

#[test]
fn interface_names_are_safe() {
    assert_eq!(
        interface_name("a very long name for a vpn"),
        "averylongnamefo"
    );
    assert_eq!(interface_name("../"), "wg0");
    assert_eq!(interface_name("ü"), "wg0");
}
