#!/usr/bin/env bash
# Usage: check-companion-pages.sh UQCTL DIR...
# Runs `uqctl integrations check --no-programs` on each companion app's uq/ folder.
# A folder without pages/ is skipped with a note. Exits 1 if any folder fails.
set -u

if [ "$#" -lt 2 ]; then
    echo "usage: $0 UQCTL DIR..." >&2
    exit 2
fi
uqctl=$1
shift
if [ ! -x "$uqctl" ]; then
    echo "check-companion-pages: $uqctl is not executable" >&2
    exit 2
fi

fail=0
for dir in "$@"; do
    if [ ! -d "$dir/pages" ]; then
        echo "skip: $dir has no pages/ yet"
        continue
    fi
    echo "== $dir"
    if ! "$uqctl" integrations check --no-programs "$dir"; then
        echo "FAILED: $dir" >&2
        fail=1
    fi
done
exit "$fail"
