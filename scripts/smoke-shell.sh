#!/usr/bin/env bash
# Smoke test: the shell on a real (headless) Hyprland.
#
#   - the uq-bar layer appears within 10 s
#   - qs ipc call uq open/close spotlight maps and unmaps the uq-spotlight layer
#   - Dispatch focus_window / move against a real window moves it (hyprctl clients -j)
#   - smoke_launch_survives_restart: a Launch'd app scope outlives a shell restart
#   - hot-plug: a new headless output gets a bar, and removing it removes the bar
#
# Exit 0: the bar check ran and every check that could run passed. Exit 77: skipped (no headless
# Hyprland here, or the compositor cannot host layer surfaces; a broken shell is a failure, not a skip); CI prints a notice and carries on.
# A sub-check that cannot run in this environment (no systemd user manager, no Wayland client)
# is skipped with a message and does not fail the run.
#
# Never touches the real session: the bridge and the shell get a private D-Bus daemon (so no uqd
# and no real settings), and Hyprland gets a private XDG_RUNTIME_DIR.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)

skip() { echo "SKIP: $*"; exit 77; }
for bin in Hyprland hyprctl qs dbus-daemon jq; do
    command -v "$bin" >/dev/null 2>&1 || skip "$bin is not installed"
done
uqctl=${UQCTL:-}
[ -z "$uqctl" ] && [ -x "$root/target/debug/uqctl" ] && uqctl="$root/target/debug/uqctl"
[ -z "$uqctl" ] && uqctl=$(command -v uqctl) || true
[ -n "$uqctl" ] && [ -x "$uqctl" ] || skip "no uqctl binary (run cargo build, or set UQCTL)"

# Short paths: unix socket names must stay under 108 bytes.
tmp=$(mktemp -d /tmp/uqs.XXXXXX)
rt="$tmp/rt"
mkdir -m 700 "$rt"
orig_rt=${XDG_RUNTIME_DIR:-}
pids=()
scope=""
cleanup() {
    [ -n "$scope" ] && systemctl --user stop "$scope" >/dev/null 2>&1
    for p in "${pids[@]}"; do kill "$p" 2>/dev/null; done
    wait 2>/dev/null
    rm -rf "$tmp"
}
trap cleanup EXIT
fail() { echo "FAIL: $*"; for f in hypr qs; do [ -f "$tmp/$f.log" ] && { echo "--- $f.log"; tail -n 30 "$tmp/$f.log"; }; done; exit 1; }

# Private bus for the shell and the bridge: uqd is absent, the real bus is never reached.
dbus-daemon --session --address="unix:path=$tmp/bus" --nofork --nopidfile >"$tmp/dbus.log" 2>&1 &
pids+=($!)
bus="unix:path=$tmp/bus"

# systemd-run (what Launch uses) needs the real user manager; only then can the scope check run.
have_systemd=0
if [ -n "$orig_rt" ] && [ -S "$orig_rt/bus" ] && timeout 5 systemctl --user show-environment >/dev/null 2>&1; then
    have_systemd=1
fi

cat > "$tmp/hypr.lua" <<'LUA'
hl.monitor({ output = "", mode = "1280x720", position = "auto", scale = 1 })
hl.config({ misc = { disable_hyprland_logo = true, disable_splash_rendering = true } })
LUA
hypr_env=(env XDG_RUNTIME_DIR="$rt" AQ_BACKENDS=headless WLR_RENDERER_ALLOW_SOFTWARE=1 LIBGL_ALWAYS_SOFTWARE=1)
[ "$have_systemd" = 1 ] && hypr_env+=(DBUS_SESSION_BUS_ADDRESS="unix:path=$orig_rt/bus")
"${hypr_env[@]}" Hyprland -c "$tmp/hypr.lua" >"$tmp/hypr.log" 2>&1 &
hypr=$!
pids+=("$hypr")

sig=""
for _ in $(seq 1 50); do
    kill -0 "$hypr" 2>/dev/null || { cat "$tmp/hypr.log"; skip "headless Hyprland exited at start"; }
    sig=$(find "$rt/hypr" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null | head -n1)
    [ -n "$sig" ] && [ -S "$rt/hypr/$sig/.socket.sock" ] && break
    sleep 0.2
done
[ -n "$sig" ] || skip "headless Hyprland never opened its socket"
hc() { env XDG_RUNTIME_DIR="$rt" HYPRLAND_INSTANCE_SIGNATURE="$sig" hyprctl "$@"; }
wl=""
for _ in $(seq 1 25); do
    wl=$(find "$rt" -maxdepth 1 -name 'wayland-*' ! -name '*.lock' -printf '%f\n' 2>/dev/null | head -n1)
    [ -n "$wl" ] && break
    sleep 0.2
done
[ -n "$wl" ] || skip "headless Hyprland has no Wayland socket"
[ -n "$(hc monitors -j 2>/dev/null | jq -r '.[0].name // empty' 2>/dev/null)" ] || hc output create headless >/dev/null 2>&1
sleep 0.5

bars() { hc layers -j 2>/dev/null | grep -o '"namespace": *"uq-bar"' | wc -l; }
wait_bars() { # wait_bars <predicate-op> <n> : poll up to 10 s until count <op> n
    for _ in $(seq 1 50); do
        local n; n=$(bars)
        case "$1" in
            ge) [ "$n" -ge "$2" ] && return 0 ;;
            eq) [ "$n" -eq "$2" ] && return 0 ;;
        esac
        sleep 0.2
    done
    return 1
}

shell_env=(env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" HYPRLAND_INSTANCE_SIGNATURE="$sig"
    DBUS_SESSION_BUS_ADDRESS="$bus" PATH="$(dirname "$uqctl"):$PATH" XDG_STATE_HOME="$tmp/state"
    QT_QPA_PLATFORM=wayland)
start_shell() {
    "${shell_env[@]}" qs -p "$root/shell" --no-duplicate >"$tmp/qs.log" 2>&1 &
    shell=$!
    pids+=("$shell")
}
start_shell
if ! wait_bars ge 1; then
    # Only an environment that cannot host the shell is a skip. A dead shell or QML errors mean
    # the shell is broken: fail, with the log.
    if ! kill -0 "$shell" 2>/dev/null; then
        fail "qs exited before uq-bar appeared"
    elif grep -qE 'TypeError|ReferenceError|is not a type|Cannot assign|Unable to assign|is not a valid import|Failed to load configuration|Cannot read property|is not a function|SyntaxError|[A-Za-z]+\.qml:[0-9]+:[0-9]*:? ' "$tmp/qs.log"; then
        fail "QML errors, uq-bar never appeared"
    elif grep -qiE 'layer.?shell.*(not|unsupported|unavailable)|(not|unsupported|unavailable).*layer.?shell|compositor does not support|failed to create.*(wl_display|egl|opengl|rhi)|could not connect to (display|wayland)|no (screens|outputs)' "$tmp/qs.log"; then
        skip "the compositor here cannot host the shell's layer surfaces$(echo; tail -n 15 "$tmp/qs.log")"
    else
        fail "uq-bar never appeared although qs is running and logged no known environment limit"
    fi
fi
base=$(bars)
echo "ok: uq-bar present ($base layer(s))"

if command -v grim >/dev/null 2>&1; then
    out=${SMOKE_OUT:-$tmp}
    mkdir -p "$out"
    env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" grim "$out/bar.png" 2>/dev/null && echo "ok: screenshot $out/bar.png"
fi

# --- shell IPC opens Spotlight --------------------------------------------------------------
layer_count() { hc layers -j 2>/dev/null | grep -o "\"namespace\": *\"$1\"" | wc -l; }
wait_layer() { # wait_layer <namespace> <n>: poll up to 5 s until exactly n such layers exist
    for _ in $(seq 1 25); do
        [ "$(layer_count "$1")" -eq "$2" ] && return 0
        sleep 0.2
    done
    return 1
}
smoke_ipc_spotlight() {
    local r
    if ! r=$("${shell_env[@]}" timeout 10 qs ipc -p "$root/shell" call uq open spotlight 2>&1); then
        echo "SKIP (sub-check): IPC open spotlight: qs ipc could not reach the shell here ($r)"
        return 0
    fi
    [ "$r" = ok ] || fail "qs ipc call uq open spotlight printed '$r', not ok"
    wait_layer uq-spotlight 1 || fail "no uq-spotlight layer after qs ipc call uq open spotlight"
    r=$("${shell_env[@]}" timeout 10 qs ipc -p "$root/shell" call uq close spotlight 2>&1)
    [ "$r" = ok ] || fail "qs ipc call uq close spotlight printed '$r', not ok"
    wait_layer uq-spotlight 0 || fail "the uq-spotlight layer stayed after qs ipc call uq close spotlight"
    echo "ok: qs ipc open/close spotlight (uq-spotlight layer mapped and unmapped)"
}
smoke_ipc_spotlight

# One bridge request, answered on stdout (the bridge stays up 1.5 s so the reply is flushed).
bridge_call() {
    { printf '%s\n' "$1"; sleep 1.5; } |
        env XDG_RUNTIME_DIR="$rt" HYPRLAND_INSTANCE_SIGNATURE="$sig" DBUS_SESSION_BUS_ADDRESS="$bus" \
            "$uqctl" bridge 2>"$tmp/bridge.err"
}
bridge_ok() { printf '%s' "$1" | grep -q '"id":1,"ok"' ; }

# --- Dispatch against a real window ---------------------------------------------------------
client=""
for c in foot kitty alacritty; do command -v "$c" >/dev/null 2>&1 && { client=$c; break; }; done
if [ -z "$client" ]; then
    echo "SKIP (sub-check): dispatch on a real window: no Wayland client (foot, kitty, alacritty) installed"
else
    case "$client" in
        foot) cmd=(foot sleep 300) ;;
        *) cmd=("$client" -e sleep 300) ;;
    esac
    env XDG_RUNTIME_DIR="$rt" WAYLAND_DISPLAY="$wl" LIBGL_ALWAYS_SOFTWARE=1 "${cmd[@]}" >"$tmp/client.log" 2>&1 &
    pids+=($!)
    addr=""
    for _ in $(seq 1 50); do
        addr=$(hc clients -j 2>/dev/null | jq -r '.[0].address // empty' 2>/dev/null)
        [ -n "$addr" ] && break
        sleep 0.2
    done
    if [ -z "$addr" ]; then
        echo "SKIP (sub-check): dispatch on a real window: $client opened no window headless ($(tail -n 1 "$tmp/client.log"))"
    else
        ws0=$(hc clients -j | jq -r --arg a "$addr" '.[] | select(.address==$a) | .workspace.id')
        r=$(bridge_call "{\"v\":1,\"id\":1,\"call\":\"Dispatch\",\"args\":[\"focus_window\",{\"address\":\"$addr\"}]}")
        bridge_ok "$r" || fail "Dispatch focus_window: $r $(cat "$tmp/bridge.err")"
        target=2; [ "$ws0" = 2 ] && target=3
        r=$(bridge_call "{\"v\":1,\"id\":1,\"call\":\"Dispatch\",\"args\":[\"move\",{\"address\":\"$addr\",\"workspace\":$target}]}")
        bridge_ok "$r" || fail "Dispatch move: $r $(cat "$tmp/bridge.err")"
        ws1=""
        for _ in $(seq 1 25); do
            ws1=$(hc clients -j | jq -r --arg a "$addr" '.[] | select(.address==$a) | .workspace.id')
            [ "$ws1" = "$target" ] && break
            sleep 0.2
        done
        [ "$ws1" = "$target" ] || fail "window $addr did not move from workspace $ws0 to $target (now ${ws1:-gone})"
        echo "ok: Dispatch focus_window and move ($client window $addr: workspace $ws0 -> $ws1)"
    fi
fi

# --- smoke_launch_survives_restart ----------------------------------------------------------
smoke_launch_survives_restart() {
    if [ "$have_systemd" != 1 ]; then
        echo "SKIP (sub-check): smoke_launch_survives_restart: no systemd user manager here"
        return 0
    fi
    local r unit
    r=$(bridge_call '{"v":1,"id":1,"call":"Launch","args":[{"argv":["sleep","300"]}]}')
    bridge_ok "$r" || fail "Launch: $r $(cat "$tmp/bridge.err")"
    for _ in $(seq 1 25); do
        unit=$(systemctl --user list-units --plain --no-legend --state=active 'app-uq-sleep-*.scope' 2>/dev/null | awk '{print $1; exit}')
        [ -n "$unit" ] && break
        sleep 0.2
    done
    [ -n "$unit" ] || fail "no active app-uq-sleep-*.scope after Launch"
    scope=$unit
    kill "$shell" 2>/dev/null; wait "$shell" 2>/dev/null
    start_shell
    wait_bars ge "$base" || fail "shell did not come back after restart"
    [ "$(systemctl --user is-active "$scope")" = active ] || fail "$scope did not survive the shell restart"
    echo "ok: smoke_launch_survives_restart ($scope)"
}
smoke_launch_survives_restart

# --- hot-plug -------------------------------------------------------------------------------
hotplug() {
    local before name
    before=$(bars)
    if ! hc output create headless >/dev/null 2>&1; then
        echo "SKIP (sub-check): hot-plug: hyprctl output create headless failed here"
        return 0
    fi
    wait_bars eq $((before + 1)) || fail "bar count did not go from $before to $((before + 1)) after plugging an output (now $(bars))"
    name=$(hc monitors -j | jq -r '.[].name' | grep -E '^HEADLESS-' | tail -n1)
    [ -n "$name" ] || fail "no HEADLESS-* output to remove"
    hc output remove "$name" >/dev/null 2>&1
    wait_bars eq "$before" || fail "bar count did not return to $before after unplugging $name (now $(bars))"
    echo "ok: hot-plug ($before -> $((before + 1)) -> $before)"
}
hotplug

# Reaching here means the bar check ran and every check that could run passed.
echo "smoke-shell ok"
