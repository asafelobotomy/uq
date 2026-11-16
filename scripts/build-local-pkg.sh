#!/usr/bin/env bash
# Builds a uQ package from this checkout instead of the tagged release tarball.
#
#   scripts/build-local-pkg.sh [--package uq|uq-hyprland-pin] [--tarball FILE] [--keep] [makepkg args...]
#
# For `uq` it makes uq-<tag>.tar.gz from HEAD with `git archive --prefix=uq-<tag>/` (the same
# layout as the release asset; uncommitted changes are NOT included), or uses --tarball FILE,
# copies the PKGBUILD into a temporary directory with its source swapped for that file and the
# checksum set to SKIP, and runs makepkg there with the remaining arguments (e.g. -si).
# uq-hyprland-pin has no source; it is built from a copy of its PKGBUILD the same way.
#
# Packages land in $PKGDEST, default target/pkg/ in the checkout. The build directory is removed
# afterwards unless --keep is given.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
pkg=uq
tarball=
keep=0
while [ $# -gt 0 ]; do
    case "$1" in
        --package) pkg=${2:?--package needs a name}; shift 2 ;;
        --tarball) tarball=$(realpath "${2:?--tarball needs a file}"); shift 2 ;;
        --keep) keep=1; shift ;;
        --) shift; break ;;
        *) break ;;
    esac
done
case "$pkg" in
    uq|uq-hyprland-pin) ;;
    *) echo "build-local-pkg: unknown package '$pkg' (uq or uq-hyprland-pin)" >&2; exit 2 ;;
esac

src=$root/packaging/$pkg/PKGBUILD
work=$(mktemp -d "${TMPDIR:-/tmp}/uq-build-$pkg.XXXXXX")
cleanup() { if [ "$keep" = 1 ]; then echo "build-local-pkg: kept $work"; else rm -rf "$work"; fi; }
trap cleanup EXIT

export PKGDEST=${PKGDEST:-$root/target/pkg}
mkdir -p "$PKGDEST"

if [ "$pkg" = uq ]; then
    pkgver=$(sed -n 's/^pkgver=//p' "$src")
    tag=${pkgver/rc/-rc.}
    name=uq-$tag.tar.gz
    if [ -n "$tarball" ]; then
        cp "$tarball" "$work/$name"
    else
        if [ -n "$(git -C "$root" status --porcelain --untracked-files=no)" ]; then
            echo "build-local-pkg: note: uncommitted changes are not in the package (it builds HEAD)" >&2
        fi
        git -C "$root" archive --format=tar.gz --prefix="uq-$tag/" -o "$work/$name" HEAD
    fi
    # The local file replaces the release URL; it has no published checksum to check against.
    sed -e "s|^source=.*|source=(\"$name\")|" -e "s|^sha256sums=.*|sha256sums=('SKIP')|" "$src" >"$work/PKGBUILD"
    grep -q "^source=(\"$name\")\$" "$work/PKGBUILD" || { echo "build-local-pkg: could not swap the source line" >&2; exit 1; }
else
    cp "$src" "$work/PKGBUILD"
fi

# The PKGBUILD's `install=` script, when the package has one.
cp "$root/packaging/$pkg/"*.install "$work/" 2>/dev/null || true

cd "$work"
makepkg "$@"
echo "build-local-pkg: packages are in $PKGDEST"
