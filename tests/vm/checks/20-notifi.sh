# shellcheck shell=bash
# docs/notes/notifi-hyprland-check.md, rows 1-9 (N1..N9). noti-fi's `notify` is built on the host
# from $NOTIFI_SRC (default ~/Projects/noti-fi, `cargo build --release -p notify-cli`) and copied
# into the guest by run.sh. The second monitor is a Hyprland headless output.

ISLAND_XY=(1193 29)

nstatus() { vgh notifi-call status 2>/dev/null; }
nfield() { nstatus | jq -r "$1" 2>/dev/null; }
nfieldc() { nstatus | jq -c "$1" 2>/dev/null; }
notif_owner() { vg busctl --user status org.freedesktop.Notifications 2>/dev/null | sed -n 's/^UserUnit=//p'; }
# Is uq-shell's qs process connected to noti-fi's socket?
shell_on_notifi() {
    local pid; pid=$(unit_prop uq-shell.service MainPID)
    vgsh "ss -xp 2>/dev/null | grep -F \"/notifd.sock\" | grep -q 'pid=$pid,'"
}
head2() { vgh hyprctl monitors -j 2>/dev/null | jq -r '.[].name' | grep '^HEADLESS-' | head -n1; }
ensure_head2() {
    local h; h=$(head2)
    [ -n "$h" ] || { vgh hyprctl output create headless >/dev/null; sleep 1; h=$(head2); }
    echo "$h"
}
focus_mon() { vgh hyprctl dispatch "hl.dsp.focus({monitor=\"$1\"})" >/dev/null 2>&1; sleep 0.5; }
# Namespaces of the layers mapped on one monitor.
layers_on() { layers | jq -r --arg m "$1" '.[$m].levels // {} | .[] | .[] | .namespace' 2>/dev/null; }
notifi_layers_on() { layers_on "$1" | grep -E '^notifd' | sort -u | tr '\n' ' '; }

c_N1() {
    local out
    # shellcheck disable=SC2088 # expanded by the guest's shell
    out=$(vgh bash -c '~/.local/bin/notify takeover --plain 2>&1'; echo "exit=$?")
    printf '%s\n' "$out" | tail -n 6 | sed 's/^/    /' >>"$EVID/$CUR.txt"
    sleep 3
    local act owner
    act=$(vg systemctl --user is-active notifd.service)
    owner=$(notif_owner)
    note "notifd.service: $act; org.freedesktop.Notifications owner unit: ${owner:-none}"
    [ "$act" = active ] || bad "expected 'notify takeover --plain' to install and start noti-fi's user service; observed notifd.service $act (takeover prints 'no --now: nothing running changes; notifd takes over at next login')"
    [ "$owner" = notifd.service ] || bad "expected busctl to name notifd as the owner of org.freedesktop.Notifications; observed ${owner:-none}"
    if [ "$act" != active ]; then
        # Carry on as a fresh login would: start it (it queues for the name uQ's built-in holds).
        vg systemctl --user start --no-block notifd.service
        wait_for 15 notifi_socket_up && note "harness: started notifd.service by hand to continue (it answers on notifd.sock)"
    fi
}
notifi_socket_up() { vgh notifi-call status >/dev/null 2>&1; }

c_N2() {
    notifi_socket_up || { bad "noti-fi is not answering on notifd.sock"; return; }
    vg notify-send -a uqvm-n2 "count me" "one" >/dev/null 2>&1
    local t0; t0=$(gnow)
    vg systemctl --user restart uq-shell.service
    wait_layer 15 uq-bar || { bad "the bar did not come back after restarting the shell"; return; }
    if wait_for 5 shell_on_notifi; then
        note "uq-shell is connected to notifd.sock within 5 s"
    else
        bad "expected the shell to connect to noti-fi (hello and subscribe) within 5 s; observed no connection from uq-shell's qs to notifd.sock after $(since "$t0") s (Notifi.qml's FileView probe of the socket never reports loaded or failed)"
    fi
    sleep 3
    note "owner of org.freedesktop.Notifications: $(notif_owner); noti-fi unread: $(nfield .unread)"
    snap island
    ujournal "$t0" uq-shell | grep -q 'noti-fi is not running' && bad "the journal has the 'noti-fi is not running' warning"
    ipc open quicksettings >/dev/null; sleep 1
    snap quick-settings
    ipc close quicksettings >/dev/null
}

c_N3() {
    local h; h=$(ensure_head2)
    [ -n "$h" ] || { bad "could not add a headless output"; return; }
    note "second monitor: $h (Hyprland headless output)"
    vgsh 'f=~/.config/notifd/rules.json; tmp=$(mktemp); jq ". + {screen: \"focused\"}" "$f" >"$tmp" && mv "$tmp" "$f"' ||
        { bad "could not add \"screen\": \"focused\" to rules.json"; return; }
    sleep 3
    focus_mon "$h"
    note "focused: $(vgh hyprctl monitors -j | jq -r '.[] | select(.focused) | .name')"
    vg notify-send -a uqvm-n3 test hello
    sleep 2
    local scr; scr=$(nfield .screens.popups)
    note "noti-fi screens: $(nfieldc .screens); notifd layers on $h: $(notifi_layers_on "$h"); on Virtual-1: $(notifi_layers_on Virtual-1)"
    [ "$scr" = "$h" ] || bad "expected the popup on the focused monitor $h; noti-fi put popups on '${scr}'"
    snap virtual-1
    focus_mon Virtual-1
}

# osd_seen KEY: press KEY and list "monitor namespace" of every OSD layer mapped in the next
# 2.5 s (polled inside the guest every 50 ms; an OSD lives about 1.5 s).
osd_seen() {
    vgh bash -c 'for i in $(seq 1 40); do hyprctl layers -j | jq -r "to_entries[] | .key as \$m | .value.levels[][]? | select(.namespace | test(\"osd\"; \"i\")) | \$m + \" \" + .namespace"; sleep 0.05; done | sort -u' >"$RUN_DIR/.osd" 2>/dev/null &
    local p=$!
    sleep 0.6
    qmp key "$1"
    wait "$p"
    tr '\n' ';' <"$RUN_DIR/.osd"
}

c_N4() {
    local h m seen; h=$(ensure_head2)
    for m in Virtual-1 "$h"; do
        focus_mon "$m"
        seen=$(osd_seen volumeup)
        note "focused $m, volume up: OSD layers seen: ${seen:-none}; noti-fi osd screen=$(nfield .screens.osd)"
        [[ $seen == *notifd* ]] || bad "focused $m: expected noti-fi's OSD; observed ${seen:-no OSD} (uQ routes the OSD to noti-fi only over its socket, see N2)"
        [[ $seen == "$m "* || $seen == *";$m "* ]] || bad "focused $m: no OSD on the focused monitor (seen: ${seen:-none})"
        sleep 1
    done
    snap after
    focus_mon Virtual-1
}

c_N5() {
    local h; h=$(ensure_head2)
    focus_mon "$h"
    vg notify-send -a uqvm-n5 -t 20000 unplug "popup on the second monitor"
    qmp key volumeup
    sleep 1
    note "before unplug: popups=$(nfield .screens.popups) osd=$(nfield .screens.osd); on $h: $(notifi_layers_on "$h")"
    vgh hyprctl output remove "$h" >/dev/null
    sleep 3
    local p act
    p=$(nfield .screens.popups)
    act=$(vg systemctl --user is-active notifd.service)
    note "after unplug: notifd $act, popups=$p osd=$(nfield .screens.osd); on Virtual-1: $(notifi_layers_on Virtual-1)"
    [ "$act" = active ] || bad "notifd.service is $act after the monitor went away"
    [ "$p" = Virtual-1 ] || bad "expected noti-fi's windows on the remaining monitor Virtual-1; noti-fi reports popups on '$p'"
    snap after-unplug
}

c_N6() {
    local c0 c1 c2
    c0=$(nfield .center)
    qmp key meta_l-n; sleep 1.5; c1=$(nfield .center)
    snap first-press
    qmp key meta_l-n; sleep 1.5; c2=$(nfield .center)
    note "SUPER+N: noti-fi center $c0 -> $c1 -> $c2; layers: $(layer_names | sort -u | tr '\n' ' ')"
    { [ "$c1" = true ] && [ "$c2" = false ]; } || bad "expected SUPER+N to open noti-fi's list then close it; observed center $c0 -> $c1 -> $c2"
    qmp click "${ISLAND_XY[@]}"; sleep 1.5
    local c3; c3=$(nfield .center)
    note "left click on the island: center -> $c3"
    snap island-click
    [ "$c3" = true ] || bad "expected a left click on the island to open noti-fi's list; observed center=$c3"
    [ "$c3" = true ] && vgh notifi-call center '{"action":"close"}' >/dev/null 2>&1
    qmp key esc
}

c_N7() {
    have_magick || { na "ImageMagick (magick) is not installed on the host: cannot compare colours"; return; }
    vgh notifi-call center '{"action":"open"}' >/dev/null 2>&1
    sleep 1.5
    snap before
    local m0 m1 m2
    m0=$(region_mean "$RUN_DIR/N7-before.png" 400x220+860+570)
    vg uqctl theme set rosewood light >/dev/null
    sleep 3; snap rosewood-light
    m1=$(region_mean "$RUN_DIR/N7-rosewood-light.png" 400x220+860+570)
    vg uqctl theme set dracula dark >/dev/null
    sleep 3; snap dracula-dark
    m2=$(region_mean "$RUN_DIR/N7-dracula-dark.png" 400x220+860+570)
    note "mean brightness of the list area: start $m0, rosewood light $m1, dracula dark $m2 (center open: $(nfield .center))"
    awk -v a="$m0" -v b="$m1" 'BEGIN { d = a - b; if (d < 0) d = -d; exit !(d > 0.05) }' ||
        bad "noti-fi's list did not change colour within 3 s of 'uqctl theme set rosewood light' (mean $m0 -> $m1)"
    awk -v a="$m1" -v b="$m2" 'BEGIN { d = a - b; if (d < 0) d = -d; exit !(d > 0.05) }' ||
        bad "noti-fi's list did not change colour within 3 s of 'uqctl theme set dracula dark' (mean $m1 -> $m2)"
    vgh notifi-call center '{"action":"close"}' >/dev/null 2>&1
    vg uqctl theme set graphite dark >/dev/null
}

c_N8() {
    vg systemd-run --user --unit=uqvm-share --collect /usr/local/bin/uqvm-env wf-recorder -y -o Virtual-1 -f /tmp/uqvm-share.mp4 >/dev/null 2>&1
    sleep 3
    local sh; sh=$(nfield .sharing)
    note "screen share (wf-recorder, wlr-screencopy): noti-fi sharing=$sh by $(nfieldc .sharingBy)"
    if [ "$sh" != true ]; then
        vg systemctl --user stop uqvm-share.service >/dev/null 2>&1
        na "a wlr-screencopy recorder (wf-recorder) is not seen as a screen share by noti-fi here; a portal share needs a browser and xdg-desktop-portal's picker, which this VM can't drive"
        return
    fi
    vg notify-send -a uqvm-n8 secret body
    sleep 1.5
    note "latest on the socket while sharing: $(nfieldc .latest)"
    [ "$(nfield '.latest.summary // empty')" = "" ] || bad "noti-fi's latest still has a title while sharing"
    qmp move "${ISLAND_XY[@]}"; sleep 1
    snap hover-island
    vg systemctl --user stop uqvm-share.service >/dev/null 2>&1
}

c_N9() {
    local sid; sid=$(gsession)
    vg loginctl lock-session "$sid"
    wait_for 5 is_locked || { bad "did not lock"; return; }
    sleep 1
    vg notify-send -a uqvm-n9 locked hello
    qmp key volumeup
    sleep 0.5
    snap locked
    if have_magick; then
        local sd; sd=$(region_sd "$RUN_DIR/N9-locked.png" 400x200+880+40)
        note "popup area (top right) while locked: standard deviation $sd (0 = flat lock background)"
        lt "$sd" 0.02 || bad "something shows in the popup area over the lock screen (see N9-locked.png)"
        shows_bar "$RUN_DIR/N9-locked.png" && bad "the desktop shows over the lock"
    fi
}

check N1 "noti-fi takeover --plain" c_N1
check N2 "Hello and subscribe after a shell restart" c_N2
check N3 "Popups on the focused monitor" c_N3
check N4 "OSD on the focused monitor" c_N4
check N5 "Unplugging a monitor" c_N5
check N6 "Centre from SUPER+N and the island" c_N6
check N7 "Theme tokens" c_N7
check N8 "Sharing hides summaries" c_N8
check N9 "Nothing above the lock" c_N9
