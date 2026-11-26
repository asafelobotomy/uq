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
    ipc open lookseedo >/dev/null; sleep 0.5
    note "before lock: $(layer_names | sort -u | tr '\n' ' ')"
    has_layer uq-lookseedo || note "(lookseedo was not open before locking)"
    ipc open quicksettings >/dev/null; sleep 0.5
    note "after opening quick settings: $(layer_names | sort -u | tr '\n' ' ')"
    # Timed inside the guest: from the key press until LockedHint=yes and both surfaces are gone.
    local sid; sid=$(gsession)
    vgh bash -c 'start=$(date +%s%N); for i in $(seq 1 100); do
            l=$(loginctl show-session '"$sid"' -p LockedHint --value)
            n=$(hyprctl layers -j | grep -cE "\"uq-(lookseedo|quick)\"")
            [ "$l" = yes ] && [ "$n" = 0 ] && { echo "$(( ($(date +%s%N) - start) / 1000000 ))"; exit 0; }
            sleep 0.03; done; echo never' >"$RUN_DIR/.l3" 2>/dev/null &
    local p=$!
    sleep 0.3
    qmp key meta_l-l
    wait "$p"
    local ms; ms=$(cat "$RUN_DIR/.l3")
    note "LockedHint=yes and lookseedo/quick settings gone ${ms} ms after SUPER+L (timed in the guest, from 0.3 s before the key)"
    if [ "$ms" = never ]; then
        bad "expected the surfaces closed and LockedHint=yes within a second of SUPER+L; observed LockedHint=$(locked_hint), layers: $(layer_names | sort -u | tr '\n' ' ')"
    elif [ "$ms" -gt 1300 ]; then
        bad "the shell saw the lock after $((ms - 300)) ms (expected within a second)"
    fi
    qmp key meta_l-spc; sleep 1
    has_layer uq-lookseedo && bad "SUPER+Space opened lookseedo over the lock"
    local s r
    for s in lookseedo appmenu quicksettings powermenu; do
        r=$(ipc open "$s")
        note "locked: qs ipc call uq open $s -> $r"
        [ "$r" = ignored ] || bad "expected 'ignored' from open $s while locked; observed '$r'"
    done
    sleep 0.5
    for s in uq-lookseedo uq-appmenu uq-quick uq-power uq-notif-centre; do
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

# The sleep and idle checks (L6..L17) and the power menu and later ones (L18..L24, LJ).
# shellcheck source=tests/vm/checks/lock-idle-sleep.sh
. "$UQVM_DIR/checks/lock-idle-sleep.sh"
# shellcheck source=tests/vm/checks/lock-idle-power.sh
. "$UQVM_DIR/checks/lock-idle-power.sh"

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
