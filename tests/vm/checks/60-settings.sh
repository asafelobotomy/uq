# shellcheck shell=bash
# Settings slices A–G, keybinds and E in the VM (docs/notes/settings-slice-*-live-check.md).
#
# Every check of tests/live runs here as its own VM check (same id): the suite is copied into the
# guest and runs there as uqtest against the installed uQ, with root (sudo) for the checks that add
# system entries or stop power-profiles-daemon. Each run saves a restore point first and restores
# it after. On top of those, the parts only a real session can do:
#   SA3k  Ctrl+Z and Ctrl+Shift+Z in the Settings app
#   SB6k  SUPER+ALT+P from user.lua runs `uqctl profile next`
#   SC1p  Pick a window: the overlay, a click on a window, Esc, a click on nothing, the 20 s timeout
#   SD1l  Startup apps at login: started in app.slice, a failing one, a delay, one turned off; a uqd
#         restart starts nothing twice (logs uqtest out and in)
#   SK2k  A rebound shortcut from a real key press: Super + K opens Spotlight, Super + Space doesn't,
#         and after a reset Super + Space does again
#
#   tests/vm/run.sh --only '^S[ABCDGKFE]'

LIVE_GUEST=/tmp/live
live_copied=0
live_copy() {
    [ "$live_copied" = 1 ] && return 0
    tar -C "$ROOT/tests" -cf - live | vgin tar -C /tmp -xf - && live_copied=1
}
# live_run ID: run one tests/live check in the guest and take over its evidence and verdict.
live_run() {
    local id=$1 out line status reason
    live_copy || { bad "copying tests/live into the guest failed"; return; }
    out=$(vgh env UQ_LIVE_YES=1 UQ_LIVE_SLOW=1 UQ_LIVE_OUT=/tmp/live-out bash "$LIVE_GUEST/run.sh" --only "^$id\$" 2>&1)
    sed -n "/^== $id:/,/^RESULT/p" <<<"$out" | sed '1d; $d'
    line=$(grep -P "^RESULT\t$id\t" <<<"$out")
    status=$(cut -f3 <<<"$line") reason=$(cut -f4 <<<"$line")
    case "$status" in
        PASS) ;;
        N/A) na "$reason" ;;
        FAIL) local r x; IFS="|" read -ra r <<<"$reason"; for x in "${r[@]}"; do bad "$x"; done ;;
        *) bad "the guest run gave no result: $(tail -n 5 <<<"$out" | tr '\n' ' ')" ;;
    esac
}
# Register every tests/live check under its own id.
while IFS= read -r l; do
    lid=${l%% *}
    eval "live_$lid() { live_run $lid; }"
    check "$lid" "${l:36}" "live_$lid"
done < <(bash "$ROOT/tests/live/run.sh" --list)
unset l lid

# --- helpers for the session-only checks ---------------------------------------------------------
UQB=(busctl --user --json=short call org.uq.Desktop1 /org/uq/Desktop1 org.uq.Desktop1)
gbus() { vgh "${UQB[@]}" "$@" 2>&1 | jq -Rr 'fromjson? // . | if type=="object" then (.data[0] // "") else . end'; }
g_look() { gbus GetSection s look | jq -c --arg k "$1" '.[$k]'; }
g_merge() { gbus SetSection sss "$1" "$(gbus GetSection s "$1" | jq -c "$2")" settings; }
g_active() { gbus ListProfiles | jq -r '[.[] | select(.active)][0].id // ""'; }
g_win() { vgh hyprctl clients -j 2>/dev/null | jq -c --arg c "$1" 'first(.[] | select(.class == $c)) // empty'; }
g_win_open() { [ -n "$(g_win "$1")" ]; }
g_open_win() {
    vgsh "setsid uqvm-env foot --app-id $1 --title uqlive-win sh -c 'sleep 600' >/dev/null 2>&1 &"
    wait_for 15 g_win_open "$1"
}
g_lua_save() { vgsh 'f=~/.config/hypr/uq/user.lua; cp -p "$f" /tmp/uqlive-user.lua'; }
g_lua_back() { vgsh 'cp -p /tmp/uqlive-user.lua ~/.config/hypr/uq/user.lua'; vgh hyprctl reload >/dev/null; }
g_point() { gbus SaveRestorePoint s "$1"; }
g_restore() { gbus Restore s "$1" >/dev/null; vgh hyprctl reload >/dev/null; }
g_settings_up() { vgh pgrep -x uq-settings >/dev/null; }

check SA3k "Ctrl+Z and Ctrl+Shift+Z in the app: three changes undo newest first, redo oldest first" c_SA3k
c_SA3k() {
    local p g0 r0 t0
    p=$(g_point "VM SA3k")
    vgh sh -c 'setsid uq-settings >/dev/null 2>&1 &'
    wait_for 20 g_settings_up || { bad "uq-settings did not start"; return; }
    sleep 4
    g0=$(g_look gaps_in) r0=$(g_look rounding) t0=$(gbus GetTheme)
    # Changes the app counts as its own (origin settings, after it started).
    g_merge look '.gaps_in = 6' >/dev/null; sleep 0.5
    g_merge look '.rounding = 3' >/dev/null; sleep 0.5
    gbus SetTheme sss velvet light settings >/dev/null; sleep 2
    snap changed
    for _ in 1 2 3; do qmp key ctrl-z; sleep 2; done
    snap undone
    note "after 3x Ctrl+Z: gaps $(g_look gaps_in) rounding $(g_look rounding) theme $(gbus GetTheme)"
    [ "$(g_look gaps_in)" = "$g0" ] || bad "Ctrl+Z: expected gaps $g0; observed $(g_look gaps_in)"
    [ "$(g_look rounding)" = "$r0" ] || bad "Ctrl+Z: expected rounding $r0; observed $(g_look rounding)"
    [ "$(gbus GetTheme)" = "$t0" ] || bad "Ctrl+Z: expected theme $t0; observed $(gbus GetTheme)"
    for _ in 1 2 3; do qmp key ctrl-shift-z; sleep 2; done
    note "after 3x Ctrl+Shift+Z: gaps $(g_look gaps_in) rounding $(g_look rounding) theme $(gbus GetTheme)"
    [ "$(g_look gaps_in)" = 6 ] || bad "Ctrl+Shift+Z: expected gaps 6; observed $(g_look gaps_in)"
    [ "$(g_look rounding)" = 3 ] || bad "Ctrl+Shift+Z: expected rounding 3; observed $(g_look rounding)"
    [ "$(gbus GetTheme)" = velvet ] || bad "Ctrl+Shift+Z: expected theme velvet; observed $(gbus GetTheme)"
    vgh pkill -x uq-settings
    g_restore "$p"
}

check SB6k "SUPER+ALT+P from user.lua runs uqctl profile next" c_SB6k
c_SB6k() {
    local p a=uqlive-ka b=uqlive-kb
    p=$(g_point "VM SB6k")
    g_lua_save
    gbus SetProfile sb "$(printf 'format = 1\nid = "%s"\nname = "uqlive ka"\n\n[look]\ngaps_in = 1\n' "$a")" true >/dev/null
    gbus SetProfile sb "$(printf 'format = 1\nid = "%s"\nname = "uqlive kb"\n\n[look]\ngaps_in = 2\n' "$b")" true >/dev/null
    printf '%s\n' 'hl.bind("SUPER + ALT + P", hl.dsp.exec_cmd("uqctl profile next"))' |
        vgin sh -c 'cat >>~/.config/hypr/uq/user.lua'
    vgh hyprctl reload >/dev/null; sleep 1
    local got=()
    for _ in 1 2 3; do qmp key meta_l-alt-p; sleep 2; got+=("$(g_active)"); done
    note "after each SUPER+ALT+P: ${got[*]}"
    [[ " ${got[*]} " == *" $a $b "* ]] || bad "expected uqlive ka then uqlive kb; observed ${got[*]}"
    gbus ProfileOff >/dev/null
    gbus DeleteProfile s "$a" >/dev/null; gbus DeleteProfile s "$b" >/dev/null
    g_lua_back
    g_restore "$p"
}

# g_pick: start a pick in the guest (it waits up to 20 s for the shell); the reply lands in
# /tmp/uqlive-pick.json.
g_pick() {
    vgsh 'rm -f /tmp/uqlive-pick.json; (uqvm-env busctl --user --json=short call org.uq.Desktop1 /org/uq/Desktop1 org.uq.Desktop1 PickWindow >/tmp/uqlive-pick.json 2>&1 &)'
    sleep 1.5
}
g_pick_done() { vgsh 'test -s /tmp/uqlive-pick.json'; }
g_pick_status() { vg cat /tmp/uqlive-pick.json | jq -r '.data[0] | fromjson | .status' 2>/dev/null; }
g_pick_class() { vg cat /tmp/uqlive-pick.json | jq -r '.data[0] | fromjson | .window.class // ""' 2>/dev/null; }

check SC1p "Pick a window: click a window, Esc, a click on nothing, and the 20 s timeout" c_SC1p
c_SC1p() {
    local w x y t0 took
    g_open_win uqlive-pick || { bad "the test window didn't open"; return; }
    w=$(g_win uqlive-pick)
    x=$(jq -r '.at[0] + (.size[0] / 2 | floor)' <<<"$w") y=$(jq -r '.at[1] + (.size[1] / 2 | floor)' <<<"$w")
    g_pick
    snap overlay
    note "layers while picking: $(layer_names | sort -u | tr '\n' ' ')"
    qmp click "$x" "$y"
    wait_for 5 g_pick_done || bad "no answer 5 s after clicking the window"
    note "click on the window at $x,$y: $(vg cat /tmp/uqlive-pick.json)"
    [ "$(g_pick_status)" = picked ] || bad "expected picked; observed $(g_pick_status)"
    [ "$(g_pick_class)" = uqlive-pick ] || bad "expected the window uqlive-pick; observed $(g_pick_class)"
    g_pick; qmp key esc
    wait_for 5 g_pick_done || bad "no answer 5 s after Esc"
    [ "$(g_pick_status)" = cancelled ] || bad "Esc: expected cancelled; observed $(g_pick_status)"
    vgh pkill -f -- '--app-id uqlive-pick' ; sleep 1
    g_pick; qmp click 640 450
    wait_for 5 g_pick_done || bad "no answer 5 s after clicking the empty desktop"
    note "click on the empty desktop: $(g_pick_status)"
    [ "$(g_pick_status)" = none ] || bad "empty desktop: expected none; observed $(g_pick_status)"
    t0=$(hnow); g_pick
    wait_for 30 g_pick_done || bad "no answer 30 s after a pick with no click"
    took=$(since "$t0")
    note "no click: $(g_pick_status) after ${took}s"
    [ "$(g_pick_status)" = timeout ] || bad "no click: expected timeout; observed $(g_pick_status)"
    has_layer uq-pick && bad "the pick overlay stayed after the timeout"
    return 0
}

# Startup entries for SD1l, in uqtest's ~/.config/autostart.
g_autostart_files() {
    vgsh 'd=~/.config/autostart; mkdir -p "$d"
        printf "[Desktop Entry]\nType=Application\nName=Live login\nExec=sleep 600\n" >"$d/uqlive-login.desktop"
        printf "[Desktop Entry]\nType=Application\nName=Live fail\nExec=sh -c \"echo boom; exit 3\"\n" >"$d/uqlive-fail.desktop"
        printf "[Desktop Entry]\nType=Application\nName=Live delay\nExec=sleep 601\n" >"$d/uqlive-delay.desktop"
        printf "[Desktop Entry]\nType=Application\nName=Live off\nExec=sleep 602\n" >"$d/uqlive-off.desktop"'
}
g_row() { gbus ListAutostart | jq -c --arg i "$1" '.[] | select(.id == $i)'; }
g_status() { g_row "$1" | jq -r .status; }
g_status_is() { [ "$(g_status "$1")" = "$2" ]; }
# g_log_has ID TEXT: the row's last lines contain TEXT. uqd reads them from the journal when the
# row is listed and tries again on the next listing while journald hasn't indexed them yet.
g_log_has() { [[ $(g_row "$1" | jq -r '.log // [] | tostring') == *"$2"* ]]; }
g_relogin() {
    vroot systemctl restart getty@tty1.service
    sleep 3
    wait_for 120 session_up
}

check SD1l "Startup at login: app.slice scopes, a failure, a delay, one off; uqd restarts start nothing twice" c_SD1l
c_SD1l() {
    local p t0 r n1 n2
    p=$(g_point "VM SD1l")
    g_autostart_files
    r=$(g_merge autostart '.files += [{id: "uqlive-delay.desktop", enabled: null, delay: 10}, {id: "uqlive-off.desktop", enabled: false, delay: null}]')
    [ -z "$r" ] || { bad "setting the delay and the off entry: $r"; return; }
    g_relogin || { bad "the session didn't come back after logging out and in"; return; }
    t0=$(hnow)
    wait_for 20 g_status_is uqlive-login.desktop running || bad "Live login isn't running after login: $(g_row uqlive-login.desktop)"
    note "delay row $(since "$t0")s after the session: $(g_row uqlive-delay.desktop)"
    [ "$(g_status uqlive-delay.desktop)" = waiting ] || note "the delayed entry wasn't waiting when first seen"
    wait_for 25 g_status_is uqlive-delay.desktop running || bad "the delayed entry isn't running 25 s after login: $(g_row uqlive-delay.desktop)"
    note "delayed entry running $(since "$t0")s after the session came up"
    ev "systemctl --user list-units --type=scope --no-legend 'app-uq-autostart-*'"
    r=$(vgsh "systemctl --user show -p Slice --value \$(systemctl --user list-units --type=scope --no-legend 'app-uq-autostart-*' | awk '/login/ {print \$1; exit}')")
    [ "$r" = app.slice ] || bad "expected the login app's scope under app.slice; observed ${r:-no scope}"
    wait_for 10 g_status_is uqlive-fail.desktop failed || bad "expected the failing entry failed; observed $(g_row uqlive-fail.desktop)"
    note "fail row: $(g_row uqlive-fail.desktop)"
    [ "$(g_row uqlive-fail.desktop | jq -r .exit)" = 3 ] || bad "expected exit 3 on the failing entry"
    wait_for 10 g_log_has uqlive-fail.desktop boom || bad "expected 'boom' in the failing entry's last lines; observed $(g_row uqlive-fail.desktop)"
    ev "journalctl --user -t uq-autostart-uqlive-fail --no-pager -o cat | tail -n 3"
    [ "$(g_status uqlive-off.desktop)" = off ] || bad "expected the turned-off entry off; observed $(g_row uqlive-off.desktop)"
    vgsh 'pgrep -fx "sleep 602" >/dev/null' && bad "the turned-off entry started"
    n1=$(vgsh 'pgrep -fc "^sleep 60[01]$"')
    vg systemctl --user restart uqd; sleep 5
    n2=$(vgsh 'pgrep -fc "^sleep 60[01]$"')
    note "startup apps running before and after restarting uqd: $n1, $n2"
    [ "$n1" = "$n2" ] || bad "restarting uqd changed the number of startup apps running ($n1 -> $n2)"
    vgsh 'pkill -fx "sleep 600"'; sleep 1
    vg systemctl --user restart uqd; sleep 4
    note "login row after killing it and restarting uqd: $(g_row uqlive-login.desktop)"
    # uqd adopted the app on the first restart, so it can't know the exit code: unknown_exit,
    # shown as "finished" (exited if it saw the exit itself).
    case "$(g_status uqlive-login.desktop)" in
        exited|unknown_exit) [[ $(g_row uqlive-login.desktop | jq -r .reason) == *finished* ]] ||
            bad "expected the killed app's reason to say finished; observed $(g_row uqlive-login.desktop)" ;;
        *) bad "expected the killed app to read finished; observed $(g_row uqlive-login.desktop)" ;;
    esac
    vgsh 'pkill -fx "sleep 601"; rm -f ~/.config/autostart/uqlive-*.desktop'
    g_restore "$p"
}

check SK2k "Rebind Open Spotlight to Super + K: the new keys open it, the old ones don't, a reset brings them back" c_SK2k
c_SK2k() {
    local p r
    p=$(g_point "VM SK2k")
    r=$(g_merge keybinds '.overrides = ((.overrides // []) + [{id: "uq.spotlight", combo: "SUPER + K"}])')
    [ -z "$r" ] || { bad "rebinding Open Spotlight: $r"; g_restore "$p"; return; }
    sleep 1.5
    ipc close spotlight >/dev/null; sleep 0.5
    qmp key meta_l-k
    if wait_for 3 has_layer uq-spotlight; then note "Super + K opened Spotlight"; else bad "Super + K did not open Spotlight after the rebind"; fi
    ipc close spotlight >/dev/null; sleep 0.7
    qmp key meta_l-spc; sleep 1.5
    has_layer uq-spotlight && bad "Super + Space still opened Spotlight after the rebind"
    ipc close spotlight >/dev/null; sleep 0.5
    vgh uqctl keys reset uq.spotlight >/dev/null || bad "uqctl keys reset uq.spotlight failed"
    sleep 1.5
    qmp key meta_l-spc
    if wait_for 3 has_layer uq-spotlight; then note "after the reset Super + Space opened Spotlight"; else bad "Super + Space did not open Spotlight after the reset"; fi
    ipc close spotlight >/dev/null
    g_restore "$p"
}
