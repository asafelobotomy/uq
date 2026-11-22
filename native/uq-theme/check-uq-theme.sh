#!/usr/bin/env bash
# check-uq-theme.sh <pinned-version> <vendored file>...
# Drift check for vendored uq-theme files (uq_theme.rs, uqtheme.h/.cpp, uqkdetheme.h/.cpp,
# uq_theme.py): each file's `uq-theme X.Y.Z` header must be the pinned version and its contents exactly that release's
# (the checksums below), so a local edit shows up in CI. Copy it into a consuming repo as
# scripts/check-uq-theme.sh. Exit 0 when all match, 1 on any mismatch or missing file, 2 on bad usage.
set -u
declare -A SUMS=(
    [1.0.0/uq_theme.rs]=1d7f38189426099831438fda3541e42f02ab282f1df54bafaf54f7f9d89c9708
    [1.0.0/uqtheme.cpp]=735f513fd3a54ad7b11e0970e2d7aa906a895e843bdca40ac21d8ba533f6685a
    [1.0.0/uqtheme.h]=4b3717e1708c131c6f164a992b8f846f97d5fa0d6b655ba0c649984333847940
    [1.1.0/uq_theme.py]=768d750f2f9766739b53374a182a38da6c0ce39ede36f32172b10c6e49643658
    [1.1.0/uq_theme.rs]=fa23bed661705a97898dbdcd3d2358a17e222219768b7597453812da76e87ef3
    [1.1.0/uqkdetheme.cpp]=8e5abe5a6ed4c92211cacaf6c65d6347911dee3bfcd0a87c70c11b8fb44c11b8
    [1.1.0/uqkdetheme.h]=db8ea34b1b3ae221cedef91bb49f8d216f3207c2b24fcafd89f3616cccd77eec
    [1.1.0/uqtheme.cpp]=4288d8434201fd06ec41045c8da02719beca81603e425bf055c33c56fb1ce64a
    [1.1.0/uqtheme.h]=9b4fa4044dea6dd4d747df2bd65daf4f85bea7954b1cdfb62244b61b1b41afb1
)
if [ $# -lt 2 ] || ! [[ $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "usage: check-uq-theme.sh <pinned-version X.Y.Z> <vendored file>..." >&2
    exit 2
fi
pinned=$1; shift
status=0
for file in "$@"; do
    name=$(basename "$file")
    if [ ! -r "$file" ]; then
        echo "check-uq-theme: cannot read $file" >&2; status=1; continue
    fi
    found=$(head -n 3 "$file" | grep -m1 -oE 'uq-theme [0-9]+\.[0-9]+\.[0-9]+' | cut -d' ' -f2)
    if [ "$found" != "$pinned" ]; then
        echo "check-uq-theme: $file is uq-theme ${found:-?}, pinned $pinned" >&2; status=1; continue
    fi
    want=${SUMS[$pinned/$name]:-}
    have=$(sha256sum "$file" | cut -d' ' -f1)
    if [ -z "$want" ]; then
        echo "check-uq-theme: this script does not know uq-theme $pinned $name; copy the newer script" >&2; status=1
    elif [ "$want" != "$have" ]; then
        echo "check-uq-theme: $file was edited (it is not uq-theme $pinned's $name); copy it again from uQ" >&2; status=1
    else
        echo "check-uq-theme: $file is uq-theme $found"
    fi
done
exit $status
