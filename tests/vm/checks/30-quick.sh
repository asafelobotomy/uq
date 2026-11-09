# shellcheck shell=bash
# Slice 3 quick-settings checks (road-to-1.0 M0 step 2) and the slice 2 SUPER-tap check.
# Wi-Fi uses mac80211_hwsim (two virtual radios) with hostapd serving an open and a WPA2 network
# on one of them; Bluetooth has no virtual adapter here.


schedule_json() { vgh busctl --user call org.uq.Desktop1 /org/uq/Desktop1 org.uq.Desktop1 GetSchedule 2>/dev/null | sed 's/^s //'; }
settings_json() { vgh busctl --user call org.uq.Desktop1 /org/uq/Desktop1 org.uq.Desktop1 GetSettings 2>/dev/null | sed 's/^s "//; s/"$//; s/\\"/"/g'; }

c_Q5() {
    local out
    out=$(vg uqctl weather set Paris --pick 1 2>&1) || { na "uqctl weather set needs the geocoding service; it failed here: $(printf '%s' "$out" | tail -n1)"; return; }
    note "uqctl weather set Paris --pick 1: $(printf '%s' "$out" | tail -n1)"
    local s0 s1 s2 pos
    s0=$(schedule_json)
    ipc open quicksettings >/dev/null
    wait_layer 8 uq-quick || { bad "quick settings did not open"; return; }
    sleep 1
    snap before
    # Found once: the first click can switch the theme (and every label's colours) at once.
    pos=$(find_in_panel schedule) || { bad "could not find the Schedule row in quick settings"; return; }
    note "quick settings: Schedule row at y=${pos#* }"
    qmp click "$QS_SWITCH_X" "${pos#* }"; sleep 2
    s1=$(schedule_json)
    snap after-first
    qmp click "$QS_SWITCH_X" "${pos#* }"; sleep 2
    s2=$(schedule_json)
    snap after-second
    note "GetSchedule: $s0 -> $s1 -> $s2"
    [ "$s0" != "$s1" ] || bad "the schedule toggle did not change the schedule (GetSchedule stayed $s0)"
    [ "$s1" != "$s2" ] || bad "the second click did not change it back (stayed $s1)"
    ipc close quicksettings >/dev/null
    vg uqctl schedule off >/dev/null 2>&1
    vg uqctl theme set graphite dark >/dev/null 2>&1
}

c_Q6() {
    local v; v=$(vg pacman -Q hyprland | awk '{print $2}')
    note "Hyprland $v (the checklist asks for 0.56.2; Arch ships $v, so this runs the same check on $v)"
    qmp key meta_l; sleep 1.5
    has_layer uq-appmenu || bad "tapping SUPER did not open the app menu"
    snap tap-open
    qmp key meta_l; sleep 1.5
    has_layer uq-appmenu && bad "a second SUPER tap did not close the app menu"
    qmp key meta_l-z; sleep 1.5
    has_layer uq-appmenu && bad "SUPER+Z (another key while SUPER was held) opened the app menu"
    ipc close appmenu >/dev/null
}

wifi_setup() {
    vroot modprobe mac80211_hwsim radios=3 || return 1
    wait_for 20 vgsh 'nmcli -t -f DEVICE,TYPE dev | grep -q "^wlan2:wifi"' || return 1
    vroot nmcli dev set wlan1 managed no
    vroot nmcli dev set wlan2 managed no
    printf 'interface=wlan1\ndriver=nl80211\nssid=uq-open\nhw_mode=g\nchannel=6\n' |
        vgin sudo tee /tmp/uqvm-ap-open.conf >/dev/null
    printf 'interface=wlan2\ndriver=nl80211\nssid=uq-secure\nhw_mode=g\nchannel=11\nwpa=2\nwpa_key_mgmt=WPA-PSK\nrsn_pairwise=CCMP\nwpa_passphrase=uqvm-test-psk\n' |
        vgin sudo tee /tmp/uqvm-ap-secure.conf >/dev/null
    vroot hostapd -B -P /run/uqvm-ap-open.pid /tmp/uqvm-ap-open.conf >/dev/null || return 1
    vroot hostapd -B -P /run/uqvm-ap-secure.pid /tmp/uqvm-ap-secure.conf >/dev/null || return 1
    wait_for 40 wifi_listed
}
wifi_listed() {
    vroot iw dev wlan0 scan >/dev/null 2>&1
    vgsh 'n=$(nmcli -t -f SSID dev wifi list ifname wlan0 --rescan yes); echo "$n" | grep -q uq-open && echo "$n" | grep -q uq-secure'
}
# scan_requests SECS: RequestScan calls to NetworkManager on the system bus during SECS seconds
# (busctl monitor as root). NetworkManager and wpa_supplicant also scan on their own, without
# this call, so counting the calls is what isolates the panel's scanner.
scan_requests() {
    vroot timeout "$1" busctl monitor 2>/dev/null | grep -c 'Member=RequestScan'
}

c_Q1() {
    if ! wifi_setup; then
        na "mac80211_hwsim or hostapd could not be set up in this guest"
        return
    fi
    note "virtual Wi-Fi: $(vgsh 'nmcli -t -f SSID,SECURITY dev wifi list ifname wlan0' | tr '\n' ' ')"
    WIFI_DEV=wlan0
    local closed open after
    closed=$(scan_requests 30)
    ipc open quicksettings >/dev/null
    wait_layer 8 uq-quick || { bad "quick settings did not open"; return; }
    open=$(scan_requests 30)
    snap open
    ipc close quicksettings >/dev/null
    sleep 2
    after=$(scan_requests 30)
    note "RequestScan calls to NetworkManager: panel closed $closed in 30 s, open $open in 30 s, closed again $after in 30 s"
    [ "$open" -ge 2 ] || bad "expected the panel to request Wi-Fi scans while open; observed $open RequestScan calls in 30 s"
    [ "$closed" -eq 0 ] || bad "Wi-Fi scans were requested while the panel was closed ($closed in 30 s)"
    [ "$after" -eq 0 ] || bad "Wi-Fi scans went on after closing the panel ($after in 30 s)"
}

c_Q2() {
    [ -n "${WIFI_DEV:-}" ] || { na "no virtual Wi-Fi (see Q1)"; return; }
    ipc open quicksettings >/dev/null
    wait_layer 8 uq-quick || { bad "quick settings did not open"; return; }
    sleep 4
    snap networks
    local open sec setup
    open=$(find_in_panel wifi-uq-open) || { bad "the open network uq-open is not listed"; return; }
    sec=$(find_in_panel wifi-uq-secure) || bad "the WPA2 network uq-secure is not listed"
    setup=$(find_in_panel set-up-in-settings) || true
    note "rows: uq-open at y=${open#* }, uq-secure at y=${sec#* }, 'Set up in Settings' at y=${setup#* }"
    # A secured, unknown network says "Set up in Settings"; an open one must not, and must connect.
    [ -n "$setup" ] && [ "${setup#* }" = "${sec#* }" ] || bad "expected 'Set up in Settings' on the secured network's row only"
    [ -n "$setup" ] && [ "${setup#* }" = "${open#* }" ] && bad "the open network says 'Set up in Settings' (treated as secured)"
    qmp click "${open% *}" "${open#* }"
    if wait_for 20 vgsh 'nmcli -t -f NAME con show --active | grep -qx uq-open'; then
        note "one click on uq-open connected it (treated as open)"
    else
        bad "clicking the open network did not connect it within 20 s (expected: open networks connect from the panel)"
    fi
    sleep 1
    snap connected
    ipc close quicksettings >/dev/null
    vroot nmcli con down uq-open >/dev/null 2>&1
    wifi_teardown
}

c_Q3() { na "no Bluetooth adapter in the VM (QEMU has no virtual HCI here); Bluetooth connect needs hardware"; }
c_Q4() { na "scroll jitter is a visual judgement on real hardware; nothing to measure headless"; }

wifi_teardown() {
    vroot sh -c 'kill $(cat /run/uqvm-ap-*.pid) 2>/dev/null; sleep 1; rmmod mac80211_hwsim 2>/dev/null' >/dev/null 2>&1 || true
}

check Q5 "Quick settings: the schedule toggle" c_Q5
check Q6 "SUPER tap opens the app menu (Hyprland from Arch)" c_Q6
check Q1 "Quick settings: Wi-Fi scans only while the panel is open" c_Q1
check Q2 "Quick settings: open networks shown as Open" c_Q2
check Q3 "Quick settings: Bluetooth connect" c_Q3
check Q4 "Quick settings: no scroll jitter" c_Q4
