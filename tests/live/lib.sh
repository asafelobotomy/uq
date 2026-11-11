# shellcheck shell=bash
# shellcheck disable=SC2034 # the paths and the framework state are read by the checks
# uQ Settings live checks: the framework and the helpers the checks share. Sourced by run.sh.
#
# Everything here runs inside a uQ session (the real desktop, the test VM's guest or the CI
# sandbox from sandbox.sh): it talks to uqd on the session bus, to Hyprland through hyprctl and to
# the files uQ writes. A check function calls:
#   note TEXT   a line of evidence
#   bad TEXT    a failed expectation ("expected ...; observed ..."); the check FAILs
#   na TEXT     the check can't run here (reason); the check is N/A
# A check with neither bad nor na PASSes. Each check is registered with
#   check ID "title" FUNCTION [NEEDS...]
# where NEEDS are capabilities (hypr systemd root ppd restart slow apps) it can't run without; a
# check whose needs this session lacks is N/A without running.

UQCTL=${UQCTL:-uqctl}
LIVE_PREFIX=uqlive

log() { printf '[uq-live %(%H:%M:%S)T] %s\n' -1 "$*" >&2; }
die() { log "ERROR: $*"; exit 2; }

# --- uqd and Hyprland ---------------------------------------------------------------------------
UQ_BUS=(org.uq.Desktop1 /org/uq/Desktop1 org.uq.Desktop1)
# bus METHOD SIG ARGS...: one uqd call; prints the reply's first value, or the error text. The exit
# status is busctl's (non-zero for an error reply).
bus() {
    local out rc
    out=$(busctl --user --json=short call "${UQ_BUS[@]}" "$@" 2>&1)
    rc=$?
    jq -Rr 'fromjson? // . | if type=="object" then (.data[0] // "") else . end' <<<"$out"
    return "$rc"
}
uqd_up() { busctl --user status org.uq.Desktop1 >/dev/null 2>&1; }
section() { bus GetSection s "$1"; }
# set_section DOMAIN JSON [ORIGIN]: prints nothing on success, the error otherwise.
set_section() { bus SetSection sss "$1" "$2" "${3:-settings}"; }
set_setting() { bus SetSetting sss "$1" "$2" "${3:-settings}"; }
explain() { bus Explain s "$1"; }
# src DOMAIN KEY: where KEY's live value comes from (Explain's source).
src() { explain "$1" | jq -r --arg k "$2" '.[$k].source'; }
history_json() { bus ListHistory u "${1:-20}"; }
newest() { history_json 1 | jq -c '.[0]'; }
newest_id() { history_json 1 | jq -r '.[0].id'; }
points() { bus ListRestorePoints; }
hc() { hyprctl "$@"; }
hypr_up() { hyprctl version -j >/dev/null 2>&1; }
config_errors() { hyprctl configerrors 2>/dev/null | grep -v '^\s*$'; }
# wait_for SECS CMD...: poll CMD until it succeeds or SECS (wall clock) have passed.
wait_for() {
    local end; end=$(($(date +%s%N) + ${1%.*} * 1000000000)); shift
    while :; do
        "$@" && return 0
        [ "$(date +%s%N)" -lt "$end" ] || return 1
        sleep 0.3
    done
}

# uqd's lifecycle: systemd on a real session and in the VM; sandbox.sh sets UQ_LIVE_UQDCTL to its
# own `stop|start` script.
uqd_stop() {
    if [ -n "${UQ_LIVE_UQDCTL:-}" ]; then "$UQ_LIVE_UQDCTL" stop; else systemctl --user stop uqd; fi
    wait_for 10 uqd_down
}
uqd_down() { ! uqd_up; }
uqd_start() {
    if [ -n "${UQ_LIVE_UQDCTL:-}" ]; then "$UQ_LIVE_UQDCTL" start; else systemctl --user start uqd; fi
    wait_for 20 uqd_up
}
uqd_restart() { uqd_stop && uqd_start; }

# Paths (uqd's own: XDG_CONFIG_HOME and XDG_STATE_HOME as the session has them).
CFG=${XDG_CONFIG_HOME:-$HOME/.config}
STATE_DIR=${XDG_STATE_HOME:-$HOME/.local/state}/uq
USER_LUA=$CFG/hypr/uq/user.lua
APPS_DIR=${XDG_DATA_HOME:-$HOME/.local/share}/applications
AUTOSTART_DIR=$CFG/autostart

# user.lua edits by a check (uQ itself never writes it): append, then take back.
user_lua_add() { printf '%s\n' "$1" >>"$USER_LUA"; wc -l <"$USER_LUA"; }
user_lua_reset() { cp -p "$WORK/user.lua" "$USER_LUA"; hypr_up && hc reload >/dev/null; }

# --- capabilities -------------------------------------------------------------------------------
declare -A HAVE
detect() {
    hypr_up && HAVE[hypr]=1
    timeout 5 systemctl --user show-environment >/dev/null 2>&1 && [ -z "${UQ_LIVE_UQDCTL:-}" ] && HAVE[systemd]=1
    sudo -n true 2>/dev/null && HAVE[root]=1
    busctl --system status net.hadess.PowerProfiles >/dev/null 2>&1 && HAVE[ppd]=1
    # A VM's power-profiles-daemon often has no platform driver, so it offers no "performance".
    [ -n "${HAVE[ppd]:-}" ] && [[ $(busctl --system --json=short get-property net.hadess.PowerProfiles \
        /net/hadess/PowerProfiles net.hadess.PowerProfiles Profiles 2>/dev/null) == *'"performance"'* ]] && HAVE[ppd_perf]=1
    { [ -n "${UQ_LIVE_UQDCTL:-}" ] || [ -n "${HAVE[systemd]:-}" ]; } && HAVE[restart]=1
    [ -n "${UQ_LIVE_SLOW:-}" ] && HAVE[slow]=1
    [ -n "${UQ_LIVE_SANDBOX:-}" ] && HAVE[sandbox]=1
    command -v foot >/dev/null && HAVE[foot]=1
    log "this session has: ${!HAVE[*]}"
}

# --- the framework ------------------------------------------------------------------------------
CHECK_IDS=()
declare -A CHECK_TITLE CHECK_FN CHECK_NEEDS
check() {
    local id=$1 title=$2 fn=$3; shift 3
    CHECK_IDS+=("$id"); CHECK_TITLE[$id]=$title; CHECK_FN[$id]=$fn; CHECK_NEEDS[$id]="$*"
}
note() { printf '  %s\n' "$*"; }
bad() { CUR_FAIL+=("$*"); printf '  FAIL: %s\n' "$*"; }
na() { CUR_NA=$*; printf '  N/A: %s\n' "$*"; }
# expect_eq WHAT WANT GOT
expect_eq() { [ "$3" = "$2" ] || bad "$1: expected $2; observed ${3:-(nothing)}"; }

lua_sum() { sha256sum "$USER_LUA" 2>/dev/null | cut -c1-16; }

run_one() {
    local id=$1 n
    CUR_FAIL=() CUR_NA=""
    printf '== %s: %s\n' "$id" "${CHECK_TITLE[$id]}"
    for n in ${CHECK_NEEDS[$id]}; do
        [ -n "${HAVE[$n]:-}" ] || { na "needs $n, which this session doesn't have"; break; }
    done
    if [ -z "$CUR_NA" ]; then
        uqd_up || uqd_start || bad "uqd is not on the bus"
        local before; before=$(lua_sum)
        "${CHECK_FN[$id]}"
        # uQ never writes user.lua; a check that edits it puts it back itself.
        [ "$(lua_sum)" = "$before" ] || { bad "user.lua changed during the check"; user_lua_reset; }
        cleanup_check
    fi
    local status reason
    if [ -n "$CUR_NA" ]; then status=N/A reason=$CUR_NA
    elif [ "${#CUR_FAIL[@]}" -gt 0 ]; then status=FAIL reason=$(IFS='|'; echo "${CUR_FAIL[*]}")
    else status=PASS reason=
    fi
    printf 'RESULT\t%s\t%s\t%s\n' "$id" "$status" "$reason"
    printf '%s\t%s\t%s\t%s\n' "$id" "$status" "${CHECK_TITLE[$id]}" "$reason" >>"$RESULTS"
}

# cleanup_check: the session back to normal between checks (no profile on, no test profiles).
cleanup_check() {
    local p
    bus ProfileOff >/dev/null 2>&1
    for p in $(bus ListProfiles | jq -r '.[].id' 2>/dev/null | grep "^$LIVE_PREFIX"); do
        bus DeleteProfile s "$p" >/dev/null 2>&1
    done
    pkill -f "uqlive-win" 2>/dev/null
    return 0
}

print_summary() {
    local p f n
    p=$(awk -F'\t' '$2=="PASS"' "$RESULTS" | wc -l)
    f=$(awk -F'\t' '$2=="FAIL"' "$RESULTS" | wc -l)
    n=$(awk -F'\t' '$2=="N/A"' "$RESULTS" | wc -l)
    echo
    printf '%-6s %-5s %s\n' ID RESULT CHECK
    awk -F'\t' '{ printf "%-6s %-5s %s\n", $1, $2, $3; if ($4 != "") { gsub(/\|/, "\n                ", $4); printf "             %s\n", $4 } }' "$RESULTS"
    echo
    echo "PASS $p  FAIL $f  N/A $n"
    [ "$f" -eq 0 ]
}

# --- test windows -------------------------------------------------------------------------------
# open_win CLASS [TITLE]: a foot window with that app id, in the background; waits until Hyprland
# lists it.
open_win() {
    local cls=$1 title=${2:-uqlive-win}
    setsid foot --app-id "$cls" --title "$title" sh -c 'sleep 600' >/dev/null 2>&1 &
    wait_for 10 win_listed "$cls"
}
win_listed() { [ -n "$(win "$1")" ]; }
# win CLASS: hyprctl's client JSON for the first window of CLASS.
win() { hc clients -j 2>/dev/null | jq -c --arg c "$1" 'first(.[] | select(.class == $c)) // empty'; }
close_win() { pkill -f -- "--app-id $1 " 2>/dev/null; wait_for 5 win_gone "$1"; }
win_gone() { ! win_listed "$1"; }
