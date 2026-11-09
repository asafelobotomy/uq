#!/usr/bin/env bash
# Regression test for lock/Auth.qml's security logic: the real file, offscreen `qs`, temp PAM stacks
# (Auth.pamDir), running as `nobody` (USER=nobody) so no real account's faillock tally is touched.
# It never loads lock/shell.qml or any lock surface: nothing here can lock the session.
#   pw    : a wrong password through the real uq-lock stack -> Failed, no unlocked(), pending null; Enter
#           while that attempt is busy is refused (not answered, not dropped silently).
#   fpask : fprintd-list (stub) lists a finger, and the fingerprint stack prompts (pam_unix): the
#           context aborts without ever responding, then fprintd-list is polled again 10 s later.
#   fpfail: fprintd-list exits 1 -> polled again at the 10 s throttle.
#   fpnone: fprintd-list not on PATH (failed start) -> polled again at the 10 s throttle.
# As `nobody`, unix_chkpwd rejects any password, so `pw` proves rejection and the no-unlock path, not
# that the stack validates a correct password.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
qs=$(command -v qs) || { echo "auth_logic skipped: qs not found"; exit 0; }
getent passwd nobody >/dev/null || { echo "auth_logic skipped: no nobody user"; exit 0; }
tmp=$(mktemp -d)
pids=()
cleanup() {
    for p in "${pids[@]}"; do kill "$p" 2>/dev/null; wait "$p" 2>/dev/null; done
    rm -rf "$tmp"
}
trap cleanup EXIT

# Stacks: the real password stack (with the system stacks it includes, when present) and a
# fingerprint stack that prompts, standing in for a module that asks a question.
mkdir -p "$tmp/pam.d"
cp "$root/packaging/pam/uq-lock" "$tmp/pam.d/"
for f in system-local-login system-login system-auth; do
    [ -f "/etc/pam.d/$f" ] && cp "/etc/pam.d/$f" "$tmp/pam.d/"
done
printf 'auth required pam_unix.so\n' > "$tmp/pam.d/uq-lock-fingerprint"

# fprintd-list stubs; each call appends a line to its scenario's log.
mkbin() { # dir, exit code, output
    mkdir -p "$1"
    printf '#!/bin/sh\necho call >> "%s/calls"\nprintf "%%b" "%s"\nexit %s\n' "$1" "$3" "$2" > "$1/fprintd-list"
    chmod +x "$1/fprintd-list"
}
mkbin "$tmp/bin-listed" 0 'Fingerprints for user nobody on X (press):\n - #0: right-index-finger\n'
mkbin "$tmp/bin-fail" 1 'No devices available\n'
mkdir -p "$tmp/bin-none"

run() { # scenario, PATH dir
    local cfg="$tmp/cfg-$1"
    mkdir -p "$cfg"
    # The real Auth.qml, imported from inside the config dir (Quickshell refuses outside imports).
    ln -s "$root/lock/Auth.qml" "$cfg/Auth.qml"
    ln -s "$root/shell/logic" "$cfg/logic"
    cat > "$cfg/shell.qml" <<QML
import QtQuick
import Quickshell
ShellRoot {
    Auth {
        id: a
        pamDir: "$tmp/pam.d"
        onUnlocked: console.log("UNLOCKED")
    }
    property int polls: 0
    Timer {
        interval: 200; running: "$1" === "pw"
        onTriggered: {
            console.log("SUBMIT1", a.submit("definitely-wrong"));
            console.log("SUBMIT2", a.submit("second"), a.message.text);
        }
    }
    Timer {
        interval: 12500; running: true
        onTriggered: {
            console.log("STATE pending", a.pending === null, "busy", a.busy, "done", a.done);
            Qt.quit();
        }
    }
}
QML
    env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY USER=nobody PATH="$2" QT_QPA_PLATFORM=offscreen \
        "$qs" -p "$cfg/shell.qml" > "$tmp/out-$1" 2>&1 &
    pids+=($!)
}
run pw "$tmp/bin-fail"
run fpask "$tmp/bin-listed"
run fpfail "$tmp/bin-fail"
run fpnone "$tmp/bin-none"
for p in "${pids[@]}"; do wait "$p"; done
pids=()

fail=0
calls() { [ -f "$1/calls" ] && wc -l < "$1/calls" || echo 0; }
check() { grep -q "$2" "$tmp/out-$1" || { echo "FAIL ($1): no '$2'"; fail=1; }; }
none() { if grep -q "$2" "$tmp/out-$1"; then echo "FAIL ($1): unexpected '$2'"; fail=1; fi; }
for s in pw fpask fpfail fpnone; do
    none $s UNLOCKED
    check $s 'STATE pending true busy false done false'
    if grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|is not a function|Syntax[Ee]rror|Failed to load' "$tmp/out-$s"; then
        echo "FAIL ($s): QML errors"; fail=1
    fi
done
check pw 'SUBMIT1 true'
check pw 'SUBMIT2 false Checking'
check pw 'uq-lock: password Failed'
none pw 'definitely-wrong'
none pw 'second'
# fpask: the prompt is aborted, never answered, and the list is polled again (attempt + retry).
check fpask 'Relaying pam message'
none fpask 'Got response'
none fpask 'uq-lock: fingerprint Success'
# pw, fpfail and fpask share counting stubs; fpnone counts through Quickshell's failed-start warning.
n=$(calls "$tmp/bin-listed"); [ "$n" -ge 2 ] || { echo "FAIL (fpask): fprintd-list polled $n times, want >= 2"; fail=1; }
n=$(calls "$tmp/bin-fail"); [ "$n" -ge 4 ] || { echo "FAIL (fpfail+pw): fprintd-list polled $n times, want >= 4"; fail=1; }
n=$(grep -c 'Process failed to start' "$tmp/out-fpnone"); [ "$n" -ge 2 ] || { echo "FAIL (fpnone): $n start attempts, want >= 2"; fail=1; }
if [ "$fail" != 0 ] || [ -n "${SHOW:-}" ]; then
    for s in pw fpask fpfail fpnone; do echo "== $s"; cat "$tmp/out-$s"; done
fi
[ "$fail" = 0 ] && echo "auth_logic ok"
exit "$fail"
