# shellcheck shell=bash
# Settings slice F (display arrangements, main display, workspace planner, touchpad gestures):
# docs/notes/settings-slice-f-live-check.md. Run through the same uqd calls the Displays and
# Input pages make (`ApplyMonitors`, `ConfirmMonitors`, `RememberMonitors`, `SetSection`), against
# the real output the session has (the CI sandbox's vkms output) and, for hotplug, a second output
# that `hyprctl output create headless NAME` adds (a headless output with a name of ours counts as
# a screen to uQ; an unnamed HEADLESS-n one doesn't, which SF6 checks). Where there is no output
# to arrange, or Hyprland can't add a headless one, the check is N/A.
#
# What stays manual, and why:
#   1   the laptop and docking screens themselves (SF7 does the same with a headless output)
#   1b  logging out and in with the same screens (SF4 restarts uqd, which is what login does)
#   2   swapping two real cables and which way round identical screens come back
#   3   flapping a real dock: SF7 plugs a headless output in and out quickly, but a dock's burst
#       of events is a person's
#   4   a mode the screen can't do, and the one notification that follows (a notification daemon
#       and a screen that refuses)
#   6   pop-ups appearing on the main display, `xrandr --listmonitors` showing the primary, and
#       whether `default_monitor` changes anything on reload (SF5 checks the rendered config)
#   7,8 the bar per screen and screens with no bar (the shell's QML, eyes)
#   9   spanning a wallpaper across screens (the shell's QML, eyes, a wide image)
#   10  workspaces that have windows moving back when a screen returns (needs a running window
#       on a real second screen; SF5 checks the planner's rules)
#   11  a real touchpad doing a swipe once per gesture; "after upgrading, before uqd's first
#       render" (SF8 checks what is rendered and what Hyprland loaded)
#   12  whether the gesture does the action, and the editor's warning and greyed Save (Qt)
#   16  a dev session on a host with screens
# Item 17 (user.lua untouched) is checked after every check by run.sh.
#
# Everything a check changes is put back by the check; fd_cleanup (below, called from lib.sh's
# cleanup_check) drops a headless output, any arrangement and any pending change a failed check
# left behind.

FD_HP=uqlive-hp
# fd_real: the name of the first real output (not a headless or fallback one).
fd_real() {
    hc monitors -j 2>/dev/null | jq -r '[.[] | select(.name | test("^(HEADLESS-|FALLBACK)") | not) | select(.name != "'"$FD_HP"'")][0].name // empty'
}
fd_names() { hc monitors -j 2>/dev/null | jq -r '.[].name' | sort | tr '\n' ' '; }
# fd_mon NAME: that output's row in `hyprctl monitors -j`.
fd_mon() { hc monitors -j 2>/dev/null | jq -c --arg n "$1" '.[] | select(.name == $n)'; }
# fd_scale NAME: its live scale.
fd_scale() { fd_mon "$1" | jq -r '.scale // empty'; }
fd_scale_is() {
    local cur; cur=$(fd_scale "$1")
    jq -en --argjson a "${cur:-null}" --argjson b "$2" '$a != null and ((($a - $b) | fabs) < 0.001)' >/dev/null 2>&1
}
# fd_out NAME SCALE [X]: one output's layout entry at its current mode, as {"outputs": {...}}.
fd_out() {
    fd_mon "$1" | jq -c --argjson s "$2" --argjson x "${3:-0}" \
        '{outputs: {(.name): {mode: "\(.width)x\(.height)", position: {x: $x, y: 0}, scale: $s}}}'
}
fd_join() { jq -sc '{outputs: (map(.outputs) | add)}'; }
# fd_keep JSON [ORIGIN]: apply a layout and keep it; prints the error, if any.
fd_keep() {
    local t
    t=$(bus ApplyMonitors s "$1") || { printf '%s\n' "$t"; return 1; }
    bus ConfirmMonitors ss "$t" "${2:-settings}"
}
fd_pending() { bus ListOutputs | jq -r '.pending.token // empty'; }
fd_stored_scale() { section monitors | jq -c --arg n "$1" '.outputs[$n].scale // null'; }
fd_profiles() { section monitors | jq -c '[.profiles[]? | .name]'; }
mon_merge() { merge monitors "$1"; }
fd_cleanup() {
    local t n
    if hypr_up; then
        t=$(fd_pending 2>/dev/null)
        [ -n "$t" ] && bus RevertMonitors s "$t" >/dev/null 2>&1
        hc output remove "$FD_HP" >/dev/null 2>&1
    fi
    section monitors 2>/dev/null | jq -e '[.profiles[]? | select(.name | startswith("uqlive"))] | length > 0' >/dev/null 2>&1 &&
        mon_merge '.profiles |= map(select(.name | startswith("uqlive") | not))' >/dev/null
    return 0
}
# fd_reset: no stored layout, no arrangement of ours (the session as the checks found it).
fd_reset() {
    [ -n "$(fd_pending)" ] && bus RevertMonitors s "$(fd_pending)" >/dev/null
    fd_keep '{"outputs":{}}' >/dev/null
    mon_merge '.profiles |= map(select(.name | startswith("uqlive") | not))' >/dev/null
}

check SF1 "Remember: saved by the screen's make, model and serial, not its port; the same screens keep one arrangement" sf1 hypr
sf1() {
    local name m id p r e n0 id2
    name=$(fd_real); [ -n "$name" ] || { na "no real output to remember (only headless ones)"; return; }
    m=$(fd_mon "$name")
    id=$(bus RememberMonitors ss "uqlive Desk" settings) || { bad "RememberMonitors: $id"; return; }
    note "id: $id; output $name: $(jq -c '{make, model, serial}' <<<"$m")"
    [[ $id == m-* ]] || bad "expected an arrangement id like m-1234; observed $id"
    p=$(section monitors | jq -c --arg i "$id" '.profiles[]? | select(.id == $i)')
    note "stored: $(jq -c . <<<"$p" | cut -c1-300)"
    expect_eq "the arrangement's name" "uqlive Desk" "$(jq -r .name <<<"$p")"
    expect_eq "the screen it holds, by identity" "$(jq -cS '{make, model, serial}' <<<"$m")" \
        "$(jq -cS --arg h "$name" '.outputs[] | select(.hint == $h) | .monitor' <<<"$p")"
    jq -e '[.outputs[] | .config | has("name")] | any | not' <<<"$p" >/dev/null || bad "a saved screen is keyed by a connector name"
    e=$(newest); note "History: $(jq -r .summary <<<"$e") ($(jq -r .origin <<<"$e"))"
    expect_eq "the arrangement ListOutputs finds for the connected screens" "$id" "$(bus ListOutputs | jq -r '.arrangement.id // empty')"
    n0=$(section monitors | jq '.profiles | length')
    id2=$(bus RememberMonitors ss "uqlive Desk renamed" settings) || bad "remembering again: $id2"
    expect_eq "the id when the same screens are remembered again" "$id" "$id2"
    expect_eq "arrangements after remembering the same screens again" "$n0" "$(section monitors | jq '.profiles | length')"
    expect_eq "its name after remembering again" "uqlive Desk renamed" "$(section monitors | jq -r --arg i "$id" '.profiles[] | select(.id == $i) | .name')"
    r=$(bus Undo sb "$(newest_id)" false) || bad "Undo: $r"
    expect_eq "its name after Undo" "uqlive Desk" "$(section monitors | jq -r --arg i "$id" '.profiles[] | select(.id == $i) | .name')"
    mon_merge '.profiles |= map(select(.name | startswith("uqlive") | not))' >/dev/null
}

check SF2 "Apply a layout: live at once, Revert puts it back, Keep saves and renders it; a plain SetSection is refused" sf2 hypr
sf2() {
    local name s0 t r layout h0
    name=$(fd_real); [ -n "$name" ] || { na "no real output to arrange"; return; }
    s0=$(fd_scale "$name"); layout=$(fd_out "$name" 2)
    note "$name at scale $s0; layout: $layout"
    t=$(bus ApplyMonitors s "$layout") || { bad "ApplyMonitors: $t"; return; }
    wait_for 5 fd_scale_is "$name" 2 || bad "expected scale 2 live on $name; observed $(fd_scale "$name")"
    [ -n "$(fd_pending)" ] || bad "ListOutputs shows no change waiting"
    expect_eq "what is stored while the change waits" null "$(fd_stored_scale "$name")"
    r=$(mon_merge '.auto_profiles = false') && bad "a write was accepted while a display change waited"
    note "write while waiting: $r"
    [[ $r == *"keep or revert"* ]] || bad "expected 'keep or revert the display change first'; observed $r"
    r=$(bus RevertMonitors s "$t") || bad "RevertMonitors: $r"
    wait_for 5 fd_scale_is "$name" "$s0" || bad "Revert: expected scale $s0 on $name; observed $(fd_scale "$name")"
    [ -z "$(fd_pending)" ] || bad "a change is still waiting after Revert"
    expect_eq "what is stored after Revert" null "$(fd_stored_scale "$name")"
    r=$(bus ConfirmMonitors ss "$t" settings) && bad "a token that was reverted still confirms"
    h0=$(newest_id)
    r=$(fd_keep "$layout") || bad "Keep: $r"
    wait_for 5 fd_scale_is "$name" 2 || bad "Keep: expected scale 2 on $name; observed $(fd_scale "$name")"
    expect_eq "the stored scale after Keep" 2.0 "$(fd_stored_scale "$name")"
    grep -qE "hl\.monitor\(\{ output = \"$name\", mode = \"[0-9]+x[0-9]+\", position = \"0x0\", scale = \"2\.0\" \}\)" "$CFG/hypr/uq/generated.lua" ||
        bad "generated.lua has no hl.monitor line for $name at scale 2: $(grep 'hl.monitor' "$CFG/hypr/uq/generated.lua" | head -n 3)"
    note "History: $(newest | jq -r .summary)"
    [ "$(newest_id)" != "$h0" ] || bad "Keep added no History row"
    # Displays change only through Apply: SetSection with another layout is refused.
    r=$(set_section monitors "$(section monitors | jq -c '.outputs = {}')") && bad "SetSection changed the layout"
    note "SetSection with another layout: $r"
    [[ $r == *"displays change through ApplyMonitors"* ]] || bad "expected 'displays change through ApplyMonitors'; observed $r"
    r=$(bus Undo sb "$(newest_id)" false) || bad "Undo of the layout: $r"
    wait_for 5 fd_scale_is "$name" "$s0" || bad "Undo: expected scale $s0 on $name; observed $(fd_scale "$name")"
    expect_eq "the stored scale after Undo" null "$(fd_stored_scale "$name")"
    fd_reset
}

check SF3 "A change nobody keeps goes back by itself after 15 s" sf3 hypr slow
sf3() {
    local name s0 t s end
    name=$(fd_real); [ -n "$name" ] || { na "no real output to arrange"; return; }
    s0=$(fd_scale "$name")
    t=$(bus ApplyMonitors s "$(fd_out "$name" 2)") || { bad "ApplyMonitors: $t"; return; }
    s=$(date +%s)
    wait_for 5 fd_scale_is "$name" 2 || bad "expected scale 2 live; observed $(fd_scale "$name")"
    sleep 6
    fd_scale_is "$name" 2 || bad "the change went back after about 6 s, not 15"
    wait_for 20 fd_scale_is "$name" "$s0" || { bad "the change was still live 20 s on"; bus RevertMonitors s "$t" >/dev/null; return; }
    end=$(($(date +%s) - s)); note "back after about $end s"
    [ "$end" -ge 13 ] && [ "$end" -le 19 ] || bad "expected about 15 s; observed $end s"
    [ -z "$(fd_pending)" ] || bad "a change is still waiting after the timer"
    expect_eq "what is stored after the timer" null "$(fd_stored_scale "$name")"
    r=$(bus ConfirmMonitors ss "$t" settings) && bad "a token that timed out still confirms"
    return 0
}

check SF4 "Restarts: a layout you kept stays; an empty one gets the saved arrangement, as at login" sf4 hypr restart
sf4() {
    local name id
    name=$(fd_real); [ -n "$name" ] || { na "no real output to arrange"; return; }
    # Arrangement A: scale 2.
    fd_keep "$(fd_out "$name" 2)" >/dev/null || { bad "keeping layout A"; return; }
    id=$(bus RememberMonitors ss "uqlive login" settings) || { bad "RememberMonitors: $id"; fd_reset; return; }
    # 1b: a different layout is kept; a restart (which is what a login is to uqd) leaves it.
    fd_keep "$(fd_out "$name" 1)" >/dev/null || bad "keeping layout B"
    uqd_restart || { bad "uqd didn't restart"; fd_reset; return; }
    wait_for 10 uqd_up
    sleep 4
    expect_eq "the stored scale after a restart with a kept layout" 1.0 "$(fd_stored_scale "$name")"
    fd_scale_is "$name" 1 || bad "the kept layout (scale 1) changed after the restart: scale is $(fd_scale "$name")"
    # Nothing stored for the screen: the arrangement for these screens applies on start.
    fd_keep '{"outputs":{}}' >/dev/null || bad "clearing the layout"
    expect_eq "the stored layout once cleared" "{}" "$(section monitors | jq -c .outputs)"
    uqd_restart || { bad "uqd didn't restart"; fd_reset; return; }
    wait_for 10 uqd_up
    wait_for 10 sf4_applied "$name" || bad "after a restart with no stored layout, expected arrangement A (scale 2); stored: $(fd_stored_scale "$name"), live: $(fd_scale "$name")"
    note "History: $(history_json 3 | jq -c '[.[] | {origin, summary}]')"
    fd_scale_is "$name" 2 || bad "arrangement A isn't live: scale is $(fd_scale "$name")"
    fd_reset
}
sf4_applied() { [ "$(fd_stored_scale "$1")" = 2.0 ]; }

check SF5 "Main display and workspace planner: rendered into generated.lua, Hyprland loads them without errors" sf5 hypr
sf5() {
    local name m r e0 n id
    name=$(fd_real); [ -n "$name" ] || { na "no real output to choose"; return; }
    m=$(fd_mon "$name"); id=$(jq -c '{make, model, serial}' <<<"$m")
    e0=$(config_errors | wc -l)
    r=$(mon_merge ".primary = $id") || { bad "choosing the main display: $r"; return; }
    note "History: $(newest | jq -r .summary)"
    expect_eq "the main display ListOutputs reports" "$name" "$(bus ListOutputs | jq -r '.roles.primary')"
    grep -qF "cursor = { default_monitor = \"$name\" }" "$CFG/hypr/uq/generated.lua" ||
        bad "generated.lua has no cursor.default_monitor = \"$name\": $(grep -n 'cursor' "$CFG/hypr/uq/generated.lua" | head -n 3)"
    note "cursor:default_monitor in Hyprland: $(hc getoption cursor:default_monitor -j 2>&1 | jq -c . 2>/dev/null | cut -c1-120)"
    r=$(mon_merge '.workspaces = {strategy: "sequential", count: 4, manual: [], move_back: true}') || bad "the planner: $r"
    # The planner needs two screens: with one it renders nothing.
    expect_eq "planned workspace rules with one screen" 0 "$(sf5_rules)"
    if r=$(hc output create headless "$FD_HP" 2>&1) && wait_for 5 sf7_has; then
        wait_for 10 sf5_four || bad "with two screens expected rules for workspaces 1 to 4; observed $(sf5_rules): $(grep 'hl.workspace_rule' "$CFG/hypr/uq/generated.lua" | head -n 6 | tr '\n' ' ')"
        expect_eq "planned rules on $name" 2 "$(grep -E 'hl\.workspace_rule' "$CFG/hypr/uq/generated.lua" | grep -cE "workspace = \"[1-4]\".*monitor = \"$name\"")"
        expect_eq "planned rules on $FD_HP" 2 "$(grep -E 'hl\.workspace_rule' "$CFG/hypr/uq/generated.lua" | grep -cE "workspace = \"[1-4]\".*monitor = \"$FD_HP\"")"
        note "hyprctl workspacerules: $(hc workspacerules -j 2>&1 | jq -c '[.[] | {workspaceString, monitor}]' 2>/dev/null | cut -c1-300)"
        expect_eq "Hyprland's errors with the planner on two screens" "$e0" "$(config_errors | wc -l)"
        hc output remove "$FD_HP" >/dev/null 2>&1
        wait_for 10 sf5_none || bad "the planner's rules stayed after the second screen went: $(sf5_rules)"
    else
        note "no second output to plan for ($r); only the one-screen case was checked"
    fi
    expect_eq "Hyprland's errors after the main display and planner" "$e0" "$(config_errors | wc -l)"
    # A main display that isn't connected renders nothing and leaves the rest alone.
    r=$(mon_merge '.primary = {make: "uqlive", model: "gone", serial: "0"}') || bad "choosing a screen that isn't connected: $r"
    grep -q 'default_monitor' "$CFG/hypr/uq/generated.lua" && bad "generated.lua sets default_monitor for a screen that isn't connected"
    r=$(mon_merge '.workspaces.count = 21') && bad "21 planned workspaces were accepted"
    note "21 workspaces: $r"
    r=$(mon_merge '.workspaces.manual = [{workspace: 9, monitor: {make: "a", model: "b", serial: "c"}}]') && bad "a manual slot outside the count was accepted"
    r=$(mon_merge 'del(.primary) | .workspaces = {strategy: "off", count: 10, manual: [], move_back: true}') || bad "turning both off: $r"
    grep -q 'default_monitor' "$CFG/hypr/uq/generated.lua" && bad "default_monitor is still in generated.lua after the main display was cleared"
    [ "$(sf5_rules)" = 0 ] || bad "planner rules are still in generated.lua after it was turned off"
    expect_eq "Hyprland's errors at the end" "$e0" "$(config_errors | wc -l)"
}

sf5_rules() { grep -E 'hl\.workspace_rule' "$CFG/hypr/uq/generated.lua" | grep -cE 'workspace = "[1-4]".*monitor = "'; }
sf5_four() { [ "$(sf5_rules)" = 4 ]; }
sf5_none() { [ "$(sf5_rules)" = 0 ]; }

check SF6 "A headless output with no name of ours isn't a screen: nothing is written when it comes and goes" sf6 hypr
sf6() {
    local before h0 g0 new r name
    name=$(fd_real); [ -n "$name" ] || { na "no real output"; return; }
    before=$(fd_names); h0=$(newest_id); g0=$(sha256sum "$CFG/hypr/uq/generated.lua")
    r=$(hc output create headless 2>&1) || { na "hyprctl output create headless: $r"; return; }
    wait_for 5 sf6_added "$before" || { na "no headless output appeared (hyprctl said: $r)"; return; }
    new=$(comm -13 <(tr ' ' '\n' <<<"$before" | sort) <(fd_names | tr ' ' '\n' | sort) | grep . | head -n1)
    note "added $new; Hyprland has: $(fd_names)"
    sleep 3
    [[ " $(bus ListOutputs | jq -r '[.live[].name] | join(" ")') " == *" $new "* ]] || note "ListOutputs doesn't list $new"
    expect_eq "History after a headless output came" "$h0" "$(newest_id)"
    expect_eq "generated.lua after a headless output came" "$g0" "$(sha256sum "$CFG/hypr/uq/generated.lua")"
    r=$(bus RememberMonitors ss "uqlive solo" settings) || bad "RememberMonitors: $r"
    expect_eq "screens in an arrangement made with a headless output present" 1 "$(section monitors | jq '[.profiles[] | select(.name == "uqlive solo")][0].outputs | length')"
    mon_merge '.profiles |= map(select(.name | startswith("uqlive") | not))' >/dev/null
    h0=$(newest_id)
    hc output remove "$new" >/dev/null 2>&1
    wait_for 5 sf6_gone "$new" || bad "$new is still there after hyprctl output remove"
    sleep 2
    expect_eq "History after the headless output went" "$h0" "$(newest_id)"
    [ -z "$(config_errors)" ] || note "Hyprland errors: $(config_errors | head -n 3 | tr '\n' ' ')"
}
sf6_added() { [ "$(fd_names)" != "$1" ]; }
sf6_gone() { [[ " $(fd_names) " != *" $1 "* ]]; }

check SF7 "Screens come and go: the saved arrangement for the new set applies; with the switch off nothing changes" sf7 hypr
sf7() {
    local name r hp_x hot
    name=$(fd_real); [ -n "$name" ] || { na "no real output"; return; }
    # The solo arrangement: the real screen at scale 1.
    fd_keep "$(fd_out "$name" 1)" >/dev/null || { bad "keeping the solo layout"; return; }
    r=$(bus RememberMonitors ss "uqlive solo" settings) || { bad "RememberMonitors: $r"; fd_reset; return; }
    r=$(hc output create headless "$FD_HP" 2>&1) || { na "hyprctl output create headless $FD_HP: $r"; fd_reset; return; }
    wait_for 5 sf7_has || { na "the headless output $FD_HP didn't appear (hyprctl said: $r)"; fd_reset; return; }
    note "Hyprland has: $(fd_names)"
    # The pair: the real screen at scale 2 and the new one beside it.
    hp_x=2000
    r=$(fd_keep "$(fd_join <<<"$(fd_out "$name" 2)
$(fd_out "$FD_HP" 1 "$hp_x")")") || { bad "keeping the pair layout: $r"; hc output remove "$FD_HP" >/dev/null 2>&1; fd_reset; return; }
    r=$(bus RememberMonitors ss "uqlive pair" settings) || bad "RememberMonitors for the pair: $r"
    note "arrangements: $(fd_profiles)"
    expect_eq "arrangements saved" 2 "$(section monitors | jq '[.profiles[] | select(.name | startswith("uqlive"))] | length')"
    hot=$(newest_id)
    # Unplug: within a few seconds the solo arrangement is back, with no dialog.
    hc output remove "$FD_HP" >/dev/null 2>&1
    wait_for 8 sf7_solo "$name" || bad "after the second screen went, expected the solo layout (scale 1) back; stored: $(fd_stored_scale "$name"), live: $(fd_scale "$name")"
    [ -z "$(fd_pending)" ] || bad "a change is waiting for a confirm after the hotplug"
    note "History after unplug: $(history_json 2 | jq -c '[.[] | {origin, summary}]')"
    # A switch for new screens is quiet: it leaves no History row.
    expect_eq "History after the switch back to the solo layout" "$hot" "$(newest_id)"
    # Plug in: the pair is back.
    hc output create headless "$FD_HP" >/dev/null 2>&1
    wait_for 8 sf7_pair "$name" || bad "after the second screen came back, expected the pair layout (scale 2); stored: $(fd_stored_scale "$name"), live: $(fd_scale "$name")"
    expect_eq "the second screen's stored position" "{\"x\":$hp_x,\"y\":0}" "$(section monitors | jq -c --arg n "$FD_HP" '.outputs[$n].position')"
    # Flapping: three quick cycles end on the solo layout, with no dialog.
    for _ in 1 2 3; do
        hc output remove "$FD_HP" >/dev/null 2>&1; sleep 0.3
        hc output create headless "$FD_HP" >/dev/null 2>&1; sleep 0.3
    done
    hc output remove "$FD_HP" >/dev/null 2>&1
    wait_for 8 sf7_solo "$name" || bad "after flapping, expected the solo layout; stored: $(fd_stored_scale "$name")"
    [ -z "$(fd_pending)" ] || bad "a change is waiting for a confirm after flapping"
    expect_eq "hotplug rows in History after everything" 0 "$(history_json 12 | jq -c '[.[] | select(.origin == "hotplug")] | length')"
    # Switch off: nothing follows the screens.
    r=$(mon_merge '.auto_profiles = false') || bad "turning the switch off: $r"
    hc output create headless "$FD_HP" >/dev/null 2>&1
    wait_for 5 sf7_has
    sleep 4
    fd_stored_scale "$name" | grep -q '^1.0$' || bad "with the switch off the layout changed: stored scale $(fd_stored_scale "$name")"
    hc output remove "$FD_HP" >/dev/null 2>&1
    wait_for 5 sf6_gone "$FD_HP"
    mon_merge '.auto_profiles = true' >/dev/null
    fd_reset
}
sf7_has() { [[ " $(fd_names) " == *" $FD_HP "* ]]; }
sf7_solo() { [ "$(fd_stored_scale "$1")" = 1.0 ] && fd_scale_is "$1" 1 && ! sf7_has; }
sf7_pair() { [ "$(fd_stored_scale "$1")" = 2.0 ] && fd_scale_is "$1" 2 && sf7_has; }

# --- touchpad gestures (the Input page) ---------------------------------------------------------
GESTURE_DEFAULTS='[{"fingers":3,"direction":"horizontal","action":"workspace"},{"fingers":4,"direction":"up","action":"uq:spotlight"}]'
in_merge() { merge input "$1"; }
swipe_opt() { hc getoption "gestures:$1" -j 2>/dev/null | jq -c '.int // .float // .bool // .' 2>/dev/null; }

check SF8 "Gestures: defaults rendered as hl.gesture, a shell one as a function, conflicts refused, swipe options reach Hyprland" sf8
sf8() {
    local orig g r e0 base n
    orig=$(section input)
    [ -n "${HAVE[hypr]:-}" ] && e0=$(config_errors | wc -l)
    g=$(gen_lua)
    base=${UQ_DATA_DIR:-/usr/share/uq}/hypr/base.lua
    if [ -r "$base" ]; then grep -q 'hl\.gesture' "$base" && bad "$base has a gesture; uQ's own belong in generated.lua"; fi
    grep -qF 'hl.gesture({ fingers = 3, direction = "horizontal", action = "workspace" })' <<<"$g" || bad "generated.lua has no three-finger workspace swipe"
    grep -qF 'hl.gesture({ fingers = 4, direction = "up", action = function() hl.dispatch(hl.dsp.global("uq:spotlight")) end })' <<<"$g" ||
        bad "generated.lua has no four-finger Spotlight gesture as function() hl.dispatch(hl.dsp.global(...)) end"
    # The editor's changes: scratch workspace, pinch with a modifier, quick settings.
    r=$(in_merge ".gestures = $GESTURE_DEFAULTS + [{fingers:3,direction:\"down\",action:\"special\"}, {fingers:4,direction:\"pinch_in\",action:\"float\",modifier:\"SUPER\"}, {fingers:4,direction:\"left\",action:\"uq:quick-settings\"}]") || { bad "adding gestures: $r"; return; }
    g=$(gen_lua)
    grep -qF 'hl.gesture({ fingers = 3, direction = "down", action = "special", workspace_name = "magic" })' <<<"$g" || bad "no scratch workspace gesture in generated.lua"
    grep -qF 'hl.gesture({ fingers = 4, direction = "pinchin", action = "float", mods = "SUPER" })' <<<"$g" || bad "no SUPER pinch gesture in generated.lua"
    grep -qF 'hl.gesture({ fingers = 4, direction = "left", action = function() hl.dispatch(hl.dsp.global("uq:quick-settings")) end })' <<<"$g" || bad "no four-finger quick settings gesture"
    note "History: $(newest | jq -r .summary)"
    [ -z "${HAVE[hypr]:-}" ] || expect_eq "Hyprland's errors after adding gestures" "$e0" "$(config_errors | wc -l)"
    # Three fingers left clashes with the sideways swipe.
    r=$(in_merge '.gestures += [{fingers:3,direction:"left",action:"close"}]') && bad "three fingers left was accepted beside three fingers sideways"
    note "clash: $r"
    [[ $r == *"3 fingers"* && $r == *"Switch workspace"* ]] || bad "expected the refusal to name the gesture and the other action; observed $r"
    r=$(in_merge '.gestures += [{fingers:2,direction:"left",action:"close"}]') && bad "two fingers was accepted"
    r=$(in_merge '.gestures += [{fingers:4,direction:"left",action:"special",workspace:"a b"}]') && bad "a workspace name with a space was accepted"
    # The default sideways swipe turned off: its line goes.
    r=$(in_merge '.gestures[0].enabled = false') || bad "turning the sideways swipe off: $r"
    gen_lua | grep -qF 'direction = "horizontal"' && bad "the sideways swipe is still in generated.lua after it was turned off"
    [ -z "${HAVE[hypr]:-}" ] || expect_eq "Hyprland's errors after turning one off" "$e0" "$(config_errors | wc -l)"
    # Swipe options.
    r=$(in_merge '.swipe = {distance: 600, invert: true, create_new: false, cancel_ratio: 0.3}') || bad "swipe options: $r"
    g=$(gen_lua)
    for n in 'workspace_swipe_distance = 600' 'workspace_swipe_invert = true' 'workspace_swipe_create_new = false' 'workspace_swipe_cancel_ratio = 0.3'; do
        grep -qF "$n" <<<"$g" || bad "generated.lua has no '$n'"
    done
    if [ -n "${HAVE[hypr]:-}" ]; then
        wait_for 5 sf8_distance 600 || bad "gestures:workspace_swipe_distance in Hyprland: expected 600; observed $(swipe_opt workspace_swipe_distance)"
        note "invert $(swipe_opt workspace_swipe_invert), create_new $(swipe_opt workspace_swipe_create_new), cancel_ratio $(swipe_opt workspace_swipe_cancel_ratio)"
    fi
    r=$(in_merge '.swipe.distance = 2001') && bad "a swipe distance of 2001 was accepted"
    note "distance 2001: $r"
    # user.lua sets the same option: the row says so (it needs Hyprland's live value to know), and
    # its value wins.
    if [ -n "${HAVE[hypr]:-}" ]; then
        n=$(user_lua_add 'hl.config({ gestures = { workspace_swipe_distance = 200 } })')
        hc reload >/dev/null
        wait_for 5 sf8_distance 200 || bad "user.lua's distance 200 should win in Hyprland; observed $(swipe_opt workspace_swipe_distance)"
        r=$(explain input | jq -c '.["swipe.distance"]')
        note "Explain swipe.distance: $r"
        expect_eq "the swipe distance's source" user_lua "$(jq -r .source <<<"$r")"
        expect_eq "its line in user.lua" "$n" "$(jq -r .detail.line <<<"$r")"
        user_lua_reset
    fi
    # A gesture in user.lua is reported as user.lua's.
    n=$(user_lua_add 'hl.gesture({ fingers = 5, direction = "left", action = "close" })')
    [ -z "${HAVE[hypr]:-}" ] || hc reload >/dev/null
    r=$(explain input | jq -c '.gestures')
    note "Explain gestures: $r"
    expect_eq "the gestures' source" user_lua "$(jq -r .source <<<"$r")"
    expect_eq "its line in user.lua" "$n" "$(jq -r .detail.line <<<"$r")"
    user_lua_reset
    r=$(set_section input "$orig") || bad "putting the input section back: $r"
    [ -z "${HAVE[hypr]:-}" ] || expect_eq "Hyprland's errors at the end" "$e0" "$(config_errors | wc -l)"
}
sf8_distance() { [ "$(swipe_opt workspace_swipe_distance)" = "$1" ]; }

check SF9 "History and restore: a main display, a bar, the planner and a gesture undo one by one; a restore puts them all back" sf9
sf9() {
    local p r s0 g0 ids
    s0=$(section monitors | jq -cS .); g0=$(section input | jq -cS .gestures)
    p=$(bus SaveRestorePoint s "uqlive screens") || { bad "save: $p"; return; }
    r=$(mon_merge '.primary = {make: "uqlive", model: "gone", serial: "0"}') || bad "main display: $r"
    r=$(mon_merge '.bar = {show: "main", on: []}') || bad "bar: $r"
    r=$(mon_merge '.workspaces = {strategy: "interleave", count: 6, manual: [], move_back: true}') || bad "planner: $r"
    r=$(in_merge ".gestures = $GESTURE_DEFAULTS + [{fingers:3,direction:\"down\",action:\"special\"}]") || bad "gesture: $r"
    note "History: $(history_json 4 | jq -r '[.[].summary] | join(" / ")')"
    r=$(history_json 4 | jq -r '[.[] | select(.origin == "settings") | .summary] | length')
    expect_eq "History rows from those four changes" 4 "$r"
    # History's newest row is the Undo itself after the first one, so take the rows' ids first.
    mapfile -t ids < <(history_json 4 | jq -r '.[].id')
    r=$(bus Undo sb "${ids[0]}" false) || bad "Undo of the gesture: $r"
    expect_eq "the gestures after Undo" "$g0" "$(section input | jq -cS .gestures)"
    r=$(bus Undo sb "${ids[1]}" false) || bad "Undo of the planner: $r"
    expect_eq "the planner after Undo" off "$(section monitors | jq -r '.workspaces.strategy // "off"')"
    # The bar and the main display share a settings area with the planner changed after them: Undo
    # asks first, and goes through when told to.
    r=$(bus Undo sb "${ids[2]}" false) && bad "Undo of the bar was accepted without asking, after another display change"
    [[ $r == *"changed again"* ]] || bad "expected the 'changed again after this' question; observed $r"
    r=$(bus Undo sb "${ids[2]}" true) || bad "Undo of the bar, anyway: $r"
    expect_eq "the bar after Undo" all "$(section monitors | jq -r '.bar.show // "all"')"
    r=$(bus Undo sb "${ids[3]}" true) || bad "Undo of the main display, anyway: $r"
    expect_eq "the main display after Undo" null "$(section monitors | jq -c '.primary // null')"
    r=$(mon_merge '.workspaces = {strategy: "sequential", count: 8, manual: [], move_back: false}') || bad "planner again: $r"
    r=$(in_merge ".gestures = $GESTURE_DEFAULTS + [{fingers:3,direction:\"down\",action:\"special\"}]") || bad "gesture again: $r"
    r=$(bus Restore s "$p") || bad "Restore: $r"
    expect_eq "the monitors section after the restore" "$s0" "$(section monitors | jq -cS .)"
    expect_eq "the gestures after the restore" "$g0" "$(section input | jq -cS .gestures)"
    bus DeleteRestorePoint s "$p" >/dev/null
}
