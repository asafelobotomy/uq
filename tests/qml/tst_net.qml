import QtQuick
import QtTest
import "../../shell/logic/net.js" as Net

TestCase {
    name: "net"

    function test_nmcli_parse() {
        var r = Net.parseNmcliActive("Home Wi-Fi:802-11-wireless\nWork\\: VPN:vpn\nlo:loopback\n");
        compare(r.length, 3);
        compare(r[0].name, "Home Wi-Fi");
        compare(r[0].type, "802-11-wireless");
        compare(r[1].name, "Work: VPN");
        compare(r[1].type, "vpn");
    }
    function test_nmcli_backslash_and_junk() {
        var r = Net.parseNmcliActive("a\\\\b:wireguard\n\nnocolon\n");
        compare(r.length, 1);
        compare(r[0].name, "a\\b");
        compare(r[0].type, "wireguard");
        compare(Net.parseNmcliActive("").length, 0);
        compare(Net.parseNmcliActive(null).length, 0);
    }
    function test_vpn_pick_ignores_wifi() {
        compare(Net.vpnName([{ name: "Home", type: "802-11-wireless" }, { name: "eth", type: "802-3-ethernet" }]), null);
    }
    function test_wireguard_counts() {
        compare(Net.vpnName([{ name: "Home", type: "802-11-wireless" }, { name: "wg0", type: "wireguard" }]), "wg0");
        compare(Net.vpnName([{ name: "corp", type: "vpn" }]), "corp");
        compare(Net.vpnName([]), null);
        compare(Net.vpnName(null), null);
    }
    function test_tailscale() {
        compare(Net.parseTailscale('{"Self":{},"Peer":{}}').exitNode, null);
        compare(Net.parseTailscale('{"ExitNodeStatus":null}').exitNode, null);
        var j = JSON.stringify({
            ExitNodeStatus: { ID: "nABC", Online: true, TailscaleIPs: ["100.1.1.1/32"] },
            Peer: { k1: { ID: "nXYZ", HostName: "other" }, k2: { ID: "nABC", HostName: "exit-box" } }
        });
        compare(Net.parseTailscale(j).exitNode, "exit-box");
        // object input and garbage
        compare(Net.parseTailscale({ ExitNodeStatus: { ID: "q" }, Peer: { a: { ID: "q", HostName: "h" } } }).exitNode, "h");
        compare(Net.parseTailscale("not json").exitNode, null);
        compare(Net.parseTailscale("").exitNode, null);
    }
    function test_tailscale_exit_by_ip_and_option() {
        var j = { ExitNodeStatus: { ID: "none", TailscaleIPs: ["100.2.2.2/32"] },
            Peer: { a: { ID: "x", HostName: "byip", TailscaleIPs: ["100.2.2.2"] } } };
        compare(Net.parseTailscale(j).exitNode, "byip");
        var k = { Peer: { a: { ID: "x", HostName: "opt", ExitNode: true } } };
        compare(Net.parseTailscale(k).exitNode, "opt");
    }
    function test_vpn_active_truth_table() {
        compare(Net.vpnActive(null, null), false);
        compare(Net.vpnActive("wg0", null), true);
        compare(Net.vpnActive(null, "exit"), true);
        compare(Net.vpnActive("wg0", "exit"), true);
        compare(Net.vpnActive("", ""), false);
    }
    function test_hostile_ssid_unchanged() {
        var evil = "<b>x</b>\"; rm -rf ~ #";
        var r = Net.parseNmcliActive(evil + ":vpn");
        compare(r[0].name, evil);
        compare(Net.vpnName(r), evil);
    }
    function test_retry_due() {
        verify(Net.retryDue(0, 1000, 300000));          // never failed
        verify(!Net.retryDue(1000, 2000, 300000));      // failed 1 s ago
        verify(Net.retryDue(1000, 301001, 300000));
    }

    function test_step_volume() {
        fuzzyCompare(Net.stepVolume(0.5, 1), 0.55, 1e-9);
        fuzzyCompare(Net.stepVolume(0.5, -1), 0.45, 1e-9);
        compare(Net.stepVolume(1.48, 1), 1.5);
        compare(Net.stepVolume(0.02, -1), 0);
        compare(Net.stepVolume(NaN, 1), 0.05);
    }
    function test_volume_icon() {
        compare(Net.volumeIcon(0.8, true), "audio-volume-muted-symbolic");
        compare(Net.volumeIcon(0, false), "audio-volume-muted-symbolic");
        compare(Net.volumeIcon(0.2, false), "audio-volume-low-symbolic");
        compare(Net.volumeIcon(0.5, false), "audio-volume-medium-symbolic");
        compare(Net.volumeIcon(1.2, false), "audio-volume-high-symbolic");
    }
    function test_wifi_icon() {
        compare(Net.wifiIcon(0.9), "network-wireless-signal-excellent-symbolic");
        compare(Net.wifiIcon(0.6), "network-wireless-signal-good-symbolic");
        compare(Net.wifiIcon(0.4), "network-wireless-signal-ok-symbolic");
        compare(Net.wifiIcon(0.2), "network-wireless-signal-weak-symbolic");
        compare(Net.wifiIcon(0), "network-wireless-signal-none-symbolic");
        compare(Net.wifiIcon(undefined), "network-wireless-signal-none-symbolic");
    }
    function test_battery_pct() {
        compare(Net.batteryPct(0.734), 73);
        compare(Net.batteryPct(1), 100);
        compare(Net.batteryPct(-1), 0);
        compare(Net.batteryPct("x"), 0);
    }

    function test_vpn_identity() {
        compare(Net.vpnIdentity(null, null), Net.vpnIdentity("", ""));
        verify(Net.identityChanged(Net.vpnIdentity(null, "A"), Net.vpnIdentity(null, "B")));
        verify(Net.identityChanged(Net.vpnIdentity("wg0", null), Net.vpnIdentity("wg1", null)));
        verify(Net.identityChanged(Net.vpnIdentity(null, null), Net.vpnIdentity(null, "A")));
        verify(Net.identityChanged(Net.vpnIdentity(null, "A"), Net.vpnIdentity("A", null)));
        verify(!Net.identityChanged(Net.vpnIdentity("wg0", "A"), Net.vpnIdentity("wg0", "A")));
        verify(Net.identityChanged(null, Net.vpnIdentity(null, null)));   // nothing sent yet
    }
    function test_safe_icon_source() {
        compare(Net.safeIconSource("image://icon/x", "fb"), "image://icon/x");
        compare(Net.safeIconSource("/usr/share/x.png", "fb"), "/usr/share/x.png");
        var bad = ["http://e.com/a.png", "https://e.com/a.png", "file://x/a.png", "file:///etc/x",
            "data:image/png;base64,AA", "", "rel/x.png", "x.png", "//host/x", null, undefined, 5];
        for (var i = 0; i < bad.length; i++)
            compare(Net.safeIconSource(bad[i], "fb"), "fb", String(bad[i]));
    }
    function test_finish_run() {
        compare(Net.finishRun(0, "x").ok, true);
        compare(Net.finishRun(0, "x").text, "x");
        compare(Net.finishRun(1, "partial").ok, false);
        compare(Net.finishRun(1, "partial").text, "");
        compare(Net.finishRun(0, null).text, "");
    }
    // A non-zero exit (service not ready, restarting) is a normal result, polled again in 10 s;
    // only a failed start (binary missing) is cached.
    function test_failure_cache_only_for_failed_start() {
        compare(Net.finishRun(1, "").ok, false);
        compare(Net.failedStart(false), true);
        compare(Net.failedStart(true), false);
    }

    function test_tailscale_details() {
        var j = { Self: { TailscaleIPs: ["100.64.0.2", "fd7a::2"] }, ExitNodeStatus: { ID: "n1" },
            Peer: { k: { ID: "n1", HostName: "nl-ams-wg-001", TailscaleIPs: ["100.80.0.9"],
                Location: { City: "Amsterdam", Country: "Netherlands" } } } };
        var t = Net.parseTailscale(j);
        compare(t.exitNode, "nl-ams-wg-001");
        compare(t.exitIp, "100.80.0.9");
        compare(t.city, "Amsterdam");
        compare(t.country, "Netherlands");
        compare(t.selfIp, "100.64.0.2");
        compare(Net.parseTailscale("x").city, "");
    }

    function test_nmcli_connection() {
        var ovpn = "connection.id:Work\\: home\nconnection.type:vpn\n"
            + "vpn.service-type:org.freedesktop.NetworkManager.openvpn\n"
            + "vpn.data:remote = nl-12.example.net:1194, proto-tcp = yes, connection-type = tls\n"
            + "GENERAL.DEVICES:tun0\nIP4.ADDRESS[1]:10.96.0.5/16\nIP4.GATEWAY:10.96.0.1\n"
            + "IP6.ADDRESS[1]:fe80\\:\\:1/64\n";
        var o = Net.parseNmcliConnection(ovpn);
        compare(o.kind, "OpenVPN over TCP");
        compare(o.server, "nl-12.example.net:1194");
        compare(o.device, "tun0");
        compare(o.address, "10.96.0.5/16");
        var wg = Net.parseNmcliConnection("connection.type:wireguard\nGENERAL.DEVICES:wg0\nIP4.ADDRESS[1]:10.2.0.2/32\nIP4.GATEWAY:--\n");
        compare(wg.kind, "WireGuard");
        compare(wg.server, "");
        compare(wg.gateway, "");
        var oc = Net.parseNmcliConnection("connection.type:vpn\nvpn.service-type:org.freedesktop.NetworkManager.openconnect\nvpn.data:gateway = vpn.example.org, protocol = gp\n");
        compare(oc.kind, "OpenConnect (gp)");
        compare(oc.server, "vpn.example.org");
        compare(Net.parseNmcliConnection(null).kind, "");
        compare(Net.parseNmcliConnection("garbage").kind, "");
    }

    function test_vpn_rows() {
        var nm = { kind: "WireGuard", server: "", device: "wg0", address: "10.2.0.2/32" };
        var ip = { country_code: "NL", country: "NL", city: "Amsterdam", region: "North Holland",
            ip: "203.0.113.9", org: "AS9009 M247" };
        var rows = Net.vpnRows("Proton NL#12", nm, null, ip, false);
        compare(rows.map(function (r) { return r.label; }),
            ["Name", "Type", "Interface", "VPN address", "Location", "Public address", "Provider"]);
        compare(rows[4].value, "Amsterdam, North Holland, NL");
        // Sharing the screen: nothing that says where the computer is.
        var shared = Net.vpnRows("Proton NL#12", { kind: "OpenVPN over UDP", server: "nl.example:1194" }, null, ip, true);
        compare(shared.map(function (r) { return r.label; }), ["Name", "Type"]);
        // Tailscale, no public lookup: the exit node's own place.
        var ts = { exitNode: "nl-ams", exitIp: "100.80.0.9", city: "Amsterdam", country: "Netherlands", selfIp: "100.64.0.2" };
        var trows = Net.vpnRows("nl-ams", null, ts, null, false);
        compare(trows[1].value, "Tailscale exit node (WireGuard)");
        compare(trows[2].value, "nl-ams · 100.80.0.9");
        compare(trows[4].value, "Amsterdam, Netherlands");
    }
}
