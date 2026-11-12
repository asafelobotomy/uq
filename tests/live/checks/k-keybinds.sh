# shellcheck shell=bash
# Settings keybinds (Phase 3 slice 4, keyboard shortcuts): docs/notes/settings-slice4-live-check.md.
# Run through the same uqd calls the Keyboard shortcuts page makes (`SetSection("keybinds")`,
# `ListBinds`, `PauseShortcuts`) and `uqctl keys`, then read back what Hyprland has bound
# (`hyprctl binds -j`) and what uQ's Explain says about each row.
#
# What stays manual, and why:
#   1   upgrading the package over a running one, moving generated.lua aside (the fallback keys
#       are base.lua's, outside uqd) and pressing keys: needs a person and a real install
#   2   that Super + K really opens Spotlight and Super + Space doesn't (a key press; SK2 checks
#       that Hyprland has the new bind and no longer the old one)
#   3   the editor naming "Close the window" before Save and Save staying disabled (Qt)
#   4   starting Firefox in its own scope with its Apps and scaling fix, and `systemctl --user
#       status 'run-*.scope'` under app.slice (a real session with systemd; SK4 runs the generated
#       command text through sh to check its quoting)
#   5   pressing Super + Space in the capture and whether the empty submap passes keys to the app
#       on 0.56.2 and 0.57.x, and a compositor link dying mid-capture (SK5 and SK6 check the
#       submap itself, the token and the 15 s timer)
#   6   the lock screen: volume keys over it, Super + Ctrl + L after killing the lock client
#   7   `uqctl profile next` from a key press (the VM's SB6k)
#   11  tapping Super alone opens the app menu
#   13  every key name in the capture (Qt key codes)
# Item 10 (`uqctl keys reset`, `keys list`) is SK1 and SK2; item 12 is SK9; item 8 is SK7; item 9 is
# SK8. user.lua's modification time is checked after every check by run.sh.
#
# Everything a check changes in the keybinds section is put back by the check; kb_cleanup (below,
# called from lib.sh's cleanup_check) drops any shortcut of ours a failed check left behind.

KB_DEFAULT_COMBO='SUPER + Space'
# kb_put JSON: write the whole keybinds section; prints the error, if any.
kb_put() { set_section keybinds "$1"; }
# kb_merge JQ: change the section with a jq program.
kb_merge() { kb_put "$(section keybinds | jq -c "$1")"; }
kb_row() { "$UQCTL" keys list | awk -F'\t' -v i="$1" '$1 == i'; }
kb_src() { explain keybinds | jq -r --arg i "binds.$1" '.[$i].source // "none"'; }
# kb_live ID: "key/modmask" of Hyprland's bind for that shortcut ("" when it has none), lowercased.
kb_live() {
    hc binds -j 2>/dev/null | jq -r --arg d "uq:$1" \
        '[.[] | select(.description == $d) | "\(.key | ascii_downcase)/\(.modmask)"] | join(",")'
}
kb_live_is() { [ "$(kb_live "$1")" = "$2" ]; }
# gen_has TEXT: generated.lua has this line (fixed-string match).
gen_has() { grep -qF -- "$1" "$CFG/hypr/uq/generated.lua"; }
gen_bind() { grep -F -- "description = \"uq:$1\"" "$CFG/hypr/uq/generated.lua"; }
kb_errors() { config_errors | wc -l; }
kb_cleanup() {
    local s
    s=$(section keybinds 2>/dev/null) || return 0
    [ -n "$s" ] || return 0
    jq -e '[.custom[]? | select(.name | startswith("uqlive"))] | length > 0' <<<"$s" >/dev/null 2>&1 || return 0
    kb_put "$(jq -c '.custom |= map(select(.name | startswith("uqlive") | not))' <<<"$s")" >/dev/null
}

check SK1 "uqctl keys list: every shortcut with its keys; Hyprland has what generated.lua says" sk1
sk1() {
    local rows n ids total
    rows=$("$UQCTL" keys list) || { bad "uqctl keys list failed"; return; }
    n=$(grep -c '^uq\.' <<<"$rows")
    note "$n of uQ's shortcuts listed; spotlight: $(grep -P '^uq\.spotlight\t' <<<"$rows")"
    expect_eq "rows in 'uqctl keys list' that are one of uQ's" "$(bus ListBinds | jq '[.binds[] | select(.custom | not)] | length')" "$n"
    [ "$n" -ge 50 ] || bad "expected at least 50 of uQ's shortcuts; observed $n"
    awk -F'\t' 'NF != 3' <<<"$rows" | grep -q . && bad "a row that isn't id, keys and label separated by tabs"
    expect_eq "Open Spotlight's keys" "$KB_DEFAULT_COMBO" "$(kb_row uq.spotlight | cut -f2)"
    expect_eq "Close the window's keys" "SUPER + Q" "$(kb_row uq.close | cut -f2)"
    [ -n "${HAVE[hypr]:-}" ] || { note "Hyprland's own binds need Hyprland"; return; }
    total=$(grep -c 'description = "uq:' "$CFG/hypr/uq/generated.lua")
    ids=$(hc binds -j | jq '[.[] | select((.description // "") | startswith("uq:"))] | length')
    note "generated.lua has $total uq: binds; Hyprland has $ids"
    expect_eq "uq: binds in Hyprland" "$total" "$ids"
    [ "$total" -ge 50 ] || bad "expected at least 50 uq: binds in generated.lua; observed $total"
    local row bad_rows
    bad_rows=$(explain keybinds | jq -r 'to_entries[] | select(.value.source == "runtime" or .value.source == "error") | .key' | tr '\n' ' ')
    [ -z "$bad_rows" ] || bad "Explain says these shortcuts aren't live, or the last write was refused: $bad_rows"
    row=$(explain keybinds | jq -c '.["binds.uq.spotlight"]')
    note "Explain: $row"
    expect_eq "Open Spotlight, live in Hyprland" "space/64" "$(kb_live uq.spotlight)"
}

check SK2 "Rebind a default: generated.lua and Hyprland follow, the old keys go, reset and Undo bring them back" sk2
sk2() {
    local r e0 e id
    e0=$(kb_errors)
    r=$(kb_merge '.overrides += [{id: "uq.spotlight", combo: "SUPER + K"}]') || { bad "rebinding: $r"; return; }
    gen_has 'hl.bind("SUPER + K", hl.dsp.global("uq:spotlight")' || bad "generated.lua has no SUPER + K bind for spotlight"
    gen_has 'hl.bind("SUPER + Space", hl.dsp.global("uq:spotlight")' && bad "generated.lua still binds SUPER + Space to spotlight"
    expect_eq "Open Spotlight in 'keys list'" "SUPER + K" "$(kb_row uq.spotlight | cut -f2)"
    e=$(newest); note "History: $(jq -r .summary <<<"$e")"
    [[ $(jq -r .summary <<<"$e") == *"Open Spotlight"*"now uses Super + K" ]] || bad "expected the History row to say Open Spotlight now uses Super + K; observed $(jq -r .summary <<<"$e")"
    if [ -n "${HAVE[hypr]:-}" ]; then
        wait_for 5 kb_live_is uq.spotlight k/64 || bad "Hyprland's bind for spotlight: expected k/64; observed $(kb_live uq.spotlight)"
        expect_eq "Explain for spotlight" uq "$(kb_src uq.spotlight)"
        [ "$(hc binds -j | jq '[.[] | select(.description == "uq:uq.spotlight")] | length')" = 1 ] || bad "Hyprland has more than one spotlight bind"
        expect_eq "Hyprland errors after the rebind" "$e0" "$(kb_errors)"
    fi
    r=$("$UQCTL" keys reset uq.spotlight) || bad "keys reset: $r"
    note "reset: $r"
    gen_has 'hl.bind("SUPER + Space", hl.dsp.global("uq:spotlight")' || bad "generated.lua has no SUPER + Space bind after the reset"
    gen_has 'hl.bind("SUPER + K", hl.dsp.global("uq:spotlight")' && bad "generated.lua still binds SUPER + K after the reset"
    expect_eq "Open Spotlight after the reset" "$KB_DEFAULT_COMBO" "$(kb_row uq.spotlight | cut -f2)"
    [ -z "${HAVE[hypr]:-}" ] || {
        wait_for 5 kb_live_is uq.spotlight space/64 || bad "Hyprland's bind after the reset: expected space/64; observed $(kb_live uq.spotlight)"
        expect_eq "Explain after the reset" default "$(kb_src uq.spotlight)"
    }
    # Undo of a rebind, from History.
    r=$(kb_merge '.overrides += [{id: "uq.spotlight", combo: "SUPER + K"}]') || bad "rebinding again: $r"
    id=$(newest_id)
    r=$(bus Undo sb "$id" false) || bad "Undo: $r"
    expect_eq "Open Spotlight after Undo" "$KB_DEFAULT_COMBO" "$(kb_row uq.spotlight | cut -f2)"
    [ -z "${HAVE[hypr]:-}" ] || wait_for 5 kb_live_is uq.spotlight space/64 || bad "Hyprland's bind after Undo: observed $(kb_live uq.spotlight)"
    # A shortcut turned off has no bind at all.
    r=$(kb_merge '.overrides += [{id: "uq.close", enabled: false}]') || bad "turning Close off: $r"
    gen_bind uq.close >/dev/null && bad "generated.lua still binds Close while it is off"
    [ -z "${HAVE[hypr]:-}" ] || {
        wait_for 5 kb_live_is uq.close '' || bad "Hyprland still has Close's bind while it is off: $(kb_live uq.close)"
    }
    r=$("$UQCTL" keys reset --all) || bad "keys reset --all: $r"
    gen_bind uq.close >/dev/null || bad "generated.lua has no Close bind after reset --all"
    expect_eq "overrides after reset --all" "[]" "$(section keybinds | jq -c .overrides)"
}

check SK3 "Conflicts and bad keys are refused in plain words, and nothing changes" sk3
sk3() {
    local r before h0 s0 c
    s0=$(section keybinds); h0=$(newest_id); before=$(sha256sum "$CFG/hypr/uq/generated.lua")
    r=$(kb_merge '.custom += [{name: "uqlive q", combo: "SUPER + Q", action: {kind: "command", command: "foot"}}]') && bad "a shortcut on Super + Q was accepted"
    note "custom on Super + Q: $r"
    [[ $r == *"uqlive q"* && $r == *"Close the window"* && $r == *"Super + Q"* ]] || bad "expected a refusal naming both shortcuts; observed $r"
    r=$(kb_merge '.overrides += [{id: "uq.spotlight", combo: "SUPER + Q"}]') && bad "Spotlight on Super + Q was accepted"
    [[ $r == *"Open Spotlight"* && $r == *"Close the window"* ]] || bad "expected both names in the refusal; observed $r"
    for c in 'SUPER + Nonsensekey' 'SUPER + CAPS + A' 'SUPER' 'A + B' ''; do
        r=$(kb_merge ".custom += [{name: \"uqlive bad\", combo: \"$c\", action: {kind: \"command\", command: \"foot\"}}]") && bad "the combo '$c' was accepted"
        note "'$c': $r"
        case "$r" in *panic* | *org.uq* | *serde* | *'at line'*) bad "the refusal for '$c' isn't plain: $r" ;; esac
    done
    r=$(kb_merge '.custom += [{name: "uqlive long", combo: "SUPER + SHIFT + F9", action: {kind: "command", command: ([range(1025) | "x"] | join(""))}}]') && bad "a 1025-character command was accepted"
    r=$(kb_merge '.custom += [{name: "uqlive lock", combo: "SUPER + SHIFT + F9", locked: true, action: {kind: "command", command: "foot"}}]') && bad "a command that works over the lock screen was accepted"
    note "locked command: $r"
    r=$(kb_merge '.overrides += [{id: "uq.nothere", combo: "SUPER + F9"}]') && bad "an override of a shortcut that doesn't exist was accepted"
    note "unknown id: $r"
    expect_eq "the keybinds section after the refusals" "$s0" "$(section keybinds)"
    expect_eq "History after the refusals" "$h0" "$(newest_id)"
    expect_eq "generated.lua after the refusals" "$before" "$(sha256sum "$CFG/hypr/uq/generated.lua")"
    # The terminal says the same about an id that isn't one of uQ's.
    r=$("$UQCTL" keys reset uq.nothere 2>&1) && bad "keys reset of an unknown id was accepted"
    [[ $r == *"isn't one of uQ's shortcuts"* ]] || bad "expected \"isn't one of uQ's shortcuts\"; observed $r"
}

check SK4 "Your own shortcut: rendered with its quoting, listed, live in Hyprland, kept by reset --all" sk4
sk4() {
    local r id cmd line lit run out=$WORK/sk4.out
    # printf '%s' 'it'"'"'s $(echo x)' writes it's $(echo x): single quotes, a dollar and a command
    # substitution that must reach the shell exactly as typed.
    cmd="printf '%s' 'it'\"'\"'s \$(echo x)' > \"$out\""
    r=$(kb_put "$(section keybinds | jq -c --arg c "$cmd" '.custom += [{name: "uqlive run", combo: "SUPER + SHIFT + F9", action: {kind: "command", command: $c}}]')") || { bad "adding it: $r"; return; }
    id=$(section keybinds | jq -r '.custom[] | select(.name == "uqlive run") | .id')
    note "id: $id"
    [[ $id == k-* ]] || bad "expected an id like k-1234; observed '$id'"
    line=$(gen_bind "$id")
    note "generated.lua: $line"
    [[ $line == 'hl.bind("SUPER + SHIFT + F9", hl.dsp.exec_cmd("systemd-run --user --scope --quiet --collect --slice=app.slice -- sh -c '* ]] ||
        bad "expected an exec_cmd through systemd-run in app.slice; observed $line"
    expect_eq "the shortcut in 'keys list'" "SUPER + SHIFT + F9" "$(kb_row "$id" | cut -f2)"
    # The Lua string literal, unescaped (the escapes uQ writes are JSON's), minus systemd-run: what
    # Hyprland hands /bin/sh.
    lit=$(sed -E "s/^hl\\.bind\\(\"[^\"]*\", hl\\.dsp\\.exec_cmd\\((.*)\\), \\{ description = \"uq:$id\" \\}\\)\$/\\1/" <<<"$line")
    run=$(jq -r . <<<"$lit" 2>/dev/null) || { bad "the generated command isn't a plain string literal: $lit"; run=''; }
    if [ -n "$run" ]; then
        rm -f -- "$out"
        sh -c "${run#*-- }" >/dev/null 2>&1
        out=$(cat "$WORK/sk4.out" 2>/dev/null)
        note "the command printed: $out"
        expect_eq "what the command wrote" 'it'"'"'s $(echo x)' "$out"
    fi
    if [ -n "${HAVE[hypr]:-}" ]; then
        wait_for 5 kb_live_is "$id" f9/65 || bad "Hyprland's bind for $id: expected f9/65; observed $(kb_live "$id")"
        expect_eq "Explain for your own shortcut" uq "$(kb_src "$id")"
    fi
    r=$("$UQCTL" keys reset --all) || bad "keys reset --all: $r"
    expect_eq "your own shortcut after reset --all" 1 "$(section keybinds | jq '[.custom[] | select(.id == "'"$id"'")] | length')"
    r=$(kb_merge '.custom |= map(select(.id != "'"$id"'"))') || bad "removing it: $r"
    gen_bind "$id" >/dev/null && bad "generated.lua still has the removed shortcut"
    [ -z "${HAVE[hypr]:-}" ] || wait_for 5 kb_live_is "$id" '' || bad "Hyprland still has the removed shortcut: $(kb_live "$id")"
    rm -f -- "$WORK/sk4.out"
}

check SK5 "Pause and resume: the empty submap while paused, a wrong token refused, a second pause replaces the first" sk5
sk5() {
    local r t1 t2
    if [ -z "${HAVE[hypr]:-}" ]; then
        r=$(bus PauseShortcuts) && bad "pausing was accepted with no Hyprland"
        note "no Hyprland: $r"
        [[ $r == *"Hyprland isn't answering"* ]] || bad "expected \"Hyprland isn't answering\"; observed $r"
        return
    fi
    note "submap before: $(hc submap)"
    t1=$(bus PauseShortcuts) || {
        bad "PauseShortcuts: $t1"
        note "hyprctl eval of the same Lua: $(hc eval 'hl.dispatch(hl.dsp.submap("uq_capture"))' 2>&1 | head -n 3); submap now $(hc submap 2>&1)"
        note "the submap is defined in generated.lua: $(grep -n 'define_submap' "$CFG/hypr/uq/generated.lua")"
        hc eval 'hl.dispatch(hl.dsp.submap("reset"))' >/dev/null 2>&1
        return
    }
    note "token: $t1; submap while paused: $(hc submap)"
    [[ $(hc submap) == *uq_capture* ]] || bad "expected the uq_capture submap while paused; observed $(hc submap)"
    r=$(bus ResumeShortcuts s "not-$t1") && bad "a wrong token was accepted"
    note "wrong token: $r"
    [[ $r == *"aren't paused for that token"* ]] || bad "expected \"Shortcuts aren't paused for that token\"; observed $r"
    [[ $(hc submap) == *uq_capture* ]] || bad "a wrong token ended the pause"
    t2=$(bus PauseShortcuts) || bad "a second PauseShortcuts: $t2"
    r=$(bus ResumeShortcuts s "$t1") && bad "the first pause's token still works after a second pause"
    [[ $(hc submap) == *uq_capture* ]] || bad "the first token's resume ended the second pause"
    r=$(bus ResumeShortcuts s "$t2") || bad "ResumeShortcuts: $r"
    note "submap after resume: $(hc submap)"
    [[ $(hc submap) == *uq_capture* ]] && bad "the shortcuts are still paused after the resume"
    r=$(bus ResumeShortcuts s "$t2") && bad "a token already used was accepted"
    # A restart of uqd mid-capture always leaves the normal submap.
    if [ -n "${HAVE[restart]:-}" ]; then
        bus PauseShortcuts >/dev/null
        uqd_restart || { bad "uqd didn't restart"; return; }
        wait_for 10 uqd_up
        wait_for 10 sk5_normal || bad "a restart of uqd mid-capture left the submap at $(hc submap)"
    fi
}
sk5_normal() { [[ $(hc submap) != *uq_capture* ]]; }

check SK6 "Pause comes back on its own after 15 s" sk6 hypr slow
sk6() {
    local t s0 s end r
    t=$(bus PauseShortcuts) || { bad "PauseShortcuts: $t"; return; }
    s0=$(date +%s)
    sleep 5
    [[ $(hc submap) == *uq_capture* ]] || bad "the shortcuts came back after 5 s, not 15"
    wait_for 20 sk5_normal || { bad "the shortcuts were still paused after 20 s"; bus ResumeShortcuts s "$t" >/dev/null; return; }
    end=$(date +%s); s=$((end - s0))
    note "back after about $s s"
    [ "$s" -ge 13 ] && [ "$s" -le 18 ] || bad "expected the shortcuts back after about 15 s; observed $s s"
    r=$(bus ResumeShortcuts s "$t") && bad "a token that timed out still resumes"
    return 0
}

check SK7 "user.lua: an unbind is reported as 'Overridden in user.lua'; your marked bind is listed; a clash saves" sk7 hypr
sk7() {
    local n e r mark
    n=$(user_lua_add 'hl.unbind("SUPER + Q")')
    hc reload >/dev/null
    wait_for 5 kb_live_is uq.close '' || note "Hyprland still has Close's bind: $(kb_live uq.close)"
    e=$(explain keybinds | jq -c '.["binds.uq.close"]')
    note "user.lua line $n; Explain: $e"
    expect_eq "Close's source" user_lua "$(jq -r .source <<<"$e")"
    expect_eq "Close's line in user.lua" "$n" "$(jq -r .detail.line <<<"$e")"
    user_lua_reset
    wait_for 5 kb_live_is uq.close q/64 || bad "Close's bind didn't come back after taking the unbind out: $(kb_live uq.close)"
    expect_eq "Close's source after taking it out" default "$(kb_src uq.close)"
    mark='hl.bind("SUPER + F10", hl.dsp.exec_cmd("foot"), { description = "uqlive" }) -- uq:bind Terminal (foot)'
    n=$(user_lua_add "$mark")
    hc reload >/dev/null
    r=$(bus ListBinds | jq -c '.user_lua')
    note "user_lua rows: $r"
    expect_eq "marked binds in user.lua" "$n" "$(jq -r '[.[] | select(.text | test("Terminal"))][0].line // empty' <<<"$r")"
    "$UQCTL" keys list | grep -q "^user.lua:$n	SUPER + F10	" || bad "uqctl keys list has no user.lua:$n row for SUPER + F10: $("$UQCTL" keys list | grep '^user.lua')"
    # A uQ shortcut on the same keys warns, but saves.
    r=$(kb_merge '.custom += [{name: "uqlive clash", combo: "SUPER + F10", action: {kind: "command", command: "foot"}}]') || bad "a shortcut that clashes with user.lua was refused: $r"
    kb_merge '.custom |= map(select(.name != "uqlive clash"))' >/dev/null
    user_lua_reset
}

check SK8 "Profiles: one can turn a shortcut off and on; a hand-edited key or a command is refused" sk8
sk8() {
    local r id=${LIVE_PREFIX}-keys
    r=$(bus SetProfile sb "$(printf 'format = 1\nid = "%s"\nname = "uqlive keys"\n\n[keybinds]\n"uq.close" = false\n' "$id")" false) || { bad "SetProfile: $r"; return; }
    r=$(bus ApplyProfile s "$id") || { bad "ApplyProfile: $r"; return; }
    note "ApplyProfile: $(jq -c . <<<"$r" | cut -c1-200)"
    gen_bind uq.close >/dev/null && bad "generated.lua still binds Close with the profile on"
    expect_eq "Close in 'keys list'" "SUPER + Q" "$(kb_row uq.close | cut -f2)"
    kb_row uq.close | grep -q '(off)' || bad "'keys list' doesn't say Close is off"
    [ -z "${HAVE[hypr]:-}" ] || wait_for 5 kb_live_is uq.close '' || bad "Hyprland still has Close's bind with the profile on: $(kb_live uq.close)"
    r=$(bus ProfileOff) || bad "ProfileOff: $r"
    gen_bind uq.close >/dev/null || bad "generated.lua has no Close bind after Back to normal"
    [ -z "${HAVE[hypr]:-}" ] || wait_for 5 kb_live_is uq.close q/64 || bad "Close's bind didn't come back: $(kb_live uq.close)"
    r=$(bus SetProfile sb "$(printf 'format = 1\nid = "%s-x"\nname = "uqlive x"\n\n[keybinds]\n"uq.spotlight" = "SUPER + K"\n' "$LIVE_PREFIX")" false) && bad "a profile holding keys for a shortcut was accepted"
    note "keys in a profile: $r"
    r=$(bus SetProfile sb "$(printf 'format = 1\nid = "%s-y"\nname = "uqlive y"\n\n[keybinds."k-1234"]\ncommand = "foot"\n' "$LIVE_PREFIX")" false) && bad "a profile holding a command was accepted"
    note "a command in a profile: $r"
}

check SK9 "History and restore: undo an added shortcut; restoring a point takes a rebind and a shortcut back" sk9
sk9() {
    local r p id
    p=$(bus SaveRestorePoint s "uqlive keys") || { bad "save: $p"; return; }
    r=$(kb_merge '.overrides += [{id: "uq.spotlight", combo: "SUPER + K"}]') || bad "rebinding: $r"
    r=$(kb_merge '.custom += [{name: "uqlive own", combo: "SUPER + SHIFT + F8", action: {kind: "command", command: "foot"}}]') || bad "adding: $r"
    id=$(section keybinds | jq -r '.custom[] | select(.name == "uqlive own") | .id')
    note "History: $(history_json 2 | jq -r '[.[].summary] | join(" / ")')"
    gen_bind "$id" >/dev/null || bad "generated.lua has no bind for the added shortcut"
    r=$(bus Undo sb "$(newest_id)" false) || bad "Undo of the added shortcut: $r"
    gen_bind "$id" >/dev/null && bad "Undo left the added shortcut in generated.lua"
    expect_eq "custom shortcuts after Undo" 0 "$(section keybinds | jq '.custom | length')"
    expect_eq "Open Spotlight after Undo (the rebind stays)" "SUPER + K" "$(kb_row uq.spotlight | cut -f2)"
    kb_merge '.custom += [{name: "uqlive own", combo: "SUPER + SHIFT + F8", action: {kind: "command", command: "foot"}}]' >/dev/null
    r=$(bus Restore s "$p") || bad "Restore: $r"
    expect_eq "Open Spotlight after the restore" "$KB_DEFAULT_COMBO" "$(kb_row uq.spotlight | cut -f2)"
    expect_eq "custom shortcuts after the restore" 0 "$(section keybinds | jq '.custom | length')"
    [ -z "${HAVE[hypr]:-}" ] || wait_for 5 kb_live_is uq.spotlight space/64 || bad "Hyprland's spotlight bind after the restore: $(kb_live uq.spotlight)"
    bus DeleteRestorePoint s "$p" >/dev/null
}
