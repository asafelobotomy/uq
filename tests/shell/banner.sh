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
[ "$fail" = 0 ] && echo "banner: ok"
exit "$fail"
