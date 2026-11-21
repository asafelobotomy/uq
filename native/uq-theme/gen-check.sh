#!/usr/bin/env bash
# Writes check-uq-theme.sh from check-uq-theme.sh.in with the current files' checksums added to
# those of earlier releases (kept in SUMS). Run after changing uq_theme.rs, uqtheme.h or uqtheme.cpp
# and bumping their version; CI fails when check-uq-theme.sh is out of date.
set -eu
cd "$(dirname "$0")"
v=$(head -n1 uq_theme.rs | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
touch SUMS
for f in uq_theme.rs uqtheme.h uqtheme.cpp; do
    grep -v "^\[$v/$f\]=" SUMS > SUMS.new || true; mv SUMS.new SUMS
    echo "[$v/$f]=$(sha256sum "$f" | cut -d' ' -f1)" >> SUMS
done
sort -o SUMS SUMS
awk 'NR==FNR { s = s "    " $0 "\n"; next } /^@SUMS@$/ { printf "%s", s; next } { print }' SUMS check-uq-theme.sh.in > check-uq-theme.sh
chmod +x check-uq-theme.sh
