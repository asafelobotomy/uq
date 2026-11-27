# shellcheck shell=bash
# 10-lock-idle.sh's sleep and idle checks, L6..L17 (docs/notes/lock-idle-live-check.md). Sourced
# by 10-lock-idle.sh, which registers them; the helpers and constants used here are there.

# --- suspend helpers -----------------------------------------------------------------------------
# In this QEMU (11.1, q35, KVM) only the first S3 of a boot really sleeps: later ones return from
# the firmware at once. So each suspend check starts from a fresh boot when this one already slept.
SLEPT_THIS_BOOT=0
fresh_boot_if_slept() {
    [ "$SLEPT_THIS_BOOT" = 0 ] && return 0
    note "(harness: rebooting the VM first: only the first S3 of a boot sleeps in this QEMU)"
    vroot sync
    vroot systemctl reboot >/dev/null 2>&1
    sleep 10
    wait_session_after_reboot || { bad "the session did not come back after the harness reboot"; return 1; }
    SLEPT_THIS_BOOT=0
}
# After a QMP wakeup: screenshot at once, then wait for ssh.
wake_and_snap() { # wake_and_snap NAME
    qmp wakeup
    sleep 0.3
    snap "$1"
    wait_for 30 vm_not_suspended && vm_wait_ssh 60
}

c_L6() {
    fresh_boot_if_slept || return
    is_unlocked || { bad "not unlocked at start"; return; }
    open_power_menu || { bad "SUPER+Escape did not open the power menu (8 s, twice)"; return; }
    snap menu
    local t0; t0=$(hnow)
    qmp click "${POWER_SUSPEND[@]}"
    if ! wait_for 15 vm_is_suspended; then
        bad "the VM did not suspend within 15 s of clicking Suspend"
        snap no-suspend
        return
    fi
    SLEPT_THIS_BOOT=1
    note "suspended $(since "$t0") s after the click"
    snap while-suspended
    shows_lock "$RUN_DIR/L6-while-suspended.png" || bad "the last frame before sleep is not the lock screen (the desktop was showing when the machine slept)"
    wake_and_snap on-wake
    shows_lock "$RUN_DIR/L6-on-wake.png" || bad "the first frame on wake is not the lock screen"
    sleep_timeline "$t0"
    is_locked || bad "LockedHint=no after waking"
    unlock_pw || bad "could not unlock after waking"
}

c_L7() {
    fresh_boot_if_slept || return
    is_unlocked || { bad "not unlocked at start"; return; }
    local inh
    inh=$(vg systemd-inhibit --list --no-pager | grep 'Lock the screen before sleep')
    note "before: $inh"
    [[ $inh == *delay* ]] || bad "expected uQ's 'Lock the screen before sleep' delay inhibitor; observed none"
    local t0; t0=$(hnow)
    vroot systemctl suspend >/dev/null 2>&1 &
    wait_for 15 vm_is_suspended || { wait; bad "the VM did not suspend within 15 s of systemctl suspend"; return; }
    wait
    SLEPT_THIS_BOOT=1
    note "suspended $(since "$t0") s after systemctl suspend"
    snap while-suspended
    shows_lock "$RUN_DIR/L7-while-suspended.png" || bad "the last frame before sleep is not the lock screen (the desktop was showing when the machine slept)"
    wake_and_snap on-wake
    shows_lock "$RUN_DIR/L7-on-wake.png" || bad "the first frame on wake is not the lock screen"
    sleep_timeline "$t0"
    is_locked || bad "LockedHint=no after waking"
    if wait_for 10 inhibitor_back; then
        note "after waking: $(vg systemd-inhibit --list --no-pager | grep 'Lock the screen before sleep')"
    else
        bad "no fresh 'Lock the screen before sleep' inhibitor 10 s after waking"
    fi
    unlock_pw || bad "could not unlock after waking"
}
# sleep_timeline HOST_EPOCH: logind, the lock unit and the kernel around that suspend.
sleep_timeline() {
    ev "sudo journalctl -b --no-pager -o short-precise --since @${1%.*} | grep -E 'suspend requested|will suspend|Starting uQ lock|Started uQ lock|uq-lock-run.*Configuration Loaded|Reached target Sleep|PM: suspend entry|Waking up from' | head -12"
}
inhibitor_back() { vg systemd-inhibit --list --no-pager 2>/dev/null | grep -q 'Lock the screen before sleep.*delay'; }

c_L8() {
    fresh_boot_if_slept || return
    is_unlocked || { bad "not unlocked at start"; return; }
    vg systemctl --user mask --runtime uq-lock.service >/dev/null 2>&1
    local t0 dt
    t0=$(hnow)
    vroot systemctl suspend >/dev/null 2>&1 &
    if wait_for 15 vm_is_suspended; then
        dt=$(since "$t0")
        SLEPT_THIS_BOOT=1
        note "with uq-lock masked: suspended $dt s after systemctl suspend"
        # The fallback is logind's InhibitDelayMaxSec (5 s by default) less 0.5 s (642d495).
        lt "$dt" 7 || bad "expected suspend within logind's inhibit delay (about 4.5 s); observed $dt s"
    else
        bad "did not suspend within 15 s with uq-lock masked"
    fi
    wait
    snap while-suspended
    wake_and_snap on-wake
    note "LockedHint after waking (documented: unlocked, the lock couldn't start): $(locked_hint)"
    vg systemctl --user unmask --runtime uq-lock.service >/dev/null 2>&1
    ev "journalctl --user -u uq-shell --since @${t0%.*} --no-pager -o cat | grep -iE 'lock|sleep|inhibit' | head -10"
}

# --- idle stages ---------------------------------------------------------------------------------
# start_idle: input now, and return the host time of that input.
start_idle() { poke; hnow; }
# idle_wait SECS PRED...: input now; print the seconds until PRED holds (nothing if not in SECS).
idle_wait() { local t0; t0=$(start_idle); wait_for "$1" "${@:2}" && since "$t0"; }
# stage_timing NAME SECS MAX LO HI PRED...: the checklist's measurement, straight after
# 'uqctl idle set' (no lock/unlock in between: a new timing must apply on its own).
stage_timing() {
    local name=$1 secs=$2 max=$3 lo=$4 hi=$5 dt; shift 5
    is_unlocked || unlock_pw || { bad "the session is locked before measuring $name"; return 1; }
    dt=$(idle_wait "$max" "$@")
    if [ -n "$dt" ]; then
        note "$name $dt s after the last input"
        { lt "$lo" "$dt" && lt "$dt" "$hi"; } || bad "expected $name at about $secs s; observed $dt s"
        return 0
    fi
    bad "expected $name $secs s after 'uqctl idle set'; observed nothing in $max s"
    return 1
}

c_L9() {
    vg rm -f /tmp/uqvm-dimkey
    hexec "foot -T uqvm-dim bash -c 'read -rn1 c; printf %s \"\$c\" >/tmp/uqvm-dimkey; sleep 60'"
    wait_for 10 window_titled uqvm-dim || { bad "could not open a foot window to receive input"; return; }
    vgh hyprctl dispatch 'hl.dsp.focus({window="title:uqvm-dim"})' >/dev/null 2>&1
    idle_set dim 30 >/dev/null || { bad "uqctl idle set dim 30 failed"; return; }
    stage_timing "the dim" 30 40 27 36 has_layer uq-dim || return
    sleep 1
    snap dimmed
    if have_magick; then
        local m; m=$(region_mean "$RUN_DIR/L9-dimmed.png" 1280x60+0+0)
        note "mean brightness of the bar area while dimmed: $m"
    fi
    qmp key x
    wait_for 3 not_layer uq-dim || bad "the dim stayed after a key press"
    sleep 1
    local got; got=$(vg cat /tmp/uqvm-dimkey 2>/dev/null)
    note "the key press that removed the dim reached the window: '${got}'"
    [ "$got" = x ] || bad "expected the first key press to reach the app under the dim; the window got '${got}'"
    # A click removes it too.
    if wait_for 40 has_layer uq-dim; then
        qmp click 640 600
        wait_for 3 not_layer uq-dim && note "a click removed the dim" || bad "the dim stayed after a click"
    else
        bad "the dim did not come back for the click test"
    fi
    idle_set dim off >/dev/null
    vg pkill -f uqvm-dimkey >/dev/null 2>&1
}
window_titled() { vgh hyprctl clients -j 2>/dev/null | jq -e --arg t "$1" 'any(.[]; .title == $t)' >/dev/null; }

c_L10() {
    idle_set lock 40 >/dev/null || { bad "uqctl idle set lock 40 failed"; return; }
    stage_timing "the lock" 40 55 37 47 is_locked && snap locked
}

c_L11() {
    idle_set screen-off 50 >/dev/null || { bad "uqctl idle set screen-off 50 failed"; return; }
    if stage_timing "screens off (dpmsStatus=false)" 50 65 47 57 dpms_off; then
        snap off
        qmp key shift
        wait_for 3 dpms_on && note "a key press turned them back on" || bad "screens stayed off after a key press"
    fi
}

c_L12() {
    idle_set locked-screen-off 30 >/dev/null || { bad "uqctl idle set locked-screen-off 30 failed"; return; }
    # (a) SUPER+L straight after input: off ~30 s after locking.
    poke
    local t0 dt
    t0=$(hnow)
    lock_key || { bad "did not lock"; return; }
    if wait_for 45 dpms_off; then
        dt=$(since "$t0")
        note "(a) SUPER+L: screens off $dt s after locking"
        { lt 27 "$dt" && lt "$dt" 37; } || bad "(a) expected screens off about 30 s after locking; observed $dt s"
    else
        bad "(a) screens still on 45 s after locking"
    fi
    qmp key shift; wait_for 3 dpms_on
    unlock_pw || { bad "could not unlock"; return; }
    # (b) idle 20 s first, then lock without input (IPC): off 30 s after the lock, not 10 s.
    poke
    sleep 20
    t0=$(hnow)
    ipc lock >/dev/null
    wait_for 5 is_locked || { bad "(b) qs ipc call uq lock did not lock"; return; }
    if wait_for 45 dpms_off; then
        dt=$(since "$t0")
        note "(b) locked after 20 s idle, via IPC: screens off $dt s after locking"
        lt 25 "$dt" || bad "(b) expected screens off ~30 s after locking (counted from the lock); observed $dt s (counted from the last input?)"
    else
        bad "(b) screens still on 45 s after locking"
    fi
    snap b-off
    qmp key shift; wait_for 3 dpms_on
}

c_L13() {
    # UPower in the VM: no battery, so the machine is always "on AC".
    local ob; ob=$(vg busctl get-property org.freedesktop.UPower /org/freedesktop/UPower org.freedesktop.UPower OnBattery 2>&1)
    note "UPower OnBattery: $ob"
    idle_set suspend 60 >/dev/null || { bad "uqctl idle set suspend 60 failed"; return; }
    idle_set lock 40 >/dev/null
    local t0; t0=$(start_idle)
    wait_for 75 vm_is_suspended && bad "on AC: the VM suspended $(since "$t0") s after the last input (expected never)"
    note "on AC: not suspended 75 s after the last input (lock 40, suspend 60); LockedHint=$(locked_hint)"
    vm_is_suspended && wake_and_snap woke
}
c_L13b() { na "needs battery power: QEMU has no battery device, so UPower is never on battery (the AC half is L13)"; }

c_L14() { na "needs battery power (UPower OnBattery); QEMU has no battery device"; }

mpv_play() {
    vg systemd-run --user --unit=uqvm-mpv --collect -E WAYLAND_DISPLAY= mpv --no-video --ao=null --loop=inf 'av://lavfi:sine=frequency=440' >/dev/null 2>&1
    wait_for 10 playing
}
playing() { [ "$(vgh playerctl status 2>/dev/null)" = Playing ]; }
mpv_stop() { vg systemctl --user stop uqvm-mpv.service >/dev/null 2>&1; }
uqd_set() { vgh busctl --user call org.uq.Desktop1 /org/uq/Desktop1 org.uq.Desktop1 SetSetting sss "$1" "$2" ""; }

c_L15() {
    mpv_play || { bad "could not start a playing MPRIS player (mpv + mpv-mpris)"; mpv_stop; return; }
    note "playerctl: $(vgh playerctl -l 2>&1 | tr '\n' ' ') status $(vgh playerctl status 2>&1)"
    idle_set dim 30 >/dev/null; idle_set lock 40 >/dev/null; idle_set screen-off 50 >/dev/null
    local t0 dt dimmed=0 off=0
    t0=$(start_idle)
    while lt "$(since "$t0")" 56; do
        has_layer uq-dim && dimmed=1
        dpms_off && off=1
        if is_locked && [ -z "${dt:-}" ]; then dt=$(since "$t0"); fi
        sleep 1
    done
    note "media playing: dimmed=$dimmed screens-off=$off locked at ${dt:-never} s"
    [ "$dimmed" = 0 ] || bad "dimmed while media was playing"
    [ "$off" = 0 ] || bad "screens went off while media was playing"
    if [ -n "${dt:-}" ]; then
        { lt 37 "$dt" && lt "$dt" 47; } || bad "expected the lock at about 40 s while playing; observed $dt s"
    else
        bad "expected it to still lock at 40 s while media plays; observed no lock in 56 s"
    fi
    qmp key shift; dpms_on || wait_for 3 dpms_on
    is_locked && unlock_pw
    # idle.media_blocks_lock on: no lock either.
    if uqd_set idle.media_blocks_lock true >/dev/null 2>&1; then
        t0=$(start_idle)
        wait_for 55 is_locked && bad "idle.media_blocks_lock on: locked $(since "$t0") s after the last input while playing"
        note "idle.media_blocks_lock on: LockedHint=$(locked_hint) after 55 s of playing"
        uqd_set idle.media_blocks_lock false >/dev/null 2>&1
    else
        bad "could not set idle.media_blocks_lock through org.uq.Desktop1 SetSetting"
    fi
    mpv_stop
}

c_L16() {
    ipc open quicksettings >/dev/null
    wait_layer 8 uq-quick || { bad "quick settings did not open"; return; }
    qs_toggle stay-awake || { bad "could not find the Stay awake row in quick settings"; return; }
    sleep 1
    snap stay-awake-on
    qmp key esc
    idle_set dim 30 >/dev/null; idle_set lock 40 >/dev/null; idle_set screen-off 50 >/dev/null
    local t0 any=""
    t0=$(start_idle)
    while lt "$(since "$t0")" 60; do
        has_layer uq-dim && any+=" dim"
        is_locked && any+=" lock"
        dpms_off && any+=" screen-off"
        [ -n "$any" ] && break
        sleep 2
    done
    note "Stay awake on, 60 s idle with dim 30, lock 40, screen-off 50: ${any:-nothing happened}"
    [ -z "$any" ] || bad "with Stay awake on, expected nothing; observed$any after $(since "$t0") s"
    settle
    ipc open quicksettings >/dev/null; wait_layer 8 uq-quick
    qs_toggle stay-awake
    sleep 1
    snap stay-awake-off
    qmp key esc
}

c_L17() {
    # From the checklist's test values, restore the defaults in the documented order.
    idle_off
    local kv
    for kv in "dim 30" "lock 40" "screen-off 50" "suspend 60"; do
        # shellcheck disable=SC2086
        idle_set $kv >/dev/null 2>&1 || bad "setting up: uqctl idle set $kv failed"
    done
    for kv in "suspend 1200" "screen-off 360" "lock 300" "dim 240" "locked-screen-off 30"; do
        local out
        # shellcheck disable=SC2086
        if out=$(idle_set $kv 2>&1); then note "uqctl idle set $kv: ok"; else bad "uqctl idle set $kv failed: $out"; fi
    done
    note "settings: $(vgh busctl --user call org.uq.Desktop1 /org/uq/Desktop1 org.uq.Desktop1 GetSettings 2>/dev/null | grep -o '\\"idle\\":{[^}]*}')"
}
