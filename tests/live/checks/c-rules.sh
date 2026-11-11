# shellcheck shell=bash
# Settings slice C (window, panel and workspace rules): docs/notes/settings-slice-c-live-check.md.
# Pick a window (the overlay and a click) is the VM's; here rules go in through the same uqd call
# the editor makes, against foot windows with a test app id.

# add_rule LIST JSON: append one rule to rules.LIST; prints the error, if any.
add_rule() { merge rules ".$1 += [$2]"; }
wrule() { # wrule ID NAME MATCH EFFECTS
    jq -nc --arg id "$1" --arg n "$2" --argjson m "$3" --argjson e "$4" '{id: $id, name: $n, enabled: true, match: $m, effects: $e}'
}
floating() { [ "$(win "$1" | jq -r .floating)" = true ]; }
tiled() { [ "$(win "$1" | jq -r .floating)" = false ]; }
CLS=uqlive-a

check SC1 "Float: a new rule floats the app once it's reopened, not the window already open" sc1 hypr foot
sc1() {
    local r
    open_win "$CLS" || { bad "the test window didn't open"; return; }
    tiled "$CLS" || note "the test window floats before any rule: $(win "$CLS" | jq -c '{floating}')"
    r=$(add_rule window "$(wrule r-ee01 "uqlive float" "{\"class\":\"^$CLS\$\"}" '{"float":true}')") || { bad "adding the rule: $r"; return; }
    sleep 1
    tiled "$CLS" || bad "the open window floated at once (a static effect should wait for a reopen)"
    close_win "$CLS"
    open_win "$CLS" || { bad "the test window didn't reopen"; return; }
    sleep 0.5
    floating "$CLS" || bad "expected the reopened window to float; observed $(win "$CLS" | jq -c '{floating}')"
    close_win "$CLS"
}

check SC2 "Opacity: a dynamic effect applies to the open window at once" sc2 hypr foot
sc2() {
    local r a out
    open_win "$CLS" || { bad "the test window didn't open"; return; }
    r=$(add_rule window "$(wrule r-ee02 "uqlive opacity" "{\"class\":\"^$CLS\$\"}" '{"opacity":0.8}')") || { bad "adding the rule: $r"; return; }
    sleep 1
    a=$(win "$CLS" | jq -r .address)
    for p in opacity alpha; do
        out=$(hc getprop "address:$a" "$p" 2>&1)
        note "hyprctl getprop address:$a $p: $out"
        [[ $out == *0.8* ]] && break
    done
    [[ $out == *0.8* ]] || bad "expected opacity 0.8 on the open window; observed $out"
    close_win "$CLS"
}

check SC3 "Explain rules: each rule reports where its switch comes from" sc3
sc3() {
    local r e
    r=$(add_rule window "$(wrule r-ee03 "uqlive by class" "{\"class\":\"^$CLS\$\"}" '{"no_blur":true}')") || { bad "$r"; return; }
    r=$(add_rule window "$(wrule r-ee04 "uqlive by title" '{"title":"^uqlive-win$"}' '{"no_shadow":true}')") || { bad "$r"; return; }
    e=$(explain rules)
    note "Explain rules: $(jq -c . <<<"$e" | cut -c1-400)"
    expect_eq "r-ee03's source" uq "$(jq -r '.["window.r-ee03"].source' <<<"$e")"
    expect_eq "r-ee04's source" uq "$(jq -r '.["window.r-ee04"].source' <<<"$e")"
    if [ -n "${HAVE[hypr]:-}" ] && [ -n "${HAVE[foot]:-}" ]; then
        open_win "$CLS" || { bad "the test window didn't open"; return; }
        r=$(bus ListWindows | jq -c --arg c "$CLS" '[.[] | select(.class == $c) | {class, title}]')
        note "ListWindows (what Why does this window…? matches against): $r"
        expect_eq "windows listed with that app id" 1 "$(jq length <<<"$r")"
        close_win "$CLS"
    fi
}

check SC4 "user.lua override: the rule shows 'Overridden in user.lua, line N', and clears after" sc4
sc4() {
    local r n e
    r=$(add_rule window "$(wrule r-ee05 "uqlive override" "{\"class\":\"^$CLS\$\"}" '{"float":true}')") || { bad "$r"; return; }
    n=$(user_lua_add 'hl.window_rule({ name = "uq:r-ee05", enabled = false })')
    hypr_up && { hc reload >/dev/null; sleep 1; }
    e=$(explain rules | jq -c '.["window.r-ee05"]')
    note "user.lua line $n turns it off; Explain: $e"
    [[ $e == *user_lua* ]] || bad "expected the rule's source to be user.lua; observed $e"
    [[ $e == *"\"line\":$n"* ]] || bad "expected line $n; observed $e"
    sed -i "${n}s/^/-- /" "$USER_LUA"
    hypr_up && { hc reload >/dev/null; sleep 1; }
    e=$(explain rules | jq -c '.["window.r-ee05"]')
    note "line commented out; Explain: $e"
    [[ $e != *user_lua* ]] || bad "expected the user.lua note gone; observed $e"
    user_lua_reset
    hypr_up && [ -n "$(config_errors)" ] && bad "Hyprland's errors: $(config_errors)"
    return 0
}

check SC5 "Pick a window, edges: no shell answers within about 2 s, and a second pick is refused" sc5 hypr
sc5() {
    local t0 r1 r2 took shell=0
    pgrep -f 'qs .*shell' >/dev/null && shell=1
    if [ "$shell" = 1 ] && [ -z "${UQ_LIVE_PICK:-}" ]; then
        na "the shell is running, so a pick would wait for a click (the VM's checks click; set UQ_LIVE_PICK=1 to wait 20 s for the timeout)"
        return
    fi
    t0=$(date +%s%N)
    bus PickWindow >"$WORK/pick1" 2>&1 &
    sleep 0.3
    r2=$(bus PickWindow)
    wait
    took=$((($(date +%s%N) - t0) / 1000000))
    r1=$(cat "$WORK/pick1")
    note "first pick (${took} ms): $r1"
    note "second pick while the first was on: $r2"
    [[ $r2 == *"Already choosing a window"* ]] || bad "expected the second pick refused with 'Already choosing a window.'; observed $r2"
    if [ "$shell" = 0 ]; then
        expect_eq "pick status with no shell" no_shell "$(jq -r .status <<<"$r1" 2>/dev/null)"
        [ "$took" -le 3000 ] || bad "expected the no-shell answer within about 2 s; observed ${took} ms"
    else
        expect_eq "pick status after 20 s" timeout "$(jq -r .status <<<"$r1" 2>/dev/null)"
    fi
}

check SC7 "Templates: the five template rules load without errors; 'on a workspace, quietly' works" sc7 hypr foot
sc7() {
    local r ws0 w
    r=$(merge rules ".window += [
        $(wrule r-ee10 "uqlive dialogs" "{\"class\":\"^$CLS\$\",\"modal\":true}" '{"float":true,"center":true}'),
        $(wrule r-ee11 "uqlive workspace" '{"class":"^uqlive-ws$"}' '{"workspace":"3","silent":true}'),
        $(wrule r-ee12 "uqlive no blur" "{\"class\":\"^$CLS\$\"}" '{"no_blur":true}'),
        $(wrule r-ee13 "uqlive pip" '{"title":"^Picture-in-Picture$"}' '{"float":true,"pin":true,"keep_aspect_ratio":true}'),
        $(wrule r-ee14 "uqlive awake" "{\"class\":\"^$CLS\$\"}" '{"idle_inhibit":"fullscreen"}')]") || { bad "adding the template rules: $r"; return; }
    sleep 1
    [ -z "$(config_errors)" ] || bad "Hyprland's errors: $(config_errors)"
    ws0=$(hc activeworkspace -j | jq -r .id)
    open_win uqlive-ws || { bad "the test window didn't open"; return; }
    w=$(win uqlive-ws | jq -r .workspace.id)
    note "the window opened on workspace $w; the active one was $ws0 and is $(hc activeworkspace -j | jq -r .id)"
    expect_eq "the window's workspace" 3 "$w"
    expect_eq "the active workspace (quietly)" "$ws0" "$(hc activeworkspace -j | jq -r .id)"
    close_win uqlive-ws
}

check SC8 "Panel and workspace rules: load, one rule per workspace, merged with its layout" sc8
sc8() {
    local r mon wr
    mon=$( { [ -n "${HAVE[hypr]:-}" ] && hc monitors -j | jq -r '.[0].name'; } || echo DP-1)
    r=$(add_rule layer '{"id":"r-ee20","name":"uqlive blur notifications","enabled":true,"match":{"namespace":"^noti-fi$"},"effects":{"blur":true}}') ||
        { bad "adding the panel rule: $r"; return; }
    r=$(add_rule workspace "{\"workspace\":\"3\",\"monitor\":\"$mon\",\"persistent\":true,\"gaps_in\":0}") || { bad "adding the workspace rule: $r"; return; }
    r=$(add_rule workspace '{"workspace":"3","gaps_out":5}') && bad "a second rule for workspace 3 was accepted"
    note "a second rule for workspace 3: $r"
    # The editor words it "Workspace 3 already has a rule; edit that one." from this refusal.
    [[ $r == *"already has a rule"* ]] || bad "expected the refusal to say the workspace already has a rule; observed $r"
    "$UQCTL" layout set 3 master >/dev/null 2>&1 || bad "uqctl layout set 3 master failed"
    expect_eq "workspace rules in uQ after the layout change" 1 "$(section rules | jq '[.workspace[] | select(.workspace == "3")] | length')"
    if [ -n "${HAVE[hypr]:-}" ]; then
        sleep 1
        [ -z "$(config_errors)" ] || bad "Hyprland's errors: $(config_errors)"
        wr=$(hc workspacerules -j | jq -c '[.[] | select(.workspaceString == "3")]')
        note "hyprctl workspacerules for 3: $wr"
        expect_eq "Hyprland's rules for workspace 3" 1 "$(jq length <<<"$wr")"
        # hyprctl's JSON has no layout field, so the merge is read from the rule uQ rendered.
        r=$(grep -F 'hl.workspace_rule({ workspace = "3"' "$CFG/hypr/uq/generated.lua")
        note "rendered: $r"
        [ "$(grep -cF 'hl.workspace_rule({ workspace = "3"' "$CFG/hypr/uq/generated.lua")" = 1 ] || bad "expected one rendered rule for workspace 3"
        [[ $r == *'layout = "master"'* && $r == *'persistent = true'* ]] || bad "expected the layout merged into the rendered rule; observed $r"
        [[ $wr == *"\"persistent\":true"* ]] || bad "expected persistent in the merged rule; observed $wr"
        hc workspaces -j | jq -e 'any(.[]; .id == 3)' >/dev/null || bad "workspace 3 isn't kept while empty"
    fi
}

check SC9 "Hyprland names: every effect loads, panel animations, tags, workspace gaps, reloads" sc9 hypr
sc9() {
    local r a tags e
    r=$(set_section rules "$(cat "$LIVE_DIR/fixtures/every-rule.json")") || { bad "the every-effect rules: $r"; return; }
    sleep 1
    e=$(config_errors); [ -z "$e" ] || bad "every effect: Hyprland's errors: $e"
    for a in fade slide popin; do
        merge rules "(.layer[] | select(.id == \"r-ee31\") | .effects.animation) = \"$a\"" >/dev/null
        sleep 0.5
        e=$(config_errors); [ -z "$e" ] && note "panel animation $a: accepted" || bad "panel animation $a: $e"
    done
    for g in 0 12; do
        merge rules "(.workspace[] | select(.workspace == \"4\") | .gaps_in) = $g" >/dev/null
        sleep 0.5
        e=$(hc workspacerules -j | jq -c '[.[] | select(.workspaceString == "4") | .gapsIn][0]')
        note "workspace 4 gaps_in $g: Hyprland has $e"
        [[ $e == *"$g"* ]] || bad "workspace gaps $g: Hyprland has $e"
    done
    [ -n "${HAVE[foot]:-}" ] || { note "no foot: tags and reloads not checked"; return; }
    open_win uqlive-tag || { bad "the test window didn't open"; return; }
    tags=$(win uqlive-tag | jq -c .tags)
    note "tags on the window: $tags"
    [[ $tags == *mytag* ]] || bad "expected the tag mytag; observed $tags"
    for _ in 1 2 3; do hc reload >/dev/null; sleep 0.7; done
    tags=$(win uqlive-tag | jq -c .tags)
    note "tags after three reloads: $tags"
    [ "$(jq '[.[] | select(test("mytag"))] | length' <<<"$tags")" = 1 ] || bad "expected mytag once after reloads; observed $tags"
    e=$(config_errors); [ -z "$e" ] || bad "after reloads: Hyprland's errors: $e"
    close_win uqlive-tag
}

check SC10 "History, undo and restore: 'Window rule “…” added', and Undo takes it away" sc10
sc10() {
    local r e p
    p=$(bus SaveRestorePoint s "$LIVE_PREFIX rules") || { bad "save: $p"; return; }
    r=$(add_rule window "$(wrule r-ee40 "uqlive history" "{\"class\":\"^$CLS\$\"}" '{"float":true}')") || { bad "$r"; return; }
    e=$(newest)
    note "newest entry: $(jq -c '{origin, summary}' <<<"$e")"
    [[ $(jq -r .summary <<<"$e") == *"Window rule “uqlive history” added"* ]] || bad "expected 'Window rule “uqlive history” added'; observed $(jq -r .summary <<<"$e")"
    r=$(bus Undo sb "$(jq -r .id <<<"$e")" false) || bad "undo: $r"
    expect_eq "rule after Undo" 0 "$(section rules | jq '[.window[] | select(.id == "r-ee40")] | length')"
    add_rule window "$(wrule r-ee41 "uqlive later" "{\"class\":\"^$CLS\$\"}" '{"float":true}')" >/dev/null
    r=$(bus Restore s "$p") || bad "restore: $r"
    expect_eq "rule after the restore" 0 "$(section rules | jq '[.window[] | select(.id == "r-ee41")] | length')"
    bus DeleteRestorePoint s "$p" >/dev/null
}

check SC11 "Refusals in plain words: bad patterns, a 201st rule, a bad panel name" sc11
sc11() {
    local p r n0 many
    n0=$(section rules | jq -c .)
    for p in 'negative:foo' '[a&&b]' '(?x)a' 'a{1001}'; do
        r=$(add_rule window "$(wrule r-ee50 "uqlive bad" "$(jq -nc --arg p "$p" '{class: $p}')" '{"float":true}')") && bad "pattern $p was accepted"
        note "pattern $p: $r"
        [[ ${r,,} == *regex* ]] && bad "the refusal of $p says 'regex': $r"
    done
    r=$(add_rule layer '{"id":"r-ee51","name":"uqlive bad panel","enabled":true,"match":{"namespace":"[a&&b]"},"effects":{"blur":true}}') && bad "panel name [a&&b] was accepted"
    note "panel name [a&&b]: $r"
    r=$(add_rule window "$(wrule r-ee52 "uqlive both" "{\"class\":\"^$CLS\$\"}" '{"float":true,"tile":true}')") && note "float and tile together were accepted: $(section rules | jq -c '.window[] | select(.id == "r-ee52") | .effects')"
    [ -n "$r" ] && note "float and tile together: $r"
    set_section rules "$n0" >/dev/null
    many=$(jq -nc --arg c "^$CLS\$" '[range(200) | {id: ("r-" + (40960 + .|tostring)), name: ("uqlive \(.)"), enabled: true, match: {class: $c}, effects: {float: true}}]')
    r=$(merge rules ".window = $many") || { bad "200 rules were refused: $r"; return; }
    r=$(add_rule window "$(wrule r-ee53 "uqlive 201" "{\"class\":\"^$CLS\$\"}" '{"float":true}')") && bad "a 201st window rule was accepted"
    note "201st rule: $r"
    set_section rules "$n0" >/dev/null
    expect_eq "rules after the refusals" "$n0" "$(section rules | jq -c .)"
}

check SC12 "Profiles turn rules on and off; a missing rule is left out with a notice" sc12
sc12() {
    local r id=${LIVE_PREFIX}-rules e cls=uqlive-p
    r=$(add_rule window "$(wrule r-ee60 "uqlive float" "{\"class\":\"^$cls\$\"}" '{"float":true}')") || { bad "$r"; return; }
    r=$(add_rule layer '{"id":"r-ee61","name":"uqlive panel","enabled":true,"match":{"namespace":"^uqlive$"},"effects":{"blur":true}}') || { bad "$r"; return; }
    r=$(bus SetProfile sb "$(printf 'format = 1\nid = "%s"\nname = "Rules"\n\n[rules]\n"r-ee60" = false\n"r-ee61" = true\n' "$id")" false) || { bad "SetProfile: $r"; return; }
    r=$(bus ApplyProfile s "$id") || { bad "ApplyProfile: $r"; return; }
    expect_eq "float rule while on" false "$(section rules | jq -r '.window[] | select(.id == "r-ee60") | .enabled')"
    e=$(history_json 3 | jq -r '.[].summary' | grep -c 'Window rule “uqlive float” turned off')
    [ "$e" -ge 1 ] || bad "expected 'Window rule “uqlive float” turned off' in History; observed $(history_json 3 | jq -c '[.[].summary]')"
    if [ -n "${HAVE[hypr]:-}" ] && [ -n "${HAVE[foot]:-}" ]; then
        open_win "$cls" && sleep 0.5
        tiled "$cls" || bad "the window floats while the profile turns the rule off"
        close_win "$cls"
    fi
    merge rules '(.layer[] | select(.id == "r-ee61") | .enabled) = false' >/dev/null
    r=$(bus ProfileOff) || { bad "ProfileOff: $r"; return; }
    note "Back to normal: $r"
    expect_eq "float rule after Back to normal" true "$(section rules | jq -r '.window[] | select(.id == "r-ee60") | .enabled')"
    [[ $r == *uqlive\ panel* ]] || bad "expected the message to name the panel rule as kept; observed $r"
    r=$(bus SetProfile sb "$(printf 'format = 1\nid = "%s"\nname = "Rules"\n\n[rules]\n"r-ee60" = false\n"r-eeff" = false\n' "$id")" true) || { bad "SetProfile: $r"; return; }
    r=$(bus ApplyProfile s "$id") || { bad "ApplyProfile: $r"; return; }
    note "with a rule that isn't here: $r"
    [[ $r == *"1 rule in this profile isn't on this computer, so it was left out."* ]] || bad "expected the missing-rule notice; observed $r"
}
