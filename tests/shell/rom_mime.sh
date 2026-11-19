#!/usr/bin/env bash
# uQ's ROM file types: the generated files are current, every icon exists, every glob resolves to
# the type it names (through update-mime-database, as pacman's hook runs it), and nothing claims an
# extension that means something else too.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
fail=0
bad() { echo "FAIL: $*"; fail=1; }

# 1. packaging/mime matches the generator.
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
cp -r "$root/packaging/mime" "$tmp/before"
python3 "$root/scripts/gen-rom-mime.py" >/dev/null || bad "gen-rom-mime.py failed"
python3 - "$tmp/before" "$root/packaging/mime" <<'P' || {
import filecmp, sys


def same(c):
    return not (c.left_only or c.right_only or c.diff_files or c.funny_files) and all(same(d) for d in c.subdirs.values())


sys.exit(0 if same(filecmp.dircmp(sys.argv[1], sys.argv[2])) else 1)
P

    bad "packaging/mime is stale: run python3 scripts/gen-rom-mime.py"
    rm -rf "$root/packaging/mime" && cp -r "$tmp/before" "$root/packaging/mime"
}

xml="$root/packaging/mime/uq-roms.xml"
upstream=/usr/share/mime/packages/freedesktop.org.xml
if [ ! -f "$upstream" ] || ! command -v update-mime-database >/dev/null 2>&1; then
    echo "skip: shared-mime-info missing"; [ "$fail" = 0 ] && echo "rom mime (generated files only) ok"; exit "$fail"
fi

# 2. Every <icon> has a file, and the file is a well-formed SVG.
for name in $(grep -o '<icon name="[^"]*"' "$xml" | cut -d'"' -f2 | sort -u); do
    f="$root/packaging/mime/icons/$name.svg"
    [ -f "$f" ] || { bad "$name.svg missing"; continue; }
    python3 -c 'import sys,xml.dom.minidom as m; m.parse(sys.argv[1])' "$f" 2>/dev/null || bad "$name.svg is not well-formed"
done

# 3. Build a private database like the pacman hook does and resolve each glob.
db="$tmp/share/mime"; mkdir -p "$db/packages"
cp "$upstream" "$xml" "$db/packages/"
update-mime-database "$db" >/dev/null 2>&1 || bad "update-mime-database rejected uq-roms.xml"
n=0
while IFS=' ' read -r type ext; do
    got=$(grep -F ":*.$ext" "$db/globs2" | grep -F ":$type:" | head -1)
    [ -n "$got" ] || bad "*.$ext does not resolve to $type"
    n=$((n + 1))
done < <(python3 - "$xml" <<'P'
import sys, xml.etree.ElementTree as ET
ns = {"m": "http://www.freedesktop.org/standards/shared-mime-info"}
for t in ET.parse(sys.argv[1]).getroot().findall("m:mime-type", ns):
    for g in t.findall("m:glob", ns):
        print(t.get("type"), g.get("pattern")[2:])
P
)
[ "$n" -gt 50 ] || bad "only $n globs checked"
# Every type gets its themed icon in the built database.
for name in $(grep -o '<icon name="[^"]*"' "$xml" | cut -d'"' -f2 | sort -u); do
    grep -q ":$name\$" "$db/icons" || bad "$name not in the database's icons file"
done

# 4. Ambiguous extensions stay unclaimed.
for ext in bin iso rom md st img o p 0; do
    grep -q "pattern=\"\*\.$ext\"" "$xml" && bad "uq-roms.xml claims the ambiguous *.$ext"
done

# 5. The package installs both.
grep -q 'packaging/mime/uq-roms.xml' "$root/packaging/uq/PKGBUILD" || bad "PKGBUILD does not install uq-roms.xml"
grep -q 'packaging/mime/icons/\*.svg' "$root/packaging/uq/PKGBUILD" || bad "PKGBUILD does not install the ROM icons"

[ "$fail" = 0 ] && echo "rom mime ok ($n globs)"
exit "$fail"
