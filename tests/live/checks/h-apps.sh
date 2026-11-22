# shellcheck shell=bash
# Other apps follow the theme ("Theme other apps", Appearance > Other apps): the files uqd writes
# for Qt, KDE, Kvantum and GTK apps at start-up and at every theme change, that it takes back a
# file another app rewrote (the qt6ct app's OK button, KDE's colour settings), that the switch
# turns all of that off, and what Qt platform theme an app Hyprland starts gets.
#
# These rewrite ~/.config/qt6ct, kdeglobals and GTK's gtk.css on purpose, so they run only in the
# sandbox (tests/live/sandbox.sh), never on a real desktop. How the apps then look is captured by
# scripts/app-theme-shots.sh (needs a person's eyes, so not a check here).

APPS_CFG=${XDG_CONFIG_HOME:-$HOME/.config}
APPS_THEME=${XDG_STATE_HOME:-$HOME/.local/state}/uq/theme
QT6CT_APP='[Appearance]
color_scheme_path=/usr/share/qt6ct/colors/darker.conf
custom_palette=true
standard_dialogs=gtk3
style=Fusion

[Fonts]
general="DejaVu LGC Sans,12,-1,5,400,0,0,0,0,0,0,0,0,0,0,1,,0,0"
'
BREEZE='[General]
ColorScheme=BreezeLight

[Colors:Window]
BackgroundNormal=239,240,241
'

# same FILE TEXT: the file holds exactly TEXT.
same() { [ "$(sha256sum <"$1" 2>/dev/null)" = "$(printf '%s' "$2" | sha256sum)" ]; }
apps_ours() { grep -qx "color_scheme_path=$APPS_THEME/qt6ct/colors/uq.conf" "$APPS_CFG/qt6ct/qt6ct.conf" 2>/dev/null; }

check SH1 "Start-up writes uQ's look for Qt, KDE and GTK apps" sh1 hypr sandbox
sh1() {
    local f
    "$UQCTL" reload >/dev/null 2>&1 || bad "uqctl reload failed"
    apps_ours || bad "qt6ct.conf doesn't load uQ's colours: $(head -c 300 "$APPS_CFG/qt6ct/qt6ct.conf" 2>&1 | tr '\n' '|')"
    grep -q '^icon_theme=' "$APPS_CFG/qt6ct/qt6ct.conf" 2>/dev/null || bad "qt6ct.conf names no icon theme"
    grep -q '^general=' "$APPS_CFG/qt6ct/qt6ct.conf" 2>/dev/null || bad "qt6ct.conf sets no font"
    f=$APPS_CFG/kdeglobals
    grep -qx 'ColorScheme=uQ' "$f" 2>/dev/null || bad "kdeglobals doesn't say ColorScheme=uQ: $(head -c 200 "$f" 2>&1 | tr '\n' '|')"
    grep -q '^\[Colors:Window\]' "$f" 2>/dev/null || bad "kdeglobals has no colours"
    grep -q '^widgetStyle=' "$f" 2>/dev/null || bad "kdeglobals names no widget style"
    for f in gtk-3.0 gtk-4.0; do
        grep -qF "$APPS_THEME/$f/gtk.css" "$APPS_CFG/$f/gtk.css" 2>/dev/null || bad "$f/gtk.css doesn't import uQ's"
        grep -q 'corners: generated' "$APPS_THEME/$f/gtk.css" 2>/dev/null || bad "uQ's $f css has no corner rules"
    done
    note "qt6ct.conf: $(tr '\n' '|' <"$APPS_CFG/qt6ct/qt6ct.conf" | cut -c1-200)"
}

check SH2 "A qt6ct.conf and kdeglobals another app rewrote are themed again, with a copy kept" sh2 hypr sandbox
sh2() {
    printf '%s' "$QT6CT_APP" >"$APPS_CFG/qt6ct/qt6ct.conf"
    printf '%s' "$BREEZE" >"$APPS_CFG/kdeglobals"
    rm -f "$APPS_CFG/qt6ct/qt6ct.conf.before-uq"* "$APPS_CFG/kdeglobals.before-uq"*
    "$UQCTL" reload >/dev/null 2>&1 || bad "uqctl reload failed"
    apps_ours || bad "qt6ct.conf wasn't taken back: $(tr '\n' '|' <"$APPS_CFG/qt6ct/qt6ct.conf" | cut -c1-200)"
    grep -q 'DejaVu\|darker' "$APPS_CFG/qt6ct/qt6ct.conf" && bad "qt6ct.conf still has the qt6ct app's font or colours"
    grep -qx 'standard_dialogs=gtk3' "$APPS_CFG/qt6ct/qt6ct.conf" || bad "qt6ct.conf lost a key uQ doesn't set (standard_dialogs)"
    same "$APPS_CFG/qt6ct/qt6ct.conf.before-uq" "$QT6CT_APP" || bad "the copy of qt6ct.conf isn't the qt6ct app's file"
    grep -qx 'ColorScheme=uQ' "$APPS_CFG/kdeglobals" || bad "kdeglobals wasn't taken back"
    grep -q '239,240,241' "$APPS_CFG/kdeglobals" && bad "kdeglobals still has Breeze's colours"
    [ -f "$APPS_CFG/kdeglobals.before-uq" ] || bad "no copy of the Breeze kdeglobals"
}

check SH3 "Theme other apps off: files another app wrote are left alone" sh3 hypr sandbox
sh3() {
    local r
    r=$(merge appearance '.theme_apps = false') || { bad "switching it off: $r"; return; }
    printf '%s' "$QT6CT_APP" >"$APPS_CFG/qt6ct/qt6ct.conf"
    printf '%s' "$BREEZE" >"$APPS_CFG/kdeglobals"
    "$UQCTL" reload >/dev/null 2>&1 || bad "uqctl reload failed"
    same "$APPS_CFG/qt6ct/qt6ct.conf" "$QT6CT_APP" || bad "qt6ct.conf changed with the switch off"
    same "$APPS_CFG/kdeglobals" "$BREEZE" || bad "kdeglobals changed with the switch off"
    r=$(merge appearance '.theme_apps = true') || bad "switching it back on: $r"
    apps_ours || bad "switching it back on didn't theme qt6ct.conf again"
}

check SH4 "Apps Hyprland starts get uQ's Qt platform theme" sh4 hypr
sh4() {
    local out=$WORK/sh4.env want
    rm -f "$out"
    hc eval "return hl.dispatch(hl.dsp.exec_cmd('env > $out'))" >/dev/null 2>&1 ||
        hc dispatch exec "env > $out" >/dev/null 2>&1
    wait_for 5 test -s "$out" || { bad "the app Hyprland started wrote no environment"; return; }
    if [ -f /usr/lib/qt6/plugins/platformthemes/KDEPlasmaPlatformTheme6.so ]; then want=kde; else want=qt6ct; fi
    expect_eq "QT_QPA_PLATFORMTHEME" "$want" "$(sed -n 's/^QT_QPA_PLATFORMTHEME=//p' "$out")"
}
