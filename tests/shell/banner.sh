#!/usr/bin/env bash
# packaging/banner/uq-banner: prints the block-letter uQ, takes the accent from the login screen's
# theme, falls back to Graphite's, always exits 0; and the PKGBUILD installs the script and units.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
b="$root/packaging/banner/uq-banner"
fail=0
bad() { echo "FAIL: $*"; fail=1; }
t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
mkdir -p "$t/etc" "$t/share/themes/rendered/velvet-dark"
printf 'theme = "velvet"\nvariant = "dark"\n' > "$t/etc/greeter.toml"
printf '{"colors": {"accent": "#b48ead"}}\n' > "$t/share/themes/rendered/velvet-dark/tokens.json"

UQ_BANNER_OUT="$t/out" UQ_ETC="$t/etc" UQ_SHARE="$t/share" sh "$b" boot || bad "boot exited non-zero"
grep -q '▀████  ▀████▀▄' "$t/out" || bad "no block letters in the boot banner"
grep -q 'starting up' "$t/out" || bad "boot banner has no 'starting up'"
grep -q $'\033\\[38;2;180;142;173m' "$t/out" || bad "accent not taken from the theme's tokens"
UQ_BANNER_OUT="$t/out2" UQ_ETC="$t/none" UQ_SHARE="$t/none" sh "$b" shutdown || bad "shutdown exited non-zero"
grep -q 'shutting down' "$t/out2" || bad "shutdown banner has no 'shutting down'"
grep -q $'\033\\[38;2;138;180;248m' "$t/out2" || bad "fallback accent is not Graphite's"
UQ_BANNER_OUT=/nonexistent/dir/x sh "$b" boot || bad "an unwritable console must not fail"

for f in 'packaging/banner/uq-banner "$pkgdir/usr/lib/uq/uq-banner"' \
         'packaging/banner/uq-banner-boot.service "$pkgdir/usr/lib/systemd/system/uq-banner-boot.service"' \
         'packaging/banner/uq-banner-shutdown.service "$pkgdir/usr/lib/systemd/system/uq-banner-shutdown.service"'; do
    grep -qF -- "$f" "$root/packaging/uq/PKGBUILD" || bad "PKGBUILD does not install: $f"
done
# The units stay off while Settings' marker exists.
for u in boot shutdown; do
    grep -qx 'ConditionPathExists=!/etc/uq/banner-off' "$root/packaging/banner/uq-banner-$u.service" ||
        bad "uq-banner-$u.service doesn't honour /etc/uq/banner-off"
done
# uq.install enables the units on install and update, unless the marker exists.
inst="$root/packaging/uq/uq.install"
mkdir -p "$t/bin"
printf '#!/bin/sh\necho "$@" >> "%s/systemctl.log"\n' "$t" > "$t/bin/systemctl"; chmod +x "$t/bin/systemctl"
run_hook() { # $1 = hook, $2 = marker path to test with
    rm -f "$t/systemctl.log"
    sed "s#/etc/uq/banner-off#$2#" "$inst" > "$t/uq.install"
    PATH="$t/bin:$PATH" bash -c ". '$t/uq.install'; $1" > /dev/null
}
for hook in post_install post_upgrade; do
    run_hook "$hook" "$t/absent"
    grep -q 'enable --quiet uq-banner-boot.service uq-banner-shutdown.service' "$t/systemctl.log" 2>/dev/null ||
        bad "$hook doesn't enable the banner units"
    : > "$t/present"
    run_hook "$hook" "$t/present"
    [ ! -s "$t/systemctl.log" ] || bad "$hook enabled the banner although it was switched off"
done
[ "$fail" = 0 ] && echo "banner: ok"
exit "$fail"
