#!/usr/bin/env bash
# Usage: compat/clone.sh DEST latest|main
# Clones every repo in compat/family.toml into DEST/<repo name>: at its latest release tag
# (vX.Y.Z, the highest), or at main. Public repos, no credentials.
set -euo pipefail
dest=${1:?DEST}
at=${2:?latest or main}
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$dest"
repos=$(python3 -c 'import sys,tomllib; m=tomllib.load(open(sys.argv[1],"rb")); print("\n".join(sorted({p["repo"] for p in m["package"].values()})))' "$here/family.toml")
for repo in $repos; do
    url="https://github.com/$repo.git"
    ref=main
    if [ "$at" = latest ]; then
        ref=$(git ls-remote --tags --refs "$url" | sed -n 's#.*refs/tags/\(v[0-9][0-9.]*\)$#\1#p' | sort -V | tail -1)
        [ -n "$ref" ] || { echo "clone: $repo has no release tag" >&2; exit 1; }
    fi
    rm -rf "$dest/${repo#*/}"
    git -c advice.detachedHead=false clone -q --depth 1 --branch "$ref" "$url" "$dest/${repo#*/}"
    echo "clone: $repo $ref"
done
