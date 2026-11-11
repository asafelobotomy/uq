# shellcheck shell=bash
# Settings slice D (startup apps, environment variables, apps and scaling):
# docs/notes/settings-slice-d-live-check.md. Items that need a fresh login (apps starting at login,
# once per session, a failing app, a delay) are the VM's, which logs out and in; Electron, X11 and
# browser sharpness at 125 % and Flatpak (items 6 and 7's looks, 10's browser list, 15) need a
# person on real hardware.
#
# System entries (an app in the system's applications folder, an entry in /etc/xdg/autostart) go
# in the sandbox's own system folders (UQ_LIVE_SYSDATA, UQ_LIVE_SYSCONFIG), or under /usr/local
# and /etc/xdg with root; each check removes the ones it added.

SYS_ADDED=()
sys_dirs() { # sets SYSAPPS and SYSAUTO, or fails
    if [ -n "${UQ_LIVE_SYSDATA:-}" ]; then
        SYSAPPS=$UQ_LIVE_SYSDATA/applications SYSAUTO=$UQ_LIVE_SYSCONFIG/autostart
    elif [ -n "${HAVE[root]:-}" ]; then
        SYSAPPS=/usr/local/share/applications SYSAUTO=/etc/xdg/autostart
    else
        return 1
    fi
}
# sys_file DIR NAME TEXT: a system file (with sudo outside the sandbox), removed by sys_cleanup.
sys_file() {
    [ -e "$1/$2" ] && { note "$1/$2 is already there; not replacing it"; return 1; }
    if [ -n "${UQ_LIVE_SYSDATA:-}" ]; then mkdir -p "$1"; printf '%s' "$3" >"$1/$2"
    else sudo -n mkdir -p "$1" && printf '%s' "$3" | sudo -n tee "$1/$2" >/dev/null
    fi
    SYS_ADDED+=("$1/$2")
}
sys_cleanup() {
    local f
    # Only files sys_file made (it never writes over one that was there).
    for f in "${SYS_ADDED[@]}"; do
        if [ -n "${UQ_LIVE_SYSDATA:-}" ]; then rm -f -- "${f:?}"; else sudo -n rm -f -- "${f:?}"; fi
    done
    SYS_ADDED=()
}
# own_rm NAME DIR: remove a test file of ours (always named uqlive-*).
own_rm() { case "$1" in "$LIVE_PREFIX"*) rm -f -- "${2:?}/${1:?}" ;; esac; }
desktop() { printf '[Desktop Entry]\nType=Application\nName=%s\nExec=%s\n%s' "$1" "$2" "${3:-}"; }
autostart_row() { bus ListAutostart | jq -c --arg i "$1" '.[] | select(.id == $i)'; }
# dump_app NAME: a desktop entry whose app writes its environment to $WORK/env.NAME.
dump_app() {
    printf '#!/bin/sh\nenv >"%s/env.$1"\n' "$WORK" >"$WORK/dumpenv"
    chmod 755 "$WORK/dumpenv"
    desktop "$1" "$WORK/dumpenv $1"
}
# hexec CMD: run CMD from Hyprland (as a keybind or the launcher would).
hexec() { hc dispatch "hl.dsp.exec_cmd(\"$1\")" >/dev/null; }
# env_of NAME CMD: run CMD from Hyprland with its environment going to $WORK/env.NAME.
env_of() {
    rm -f -- "${WORK:?}/env.$1"
    hexec "env > $WORK/env.$1"
    wait_for 5 test -s "$WORK/env.$1"
}
gen_lua() { cat "$CFG/hypr/uq/generated.lua" 2>/dev/null; }
ENVD=$CFG/environment.d/60-uq.conf

check SD2 "A system startup entry turned off and on; 'Not for this desktop'; your own file untouched" sd2
sd2() {
    sys_dirs || { na "needs the sandbox's system folders or root"; return; }
    local r mine=${LIVE_PREFIX}-mine.desktop sys=${LIVE_PREFIX}-sys.desktop f sum
    f=$AUTOSTART_DIR/$sys
    sys_file "$SYSAUTO" "$sys" "$(desktop "Live sys" true)"
    sys_file "$SYSAUTO" "${LIVE_PREFIX}-gnome.desktop" "$(desktop "Live GNOME" true 'OnlyShowIn=GNOME;
')"
    mkdir -p "$AUTOSTART_DIR"
    desktop "Live mine" true >"$AUTOSTART_DIR/$mine"
    sum=$(sha256sum "$AUTOSTART_DIR/$mine")
    r=$(merge autostart ".files += [{id: \"$sys\", enabled: false, delay: null}, {id: \"$mine\", enabled: false, delay: null}]") ||
        { bad "turning them off: $r"; sys_cleanup; return; }
    if [ -f "$f" ]; then
        note "hidden copy: $(tr '\n' ' ' <"$f")"
        grep -qx 'Hidden=true' "$f" || bad "the hidden copy has no Hidden=true"
        grep -qx 'X-uQ-Autostart=true' "$f" || bad "the hidden copy has no X-uQ-Autostart=true"
    else
        bad "turning off a system entry wrote no $f"
    fi
    expect_eq "your own file after turning it off" "$sum" "$(sha256sum "$AUTOSTART_DIR/$mine")"
    expect_eq "your own entry's status" off "$(autostart_row "$mine" | jq -r .status)"
    expect_eq "the GNOME-only entry's reason" "Not for this desktop" "$(autostart_row "${LIVE_PREFIX}-gnome.desktop" | jq -r .reason)"
    note "History: $(newest | jq -r .summary)"
    r=$(merge autostart "(.files[] | select(.id == \"$sys\") | .enabled) = true") || bad "turning it on: $r"
    [ -e "$f" ] && bad "turning the system entry back on left $f"
    own_rm "$mine" "$AUTOSTART_DIR"
    sys_cleanup
}

check SD6 "Electron on Wayland: new apps get ELECTRON_OZONE_PLATFORM_HINT=auto" sd6
sd6() {
    local r
    r=$(merge compat '.electron_wayland = true') || { bad "$r"; return; }
    grep -qF 'hl.env("ELECTRON_OZONE_PLATFORM_HINT", "auto"' <<<"$(gen_lua)" || bad "generated.lua sets no ELECTRON_OZONE_PLATFORM_HINT"
    if [ -n "${HAVE[hypr]:-}" ]; then
        env_of sd6 || { bad "the test command didn't run"; return; }
        grep -qx 'ELECTRON_OZONE_PLATFORM_HINT=auto' "$WORK/env.sd6" || bad "a new app's environment has no ELECTRON_OZONE_PLATFORM_HINT=auto"
    fi
    note "how sharp a real Electron app looks at 125 % is for a person (the real-desktop plan)"
}

check SD7 "Sharp X11 apps: force_zero_scaling is on, GDK_SCALE reaches new apps, not environment.d" sd7 hypr
sd7() {
    local r z
    r=$(merge compat '.sharp_x11 = true') || { bad "$r"; return; }
    sleep 0.5
    z=$(hc getoption xwayland:force_zero_scaling -j 2>/dev/null | jq -c .)
    note "xwayland:force_zero_scaling: $z"
    [[ $z == *'"int":1'* || $z == *true* ]] || bad "expected force_zero_scaling on; observed $z"
    env_of sd7 || { bad "the test command didn't run"; return; }
    note "a new app: $(grep '^GDK_SCALE=' "$WORK/env.sd7" || echo 'no GDK_SCALE')"
    grep -q '^GDK_SCALE=' "$WORK/env.sd7" || bad "a new app's environment has no GDK_SCALE"
    grep -q GDK_SCALE "$ENVD" 2>/dev/null && bad "GDK_SCALE is in 60-uq.conf"
    return 0
}

check SD8 "Fix this app: Run on X11 at 150 % gives QT_SCALE_FACTOR=1.5 and GDK_SCALE=1" sd8
sd8() {
    sys_dirs || { na "needs the sandbox's system folders or root"; return; }
    local r id=${LIVE_PREFIX}-x
    sys_file "$SYSAPPS" "$id.desktop" "$(dump_app "$id")"
    r=$(merge compat ".apps += [{id: \"$id\", platform: \"x11\", scale: 1.5, env: [], args: [], elsewhere: false}]") || { bad "$r"; sys_cleanup; return; }
    note "History: $(newest | jq -r .summary)"
    if [ -n "${HAVE[hypr]:-}" ] && [ -n "${HAVE[systemd]:-}" ]; then
        rm -f -- "${WORK:?}/env.$id"
        r=$("$UQCTL" launch "$id" 2>&1) || bad "uqctl launch: $r"
        if wait_for 5 test -s "$WORK/env.$id"; then
            grep -qx 'QT_SCALE_FACTOR=1.5' "$WORK/env.$id" || bad "uqctl launch: no QT_SCALE_FACTOR=1.5"
            grep -qx 'GDK_SCALE=1' "$WORK/env.$id" || bad "uqctl launch: no GDK_SCALE=1"
            grep -qx 'GDK_BACKEND=x11' "$WORK/env.$id" || bad "uqctl launch: no GDK_BACKEND=x11"
        else
            bad "uqctl launch didn't start the app"
        fi
    else
        note "uqctl launch needs Hyprland and systemd; the override's Exec is checked in SD9"
    fi
    sys_cleanup
}

check SD9 "Other launchers: the override file follows the fix, and your own copy is refused" sd9
sd9() {
    sys_dirs || { na "needs the sandbox's system folders or root"; return; }
    local r id=${LIVE_PREFIX}-y o
    o=$APPS_DIR/$id.desktop
    sys_file "$SYSAPPS" "$id.desktop" "$(dump_app "$id")"
    r=$(merge compat ".apps += [{id: \"$id\", platform: \"x11\", scale: 1.5, env: [], args: [], elsewhere: true}]") || { bad "$r"; sys_cleanup; return; }
    if [ -f "$o" ]; then
        note "override Exec: $(grep '^Exec=' "$o")"
        grep -qx 'X-uQ-Override=true' "$o" || bad "the override has no X-uQ-Override=true"
        grep -q '^Exec=env -- .*QT_SCALE_FACTOR=1.5' "$o" || bad "the override's Exec isn't an env -- with the fix"
    else
        bad "no override at $o"
    fi
    if command -v gtk-launch >/dev/null && [ -n "${HAVE[hypr]:-}" ]; then
        rm -f -- "${WORK:?}/env.$id"
        hexec "gtk-launch $id"
        if wait_for 5 test -s "$WORK/env.$id"; then
            grep -qx 'QT_SCALE_FACTOR=1.5' "$WORK/env.$id" || bad "gtk-launch: no QT_SCALE_FACTOR=1.5"
        else
            bad "gtk-launch didn't start the app"
        fi
    fi
    r=$(merge compat ".apps = [.apps[] | select(.id != \"$id\" and .id != \"$id.desktop\")]") || bad "removing the fix: $r"
    [ -e "$o" ] && bad "removing the fix left $o"
    mkdir -p "$APPS_DIR"
    desktop "My own" true >"$o"
    r=$(merge compat ".apps += [{id: \"$id\", platform: \"x11\", scale: 1.5, env: [], args: [], elsewhere: true}]") && bad "the switch was accepted over your own copy"
    note "with your own copy: $r"
    [[ $r == *"You already have your own copy of this app's file"* ]] || bad "expected the own-copy refusal; observed $r"
    grep -q 'My own' "$o" || bad "your own copy was changed"
    own_rm "$id.desktop" "$APPS_DIR"
    sys_cleanup
}

check SD10 "Chromium on Wayland: the flag file with uQ's marker; your own file is left alone" sd10
sd10() {
    local r f=$CFG/chromium-flags.conf
    # uQ writes the flag file only for a Chromium-based browser it finds installed.
    if [ "$(explain compat | jq '.chromium_wayland.detail.browsers // [] | length')" = 0 ]; then
        sys_dirs || { na "no Chromium-based browser is installed, and adding one needs the sandbox's system folders or root"; return; }
        sys_file "$SYSAPPS" chromium.desktop "$(desktop Chromium "chromium %U")"
    fi
    if [ -e "$f" ] && ! head -n1 "$f" | grep -q '^# Managed by uQ'; then
        cp -p "$f" "$WORK/chromium-flags.mine"
        r=$(merge compat '.chromium_wayland = true') || { bad "$r"; return; }
        cmp -s "$f" "$WORK/chromium-flags.mine" || bad "uQ changed your own chromium-flags.conf"
        note "your own chromium-flags.conf was left alone"
        return 0
    fi
    r=$(merge compat '.chromium_wayland = true') || { bad "$r"; return; }
    note "flag file: $(tr '\n' ' ' <"$f" 2>/dev/null)"
    head -n1 "$f" 2>/dev/null | grep -q '^# Managed by uQ' || bad "chromium-flags.conf has no marker line"
    grep -qx -- '--ozone-platform-hint=auto' "$f" 2>/dev/null || bad "chromium-flags.conf has no --ozone-platform-hint=auto"
    merge compat '.chromium_wayland = false' >/dev/null
    [ -e "$f" ] && bad "turning it off left chromium-flags.conf"
    printf -- '--my-flag\n' >"$f"
    r=$(merge compat '.chromium_wayland = true')
    note "turning it on over your own file: ${r:-accepted}; Explain: $(explain compat | jq -c '.chromium_wayland // empty' | cut -c1-200)"
    [ "$(cat "$f")" = "--my-flag" ] || bad "uQ changed your own chromium-flags.conf"
    merge compat '.chromium_wayland = false' >/dev/null
    [ "$(cat "$f" 2>/dev/null)" = "--my-flag" ] || bad "turning it off removed or changed your own chromium-flags.conf"
    rm -f -- "${f:?}"
}

check SD11 "Environment variables: written, refused plainly, overridden in user.lua, removed" sd11
sd11() {
    local r n e v
    r=$(merge env '.vars += [{name: "UQLIVE_FOO", value: "bar"}]') || { bad "$r"; return; }
    grep -qx "UQLIVE_FOO='bar'" "$ENVD" || bad "60-uq.conf doesn't have UQLIVE_FOO='bar': $(cat "$ENVD" 2>/dev/null)"
    grep -qF 'hl.env("UQLIVE_FOO", "bar"' <<<"$(gen_lua)" || bad "generated.lua doesn't set UQLIVE_FOO"
    if [ -n "${HAVE[systemd]:-}" ]; then
        grep -qx 'UQLIVE_FOO=bar' <<<"$(systemctl --user show-environment)" || bad "systemctl --user show-environment has no UQLIVE_FOO=bar"
    fi
    if [ -n "${HAVE[hypr]:-}" ]; then
        if env_of sd11; then grep -qx 'UQLIVE_FOO=bar' "$WORK/env.sd11" || bad "a new app has no UQLIVE_FOO=bar"
        else bad "the test command didn't run"; fi
    fi
    for v in 'XDG_SESSION_TYPE=x' 'XDG_CONFIG_HOME=x' 'UQLIVE_BAD=A$B'; do
        r=$(merge env ".vars += [{name: \"${v%%=*}\", value: \"${v#*=}\"}]") && bad "$v was accepted"
        note "$v: $r"
    done
    n=$(user_lua_add 'hl.env("UQLIVE_FOO", "x")')
    e=$(explain env | jq -c '.. | objects | select(.source? == "user_lua")' | head -n1)
    note "user.lua line $n; Explain env: $(explain env | jq -c . | cut -c1-300)"
    [[ $e == *"\"line\":$n"* ]] || bad "expected UQLIVE_FOO overridden in user.lua, line $n; observed ${e:-nothing}"
    user_lua_reset
    r=$(merge env '.vars = [.vars[] | select(.name != "UQLIVE_FOO")]') || bad "removing it: $r"
    grep -q UQLIVE_FOO "$ENVD" 2>/dev/null && bad "60-uq.conf still has UQLIVE_FOO"
    return 0
}

check SD12 "History and restore: undo a switch, a fix and a startup toggle; the files follow" sd12
sd12() {
    sys_dirs || { na "needs the sandbox's system folders or root"; return; }
    local r id=${LIVE_PREFIX}-z s=${LIVE_PREFIX}-s12.desktop o p hid e1 e2 e3
    o=$APPS_DIR/$id.desktop hid=$AUTOSTART_DIR/$s
    sys_file "$SYSAPPS" "$id.desktop" "$(dump_app "$id")"
    sys_file "$SYSAUTO" "$s" "$(desktop "Live s12" true)"
    p=$(bus SaveRestorePoint s "$LIVE_PREFIX startup") || { bad "save: $p"; sys_cleanup; return; }
    merge compat '.qt_wayland = true' >/dev/null; e1=$(newest)
    merge compat ".apps += [{id: \"$id\", platform: \"x11\", scale: 2, env: [], args: [], elsewhere: true}]" >/dev/null; e2=$(newest)
    merge autostart ".files += [{id: \"$s\", enabled: false, delay: null}]" >/dev/null; e3=$(newest)
    note "rows: $(jq -r .summary <<<"$e1") / $(jq -r .summary <<<"$e2") / $(jq -r .summary <<<"$e3")"
    [ -f "$o" ] || bad "no override after the fix"
    [ -f "$hid" ] || bad "no hidden copy after turning the entry off"
    r=$(bus Undo sb "$(jq -r .id <<<"$e3")" false) || bad "undo the startup toggle: $r"
    [ -e "$hid" ] && bad "undoing the toggle left the hidden copy"
    r=$(bus Undo sb "$(jq -r .id <<<"$e2")" true) || bad "undo the fix: $r"
    [ -e "$o" ] && bad "undoing the fix left the override"
    r=$(bus Undo sb "$(jq -r .id <<<"$e1")" true) || bad "undo the switch: $r"
    expect_eq "Qt prefers Wayland after undo" false "$(section compat | jq -r .qt_wayland)"
    merge compat '.steam_scale = true' >/dev/null
    r=$(bus Restore s "$p") || bad "restore: $r"
    expect_eq "Steam UI scale after the restore" false "$(section compat | jq -r .steam_scale)"
    bus DeleteRestorePoint s "$p" >/dev/null
    sys_cleanup
}

check SD13 "Profiles: a Work profile applies startup settings and goes back; bad ones are refused" sd13
sd13() {
    sys_dirs || { na "needs the sandbox's system folders or root"; return; }
    local r id=${LIVE_PREFIX}-work e app=${LIVE_PREFIX}-w s=${LIVE_PREFIX}-ws.desktop
    sys_file "$SYSAPPS" "$app.desktop" "$(dump_app "$app")"
    sys_file "$SYSAUTO" "$s" "$(desktop "Live ws" true)"
    merge compat ".apps += [{id: \"$app\", platform: \"auto\", scale: 1.25, env: [], args: [], elsewhere: false}]" >/dev/null
    r=$(bus SetProfile sb "$(printf 'format = 1\nid = "%s"\nname = "Work"\n\n[env]\nUQLIVE_WORK = "1"\n\n[compat]\nqt_wayland = true\n\n[compat.apps."%s.desktop"]\nscale = 1.5\n\n[autostart."%s"]\nenabled = false\n' "$id" "$app" "$s")" false) ||
        { bad "SetProfile: $r"; sys_cleanup; return; }
    r=$(bus ApplyProfile s "$id") || { bad "ApplyProfile: $r"; sys_cleanup; return; }
    note "ApplyProfile: $r"
    [[ $r == *log* ]] || bad "expected the notice that startup apps change at the next login; observed $r"
    grep -qx "UQLIVE_WORK='1'" "$ENVD" || bad "60-uq.conf doesn't have UQLIVE_WORK"
    expect_eq "Qt prefers Wayland while on" true "$(section compat | jq -r .qt_wayland)"
    e=$(history_json 6 | jq -r --arg p "profile:$id" '[.[] | select(.origin == $p)] | length')
    [ "$e" -ge 1 ] || bad "expected History entries from the Work profile"
    note "Explain compat: $(explain compat | jq -c '.qt_wayland // empty' | cut -c1-200)"
    r=$(bus ProfileOff) || bad "ProfileOff: $r"
    grep -q UQLIVE_WORK "$ENVD" 2>/dev/null && bad "UQLIVE_WORK stayed after Back to normal"
    expect_eq "Qt prefers Wayland after Back to normal" false "$(section compat | jq -r .qt_wayland)"
    r=$(bus SetProfile sb "$(printf 'format = 1\nid = "%s-argv"\nname = "Argv"\n\n[autostart."a-1234"]\nargv = ["x"]\n' "$LIVE_PREFIX")" false) && bad "a profile with argv was accepted"
    note "argv in a profile: $r"
    r=$(bus SetProfile sb "$(printf 'format = 1\nid = "%s-gone"\nname = "Gone"\n\n[autostart."%s-nothere.desktop"]\nenabled = false\n' "$LIVE_PREFIX" "$LIVE_PREFIX")" false) ||
        { bad "SetProfile: $r"; sys_cleanup; return; }
    r=$(bus ApplyProfile s "${LIVE_PREFIX}-gone") || bad "ApplyProfile: $r"
    note "a startup entry that isn't here: $r"
    [[ $r == *"isn't on this computer, so it was left out"* ]] || bad "expected the missing-entry notice; observed $r"
    sys_cleanup
}

check SD14 "Dev session: no startup apps, and 'other launchers' is refused" sd14 sandbox
sd14() {
    sys_dirs || { na "needs the sandbox's system folders"; return; }
    local r id=${LIVE_PREFIX}-dev s=${LIVE_PREFIX}-devs.desktop
    sys_file "$SYSAPPS" "$id.desktop" "$(dump_app "$id")"
    sys_file "$SYSAUTO" "$s" "$(desktop "Live devs" true)"
    UQ_DEV_SESSION=1 uqd_restart || { bad "uqd didn't restart as a dev session"; sys_cleanup; return; }
    bus ShellReady >/dev/null; sleep 1
    r=$(autostart_row "$s")
    note "startup row: $r"
    [[ $r == *"Not started in a test session"* ]] || bad "expected 'Not started in a test session'; observed $r"
    r=$(merge compat ".apps += [{id: \"$id\", platform: \"x11\", scale: 1.5, env: [], args: [], elsewhere: true}]") && bad "'other launchers' was accepted in a dev session"
    note "other launchers: $r"
    [[ $r == *"Not available in a test session"* ]] || bad "expected 'Not available in a test session'; observed $r"
    [ -e "$APPS_DIR/$id.desktop" ] && bad "a dev session wrote $APPS_DIR/$id.desktop"
    uqd_restart || bad "uqd didn't restart normally"
    sys_cleanup
}
