#!/usr/bin/env bash
# uQ Settings live checks: the parts of docs/notes/settings-slice-*-live-check.md that don't need a
# person, run against the uqd and Hyprland of the session this runs in. Each check id starts with
# its slice's letters; checks/<file>.sh says which items of its note stay manual, and why.
#
#   SA  checks/a-trust.sh       slice A: trust, History, restore points
#   SB  checks/b-profiles.sh    slice B: profiles
#   SC  checks/c-rules.sh       slice C: window, panel and workspace rules
#   SD  checks/d-startup.sh     slice D: startup apps, environment, apps and scaling
#   SG  checks/g-themes.sh      slice G: your own themes (.uqtheme import, export, delete)
#   SK  checks/k-keybinds.sh    Phase 3 slice 4: keyboard shortcuts
#   SF  checks/f-displays.sh    slice F: display arrangements, main display, workspace planner,
#                               touchpad gestures
#   SE  checks/e-companions.sh  slice E: pages from other apps (`uqctl integrations`)
#
#   tests/live/run.sh [--only REGEX] [--list] [--slow] [--yes]
#
#   --only RE   run only the checks whose id matches the extended regex RE (e.g. '^SB' or 'SA[13]')
#   --list      print the checks and what each needs, run nothing
#   --slow      also run the checks that wait for a clock (the light/dark schedule hold, 15 s
#               timers; ~5 min in all)
#   --yes       run on a real desktop without asking first
#
# Where it runs: a real uQ desktop, the test VM's guest (tests/vm/checks/60-settings.sh copies it
# there) and the CI sandbox (tests/live/sandbox.sh: a private bus and a headless Hyprland). A check
# that needs something this session lacks (Hyprland, systemd, root, power-profiles-daemon) is N/A.
#
# What it changes: settings, profiles, rules, shortcuts, display arrangements, user themes and
# files under ~/.config and ~/.local/share, all of which it puts back (the display checks also add
# a headless output with `hyprctl output create headless uqlive-hp`, and remove it again). Before the first check it saves a restore point ("Before the live checks")
# and copies user.lua; at the end it restores that point, puts user.lua back and removes every
# file and profile it made. History keeps the changes (marked "from Settings" or "from the
# terminal"), as it would after a manual check.
#
# Exit 0 when nothing FAILed, 1 when something did, 2 when it couldn't start. Results are in
# $UQ_LIVE_OUT (default a new directory under /tmp) as results.tsv, with the evidence as log.txt.
set -uo pipefail
LIVE_DIR=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib.sh
. "$LIVE_DIR/lib.sh"

ONLY='' LIST=0 YES=${UQ_LIVE_YES:-0}
while [ $# -gt 0 ]; do
    case "$1" in
        --only) ONLY=${2:?--only needs a pattern}; shift ;;
        --list) LIST=1 ;;
        --slow) export UQ_LIVE_SLOW=1 ;;
        --yes) YES=1 ;;
        -h|--help) sed -n '2,/^set -uo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
        *) die "unknown option $1" ;;
    esac
    shift
done

for f in "$LIVE_DIR"/checks/*.sh; do
    # shellcheck disable=SC1090
    . "$f"
done
if [ "$LIST" = 1 ]; then
    for id in "${CHECK_IDS[@]}"; do printf '%-6s %-28s %s\n' "$id" "${CHECK_NEEDS[$id]:--}" "${CHECK_TITLE[$id]}"; done
    exit 0
fi

for t in busctl jq "$UQCTL" sha256sum; do command -v "$t" >/dev/null || die "$t is not installed"; done
uqd_up || die "uqd isn't on the session bus (DBUS_SESSION_BUS_ADDRESS=${DBUS_SESSION_BUS_ADDRESS:-unset})"
if [ "$YES" != 1 ] && [ -z "${UQ_LIVE_SANDBOX:-}" ] && [ -t 0 ]; then
    echo "This changes your uQ settings for a few minutes and puts them back at the end (a restore"
    echo "point named \"Before the live checks\" is saved first). Run it? [y/N]"
    read -r a; [ "$a" = y ] || [ "$a" = Y ] || exit 2
fi

OUT=${UQ_LIVE_OUT:-$(mktemp -d /tmp/uq-live.XXXXXX)}
mkdir -p "$OUT"
RESULTS=$OUT/results.tsv
: >"$RESULTS"
WORK=$(mktemp -d /tmp/uq-live-work.XXXXXX)
export WORK RESULTS

# --- the bracket: save everything first, put it all back at the end -------------------------------
mkdir -p "$(dirname "$USER_LUA")"
[ -f "$USER_LUA" ] || touch "$USER_LUA"
cp -p "$USER_LUA" "$WORK/user.lua"
ls -A "$AUTOSTART_DIR" 2>/dev/null | sort >"$WORK/autostart.before"
ls -A "$APPS_DIR" 2>/dev/null | sort >"$WORK/apps.before"
START_POINT=$(bus SaveRestorePoint s "Before the live checks") || die "saving a restore point failed: $START_POINT"
START_THEME=$(bus GetTheme)
log "saved restore point $START_POINT; results in $OUT"

finish() {
    log "putting everything back"
    cleanup_check
    uqd_up || uqd_start
    cp -p "$WORK/user.lua" "$USER_LUA"
    local f
    # Files the checks added (uQ's own hidden copies and overrides go with the restore below).
    for f in $(ls -A "$AUTOSTART_DIR" 2>/dev/null | sort | comm -13 "$WORK/autostart.before" -); do
        case "$f" in "$LIVE_PREFIX"*) rm -f "${AUTOSTART_DIR:?}/${f:?}" ;; esac
    done
    rm -f "${APPS_DIR:?}/${LIVE_PREFIX:?}"*.desktop
    local r; r=$(bus Restore s "$START_POINT") || log "restoring $START_POINT failed: $r"
    hypr_up && hc reload >/dev/null
    log "restored $START_POINT (theme was $START_THEME, now $(bus GetTheme))"
    rm -rf "${WORK:?}"
}
trap finish EXIT
trap 'exit 130' INT TERM

detect
{
    for id in "${CHECK_IDS[@]}"; do
        [ -n "$ONLY" ] && ! [[ $id =~ $ONLY ]] && continue
        run_one "$id"
    done
} 2>&1 | tee "$OUT/log.txt"
print_summary
