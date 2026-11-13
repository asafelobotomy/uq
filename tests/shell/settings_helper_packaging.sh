#!/usr/bin/env bash
# Packaging of uq-settings-helper, uQ's only root-side code: its systemd unit keeps the hardening
# from the Phase 3 spec (§3), the D-Bus files let only root own the name, the polkit actions keep
# auto-login uncached, and the PKGBUILD builds and installs every file.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
fail=0
no() { echo "FAIL: $*"; fail=1; }

unit="$root/packaging/uq-settings-helper.service"
svc="$root/packaging/dbus/org.uq.SettingsHelper1.service"
conf="$root/packaging/dbus/org.uq.SettingsHelper1.conf"
policy="$root/packaging/polkit/org.uq.settings.policy"
tmpfiles="$root/packaging/tmpfiles/uq-greeter.conf"
pkgbuild="$root/packaging/uq/PKGBUILD"
for f in "$unit" "$svc" "$conf" "$policy" "$tmpfiles"; do
    [ -f "$f" ] || { no "${f#"$root"/} missing"; }
done
[ "$fail" = 0 ] || exit 1

# --- systemd unit ---------------------------------------------------------------------------
# D-Bus activation only: no [Install] section.
grep -q '^\[Install\]' "$unit" && no "uq-settings-helper.service has an [Install] section"
for want in \
    '^Type=dbus$' \
    '^BusName=org\.uq\.SettingsHelper1$' \
    '^ExecStart=/usr/lib/uq/uq-settings-helper$' \
    '^ProtectSystem=strict$' \
    '^ReadWritePaths=-/etc/systemd/logind\.conf\.d -/etc/uq -/var/lib/uq-greeter$' \
    '^ProtectHome=yes$' \
    '^PrivateNetwork=yes$' \
    '^NoNewPrivileges=yes$' \
    '^CapabilityBoundingSet=$' \
    '^AmbientCapabilities=$' \
    '^RestrictAddressFamilies=AF_UNIX$' \
    '^PrivateTmp=yes$' \
    '^PrivateDevices=yes$' \
    '^RestrictSUIDSGID=yes$' \
    '^MemoryDenyWriteExecute=yes$' \
    '^SystemCallArchitectures=native$' \
    '^SystemCallFilter=@system-service$' \
    '^SystemCallFilter=~@privileged$'; do
    grep -q "$want" "$unit" || no "uq-settings-helper.service lacks $want"
done
# Exactly one ReadWritePaths line, and no other way to widen what it may write.
[ "$(grep -c '^ReadWritePaths=' "$unit")" = 1 ] || no "uq-settings-helper.service has more than one ReadWritePaths"
grep -qE '^(ReadOnlyPaths|BindPaths|InaccessiblePaths|ProtectSystem=(no|yes|full)|ProtectHome=(no|read-only|tmpfs)|User=[^r])' "$unit" &&
    no "uq-settings-helper.service loosens the sandbox"
if command -v systemd-analyze >/dev/null 2>&1; then
    # Verify needs the binary to exist; only unit syntax problems matter here.
    out=$(systemd-analyze verify "$unit" 2>&1 | grep -v -e 'Command .* is not executable' -e 'No such file' -e '^$') || true
    case "$out" in *"Unknown"*|*"Invalid"*|*"Failed to parse"*) no "verify uq-settings-helper.service: $out";; esac
fi

# --- D-Bus activation file and bus policy ----------------------------------------------------
for want in '^\[D-BUS Service\]$' '^Name=org\.uq\.SettingsHelper1$' '^Exec=/usr/lib/uq/uq-settings-helper$' \
    '^User=root$' '^SystemdService=uq-settings-helper\.service$'; do
    grep -q "$want" "$svc" || no "org.uq.SettingsHelper1.service lacks $want"
done
if command -v xmllint >/dev/null 2>&1; then
    xmllint --noout "$conf" || no "org.uq.SettingsHelper1.conf isn't well-formed XML"
    xmllint --noout "$policy" || no "org.uq.settings.policy isn't well-formed XML"
    # Only root may own the name.
    owners=$(xmllint --xpath 'count(//allow[@own])' "$conf")
    root_owner=$(xmllint --xpath 'count(//policy[@user="root"]/allow[@own="org.uq.SettingsHelper1"])' "$conf")
    [ "$owners" = 1 ] && [ "$root_owner" = 1 ] || no "org.uq.SettingsHelper1.conf lets someone other than root own the name"
    # Anyone may call the helper's interface; nothing in the default policy is broader than that.
    calls=$(xmllint --xpath 'count(//policy[@context="default"]/allow[@send_destination="org.uq.SettingsHelper1" and @send_interface="org.uq.SettingsHelper1"])' "$conf")
    [ "$calls" = 1 ] || no "org.uq.SettingsHelper1.conf doesn't let users call the interface"
    broad=$(xmllint --xpath 'count(//allow[not(@send_destination) or not(@send_interface)][not(@own)])' "$conf")
    [ "$broad" = 0 ] || no "org.uq.SettingsHelper1.conf has a send rule without a destination and interface"

    # The polkit actions and their defaults.
    def() { xmllint --xpath "string(//action[@id=\"$1\"]/defaults/$2)" "$policy"; }
    for a in org.uq.settings.power org.uq.settings.greeter; do
        [ "$(def "$a" allow_active)" = auth_admin_keep ] || no "$a allow_active isn't auth_admin_keep"
        [ "$(def "$a" allow_any)" = auth_admin ] || no "$a allow_any isn't auth_admin"
        [ "$(def "$a" allow_inactive)" = auth_admin ] || no "$a allow_inactive isn't auth_admin"
    done
    for k in allow_any allow_inactive allow_active; do
        [ "$(def org.uq.settings.autologin "$k")" = auth_admin ] || no "org.uq.settings.autologin $k isn't auth_admin (never kept)"
    done
    [ "$(xmllint --xpath 'count(//action)' "$policy")" = 3 ] || no "org.uq.settings.policy should have exactly 3 actions"
else
    grep -q '<allow own="org.uq.SettingsHelper1"/>' "$conf" || no "org.uq.SettingsHelper1.conf lacks the root owner rule"
fi
grep -q '<allow_active>yes</allow_active>\|<allow_any>yes</allow_any>\|<allow_inactive>yes</allow_inactive>' "$policy" &&
    no "org.uq.settings.policy allows an action without authentication"

# --- tmpfiles.d -------------------------------------------------------------------------------
grep -qx 'd /var/lib/uq-greeter 0755 root root -' "$tmpfiles" || no "tmpfiles lacks /var/lib/uq-greeter (root 0755)"
grep -qE '^d /var/cache/uq-greeter 0[0-7]{3} greeter greeter -$' "$tmpfiles" || no "tmpfiles lacks /var/cache/uq-greeter (greeter)"

# --- PKGBUILD -------------------------------------------------------------------------------
grep -q '^  cargo build .*-p uq-settings-helper' "$pkgbuild" || no "PKGBUILD doesn't build uq-settings-helper"
# The depends=( ... ) array, which may span lines; polkit must be one of its words.
sed -n '/^depends=(/,/)/p' "$pkgbuild" | tr -s ' ()\t' '\n' | grep -qx polkit || no "PKGBUILD doesn't depend on polkit"
for line in \
    'install -Dm755 target/release/uq-settings-helper "$pkgdir/usr/lib/uq/uq-settings-helper"' \
    'install -Dm644 packaging/dbus/org.uq.SettingsHelper1.service "$pkgdir/usr/share/dbus-1/system-services/org.uq.SettingsHelper1.service"' \
    'install -Dm644 packaging/dbus/org.uq.SettingsHelper1.conf "$pkgdir/usr/share/dbus-1/system.d/org.uq.SettingsHelper1.conf"' \
    'install -Dm644 packaging/uq-settings-helper.service "$pkgdir/usr/lib/systemd/system/uq-settings-helper.service"' \
    'install -Dm644 packaging/polkit/org.uq.settings.policy "$pkgdir/usr/share/polkit-1/actions/org.uq.settings.policy"' \
    'install -dm755 "$pkgdir/etc/uq" "$pkgdir/etc/systemd/logind.conf.d"'; do
    grep -qF "  $line" "$pkgbuild" || no "PKGBUILD lacks: $line"
done

[ "$fail" = 0 ] && echo "settings helper packaging ok"
exit "$fail"
