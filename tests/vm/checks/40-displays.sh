# shellcheck shell=bash
# Settings slice 3, Displays (docs/notes/settings-slice3-live-check.md): apply, keep and the
# automatic revert, driven through uqd's D-Bus API. The mirror and on/off checks need two outputs:
#   UQVM_GPU2="bochs-display,xres=1280,yres=800" tests/vm/run.sh --only '^D'
# The 15 s window is timed inside the guest (guest/uq-revert-timer), so ssh lag doesn't count.

UQ_BUS=(org.uq.Desktop1 /org/uq/Desktop1 org.uq.Desktop1)
# uqbus METHOD SIG ARGS...: one uqd call; prints the reply's first value, or the error text.
uqbus() {
    vgh busctl --user --json=short call "${UQ_BUS[@]}" "$@" 2>&1 |
        jq -Rr 'fromjson? // . | if type=="object" then (.data[0] // "") else . end'
}
# The live layout, compared by value (`monitors all` includes disabled outputs). Retries: hyprctl
# can miss a reply while Hyprland reconfigures outputs.
live() {
    local _ out
    for _ in 1 2 3 4 5; do
        out=$(vgh hyprctl monitors all -j 2>/dev/null |
            jq -c '[.[] | {name, id, width, height, refreshRate: (.refreshRate|floor), x, y, scale, disabled, mirrorOf, vrr}] | sort_by(.name)' 2>/dev/null) &&
            [ -n "$out" ] && { echo "$out"; return 0; }
        sleep 0.3
    done
    return 1
}
outputs() { live | jq -r '.[].name'; }
pending() { uqbus ListOutputs | jq -c '.pending'; }
apply() { uqbus ApplyMonitors s "$1"; } # prints the token, or the error
is_token() { [[ $1 =~ ^[0-9a-f]{32}$ ]]; }
out_field() { live | jq -r ".[] | select(.name==\"$1\") | .$2"; } # out_field OUTPUT FIELD
scale_is() { [ "$(out_field "$1" scale)" = "$2" ]; }
settings_up() { vgh pgrep -x uq-settings >/dev/null; }
uqd_active() { [ "$(vg systemctl --user is-active uqd)" = active ]; }

# wait_live WANT SECS: poll until the live layout is WANT; prints seconds taken, fails after SECS.
wait_live() {
    local want=$1 t0
    t0=$(hnow)
    while :; do
        [ "$(live)" = "$want" ] && { since "$t0"; return 0; }
        python3 -c "import sys,time; sys.exit(0 if time.time()-float(sys.argv[1]) > float(sys.argv[2]) else 1)" "$t0" "$2" && return 1
        sleep 0.3
    done
}

# expect_revert BEFORE T0: the layout goes back to BEFORE on its own, 13 to 25 s after T0 (host time
# of the apply; ssh lag and a slow config check add to it, so the 15 s itself is timed in the guest
# by expect_timed_revert).
expect_revert() {
    local before=$1 t0=$2 took
    if wait_live "$before" 35 >/dev/null; then
        took=$(since "$t0")
        note "back to the old layout ${took}s after the apply (host time)"
        python3 -c "import sys; sys.exit(0 if 13 <= float(sys.argv[1]) <= 25 else 1)" "$took" ||
            bad "expected the revert 13 to 25 s after the apply; observed ${took}s"
    else
        bad "expected the old layout back within 35 s; observed $(live)"
    fi
    [ "$(pending)" = null ] || bad "expected nothing pending after the revert; observed $(pending)"
}

# timed_apply JSON: apply in the guest and time the automatic revert there. Sets TIMED.
timed_apply() { TIMED=$(vgh uq-revert-timer "$1"); note "timer: $TIMED"; }
# expect_timed_revert: the old layout came back 15 s (14 to 16.5) after the change went live.
expect_timed_revert() {
    local r b w
    r=$(jq -r .returned_ms <<<"$TIMED") b=$(jq -r .back_ms <<<"$TIMED")
    [[ $(jq -r .reply <<<"$TIMED") == s\ * ]] || { bad "apply: $(jq -r .reply <<<"$TIMED")"; return; }
    [ "$b" != null ] || { bad "expected the old layout back on its own; it was not back 35 s after the apply"; return; }
    w=$((b - r))
    note "the apply took ${r} ms; the old layout was back ${w} ms after it returned"
    { [ "$w" -ge 14000 ] && [ "$w" -le 16500 ]; } ||
        bad "expected the revert 15 s after the change went live; observed ${w} ms"
    [ "$(pending)" = null ] || bad "expected nothing pending after the revert; observed $(pending)"
}

D1_OUT=Virtual-1
displays_prepare() {
    vput "$UQVM_DIR/guest/uq-revert-timer" /tmp/uq-revert-timer &&
        vroot install -m755 /tmp/uq-revert-timer /usr/local/bin/uq-revert-timer
    D1_OUT=$(outputs | head -n1)
    note "outputs: $(outputs | tr '\n' ' ')"
}

check D1 "Blackout: a mode the screen can't show reverts by itself within 15 s" d1
d1() {
    displays_prepare
    timed_apply "{\"outputs\":{\"$D1_OUT\":{\"mode\":\"7680x4320@240\"}}}"
    snap after
    expect_timed_revert
}

check D1b "Revert: a visible change (scale 2) goes live and reverts 15 s later" d1b
d1b() {
    displays_prepare
    timed_apply "{\"outputs\":{\"$D1_OUT\":{\"scale\":2}}}"
    [ "$(jq -r .live_ms <<<"$TIMED")" != null ] || bad "expected scale 2 to go live"
    expect_timed_revert
}

check D2 "Only output off: refused with 'at least one display must stay on'" d2
d2() {
    displays_prepare
    local before all r
    before=$(live)
    all=$(outputs | jq -R . | jq -sc 'map({key: ., value: {enabled: false}}) | from_entries | {outputs: .}')
    r=$(apply "$all")
    note "apply $all: $r"
    [[ $r == *"at least one display must stay on"* ]] || bad "expected the refusal; observed $r"
    [ "$(live)" = "$before" ] || bad "expected the layout unchanged; observed $(live)"
}

check D3 "Crash: killing uq-settings during the window still reverts on time" d3
d3() {
    displays_prepare
    vgh sh -c 'setsid uq-settings >/dev/null 2>&1 &'
    wait_for 15 settings_up || bad "uq-settings did not start"
    sleep 3
    vgsh '(sleep 3; pkill -KILL -x uq-settings) >/dev/null 2>&1 &'
    timed_apply "{\"outputs\":{\"$D1_OUT\":{\"scale\":2}}}"
    if settings_up; then bad "uq-settings was still running"; else note "uq-settings was killed during the window"; fi
    expect_timed_revert
}

check D4 "uqd restart: SIGKILL uqd during the window; the old layout is back after the restart" d4
d4() {
    displays_prepare
    local before tok t0
    before=$(live)
    t0=$(hnow)
    tok=$(apply "{\"outputs\":{\"$D1_OUT\":{\"scale\":2}}}")
    is_token "$tok" || { bad "expected a token; observed $tok"; return; }
    sleep 2
    vg systemctl --user kill -s KILL uqd
    note "killed uqd 2 s into the window"
    if wait_live "$before" 30 >/dev/null; then
        note "old layout back $(since "$t0")s after the apply"
    else
        bad "expected the old layout back after uqd restarted; observed $(live)"
    fi
    wait_for 20 uqd_active || bad "uqd did not come back: $(vg systemctl --user is-active uqd)"
    [ "$(pending)" = null ] || bad "expected nothing pending after the restart; observed $(pending)"
}

check D7 "Second apply: two applies within 15 s return to the layout from before the first" d7
d7() {
    displays_prepare
    local before tok1 tok2 t1 r
    before=$(live)
    tok1=$(apply "{\"outputs\":{\"$D1_OUT\":{\"scale\":2}}}")
    is_token "$tok1" || { bad "first apply: $tok1"; return; }
    sleep 5
    t1=$(hnow)
    tok2=$(apply "{\"outputs\":{\"$D1_OUT\":{\"scale\":1.5}}}")
    is_token "$tok2" || { bad "second apply: $tok2"; return; }
    note "second apply 5 s after the first; live: $(live | jq -c "[.[] | {name, scale}]")"
    r=$(uqbus ConfirmMonitors ss "$tok1" settings)
    if [ -n "$r" ]; then note "confirm with the replaced token: $r"; else bad "the replaced token was accepted for confirm"; fi
    expect_revert "$before" "$t1"
}

check D8 "Mirror and on/off: mirror one output onto the other, and turn one of two off" d8
d8() {
    displays_prepare
    local before a b tok t0 aid
    mapfile -t o < <(outputs)
    [ "${#o[@]}" -ge 2 ] || { na "needs two outputs (UQVM_GPU2=bochs-display,xres=1280,yres=800)"; return; }
    a=${o[0]} b=${o[1]}
    before=$(live)
    aid=$(out_field "$a" id)
    t0=$(hnow)
    tok=$(apply "{\"outputs\":{\"$b\":{\"mirror_of\":\"$a\"}}}")
    is_token "$tok" || { bad "mirror apply: $tok"; return; }
    sleep 1.5
    note "mirror: $(live)"
    [ "$(out_field "$b" mirrorOf)" = "$aid" ] || bad "expected $b to mirror $a (id $aid); observed $(live)"
    snap mirror
    expect_revert "$before" "$t0"
    t0=$(hnow)
    tok=$(apply "{\"outputs\":{\"$b\":{\"enabled\":false}}}")
    is_token "$tok" || { bad "off apply: $tok"; return; }
    sleep 1.5
    note "off: $(live)"
    [ "$(out_field "$b" disabled)" = true ] || bad "expected $b disabled; observed $(live)"
    expect_revert "$before" "$t0"
}

check D9 "Scale: 1.25 and 1.5 at 1920x1080 and 1920x1440 (the VM has no 4K mode)" d9
d9() {
    displays_prepare
    local before mode s tok got
    before=$(live)
    for mode in 1920x1080@60 1920x1440@60; do
        for s in 1.25 1.5; do
            tok=$(apply "{\"outputs\":{\"$D1_OUT\":{\"mode\":\"$mode\",\"scale\":$s}}}")
            is_token "$tok" || { bad "$mode scale $s: $tok"; continue; }
            sleep 1.5
            got=$(live | jq -c ".[] | select(.name==\"$D1_OUT\") | {width,height,scale}")
            note "$mode scale $s -> $got"
            [ "$(jq -r .scale <<<"$got")" = "$s" ] || bad "$mode: expected scale $s; observed $got"
            snap "${mode%@*}-$s"
            uqbus RevertMonitors s "$tok" >/dev/null
            wait_live "$before" 10 >/dev/null || bad "Revert did not restore the layout: $(live)"
        done
    done
}

check D10 "VRR: on and off are accepted" d10
d10() {
    displays_prepare
    local before v tok
    before=$(live)
    for v in 1 0; do
        tok=$(apply "{\"outputs\":{\"$D1_OUT\":{\"vrr\":$v}}}")
        is_token "$tok" || { bad "vrr $v: $tok"; continue; }
        sleep 1
        note "vrr $v -> $(out_field "$D1_OUT" vrr)"
        uqbus RevertMonitors s "$tok" >/dev/null
        wait_live "$before" 10 >/dev/null || bad "Revert did not restore the layout: $(live)"
    done
    note "the VM's display has no adaptive sync, so the live flag can't turn on here; only acceptance is checked"
}

check D11 "Gaps during the window: a Look change keeps the displays on the candidate" d11
d11() {
    displays_prepare
    local before tok t0 look r
    before=$(live)
    look=$(uqbus GetSection s look)
    t0=$(hnow)
    tok=$(apply "{\"outputs\":{\"$D1_OUT\":{\"scale\":2}}}")
    is_token "$tok" || { bad "apply: $tok"; return; }
    sleep 3
    r=$(uqbus SetSection sss look "$(jq -c '.gaps_out = 17' <<<"$look")" settings)
    note "gaps_out 17 during the window: ${r:-ok}"
    [ -z "$r" ] || bad "the Look change was refused: $r"
    sleep 2
    scale_is "$D1_OUT" 2 ||
        bad "expected the displays to stay on the candidate (scale 2) after the Look change; observed $(live)"
    expect_revert "$before" "$t0"
    uqbus SetSection sss look "$look" settings >/dev/null
}

check D14 "Keep, History and Undo: one Displays entry; Undo restores without a countdown" d14
d14() {
    displays_prepare
    local before tok after h id r
    before=$(live)
    tok=$(apply "{\"outputs\":{\"$D1_OUT\":{\"scale\":1.5}}}")
    is_token "$tok" || { bad "apply: $tok"; return; }
    sleep 2
    r=$(uqbus ConfirmMonitors ss "$tok" settings)
    [ -z "$r" ] || bad "confirm: $r"
    after=$(live)
    sleep 16
    [ "$(live)" = "$after" ] || bad "expected the kept layout to stay after 16 s; observed $(live)"
    note "stored after keep: $(uqbus ListOutputs | jq -c .stored)"
    h=$(uqbus ListHistory u 5)
    note "newest history entry: $(jq -c '.[0]' <<<"$h" 2>/dev/null || echo "$h")"
    id=$(jq -r '.[0].id' <<<"$h" 2>/dev/null)
    [ "$(jq -r '.[0].domain' <<<"$h" 2>/dev/null)" = monitors ] && [ "$(jq -r '.[0].origin' <<<"$h" 2>/dev/null)" = settings ] ||
        bad "expected the newest entry to be monitors from settings; observed $(jq -c '.[0]' <<<"$h")"
    r=$(uqbus Undo sb "$id" false)
    note "undo $id: $r"
    if wait_live "$before" 5 >/dev/null; then note "undo restored the old layout"; else bad "expected the old layout after Undo; observed $(live)"; fi
    [ "$(pending)" = null ] || bad "expected no countdown after Undo; observed $(pending)"
}

check D15 "eval: the candidate goes live without saving; record the Hyprland version" d15
d15() {
    displays_prepare
    local before tok t0 st
    note "hyprland: $(vgh hyprctl version -j | jq -r '.tag // .version // .commit' 2>/dev/null)"
    before=$(live)
    st=$(uqbus ListOutputs | jq -c .stored)
    t0=$(hnow)
    tok=$(apply "{\"outputs\":{\"$D1_OUT\":{\"scale\":2}}}")
    is_token "$tok" || { bad "apply: $tok"; return; }
    if wait_for 5 scale_is "$D1_OUT" 2; then
        note "scale 2 live $(since "$t0")s after the call (host time)"
    else
        bad "expected scale 2 live; observed $(live)"
    fi
    [ "$(uqbus ListOutputs | jq -c .stored)" = "$st" ] || bad "expected nothing saved during the window"
    vgsh 'grep -q "scale = \"2" ~/.config/hypr/uq/generated.lua' && bad "generated.lua changed during the window"
    uqbus RevertMonitors s "$tok" >/dev/null
    wait_live "$before" 10 >/dev/null || bad "Revert did not restore the layout"
}
