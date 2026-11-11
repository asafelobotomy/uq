# shellcheck shell=bash
# Settings slice B (profiles): docs/notes/settings-slice-b-live-check.md. The quick-settings row
# and the SUPER+ALT+P key press are the VM's; here the same uqd calls run.

# profile_toml ID NAME [EXTRA TOML]: a profile with gaps 0 and animations off (and a power mode
# when this session has power-profiles-daemon).
profile_toml() {
    printf 'format = 1\nid = "%s"\nname = "%s"\n\n[look]\ngaps_in = 0\nanimations = false\n' "$1" "$2"
    [ -n "${HAVE[ppd]:-}" ] && printf '\n[power]\nprofile = "performance"\n'
    [ $# -ge 3 ] && printf '\n%s\n' "$3"
    return 0
}
add_profile() { # add_profile ID NAME [EXTRA]: prints the error, if any
    local r; r=$(bus SetProfile sb "$(profile_toml "$@")" false) || { echo "$r"; return 1; }
}
active_profile() { bus ListProfiles | jq -r '[.[] | select(.active)][0].id // ""'; }
power_now() { powerprofilesctl get 2>/dev/null || busctl --system get-property net.hadess.PowerProfiles /net/hadess/PowerProfiles net.hadess.PowerProfiles ActiveProfile 2>/dev/null | awk '{print $2}' | tr -d '"'; }
GAMING=${LIVE_PREFIX}-gaming

check SB1 "Create and turn on: settings, power, History, a restore point and 'From the profile'" sb1
sb1() {
    local r g0 p0 e
    g0=$(look_val gaps_in) p0=$(power_now)
    r=$(add_profile "$GAMING" "Live check") || { bad "SetProfile: $r"; return; }
    r=$(bus ApplyProfile s "$GAMING") || { bad "ApplyProfile: $r"; return; }
    note "ApplyProfile: $r"
    expect_eq "gaps while on" 0 "$(look_val gaps_in)"
    expect_eq "animations while on" false "$(look_val animations)"
    expect_eq "active profile" "$GAMING" "$(active_profile)"
    if [ -n "${HAVE[hypr]:-}" ]; then
        e=$(explain look | jq -c '.gaps_in.live')
        note "live gaps_in: $e"
        [[ $e == *0* ]] || bad "expected live gaps 0; observed $e"
    fi
    if [ -n "${HAVE[ppd]:-}" ]; then
        expect_eq "power mode while on (was $p0)" performance "$(power_now)"
    fi
    r=$(history_json 5 | jq -r --arg p "profile:$GAMING" '[.[] | select(.origin == $p)] | length')
    [ "$r" -ge 1 ] || bad "expected History entries from the profile; observed $(history_json 3 | jq -c '[.[] | .origin]')"
    r=$(points | jq -r '[.[] | select(.name == "Before the Live check profile")] | length')
    [ "$r" -ge 1 ] || bad "expected a 'Before the Live check profile' restore point; observed $(points | jq -c '[.[].name]')"
    e=$(explain look | jq -c '.gaps_in')
    note "Explain gaps_in: $e"
    expect_eq "gaps source" profile "$(jq -r .source <<<"$e")"
    expect_eq "gaps 'From the ... profile'" "Live check" "$(jq -r .detail.name <<<"$e")"
    r=$(bus ProfileOff) || bad "ProfileOff: $r"
    expect_eq "gaps after Back to normal" "$g0" "$(look_val gaps_in)"
    [ -n "${HAVE[ppd]:-}" ] && expect_eq "power mode after Back to normal" "$p0" "$(power_now)"
}

check SB2 "A hand change survives Back to normal, and the message names it" sb2
sb2() {
    local r a0
    a0=$(look_val animations)
    r=$(add_profile "$GAMING" "Live check") || { bad "SetProfile: $r"; return; }
    bus ApplyProfile s "$GAMING" >/dev/null || { bad "ApplyProfile failed"; return; }
    merge look '.gaps_in = 4' >/dev/null
    r=$(bus ProfileOff) || { bad "ProfileOff: $r"; return; }
    note "Back to normal: $r"
    expect_eq "gaps after Back to normal" 4 "$(look_val gaps_in)"
    expect_eq "animations after Back to normal" "$a0" "$(look_val animations)"
    [[ $r == *"Gaps between windows"* ]] || bad "expected the message to name 'Gaps between windows'; observed $r"
}

check SB3 "Restarts keep it: uqd comes back with the profile on, and Back to normal works" sb3 restart
sb3() {
    local r g0
    g0=$(look_val gaps_in)
    r=$(add_profile "$GAMING" "Live check") || { bad "SetProfile: $r"; return; }
    bus ApplyProfile s "$GAMING" >/dev/null || { bad "ApplyProfile failed"; return; }
    uqd_restart || { bad "uqd did not restart"; return; }
    expect_eq "active profile after the restart" "$GAMING" "$(active_profile)"
    r=$(bus ProfileOff) || bad "ProfileOff after the restart: $r"
    expect_eq "gaps after Back to normal" "$g0" "$(look_val gaps_in)"
    expect_eq "active profile after Back to normal" "" "$(active_profile)"
}

check SB4 "No power service: everything else applies, the message says so, within 3 s" sb4
sb4() {
    local r t0 took masked=0
    if [ -n "${HAVE[ppd]:-}" ]; then
        [ -n "${HAVE[root]:-}" ] || { na "power-profiles-daemon is running and stopping it needs root"; return; }
        sudo -n systemctl mask --now power-profiles-daemon >/dev/null 2>&1 && masked=1
        note "masked power-profiles-daemon"
    fi
    r=$(bus SetProfile sb "$(profile_toml "$GAMING" "Live check" '[power]
profile = "performance"')" false) || { bad "SetProfile: $r"; return; }
    t0=$(date +%s%N)
    r=$(bus ApplyProfile s "$GAMING")
    took=$((($(date +%s%N) - t0) / 1000000))
    note "ApplyProfile took ${took} ms: $r"
    [ "$took" -le 3500 ] || bad "expected the call back within about 3 s; observed ${took} ms"
    [[ $r == *"Power mode wasn't changed"* ]] || bad "expected \"Power mode wasn't changed: ...\"; observed $r"
    expect_eq "gaps while on" 0 "$(look_val gaps_in)"
    if [ "$masked" = 1 ]; then
        sudo -n systemctl unmask power-profiles-daemon >/dev/null 2>&1
        sudo -n systemctl start power-profiles-daemon >/dev/null 2>&1
        note "unmasked and started power-profiles-daemon again"
    fi
}

check SB5 "Export and import: round trip, duplicate, --replace, too big, [keybinds] refused" sb5
sb5() {
    local r f=$WORK/g.toml n0
    r=$(add_profile "$GAMING" "Live check") || { bad "SetProfile: $r"; return; }
    "$UQCTL" profile export "$GAMING" "$f" || { bad "export failed"; return; }
    r=$("$UQCTL" profile delete "$GAMING" 2>&1) || { bad "delete: $r"; return; }
    r=$("$UQCTL" profile import "$f" 2>&1) || bad "import: $r"
    note "import: $r"
    bus ListProfiles | jq -e --arg p "$GAMING" 'any(.[]; .id == $p)' >/dev/null || bad "the profile didn't come back"
    r=$("$UQCTL" profile import "$f" 2>&1) && bad "a second import was accepted: $r"
    note "second import: $r"
    [[ $r == *"already"* ]] || bad "expected the second import to say it already exists; observed $r"
    r=$("$UQCTL" profile import "$f" --replace 2>&1) || bad "import --replace: $r"
    n0=$(bus ListProfiles | jq length)
    head -c 1048576 /dev/zero >"$WORK/big.toml"
    r=$("$UQCTL" profile import "$WORK/big.toml" 2>&1) && bad "a 1 MiB file was accepted"
    note "1 MiB: $r"
    [[ $r == *"The file is too big to be a profile."* ]] || bad "expected 'The file is too big to be a profile.'; observed $r"
    profile_toml "${LIVE_PREFIX}-keys" Keys '[keybinds]
"SUPER + T" = "exec foot"' >"$WORK/k.toml"
    r=$("$UQCTL" profile import "$WORK/k.toml" 2>&1) && bad "a profile with [keybinds] was accepted"
    note "[keybinds]: $r"
    [[ $r == *"can't do"* ]] || bad "expected a plain 'can't do that yet' message; observed $r"
    expect_eq "profiles after the refusals" "$n0" "$(bus ListProfiles | jq length)"
}

check SB6 "next: first, second, then back to normal (by name, ignoring case); the keybind loads" sb6
sb6() {
    local r a b got=()
    a=${LIVE_PREFIX}-a b=${LIVE_PREFIX}-b
    # "uqlive alpha" sorts before "Uqlive beta" only when case is ignored.
    r=$(add_profile "$b" "Uqlive beta") || { bad "SetProfile: $r"; return; }
    r=$(add_profile "$a" "uqlive alpha") || { bad "SetProfile: $r"; return; }
    for _ in 1 2 3; do
        "$UQCTL" profile next >/dev/null 2>&1 || bad "uqctl profile next failed"
        got+=("$(active_profile)")
    done
    note "uqctl profile next x3: ${got[*]:-} (other profiles: $(bus ListProfiles | jq -c '[.[] | .name]'))"
    # Other people's profiles count too; only the order of these two is checked.
    local seq=" ${got[*]} "
    [[ $seq == *" $a $b "* ]] || bad "expected alpha then Beta; observed ${got[*]}"
    if [ -n "${HAVE[hypr]:-}" ]; then
        user_lua_add 'hl.bind("SUPER + ALT + P", hl.dsp.exec_cmd("uqctl profile next"))' >/dev/null
        hc reload >/dev/null; sleep 1
        # SUPER is modmask 64 and ALT 8; a Lua dispatcher's arg isn't always printed, so match the key.
        r=$(hc binds -j 2>/dev/null | jq -c '[.[] | select((.key | ascii_upcase) == "P" and .modmask == 72)]')
        note "SUPER+ALT+P binds: $r"
        [ "$(jq length <<<"${r:-[]}")" -ge 1 ] || bad "the SUPER+ALT+P bind from user.lua isn't loaded"
        user_lua_reset
    fi
}

check SB7 "Restore ends a profile: the 'Before' point brings the settings back, none is on" sb7
sb7() {
    local r g0 p
    g0=$(look_val gaps_in)
    r=$(add_profile "$GAMING" "Live check") || { bad "SetProfile: $r"; return; }
    bus ApplyProfile s "$GAMING" >/dev/null || { bad "ApplyProfile failed"; return; }
    p=$(points | jq -r '[.[] | select(.name == "Before the Live check profile")][0].id')
    r=$(bus Restore s "$p") || { bad "Restore $p: $r"; return; }
    expect_eq "gaps after the restore" "$g0" "$(look_val gaps_in)"
    expect_eq "active profile after the restore" "" "$(active_profile)"
}

check SB8 "Light or dark hold: the schedule waits while on, and catches up after Back to normal" sb8 slow
sb8() {
    local r now dark_at
    # Light now, dark due in about two minutes; the profile holds light.
    now=$(date +%s)
    dark_at=$(date -d "@$(((now / 60 + 2) * 60))" +%H:%M)
    r=$("$UQCTL" schedule fixed "$(date -d "@$((now - 3600))" +%H:%M)" "$dark_at" 2>&1) || { bad "schedule: $r"; return; }
    note "schedule: light from an hour ago, dark from $dark_at"
    wait_for 70 variant_is light || bad "the schedule didn't switch to light"
    r=$(bus SetProfile sb "$(printf 'format = 1\nid = "%s"\nname = "Hold"\n\n[appearance]\nvariant = "light"\n' "${LIVE_PREFIX}-hold")" false) ||
        { bad "SetProfile: $r"; return; }
    bus ApplyProfile s "${LIVE_PREFIX}-hold" >/dev/null || { bad "ApplyProfile failed"; return; }
    note "waiting past $dark_at with the profile on"
    while [ "$(date +%H:%M)" != "$dark_at" ]; do sleep 5; done
    sleep 75
    variant_is light || bad "expected light to hold while the profile is on; observed $(bus GetTheme)"
    r=$(bus ProfileOff) || bad "ProfileOff: $r"
    note "Back to normal: $r"
    if wait_for 75 variant_is dark; then note "dark came within a minute of Back to normal"
    else bad "expected the schedule to switch to dark within a minute of Back to normal; observed $(bus GetTheme)"; fi
    "$UQCTL" schedule off >/dev/null 2>&1
}
variant_is() { [ "$(theme_variant)" = "$1" ]; }
theme_variant() { busctl --user --json=short call "${UQ_BUS[@]}" GetTheme 2>/dev/null | jq -r '.data[1]'; }
