# shellcheck shell=bash
# Settings slice A (trust): docs/notes/settings-slice-a-live-check.md. The app's own keys (Ctrl+Z,
# Ctrl+Shift+Z) are the VM's (tests/vm/checks/60-settings.sh); here undo and redo go through the
# same uqd calls the app makes (a redo is an undo of the undo).

# merge DOMAIN JQ [ORIGIN]: set the section to (section | JQ).
merge() { set_section "$1" "$(section "$1" | jq -c "$2")" "${3:-settings}"; }
look_val() { section look | jq -c --arg k "$1" '.[$k]'; }
kind_of() { jq -r '.kind | if type == "object" then (to_entries[0] | "\(.key):\(.value)") else tostring end'; }

check SA1 "Overridden in user.lua: the gaps row names user.lua and its line, and clears after" sa1 hypr
sa1() {
    local r n e
    r=$(merge look '.gaps_in = 5') || { bad "setting gaps to 5: $r"; return; }
    n=$(user_lua_add 'hl.config({ general = { gaps_in = 2 } })')
    hc reload >/dev/null; sleep 1
    e=$(explain look | jq -c .gaps_in)
    note "user.lua line $n sets gaps_in = 2; Explain: $e"
    expect_eq "gaps source" user_lua "$(jq -r .source <<<"$e")"
    expect_eq "user.lua line" "$n" "$(jq -r .detail.line <<<"$e")"
    user_lua_reset; sleep 1
    e=$(explain look | jq -c .gaps_in)
    note "line removed and reloaded; Explain: $e"
    [ "$(jq -r .source <<<"$e")" != user_lua ] || bad "expected the user.lua note gone after removing the line; observed $e"
}

check SA2 "Changed while running: a hyprctl change shows as runtime, and Re-apply puts it back" sa2 hypr
sa2() {
    local r e out
    r=$(merge look '.gaps_out = 12') || { bad "setting outer gaps: $r"; return; }
    sleep 0.5
    out=$(hc keyword general:gaps_out 30 2>&1)
    note "hyprctl keyword general:gaps_out 30: $out"
    if [[ $out != *ok* ]]; then
        out=$(hc eval 'hl.config({ general = { gaps_out = 30 } })' 2>&1)
        note "hyprctl eval: $out"
    fi
    sleep 0.5
    e=$(explain look | jq -c .gaps_out)
    note "Explain after the change: $e"
    expect_eq "gaps_out source" runtime "$(jq -r .source <<<"$e")"
    r=$(bus Reapply s look) || bad "Reapply: $r"
    sleep 1
    e=$(explain look | jq -c .gaps_out)
    note "Explain after Re-apply: $e"
    [ "$(jq -r .source <<<"$e")" = uq ] || bad "expected the stored value live again after Re-apply; observed $e"
}

check SA3 "Undo and redo: three changes undo newest first and redo oldest first" sa3
sa3() {
    local g0 r0 t0 ids=() undos=() u i
    g0=$(look_val gaps_in) r0=$(look_val rounding) t0=$(bus GetTheme)
    merge look '.gaps_in = 6' >/dev/null; ids+=("$(newest_id)")
    merge look '.rounding = 3' >/dev/null; ids+=("$(newest_id)")
    bus SetTheme sss velvet light settings >/dev/null; ids+=("$(newest_id)")
    note "changes: ${ids[*]}"
    # The app forces a step when only its own undos and redos changed the domain since (two of
    # the three changes are on Look), so only the first step goes through uqd's conflict check.
    local force=false
    for ((i = 2; i >= 0; i--)); do
        u=$(bus Undo sb "${ids[$i]}" "$force") || { bad "undo ${ids[$i]}: $u"; return; }
        undos+=("$u") force=true
    done
    expect_eq "gaps after undo" "$g0" "$(look_val gaps_in)"
    expect_eq "rounding after undo" "$r0" "$(look_val rounding)"
    expect_eq "theme after undo" "$t0" "$(bus GetTheme)"
    # Redo: undo the undos, oldest undone first (the last one in undos).
    for ((i = 2; i >= 0; i--)); do
        u=$(bus Undo sb "${undos[$i]}" true) || bad "redo ${undos[$i]}: $u"
    done
    expect_eq "gaps after redo" 6 "$(look_val gaps_in)"
    expect_eq "rounding after redo" 3 "$(look_val rounding)"
    expect_eq "theme after redo" velvet "$(bus GetTheme)"
    note "history now: $(history_json 6 | jq -r '.[] | "\(.origin): \(.summary)"' | tr '\n' ';')"
}

check SA4 "Changes from the terminal: origin uqctl, and uqctl undo reverts it" sa4
sa4() {
    local t0 e out
    t0=$(bus GetTheme)
    out=$("$UQCTL" theme set rosewood light 2>&1) || { bad "uqctl theme set: $out"; return; }
    e=$(newest)
    note "newest entry: $(jq -c '{origin, domain, summary}' <<<"$e")"
    expect_eq "origin" uqctl "$(jq -r .origin <<<"$e")"
    grep -q uqctl <<<"$("$UQCTL" history --limit 3)" || bad "uqctl history doesn't show the uqctl change"
    out=$("$UQCTL" undo 2>&1) || bad "uqctl undo: $out"
    note "uqctl undo: $out"
    expect_eq "theme after uqctl undo" "$t0" "$(bus GetTheme)"
}

check SA5 "Restore points: restore brings every page back, and the automatic point undoes it" sa5
sa5() {
    local p g0 t0 l0 auto newest_p r
    p=$(bus SaveRestorePoint s "${LIVE_PREFIX} mine") || { bad "save: $p"; return; }
    grep -q "$LIVE_PREFIX mine" <<<"$("$UQCTL" restore list)" || bad "uqctl restore list doesn't list the new point"
    g0=$(look_val gaps_in) t0=$(bus GetTheme) l0=$(section layouts | jq -c .)
    merge look '.gaps_in = 9' >/dev/null
    bus SetTheme sss forest dark settings >/dev/null
    "$UQCTL" layout set 2 master >/dev/null 2>&1
    merge input '.mouse.sensitivity = 0.3' >/dev/null
    note "changed look, theme, layouts and input"
    r=$("$UQCTL" restore "$p" 2>&1) || { bad "uqctl restore $p: $r"; return; }
    expect_eq "gaps after restore" "$g0" "$(look_val gaps_in)"
    expect_eq "theme after restore" "$t0" "$(bus GetTheme)"
    expect_eq "layouts after restore" "$l0" "$(section layouts | jq -c .)"
    newest_p=$(points | jq -c '.[0]')
    note "newest point: $newest_p"
    expect_eq "newest point's kind" auto:before_restore "$(kind_of <<<"$newest_p")"
    auto=$(jq -r .id <<<"$newest_p")
    r=$(bus Restore s "$auto") || { bad "restore $auto: $r"; return; }
    expect_eq "gaps after restoring the automatic point" 9 "$(look_val gaps_in)"
    expect_eq "theme after restoring the automatic point" forest "$(bus GetTheme)"
    expect_eq "pointer speed after restoring the automatic point" 0.3 "$(section input | jq -r .mouse.sensitivity)"
    bus DeleteRestorePoint s "$p" >/dev/null
}

check SA6 "New version: one automatic point per version, none on a second start" sa6 restart
sa6() {
    local f n v
    v=$("$UQCTL" --version 2>/dev/null | awk '{print $NF}')
    uqd_stop || { bad "uqd did not stop"; return; }
    for f in "$STATE_DIR"/restore/*.meta.json; do
        [ -e "$f" ] || continue
        if jq -e '.kind.auto == "new_version"' "$f" >/dev/null 2>&1; then
            rm -f "$f" "${f%.meta.json}.toml"; note "removed ${f##*/}"
        fi
    done
    uqd_start || { bad "uqd did not start"; return; }
    n=$(points | jq '[.[] | select(.kind.auto? == "new_version")] | length')
    expect_eq "new_version points after a start" 1 "$n"
    note "its name: $(points | jq -r '[.[] | select(.kind.auto? == "new_version")][0].name') (uqctl says $v)"
    expect_eq "the point's name" "$v" "$(points | jq -r '[.[] | select(.kind.auto? == "new_version")][0].name')"
    uqd_restart || { bad "uqd did not restart"; return; }
    n=$(points | jq '[.[] | select(.kind.auto? == "new_version")] | length')
    expect_eq "new_version points after a second start" 1 "$n"
}
