#!/usr/bin/env bash
# The greeter's packaged config:
#   - /etc/uq/greetd.toml runs uq-greeter-session as `greeter` on VT 1
#   - /etc/uq/greeter.toml's defaults are the default theme, dark, list shown, motion on
#   - the greetd.service drop-in uses uQ's config while it exists and greetd's stock one otherwise
#     (its ExecStart run with a stub greetd)
#   - the tmpfiles entry creates the two directories with the right owners and modes
#   - the PKGBUILD installs all of it, the greeter tree and its symlinks
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
p="$root/packaging"
fail=0
bad() { echo "FAIL: $*"; fail=1; }
has() { grep -qx -- "$2" "$1" || bad "${1#"$root"/} lacks: $2"; }

has "$p/greeter/greetd.toml" '\[general\]'
has "$p/greeter/greetd.toml" 'service = "uq-greetd"'
has "$p/pam/uq-greetd" 'auth       include      system-local-login'
has "$p/pam/uq-greetd" '-auth      optional     pam_gnome_keyring.so'
has "$p/pam/uq-greetd" '-session   optional     pam_gnome_keyring.so auto_start'
has "$p/uq/PKGBUILD" '  install -Dm644 packaging/pam/uq-greetd "$pkgdir/etc/pam.d/uq-greetd"'
has "$p/greeter/greetd.toml" '\[terminal\]'
has "$p/greeter/greetd.toml" 'vt = 1'
has "$p/greeter/greetd.toml" '\[default_session\]'
has "$p/greeter/greetd.toml" 'command = "/usr/lib/uq/uq-greeter-session"'
has "$p/greeter/greetd.toml" 'user = "greeter"'
grep -q 'initial_session' "$p/greeter/greetd.toml" && grep -q '^\[initial_session\]' "$p/greeter/greetd.toml" \
    && bad "greetd.toml must not auto-login by default"

has "$p/greeter/greeter.toml" 'theme = "graphite"'
has "$p/greeter/greeter.toml" 'variant = "dark"'
has "$p/greeter/greeter.toml" 'hide_users = false'
has "$p/greeter/greeter.toml" 'reduce_motion = false'

dropin="$p/greeter/greetd-uq.conf"
has "$dropin" '\[Service\]'
has "$dropin" 'ExecStart='
grep -q '^\[Install\]' "$dropin" && bad "the drop-in must not have an [Install] section"
# Run the drop-in's command with the paths pointed at a stub greetd and a temp config.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
printf '#!/bin/sh\necho "greetd $*"\n' > "$tmp/greetd"
chmod +x "$tmp/greetd"
cmd=$(sed -n "s/^ExecStart=\/bin\/sh -c '\(.*\)'$/\1/p" "$dropin")
[ -n "$cmd" ] || bad "drop-in ExecStart is not a /bin/sh -c line"
cmd=${cmd//\/usr\/bin\/greetd/$tmp\/greetd}
cmd=${cmd//\/etc\/uq\/greetd.toml/$tmp\/greetd.toml}
[ "$(sh -c "$cmd")" = "greetd " ] || bad "without uQ's config: $(sh -c "$cmd")"
: > "$tmp/greetd.toml"
[ "$(sh -c "$cmd")" = "greetd --config $tmp/greetd.toml" ] || bad "with uQ's config: $(sh -c "$cmd")"

has "$p/tmpfiles/uq-greeter.conf" 'd /var/lib/uq-greeter 0755 root root -'
has "$p/tmpfiles/uq-greeter.conf" 'd /var/cache/uq-greeter 0700 greeter greeter -'

pkg="$p/uq/PKGBUILD"
for want in 'packaging/uq-greeter-session "$pkgdir/usr/lib/uq/uq-greeter-session"' \
    'packaging/greeter/greetd.toml "$pkgdir/etc/uq/greetd.toml"' \
    'packaging/greeter/greeter.toml "$pkgdir/etc/uq/greeter.toml"' \
    'packaging/greeter/greetd-uq.conf "$pkgdir/usr/lib/systemd/system/greetd.service.d/uq.conf"' \
    'packaging/tmpfiles/uq-greeter.conf "$pkgdir/usr/lib/tmpfiles.d/uq-greeter.conf"' \
    'packaging/uq-session "$pkgdir/usr/bin/uq-session"' \
    'packaging/uq.desktop "$pkgdir/usr/share/wayland-sessions/uq.desktop"' \
    'find greeter -type f' 'for d in core logic auth; do ln -s "../shell/$d" "$pkgdir/usr/share/uq/greeter/$d"' \
    'ln -s ../shell/auth "$pkgdir/usr/share/uq/lock/auth"' 'render-themes'; do
    grep -qF -- "$want" "$pkg" || bad "PKGBUILD lacks: $want"
done
grep -q "^backup=(.*etc/uq/greetd.toml.*etc/uq/greeter.toml" "$pkg" || bad "PKGBUILD backup lacks the greeter configs"
grep -q "'greetd: " "$pkg" || bad "greetd is not an optdepend"
grep -q "^depends=.*greetd" "$pkg" && bad "greetd must stay optional"
# The symlinks the PKGBUILD recreates are the ones in the tree.
for d in core logic auth; do
    [ "$(readlink "$root/greeter/$d")" = "../shell/$d" ] || bad "greeter/$d is not a symlink to ../shell/$d"
    [ "$(readlink "$root/lock/$d")" = "../shell/$d" ] || bad "lock/$d is not a symlink to ../shell/$d"
done
has "$p/uq.desktop" 'Name=uQ'
has "$p/uq.desktop" 'Exec=/usr/bin/uq-session'

[ "$fail" = 0 ] && echo "greeter_packaging ok"
exit "$fail"
