#!/usr/bin/env bash
# Gate test for integrations (Phase 4 spec §7, success criteria 1-2), headless, on a private bus:
#   1. with an empty integration tree, `uqctl integrations list` and the bridge report nothing
#      and log no integration errors;
#   2. each fixture `uq/` folder (tests/fixtures/integrations/<name>/uq/: a widget, a settings
#      page, a launcher provider, an app action), installed into the tree as its package would,
#      appears while the bridge keeps running;
#   3. removed again, it goes.
# Never touches the user's session bus or /usr/share/uq.
set -u
root=$(cd "$(dirname "$0")/../.." && pwd)
uqctl=${UQCTL:-$root/target/debug/uqctl}
if [ ! -x "$uqctl" ]; then
    (cd "$root" && cargo build -q -p uqctl) || { echo "FAIL: couldn't build uqctl"; exit 1; }
fi
command -v jq >/dev/null || { echo "SKIP: no jq"; exit 0; }
tmp=$(mktemp -d)
bus_pid=""
br_pid=""
cleanup() {
    [ -n "$br_pid" ] && kill "$br_pid" 2>/dev/null
    [ -n "$bus_pid" ] && kill "$bus_pid" 2>/dev/null
    rm -rf "$tmp"
}
trap cleanup EXIT
fail=0
bad() { echo "FAIL: $*"; fail=1; }

tree="$tmp/share/uq"   # stands in for /usr/share/uq
mkdir -p "$tree"

# A private session bus (the bridge watches bus names); without dbus-daemon, no bus at all.
bus_addr="unix:path=$tmp/no-bus"
if command -v dbus-daemon >/dev/null; then
    cat > "$tmp/bus.conf" <<CONF
<busconfig><type>session</type><listen>unix:path=$tmp/bus</listen>
<policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/>
<allow own="*"/></policy></busconfig>
CONF
    dbus-daemon --nofork --print-address=3 --config-file="$tmp/bus.conf" 3>"$tmp/bus.addr" 2>/dev/null &
    bus_pid=$!
    for _ in $(seq 50); do [ -s "$tmp/bus.addr" ] && break; sleep 0.1; done
    [ -s "$tmp/bus.addr" ] && bus_addr=$(head -1 "$tmp/bus.addr")
fi

list() {
    DBUS_SESSION_BUS_ADDRESS=$bus_addr "$uqctl" integrations list --json --root "$tree" 2>"$tmp/list.err"
}

# --- 1. empty tree ---
out=$(list) || bad "integrations list failed on an empty tree"
[ "$out" = "[]" ] || bad "integrations list on an empty tree: $out"
[ -s "$tmp/list.err" ] && bad "integrations list wrote errors: $(cat "$tmp/list.err")"

mkfifo "$tmp/in"
env -u WAYLAND_DISPLAY -u HYPRLAND_INSTANCE_SIGNATURE DBUS_SESSION_BUS_ADDRESS="$bus_addr" \
    DBUS_SYSTEM_BUS_ADDRESS="$bus_addr" XDG_DATA_HOME="$tmp/data" \
    "$uqctl" bridge --root "$tree" <"$tmp/in" >"$tmp/out" 2>"$tmp/err" &
br_pid=$!
exec 3>"$tmp/in"   # keeps the bridge's stdin open

# The data of the newest `event` line matching the jq test `cond` (on .data), within 10 s.
await() {
    local event=$1 cond=$2
    for _ in $(seq "${AWAIT_TICKS:-100}"); do
        if [ -s "$tmp/out" ] && jq -e -s --arg e "$event" \
            "[.[] | select(.event == \$e)] | last | select(. != null) | .data | $cond" \
            "$tmp/out" >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.1
    done
    return 1
}

await Hello 'true' || bad "no Hello from the bridge"
await Widgets '. == []' || bad "Widgets is not [] for an empty tree"
await AppActions '. == []' || bad "AppActions is not [] for an empty tree"

# --- 2. install each fixture as its package would (spec §2.6) ---
install_uq() {
    local uq=$1 d f
    for d in "$uq"/widgets/*/; do [ -d "$d" ] && mkdir -p "$tree/shell/widgets" && cp -r "$d" "$tree/shell/widgets/"; done
    for d in "$uq"/pages/*/; do [ -d "$d" ] && mkdir -p "$tree/settings/pages" && cp -r "$d" "$tree/settings/pages/"; done
    for f in "$uq"/launcher/*.provider; do [ -f "$f" ] && mkdir -p "$tree/launcher" && cp "$f" "$tree/launcher/"; done
    for f in "$uq"/launcher/actions/*.appaction; do
        [ -f "$f" ] && mkdir -p "$tree/launcher/actions" && cp "$f" "$tree/launcher/actions/"
    done
    return 0
}
remove_uq() {
    local uq=$1 d f
    for d in "$uq"/widgets/*/; do [ -d "$d" ] && rm -rf "$tree/shell/widgets/$(basename "$d")"; done
    for d in "$uq"/pages/*/; do [ -d "$d" ] && rm -rf "$tree/settings/pages/$(basename "$d")"; done
    for f in "$uq"/launcher/*.provider; do [ -f "$f" ] && rm -f "$tree/launcher/$(basename "$f")"; done
    for f in "$uq"/launcher/actions/*.appaction; do [ -f "$f" ] && rm -f "$tree/launcher/actions/$(basename "$f")"; done
    return 0
}
listed() {   # kind id: valid and shown in `integrations list`
    list | jq -e --arg k "$1" --arg i "$2" 'any(.[]; .kind == $k and .id == $i and .valid and .visible)' >/dev/null
}

fixtures="$root/tests/fixtures/integrations"
for name in gate-widget gate-page gate-provider gate-action; do
    uq="$fixtures/$name/uq"
    [ -d "$uq" ] || { bad "fixture $name missing"; continue; }
    install_uq "$uq"
    case $name in
        gate-widget)
            await Widgets 'any(.[]; .id == "gate-widget" and .visible and .slot == "bar")' \
                || bad "the bridge didn't report gate-widget after install"
            listed widget gate-widget || bad "integrations list doesn't show gate-widget" ;;
        gate-page)
            listed page gate-page || bad "integrations list doesn't show gate-page" ;;
        gate-provider)
            listed provider org.uq.test.GateProvider || bad "integrations list doesn't show the gate provider" ;;
        gate-action)
            await AppActions 'any(.[]; .id == "gate-action" and .name == "Gate action")' \
                || bad "the bridge didn't report gate-action after install"
            listed app_action gate-action || bad "integrations list doesn't show gate-action" ;;
    esac
done

# --- 3. remove them again ---
for name in gate-widget gate-page gate-provider gate-action; do
    remove_uq "$fixtures/$name/uq"
done
await Widgets '. == []' || bad "gate-widget still reported after removal"
await AppActions '. == []' || bad "gate-action still reported after removal"
out=$(list)
[ "$out" = "[]" ] || bad "integrations list after removal: $out"

# No integration errors in the bridge's log (environment lines like logind's are fine).
if grep -Ei 'widget|app action|skipped|panicked' "$tmp/err"; then
    bad "the bridge logged integration errors"
fi
kill -0 "$br_pid" 2>/dev/null || bad "the bridge exited"

[ -n "${SHOW:-}" ] && { echo "--- out"; cat "$tmp/out"; echo "--- err"; cat "$tmp/err"; echo "bus=$bus_addr"; }
[ "$fail" = 0 ] && echo "integrations_gate: ok"
exit "$fail"
