#!/usr/bin/env bash
# scripts/uq-dev without touching the system: promote and session run against fake makepkg,
# Hyprland, dbus-run-session and qs, and the test checks what they were handed.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
fail=0
no() { echo "FAIL: $*"; fail=1; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fake=$tmp/fake
mkdir -p "$fake" "$tmp/projects"
cat >"$fake/makepkg" <<'SH'
#!/bin/sh
cp PKGBUILD "$UQ_DEV_TEST_OUT/PKGBUILD"
ls >"$UQ_DEV_TEST_OUT/files"
tar tzf ./*.tar.gz | head -n 1 >"$UQ_DEV_TEST_OUT/top"
echo "$*" >"$UQ_DEV_TEST_OUT/args"
SH
cat >"$fake/Hyprland" <<'SH'
#!/bin/sh
env >"$UQ_DEV_TEST_OUT/env"
cp "$2" "$UQ_DEV_TEST_OUT/hyprland.lua"
ls -A "$XDG_RUNTIME_DIR" >"$UQ_DEV_TEST_OUT/rt"
SH
printf '#!/bin/sh\nshift\nexec "$@"\n' >"$fake/dbus-run-session"
printf '#!/bin/sh\n' >"$fake/qs"
chmod +x "$fake"/*

export UQ_DEV_TEST_OUT=$tmp/out UQ_DEV_HOME=$tmp/devhome UQ_DEV_PROJECTS=$tmp/projects
mkdir -p "$UQ_DEV_TEST_OUT"
ln -s "$root" "$tmp/projects/uq"
run() { PATH="$fake:$PATH" "$root/scripts/uq-dev" "$@"; }

# promote: HEAD packed under the PKGBUILD's own top folder, source swapped, pkgrel bumped.
if run promote uq >"$tmp/promote.log" 2>&1; then
    ver=$(sed -n 's/^pkgver=//p' "$root/packaging/uq/PKGBUILD")
    tag=${ver/rc/-rc.}
    grep -qx "source=(\"uq-$tag.tar.gz\")" "$UQ_DEV_TEST_OUT/PKGBUILD" || no "promote: source not swapped"
    grep -qx "sha256sums=(SKIP)" "$UQ_DEV_TEST_OUT/PKGBUILD" || no "promote: checksum not SKIP"
    grep -q '^pkgrel=[0-9][0-9]*\.[0-9][0-9]*$' "$UQ_DEV_TEST_OUT/PKGBUILD" || no "promote: pkgrel is not <rel>.<count>"
    grep -q 'releases/download' "$UQ_DEV_TEST_OUT/PKGBUILD" && no "promote: the release URL is still there"
    grep -qx "uq-$tag/" "$UQ_DEV_TEST_OUT/top" || no "promote: tarball top folder is $(cat "$UQ_DEV_TEST_OUT/top")"
    grep -qx -- "-si" "$UQ_DEV_TEST_OUT/args" || no "promote: makepkg was not run with -si"
    # The rest of the PKGBUILD is untouched.
    diff <(grep -v -e '^pkgrel=' -e '^source=' -e '^sha256sums=' "$root/packaging/uq/PKGBUILD") \
         <(grep -v -e '^pkgrel=' -e '^source=' -e '^sha256sums=' "$UQ_DEV_TEST_OUT/PKGBUILD") >/dev/null ||
        no "promote: changed more than pkgrel, source and sha256sums"
else
    no "promote failed: $(cat "$tmp/promote.log")"
fi
run promote romforge >/dev/null 2>&1 && no "promote accepted a repo with no checkout"

# session: a fake checkout with built binaries is enough.
fu=$tmp/fakeuq
mkdir -p "$fu/.git" "$fu/target/debug" "$fu/config/hypr" "$fu/themes" "$fu/templates" "$fu/shell"
printf '#!/bin/sh\n' >"$fu/target/debug/uqd"; cp "$fu/target/debug/uqd" "$fu/target/debug/uqctl"
chmod +x "$fu/target/debug/"*
rm "$tmp/projects/uq"; ln -s "$fu" "$tmp/projects/uq"
fu=$tmp/projects/uq   # the path uq-dev sees
prt=$tmp/prt
mkdir -p "$prt/pulse" "$prt/systemd"; touch "$prt/pipewire-0" "$prt/bus" "$prt/notifd.sock"
if WAYLAND_DISPLAY=wayland-9 XDG_RUNTIME_DIR=$prt HYPRLAND_INSTANCE_SIGNATURE=parent run session --no-build >"$tmp/session.log" 2>&1; then
    e=$UQ_DEV_TEST_OUT/env
    grep -qx 'UQ_DEV_SESSION=1' "$e" || no "session: UQ_DEV_SESSION not set"
    grep -qx "WAYLAND_DISPLAY=$prt/wayland-9" "$e" || no "session: parent Wayland socket not absolute"
    grep -q '^HYPRLAND_INSTANCE_SIGNATURE=' "$e" && no "session: the parent's Hyprland signature leaked in"
    grep -qx "XDG_CONFIG_HOME=$UQ_DEV_HOME/session/config" "$e" || no "session: config not sandboxed"
    grep -qx "XDG_STATE_HOME=$UQ_DEV_HOME/session/state" "$e" || no "session: state not sandboxed"
    grep -q "^XDG_RUNTIME_DIR=$prt\$" "$e" && no "session: runtime dir is the real one"
    grep -q "^PATH=$UQ_DEV_HOME/session/bin:$fu/target/debug:" "$e" || no "session: PATH lacks the shim and target/debug"
    for f in pipewire-0 pulse systemd; do grep -qx "$f" "$UQ_DEV_TEST_OUT/rt" || no "session: $f not shared"; done
    for f in bus notifd.sock; do grep -qx "$f" "$UQ_DEV_TEST_OUT/rt" && no "session: $f must not be shared"; done
    l=$UQ_DEV_TEST_OUT/hyprland.lua
    grep -qF "require([[$fu/config/hypr/base.lua]])" "$l" || no "session: base.lua not from the checkout"
    grep -qF "$fu/target/debug/uqd" "$l" || no "session: uqd not started from the checkout"
    grep -qF "qs -p $fu/shell" "$l" || no "session: shell not started from the checkout"
    [ -L "$UQ_DEV_HOME/session/share/hypr" ] || no "session: UQ_DATA_DIR has no hypr link"
    # The shim refuses uQ's units and passes the rest through.
    out=$("$UQ_DEV_HOME/session/bin/systemctl" --user restart uq-shell.service 2>&1)
    case "$out" in *refused*) ;; *) no "session: systemctl shim let 'restart uq-shell' through" ;; esac
    "$UQ_DEV_HOME/session/bin/systemctl" --user is-active uq-lock.service >/dev/null 2>&1
    [ $? = 3 ] || no "session: shim's is-active for uq-lock is not 'inactive'"
else
    no "session failed: $(cat "$tmp/session.log")"
fi
WAYLAND_DISPLAY='' run session --no-build >/dev/null 2>&1 && no "session ran outside a Wayland session"

[ "$fail" = 0 ] && echo "uq_dev: ok"
exit "$fail"
