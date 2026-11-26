# shellcheck shell=bash
# 10-lock-idle.sh's power menu, polkit, faillock and recovery checks, L18..L24, and LJ. Sourced
# by 10-lock-idle.sh, which registers them; the helpers and constants used here are there.

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
    # Only the processes' own output (stdout/stderr) counts: PAM modules' syslog lines, such as
    # pam_unix's "authentication failure", are the system's audit trail, as for any login.
    hits=$(vg journalctl --user --no-pager -o cat -b -u uq-shell -u uq-lock _TRANSPORT=stdout 2>/dev/null | grep -iE 'pam' | sort | uniq -c | sort -rn | head -8)
    note "PAM lines in the stdout/stderr of uq-shell and uq-lock (this boot):"
    printf '%s\n' "$hits" | sed 's/^/    /' >>"$EVID/$CUR.txt"
    [ -z "$hits" ] || bad "PAM messages reach the journal (uq-lock-run's filter misses them): $(printf '%s' "$hits" | head -2 | sed 's/^ *//' | tr '\n' ';')"
    # The filter (uq-lock-run's grep) must not outlive the lock unit.
    hits=$(vg journalctl --user --no-pager -o cat -b -u uq-lock 2>/dev/null | grep -c 'remains running after unit stopped')
    note "'remains running after unit stopped' lines for uq-lock this boot: ${hits:-0}"
    [ "${hits:-0}" = 0 ] || bad "a uq-lock process outlived the unit ($hits times this boot)"
}
