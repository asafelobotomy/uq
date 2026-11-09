#!/usr/bin/env bash
# Headless check of the two lock PAM stacks (packaging/pam/uq-lock{,-fingerprint}) through
# Quickshell's own PamContext, offscreen, against a temp configDirectory (never /etc/pam.d, never a
# lock surface: nothing here can lock the session).
#   - The fingerprint stack never succeeds. Without pam_fprintd (CI) it errors; it never answers a
#     prompt.
#   - The password stack, answered with a wrong password, fails.
#   - Neither ever reports success.
# Both run as `nobody` (PamContext refuses unknown users), never as $USER: a wrong password must
# never count towards the tester's pam_faillock tally (/run/faillock/$USER is writable by its owner).
# As `nobody`, unix_chkpwd rejects any password (nobody has none), so the wrong-password case proves
# the stack asks an echo-off question and fails closed, not that it validates a correct password.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
command -v qs >/dev/null 2>&1 || { echo "pam_stacks skipped: qs not found"; exit 0; }
for f in uq-lock uq-lock-fingerprint; do
    [ -f "$root/packaging/pam/$f" ] || { echo "FAIL: packaging/pam/$f missing"; exit 1; }
done
tmp=$(mktemp -d)
qs_pid=""
cleanup() {
    [ -n "$qs_pid" ] && kill "$qs_pid" 2>/dev/null && wait "$qs_pid" 2>/dev/null
    rm -rf "$tmp"
}
trap cleanup EXIT

mkdir -p "$tmp/pam.d" "$tmp/cfg"
cp "$root/packaging/pam/uq-lock" "$root/packaging/pam/uq-lock-fingerprint" "$tmp/pam.d/"
# `include` may resolve inside the confdir: carry the system stacks the password stack includes
# (when this system has them; without them the include fails closed, which must also not succeed).
for f in system-local-login system-login system-auth; do
    [ -f "/etc/pam.d/$f" ] && cp "/etc/pam.d/$f" "$tmp/pam.d/"
done
# The password stack's file must be the packaged one, unchanged.
cmp -s "$root/packaging/pam/uq-lock" "$tmp/pam.d/uq-lock" || { echo "FAIL: uq-lock copy differs"; exit 1; }

user=nobody
getent passwd "$user" >/dev/null || { echo "pam_stacks skipped: no $user user"; exit 0; }
cat > "$tmp/cfg/shell.qml" <<QML
import QtQuick
import Quickshell
import Quickshell.Services.Pam
ShellRoot {
    property var finished: ({})
    // Each context reports once (an error is followed by a completed): quit when both have.
    function done(which) {
        finished[which] = true;
        if (finished.pw && finished.fp) Qt.quit();
    }
    function name(result) { return result === PamResult.Success ? "Success" : "NotSuccess:" + PamResult.toString(result); }
    PamContext {
        id: pw
        config: "uq-lock"
        configDirectory: "$tmp/pam.d"
        user: "$user"
        onResponseRequiredChanged: {
            if (!responseRequired) return;
            console.log("PW_PROMPT", responseVisible);
            respond("definitely-not-the-password");
        }
        onCompleted: result => { console.log("PW_RESULT", name(result)); done("pw"); }
        onError: error => { console.log("PW_ERROR", PamError.toString(error)); done("pw"); }
    }
    PamContext {
        id: fp
        config: "uq-lock-fingerprint"
        configDirectory: "$tmp/pam.d"
        user: "$user"
        onResponseRequiredChanged: {
            if (!responseRequired) return;
            console.log("FP_PROMPT");
            abort();
            done("fp");
        }
        onCompleted: result => { console.log("FP_RESULT", name(result)); done("fp"); }
        onError: error => { console.log("FP_ERROR", PamError.toString(error)); done("fp"); }
    }
    Component.onCompleted: {
        var a = pw.start(), b = fp.start();
        console.log("PW_START", a);
        console.log("FP_START", b);
        if (!a) done("pw");
        if (!b) done("fp");
    }
    // Backstop: pam_unix delays ~2 s on failure; pam_fprintd's own timeout is 30 s.
    Timer { interval: 45000; running: true; onTriggered: { console.log("TIMEOUT"); Qt.quit(); } }
}
QML
env -u HYPRLAND_INSTANCE_SIGNATURE -u WAYLAND_DISPLAY QT_QPA_PLATFORM=offscreen \
    qs -p "$tmp/cfg/shell.qml" --no-duplicate > "$tmp/out" 2>&1 &
qs_pid=$!
for _ in $(seq 500); do kill -0 "$qs_pid" 2>/dev/null || break; sleep 0.1; done
if kill -0 "$qs_pid" 2>/dev/null; then echo "FAIL: qs did not finish"; cat "$tmp/out"; exit 1; fi
wait "$qs_pid"
qs_pid=""

fail=0
show() { cat "$tmp/out"; }
if grep -qE '(PW|FP)_RESULT Success' "$tmp/out"; then echo "FAIL: a lock stack reported success"; fail=1; fi
grep -q 'TIMEOUT' "$tmp/out" && { echo "FAIL: a context never finished"; fail=1; }
grep -q 'PW_START true' "$tmp/out" || { echo "FAIL: the password context did not start"; fail=1; }
grep -qE 'PW_(RESULT|ERROR) ' "$tmp/out" || { echo "FAIL: the password context never finished"; fail=1; }
grep -qE 'FP_START false|FP_(RESULT|ERROR|PROMPT)' "$tmp/out" || { echo "FAIL: the fingerprint context never finished"; fail=1; }
# Where the system stack exists here, the wrong password must actually be asked for and rejected.
if [ -f /etc/pam.d/system-local-login ] && ! grep -q 'PW_PROMPT false' "$tmp/out"; then
    echo "FAIL: the password stack never asked for the (echo-off) password"; fail=1
fi
if grep -q 'PW_PROMPT false' "$tmp/out" && ! grep -qE 'PW_RESULT NotSuccess|PW_ERROR' "$tmp/out"; then
    echo "FAIL: the wrong password was not rejected"; fail=1
fi
if grep -E 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|is not a function|Syntax[Ee]rror' "$tmp/out"; then
    echo "FAIL: QML errors"; fail=1
fi
[ "$fail" = 0 ] || { show; exit 1; }
[ -n "${SHOW:-}" ] && show
echo "pam_stacks ok"
