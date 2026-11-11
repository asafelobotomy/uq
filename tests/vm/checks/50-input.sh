# shellcheck shell=bash
# Settings slice 3, Input (docs/notes/settings-slice3-live-check.md): what uqd writes reaches
# Hyprland. Feel (is a 250 ms delay noticeable?) still needs a person; these check the values.
# Uses uqbus from 40-displays.sh.

# hopt PATH: one live Hyprland option's value (`hyprctl -j getoption`).
hopt() { vgh hyprctl -j getoption "$1" 2>/dev/null | jq -c 'if has("int") then .int elif has("float") then (.float + 0) elif has("str") then .str elif has("bool") then .bool else .custom end' 2>/dev/null; }
set_input() { uqbus SetSection sss input "$1" settings; }
input_now() { uqbus GetSection s input; }
INPUT_SAVED=
input_save() { [ -n "$INPUT_SAVED" ] || INPUT_SAVED=$(input_now); note "input before: $INPUT_SAVED"; }
input_restore() { [ -n "$INPUT_SAVED" ] && set_input "$INPUT_SAVED" >/dev/null; }
expect_opt() { # expect_opt PATH WANT
    local got
    got=$(hopt "$1")
    if [ "$got" = "$2" ]; then note "$1 = $got"; else bad "expected $1 = $2; observed $got"; fi
}
keymap() { vgh hyprctl devices -j | jq -r '([.keyboards[] | select(.main)] + .keyboards)[0].active_keymap'; }

check I1 "Keyboard: two layouts and a switch combo; the combo switches; reorder and remove" i1
i1() {
    input_save
    local r k1 k2
    r=$(set_input '{"keyboard":{"layouts":[{"layout":"us"},{"layout":"de"}],"switch_combo":"grp:alt_shift_toggle"}}')
    [ -z "$r" ] || { bad "set: $r"; return; }
    sleep 1
    expect_opt input:kb_layout '"us,de"'
    expect_opt input:kb_options '"grp:alt_shift_toggle"'
    k1=$(keymap)
    qmp key alt-shift >/dev/null 2>&1
    sleep 1
    k2=$(keymap)
    note "keymap before the combo: $k1; after: $k2"
    [ "$k1" != "$k2" ] || bad "expected Alt+Shift to switch the layout; it stayed $k1"
    set_input '{"keyboard":{"layouts":[{"layout":"de"},{"layout":"us"}],"switch_combo":"grp:alt_shift_toggle"}}' >/dev/null
    sleep 1
    expect_opt input:kb_layout '"de,us"'
    set_input '{"keyboard":{"layouts":[{"layout":"de"}]}}' >/dev/null
    sleep 1
    expect_opt input:kb_layout '"de"'
    input_restore
}

check I2 "Repeat: delay 250 ms and rate 50 reach Hyprland" i2
i2() {
    input_save
    local r
    r=$(set_input '{"keyboard":{"repeat_delay":250,"repeat_rate":50}}')
    [ -z "$r" ] || { bad "set: $r"; return; }
    sleep 1
    expect_opt input:repeat_delay 250
    expect_opt input:repeat_rate 50
    note "whether it feels faster needs a person at a keyboard"
    input_restore
}

check I3 "Mouse: uqctl mouse speed -0.5 shows at once" i3
i3() {
    input_save
    local r t0
    vgh uqctl settings input >/dev/null 2>&1
    sleep 4
    t0=$(hnow)
    r=$(vgh uqctl mouse speed -0.5 2>&1)
    note "uqctl: ${r:-ok} ($(since "$t0")s)"
    [ "$(input_now | jq -r .mouse.sensitivity)" = -0.5 ] || bad "expected mouse.sensitivity -0.5 in uqd; observed $(input_now)"
    sleep 1
    expect_opt input:sensitivity -0.5
    snap input-page
    vg pkill -x uq-settings
    input_restore
}

check I4 "Touchpad: tap-to-click off, natural scroll on and off" i4
i4() {
    input_save
    local n
    set_input '{"touchpad":{"tap_to_click":false,"natural_scroll":true}}' >/dev/null
    sleep 1
    expect_opt input:touchpad:tap-to-click false
    expect_opt input:touchpad:natural_scroll true
    set_input '{"touchpad":{"natural_scroll":false}}' >/dev/null
    sleep 1
    expect_opt input:touchpad:natural_scroll false
    n=$(vgh hyprctl devices -j | jq '[.mice[] | select(.name | test("touchpad"; "i"))] | length')
    [ "$n" = 0 ] && note "the VM has no touchpad: the options are set; the gestures themselves need real hardware"
    input_restore
}

check I5 "Per device: an override differs from the general value; removing it goes back" i5
i5() {
    input_save
    local dev gen r errs
    dev=$(vgh hyprctl devices -j | jq -r '.mice[0].name')
    note "device: $dev"
    r=$(set_input "{\"mouse\":{\"sensitivity\":0.2},\"devices\":{\"$dev\":{\"sensitivity\":-0.7}}}")
    [ -z "$r" ] || { bad "set: $r"; return; }
    sleep 1
    expect_opt input:sensitivity 0.2
    gen=$(vgsh 'grep -h "hl.device" ~/.config/hypr/uq/generated.lua')
    note "generated: $gen"
    [[ $gen == *"$dev"*"sensitivity = -0.7"* ]] || bad "expected an hl.device line for $dev at -0.7; observed $gen"
    errs=$(vgh hyprctl configerrors 2>&1 | grep -v '^$')
    [ -z "$errs" ] || bad "Hyprland config errors: $errs"
    set_input '{"mouse":{"sensitivity":0.2}}' >/dev/null
    sleep 1
    gen=$(vgsh 'grep -c "hl.device" ~/.config/hypr/uq/generated.lua')
    if [ "$gen" = 0 ]; then note "override removed; the general value applies"; else bad "expected no hl.device line after removing it; observed $gen"; fi
    note "a device's own live value isn't readable over hyprctl; the setting reaching Hyprland is checked, the feel is not"
    input_restore
}

check I6 "No xkeyboard-config list: the page offers a layout code field" i6
i6() {
    local f=/usr/share/X11/xkb/rules/evdev.lst
    vroot mv "$f" "$f.uqvm" || { bad "could not hide $f"; return; }
    vgh uqctl settings input >/dev/null 2>&1
    sleep 5
    snap no-evdev-list
    note "check the screenshot: a layout code field instead of a list"
    vg pkill -x uq-settings
    vroot mv "$f.uqvm" "$f"
}
