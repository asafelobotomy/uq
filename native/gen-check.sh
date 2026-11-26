#!/usr/bin/env bash
# Writes check-uq-native.sh from check-uq-native.sh.in with every kit's checksums: the current
# files' sums are added to each kit's SUMS (which keeps earlier releases'), so the script knows
# every release. Run after changing a kit's files and bumping its version (native/uq-consts:
# run its gen.py first). CI fails when check-uq-native.sh or a SUMS file is out of date.
set -eu
cd "$(dirname "$0")/.."
# kit  folder  file that carries the version  vendored files
kits=(
    "uq-theme native/uq-theme uq_theme.rs uq_theme.rs uqtheme.h uqtheme.cpp uqkdetheme.h uqkdetheme.cpp uq_theme.py"
    "uq-diag native/uq-diag uq_diag.rs uq_diag.rs uqdiag.h uqdiag.cpp"
    "uq-consts native/uq-consts uq_consts.rs uq_consts.rs uqconsts.h uq_consts.py uq-consts.js uq-consts.sh UqThemeBase.qml"
    "uq-tokens web/uq-tokens uq-tokens.js uq-tokens.js uq-tokens.css"
    "uq-loc native/uq-loc check-loc.sh check-loc.sh"
)
all=""
for k in "${kits[@]}"; do
    # shellcheck disable=SC2086
    set -- $k
    kit=$1 dir=$2 head=$3; shift 3
    v=$(head -n 5 "$dir/$head" | grep -m1 -oE "$kit [0-9]+\.[0-9]+\.[0-9]+" | cut -d' ' -f2)
    [ -n "$v" ] || { echo "gen-check: no '$kit X.Y.Z' header in $dir/$head" >&2; exit 1; }
    touch "$dir/SUMS"
    for f in "$@"; do
        h=$(head -n 5 "$dir/$f" | grep -m1 -oE "$kit [0-9]+\.[0-9]+\.[0-9]+" | cut -d' ' -f2)
        [ "$h" = "$v" ] || { echo "gen-check: $dir/$f is $kit ${h:-?}, $head is $v" >&2; exit 1; }
        grep -v "^\[$v/$f\]=" "$dir/SUMS" > "$dir/SUMS.new" || true
        mv "$dir/SUMS.new" "$dir/SUMS"
        echo "[$v/$f]=$(sha256sum "$dir/$f" | cut -d' ' -f1)" >> "$dir/SUMS"
    done
    sort -o "$dir/SUMS" "$dir/SUMS"
    all+=$(sed "s/^\[/[$kit\//" "$dir/SUMS")$'\n'
done
printf '%s' "$all" | awk 'NR==FNR { s = s "    " $0 "\n"; next } /^@SUMS@$/ { printf "%s", s; next } { print }' - native/check-uq-native.sh.in > native/check-uq-native.sh
chmod +x native/check-uq-native.sh native/gen-check.sh
