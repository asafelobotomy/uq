# shellcheck shell=bash
# docs/notes/lock-idle-live-check.md, rows 1-24 (L1..L24), plus LJ (the journal rule in its
# preamble). Coordinates are for the VM's single 1280x800 output.

POWER_SUSPEND=(639 400) POWER_RESTART=(738 400)
POWER_CANCEL=(688 414)
APPMENU_LOCK=(284 445) APPMENU_RESTART=(390 445) APPMENU_SHUTDOWN=(424 445)
WRONG_PW=wrongpass1

# The lock can take several seconds to load under llvmpipe (see L6/L7), so allow 10 s.
lock_key() { qmp key meta_l-l; wait_for 10 is_locked && { wait_for 20 lock_visible || true; }; }
uq_lock_pid() { unit_prop uq-lock.service MainPID; }
uq_shell_pid() { unit_prop uq-shell.service MainPID; }
boot_id() { vg cat /proc/sys/kernel/random/boot_id 2>/dev/null; }

# --- L1 ----------------------------------------------------------------------------------------
c_L1() {
    local restarts0 props
    restarts0=$(unit_prop uq-lock.service NRestarts)
    lock_key || { bad "SUPER+L did not lock within 10 s (LockedHint=$(locked_hint))"; return; }
    note "SUPER+L: LockedHint=yes, uq-lock $(unit_prop uq-lock.service ActiveState)"
    sleep 1
    snap locked
    shows_lock "$RUN_DIR/L1-locked.png" || bad "the lock screen is not what the output shows while locked"
    qmp type "$WRONG_PW" && qmp key ret
    # Wait for PAM's answer (the red message) before typing again: keys typed while the attempt
    # is still running are not taken.
    wait_for 15 failed_shown || note "no red message 15 s after the wrong password"
    sleep 0.5
    snap wrong-password
    is_locked || bad "a wrong password unlocked the session"
    if have_magick; then
        local r; r=$(red_px "$RUN_DIR/L1-wrong-password.png" 400x40+440+550)
        note "red message pixels under the field after the wrong password: $r"
        [ "${r:-0}" -gt 20 ] || bad "expected PAM's error under the field after a wrong password; observed no red message (see L1-wrong-password.png)"
    fi
    # Typing the right password straight after the failure only unlocks if the field was cleared.
    qmp type "$UQVM_PASS" && qmp key ret
    if ! wait_for 15 is_unlocked; then
        bad "the right password (typed right after a failed try, without clearing) did not unlock: the field may not be cleared after a try"
        return
    fi
    note "right password: LockedHint=no"
    sleep 3
    props=$(vg systemctl --user show uq-lock.service -p ActiveState -p SubState -p Result -p ExecMainStatus -p NRestarts | tr '\n' ' ')
    note "3 s after unlocking: $props"
    [[ $props == *ActiveState=inactive* ]] || bad "expected uq-lock inactive after unlocking; observed $props"
    [[ $props == *ExecMainStatus=0* ]] || bad "expected the lock to exit 0; observed $props"
    [[ $props == *Result=success* ]] || bad "expected Result=success; observed $props"
    [[ $props == *"NRestarts=$restarts0 "* ]] || bad "uq-lock restarted after the unlock (NRestarts $restarts0 -> $props)"
    snap unlocked
}

failed_shown() { local f=$RUN_DIR/.failed.png; qmp screendump "$f" >/dev/null 2>&1 && FIND_MAX_RMSE=0.12 find_template auth-failed 500x120+390+500 "$f" >/dev/null; }

# --- L2 ----------------------------------------------------------------------------------------
fp_ready() { vroot fprintd-list "$UQVM_USER" 2>/dev/null | grep -q 'right-index-finger'; }
c_L2() {
    fp_ready || { na "no virtual fingerprint enrolled (fprintd/libfprint virtual_device setup failed; see run log)"; return; }
    note "libfprint virtual_device (FP_VIRTUAL_DEVICE) with right-index-finger enrolled as 'finger-1'"
    local t; t=$(gnow)
    lock_key || { bad "did not lock"; return; }
    sleep 3
    vroot fp-virtual SCAN wrong-finger
    sleep 2
    snap wrong-finger
    is_locked || bad "a wrong finger unlocked the session"
    ujournal "$t" uq-lock | grep -q 'Failed to match' && note "wrong finger: 'Failed to match fingerprint' relayed to the lock"
    sleep 2
    vroot fp-virtual SCAN finger-1
    if wait_for 6 is_unlocked; then
        note "enrolled finger: unlocked without the password"
    else
        bad "expected the enrolled finger to unlock after a wrong one (polling should continue); observed still locked"
        snap still-locked
    fi
    ev "journalctl --user -u uq-lock --since @${t%.*} --no-pager -o cat | grep -E 'fingerprint|Swipe|match' | head -8"
}

# --- L3 ----------------------------------------------------------------------------------------
c_L3() {
    ipc open spotlight >/dev/null; sleep 0.5
    note "before lock: $(layer_names | sort -u | tr '\n' ' ')"
    has_layer uq-spotlight || note "(spotlight was not open before locking)"
    ipc open quicksettings >/dev/null; sleep 0.5
    note "after opening quick settings: $(layer_names | sort -u | tr '\n' ' ')"
    # Timed inside the guest: from the key press until LockedHint=yes and both surfaces are gone.
    local sid; sid=$(gsession)
    vgh bash -c 'start=$(date +%s%N); for i in $(seq 1 100); do
            l=$(loginctl show-session '"$sid"' -p LockedHint --value)
            n=$(hyprctl layers -j | grep -cE "\"uq-(spotlight|quick)\"")
            [ "$l" = yes ] && [ "$n" = 0 ] && { echo "$(( ($(date +%s%N) - start) / 1000000 ))"; exit 0; }
            sleep 0.03; done; echo never' >"$RUN_DIR/.l3" 2>/dev/null &
    local p=$!
    sleep 0.3
    qmp key meta_l-l
    wait "$p"
    local ms; ms=$(cat "$RUN_DIR/.l3")
    note "LockedHint=yes and spotlight/quick settings gone ${ms} ms after SUPER+L (timed in the guest, from 0.3 s before the key)"
    if [ "$ms" = never ]; then
        bad "expected the surfaces closed and LockedHint=yes within a second of SUPER+L; observed LockedHint=$(locked_hint), layers: $(layer_names | sort -u | tr '\n' ' ')"
    elif [ "$ms" -gt 1300 ]; then
        bad "the shell saw the lock after $((ms - 300)) ms (expected within a second)"
    fi
    qmp key meta_l-spc; sleep 1
    has_layer uq-spotlight && bad "SUPER+Space opened spotlight over the lock"
    local s r
    for s in spotlight appmenu quicksettings powermenu; do
        r=$(ipc open "$s")
        note "locked: qs ipc call uq open $s -> $r"
        [ "$r" = ignored ] || bad "expected 'ignored' from open $s while locked; observed '$r'"
    done
    sleep 0.5
    for s in uq-spotlight uq-appmenu uq-quick uq-power uq-notif-centre; do
        has_layer "$s" && bad "$s is mapped while locked"
    done
    qmp key meta_l-n; sleep 1
    has_layer uq-notif-centre && bad "SUPER+N opened the centre while locked"
    snap locked
    note "LockedHint while locked: $(locked_hint)"
    unlock_pw || { bad "could not unlock"; return; }
    note "LockedHint after unlock: $(locked_hint)"
}

# --- L4 ----------------------------------------------------------------------------------------
c_L4() {
    lock_key || { bad "did not lock"; return; }
    sleep 2
    local p0 out
    p0=$(uq_lock_pid)
    # Kill and time the restart inside the guest (ssh round trips would blur it).
    out=$(vgsh 'p0=$(systemctl --user show uq-lock -p MainPID --value); pkill -9 -f "/usr/share/uq/[l]ock"; t0=$(date +%s%N)
        for i in $(seq 1 100); do p=$(systemctl --user show uq-lock -p MainPID --value); a=$(systemctl --user is-active uq-lock)
            if [ "$p" != 0 ] && [ "$p" != "$p0" ] && [ "$a" = active ]; then echo "back pid=$p after $(( ($(date +%s%N) - t0) / 1000000 )) ms"; exit 0; fi
            sleep 0.05; done; echo "not back after 5 s"')
    snap after-kill
    note "uq-lock pid $p0 killed: $out"
    [[ $out == back* ]] || bad "expected the lock back within about 1 s; observed: $out"
    local ms=${out##*after }; ms=${ms%% ms}
    [[ $out == back* ]] && [ "${ms:-9999}" -gt 2000 ] && bad "the lock came back after $ms ms (expected about 1 s)"
    sleep 1.5
    snap back
    is_locked || bad "LockedHint went to no after the lock was killed"
    shows_bar "$RUN_DIR/L4-after-kill.png" && bad "the desktop was visible right after kill -9 of the lock"
    shows_lock "$RUN_DIR/L4-back.png" || bad "the lock screen is not showing after the lock came back"
    unlock_pw || bad "the restarted lock did not unlock with the password"
}

# --- L5 ----------------------------------------------------------------------------------------
c_L5() {
    lock_key || { bad "did not lock"; return; }
    sleep 2
    local s0 s1 t0
    s0=$(uq_shell_pid)
    t0=$(hnow)
    vg pkill -9 -f '/usr/share/uq/[s]hell'
    wait_for 10 shell_restarted "$s0" || bad "uq-shell did not restart within 10 s"
    s1=$(uq_shell_pid)
    note "uq-shell pid $s0 killed; new pid $s1 after $(since "$t0") s"
    sleep 3
    snap locked-after-shell-kill
    is_locked || bad "LockedHint went to no after the shell was killed"
    [ "$(unit_prop uq-lock.service ActiveState)" = active ] || bad "uq-lock not active after the shell restart"
    shows_lock "$RUN_DIR/L5-locked-after-shell-kill.png" || bad "the lock screen is not showing after the shell restart"
    unlock_pw || { bad "could not unlock after the shell restart"; return; }
    wait_layer 10 uq-bar || bad "no bar after unlocking"
    idle_set dim 30 >/dev/null; poke
    if wait_for 36 has_layer uq-dim; then
        note "idle still works after the restart: uq-dim mapped after dim 30"
        poke
        wait_for 3 not_layer uq-dim || bad "the dim stayed after input (after the shell restart)"
    else
        bad "after the shell restart and unlock, dim 30 never dimmed (idle stages not working)"
    fi
    idle_set dim off >/dev/null
}
shell_restarted() { local p; p=$(uq_shell_pid); [ -n "$p" ] && [ "$p" != 0 ] && [ "$p" != "$1" ] && [ "$(unit_prop uq-shell.service ActiveState)" = active ]; }

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
        lt "$dt" 4 || bad "expected suspend within about 2 s (the inhibitor's fallback); observed $dt s"
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
uqd_set() { vgh busctl --user call org.uq.Desktop1 /org/uq/Desktop1 org.uq.Desktop1 SetSetting ss "$1" "$2"; }

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

open_power_menu() {
    qmp key meta_l-esc
    wait_layer 8 uq-power && return 0
    qmp key meta_l-esc
    wait_layer 8 uq-power
}
power_restart_open() { # open the power menu and start the Restart countdown (checked on screen)
    open_power_menu || return 1
    for _ in 1 2; do
        qmp click "${POWER_RESTART[@]}"
        sleep 1
        find_template restarting 600x200+340+300 >/dev/null && return 0
    done
    note "the Restart countdown did not appear after clicking Restart"
    return 1
}

c_L18() {
    local b0; b0=$(boot_id)
    power_restart_open || { bad "the power menu did not open"; return; }
    snap countdown
    # Esc
    qmp key esc; sleep 1
    note "Esc: uq-power mapped=$(has_layer uq-power && echo yes || echo no)"
    # Cancel
    power_restart_open; qmp click "${POWER_CANCEL[@]}"; sleep 1
    snap after-cancel
    qmp key esc; sleep 0.5
    # A click outside
    power_restart_open; qmp click 150 700; sleep 1
    note "click outside: uq-power mapped=$(has_layer uq-power && echo yes || echo no)"
    # SUPER+L
    local t_last; t_last=$(hnow)
    power_restart_open; qmp key meta_l-l; wait_for 5 is_locked || bad "SUPER+L during the countdown did not lock"
    unlock_pw
    # Any countdown not aborted restarts the machine 30 s after it started.
    while lt "$(since "$t_last")" 36; do sleep 2; done
    vm_wait_ssh 60
    if [ "$(boot_id)" = "$b0" ]; then
        note "no restart 36 s after the last aborted countdown (Esc, Cancel, click outside, SUPER+L)"
    else
        bad "the machine restarted after the countdown was aborted"
        wait_session_after_reboot
        return
    fi
    # Let one reach 0.
    vroot sync
    power_restart_open || { bad "the power menu did not open for the full countdown"; return; }
    local t0; t0=$(hnow)
    sleep 28
    if wait_for 150 booted_differently "$b0"; then
        note "the countdown reached 0: the machine restarted (new boot id) $(since "$t0") s after starting it"
    else
        bad "the countdown reached 0 but the machine did not restart"
    fi
    wait_session_after_reboot || bad "the session did not come back after the restart"
}
booted_differently() { local b; b=$(boot_id); [ -n "$b" ] && [ "$b" != "$1" ]; }
wait_session_after_reboot() {
    vm_wait_ssh 180
    wait_for 120 session_ready || return 1
    settle
}
session_ready() { [ -n "$(vgh sh -c 'echo $HYPRLAND_INSTANCE_SIGNATURE')" ] && has_layer uq-bar; }

c_L19() {
    local b0 name xy
    b0=$(boot_id)
    for name in restart shutdown; do
        if [ "$name" = restart ]; then xy=("${APPMENU_RESTART[@]}"); else xy=("${APPMENU_SHUTDOWN[@]}"); fi
        ipc open appmenu >/dev/null
        wait_layer 8 uq-appmenu || { bad "the app menu did not open"; return; }
        qmp click "${xy[@]}"
        if wait_layer 8 uq-power; then
            sleep 0.5
            snap "$name"
            note "app menu $name: power menu opened"
        else
            bad "app menu $name: expected the power menu at its countdown; observed no uq-power layer"
        fi
        qmp key esc; sleep 1
    done
    sleep 32
    [ "$(boot_id)" = "$b0" ] && vm_running || bad "the machine restarted or shut down after the countdown was aborted with Esc"
}

c_L20() {
    local agents; agents=$(vg pgrep -a -f 'polkit-.*agent|polkit-kde|lxpolkit|polkit-gnome' 2>/dev/null)
    note "other polkit agents running: ${agents:-none}"
    vg rm -f /tmp/uqvm-pk1 /tmp/uqvm-pk2 /tmp/uqvm-pk3
    # 1: wrong password, then the right one.
    hexec 'pkexec true; echo $? >/tmp/uqvm-pk1'
    wait_layer 10 uq-polkit || { bad "no uq-polkit dialog 10 s after pkexec true"; ev "journalctl --user -u uq-shell --since -30s --no-pager -o cat | tail -5"; return; }
    sleep 1
    snap dialog
    qmp type "$WRONG_PW"; qmp key ret
    sleep 4
    snap wrong-password
    has_layer uq-polkit || bad "the dialog closed after a wrong password (expected an error and a retry)"
    qmp type "$UQVM_PASS"; qmp key ret
    wait_for 10 guest_file_is /tmp/uqvm-pk1 0 && note "right password: pkexec exited 0" ||
        bad "expected pkexec to succeed after the right password; observed exit '$(vg cat /tmp/uqvm-pk1 2>/dev/null)'"
    wait_for 3 not_layer uq-polkit || bad "the dialog stayed after success"
    # 2: Esc cancels.
    hexec 'pkexec true; echo $? >/tmp/uqvm-pk2'
    wait_layer 10 uq-polkit || { bad "no dialog for the Esc test"; return; }
    sleep 1; qmp key esc
    if wait_for 10 guest_file_nonzero /tmp/uqvm-pk2; then note "Esc: pkexec exited $(vg cat /tmp/uqvm-pk2)"; else bad "Esc: expected pkexec to fail; observed '$(vg cat /tmp/uqvm-pk2 2>/dev/null)'"; fi
    # 3: lock while open: hidden while locked, back after unlocking.
    hexec 'pkexec true; echo $? >/tmp/uqvm-pk3'
    wait_layer 10 uq-polkit || { bad "no dialog for the lock test"; return; }
    sleep 1
    lock_key || { bad "did not lock with the dialog open"; return; }
    sleep 2
    has_layer uq-polkit && bad "the polkit dialog is still mapped while locked"
    snap locked-with-dialog
    unlock_pw || { bad "could not unlock"; return; }
    if wait_layer 5 uq-polkit; then
        note "the dialog came back after unlocking"
        sleep 1
        snap dialog-back
        qmp type "$UQVM_PASS"; qmp key ret
        wait_for 10 guest_file_is /tmp/uqvm-pk3 0 || bad "after unlocking, the dialog did not authenticate (exit '$(vg cat /tmp/uqvm-pk3 2>/dev/null)')"
    else
        bad "the polkit dialog did not come back after unlocking"
    fi
}
guest_file_is() { [ "$(vg cat "$1" 2>/dev/null)" = "$2" ]; }
guest_file_nonzero() { local v; v=$(vg cat "$1" 2>/dev/null); [ -n "$v" ] && [ "$v" != 0 ]; }

c_L21() {
    local t0; t0=$(gnow)
    vroot faillock --user "$UQVM_USER" --reset
    note "faillock.conf: $(vg grep -E '^[^#]' /etc/security/faillock.conf | tr '\n' ' ') (defaults: deny=3, unlock_time=600)"
    lock_key || { bad "did not lock"; return; }
    sleep 2
    local _i
    for _i in 1 2 3; do qmp type "$WRONG_PW"; qmp key ret; sleep 4; done
    snap after-3-wrong
    ev "sudo faillock --user $UQVM_USER"
    qmp type "$UQVM_PASS"; qmp key ret
    sleep 4
    snap right-password-during-lockout
    if is_unlocked; then
        bad "after 3 wrong passwords the right one unlocked at once: expected faillock to lock the account (pam_faillock records nothing, see the tally above)"
    else
        note "right password during the lockout: still locked"
        # uq-lock-run keeps PAM's messages out of the journal, so the screen is the evidence.
        local png=$RUN_DIR/L21-right-password-during-lockout.png
        if FIND_MAX_RMSE=0.12 find_template account-locked 500x160+390+500 "$png" >/dev/null; then
            note "the lock shows faillock's message (templates/account-locked.png: 'The account is locked …')"
        elif FIND_MAX_RMSE=0.12 find_template auth-failed 500x120+390+500 "$png" >/dev/null; then
            bad "expected faillock's message on the lock; observed 'Authentication failed'"
        else
            bad "expected faillock's message on the lock; found neither it nor 'Authentication failed' (see the screenshot)"
        fi
        if fp_ready; then
            vroot fp-virtual SCAN finger-1
            wait_for 6 is_unlocked && note "fingerprint unlocked during the lockout" || bad "fingerprint did not unlock during the lockout"
        fi
    fi
    ev "journalctl --user -u uq-lock --since -40s --no-pager -o cat | grep -iE 'locked|faillock|Failed' | head -8"
    vroot faillock --user "$UQVM_USER" --reset
}

c_L22() {
    local sid t0
    sid=$(gsession)
    t0=$(hnow)
    vg loginctl lock-session "$sid"
    if wait_for 3 is_locked && [ "$(unit_prop uq-lock.service ActiveState)" = active ]; then
        note "loginctl lock-session $sid: locked (LockedHint=yes, uq-lock active) after $(since "$t0") s"
        lt "$(since "$t0")" 2.5 || bad "locked, but later than about 1 s"
    else
        bad "loginctl lock-session $sid did not lock within 3 s (LockedHint=$(locked_hint))"
    fi
    snap locked
}

c_L23() {
    ipc open appmenu >/dev/null
    wait_layer 8 uq-appmenu || { bad "the app menu did not open"; return; }
    qmp click "${APPMENU_LOCK[@]}"
    wait_for 5 is_locked || bad "clicking Lock in the app menu did not lock"
    has_layer uq-appmenu && bad "the app menu is still mapped after Lock"
    snap locked
}

c_L24() {
    lock_key || { bad "did not lock"; return; }
    sleep 2
    local p0; p0=$(uq_lock_pid)
    vg pkill -9 -f '/usr/share/uq/[l]ock'
    qmp key ctrl-meta_l-l
    sleep 0.3
    snap after-kill
    wait_for 5 lock_back "$p0" || bad "the lock did not come back within 5 s of SUPER+CTRL+L"
    is_locked || bad "LockedHint=no during the recovery"
    sleep 1
    snap back
    note "lock processes: $(vg pgrep -a -f '/usr/share/uq/lock|hyprlock' | tr '\n' ';')"
    unlock_pw || bad "the recovered lock did not unlock with the password"
    sleep 2
    vg pgrep -x hyprlock >/dev/null && bad "hyprlock is still running after unlocking (the bind's fallback ran alongside uq-lock)"
}
lock_back() { local p; p=$(uq_lock_pid); [ -n "$p" ] && [ "$p" != 0 ] && [ "$p" != "$1" ]; }

c_LJ() {
    local hits
    hits=$(vgsh "journalctl --user --no-pager -o cat -b 2>/dev/null; sudo journalctl --no-pager -o cat -b 2>/dev/null" |
        grep -F -e "$UQVM_PASS" -e "$WRONG_PW" | head -5)
    [ -z "$hits" ] && note "no password typed in this run appears in the user or system journal" ||
        bad "a password appears in the journal: $hits"
    hits=$(vg journalctl --user --no-pager -o cat -b -u uq-shell -u uq-lock 2>/dev/null | grep -iE 'pam' | sort | uniq -c | sort -rn | head -8)
    note "PAM lines in journalctl --user -u uq-shell -u uq-lock (this boot):"
    printf '%s\n' "$hits" | sed 's/^/    /' >>"$EVID/$CUR.txt"
    [ -z "$hits" ] || bad "PAM messages reach the journal (uq-lock-run's filter misses them): $(printf '%s' "$hits" | head -2 | sed 's/^ *//' | tr '\n' ';')"
    # The filter (uq-lock-run's grep) must not outlive the lock unit.
    hits=$(vg journalctl --user --no-pager -o cat -b -u uq-lock 2>/dev/null | grep -c 'remains running after unit stopped')
    note "'remains running after unit stopped' lines for uq-lock this boot: ${hits:-0}"
    [ "${hits:-0}" = 0 ] || bad "a uq-lock process outlived the unit ($hits times this boot)"
}

check L1 "Lock and unlock with a password" c_L1
check L2 "Unlock with a fingerprint (libfprint virtual device)" c_L2
check L3 "LockState from Hyprland closes and blocks surfaces" c_L3
check L4 "kill -9 the lock" c_L4
check L5 "kill -9 the shell" c_L5
check L6 "Suspend from the power menu" c_L6
check L7 "Suspend from logind (systemctl suspend)" c_L7
check L8 "Sleep fallback with uq-lock masked" c_L8
check L9 "Dim" c_L9
check L10 "Lock on idle" c_L10
check L11 "Screens off" c_L11
check L12 "Screens off after locking" c_L12
check L13 "Suspend on idle: never on AC" c_L13
check L13b "Suspend on idle: on battery, only behind a secure lock" c_L13b
check L14 "Suspend counts from last input" c_L14
check L15 "Media playing" c_L15
check L16 "Stay awake" c_L16
check L17 "Restore the defaults" c_L17
check L19 "App menu restart and shut down" c_L19
check L20 "Polkit prompt" c_L20
check L21 "Faillock lockout" c_L21
check L22 "loginctl lock-session" c_L22
check L23 "App menu Lock" c_L23
check L24 "Recovery bind SUPER+CTRL+L" c_L24
check LJ "Journal: no password or PAM message" c_LJ
