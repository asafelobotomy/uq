#!/usr/bin/env bash
# Tests for native/check-uq-native.sh: a clean repo passes, an edited or mis-pinned file fails,
# --sync brings a repo up to the kits' current versions, and the uq-diag code check works.
set -u
uq=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d); trap 'rm -r "$tmp"' EXIT
fails=0
ok() { if "$@" >/dev/null 2>&1; then echo "ok: $*"; else echo "FAIL: $*"; fails=$((fails + 1)); fi; }
no() { if "$@" >/dev/null 2>&1; then echo "FAIL (passed): $*"; fails=$((fails + 1)); else echo "ok (fails): $*"; fi; }

repo=$tmp/app; mkdir -p "$repo/src" "$repo/uq/diag" "$repo/scripts"
cp "$uq/native/check-uq-native.sh" "$repo/scripts/"
cp "$uq/native/uq-theme/uq_theme.rs" "$uq/native/uq-consts/UqThemeBase.qml" "$repo/src/"
v_theme=$(head -1 "$uq/native/uq-theme/uq_theme.rs" | grep -oE '[0-9.]+$')
v_consts=$(head -1 "$uq/native/uq-consts/uq_consts.rs" | grep -oE '[0-9.]+$')
cat > "$repo/uq/native.toml" <<TOML
# kits
[uq-theme]
version = "$v_theme"
files = ["src/uq_theme.rs"]

[uq-consts]
version = "$v_consts"   # the QML base
files = ["src/UqThemeBase.qml"]
TOML
cd "$repo" || exit 1
ok scripts/check-uq-native.sh

echo "// local edit" >> src/uq_theme.rs
no scripts/check-uq-native.sh
cp "$uq/native/uq-theme/uq_theme.rs" src/

sed -i "s/^version = \"$v_theme\"/version = \"0.0.1\"/" uq/native.toml
no scripts/check-uq-native.sh
ok scripts/check-uq-native.sh --sync "$uq"
ok grep -q "version = \"$v_theme\"" uq/native.toml
ok grep -q "version = \"$v_consts\"   # the QML base" uq/native.toml

printf '[uq-thing]\nversion = "1.0.0"\nfiles = ["x"]\n' > "$tmp/bad.toml"
no scripts/check-uq-native.sh "$tmp/bad.toml"
printf '[uq-theme]\nversion = "1.0.0"\nfiles = ["src/uq_theme.rs"]\nwhat = "x"\n' > "$tmp/bad2.toml"
no scripts/check-uq-native.sh "$tmp/bad2.toml"

# uq-diag's code check
cp "$uq/native/uq-diag/uq_diag.rs" src/
cat > uq/diag/app.toml <<'TOML'
[[code]]
id = "APP-001"
severity = "warning"
title = "t"
means = "m"
fix = "f"
TOML
echo 'log("[APP-001] x");' > src/main.rs
printf '\n[uq-diag]\nversion = "%s"\nfiles = ["src/uq_diag.rs"]\ncodes = "uq/diag/app.toml"\nsources = ["src"]\n' \
    "$(head -1 "$uq/native/uq-diag/uq_diag.rs" | grep -oE '[0-9.]+$')" >> uq/native.toml
ok scripts/check-uq-native.sh
echo 'log("[APP-002] y");' >> src/main.rs
no scripts/check-uq-native.sh

[ $fails = 0 ] && echo "check-uq-native: all tests passed"
exit $fails
