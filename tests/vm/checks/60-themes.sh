# shellcheck shell=bash
# Slice G (your own themes): export, import and switch to a user theme through uqctl, the
# quick-settings chips showing names, and deleting the active user theme.

TOK='${XDG_STATE_HOME:-$HOME/.local/state}/uq/theme/tokens.json'

theme_bg() { vgsh "jq -r .colors.bg $TOK" 2>/dev/null; }

c_T1() {
    local want got
    vg rm -f /tmp/g.uqtheme >/dev/null 2>&1
    vg uqctl theme export graphite /tmp/g.uqtheme || { bad "uqctl theme export graphite failed"; return; }
    vg uqctl theme import /tmp/g.uqtheme --replace || { bad "uqctl theme import failed"; return; }
    vg uqctl theme list --long | grep -q 'user-graphite' || bad "user-graphite is not in 'uqctl theme list --long'"
    vg uqctl theme set graphite dark >/dev/null 2>&1
    sleep 2
    want=$(theme_bg)
    vg uqctl theme set user-graphite dark || { bad "uqctl theme set user-graphite dark failed"; return; }
    sleep 3
    got=$(theme_bg)
    note "tokens.json bg: graphite $want, user-graphite $got"
    [ -n "$got" ] && [ "$got" = "$want" ] || bad "the user copy's background ($got) is not graphite's ($want)"
    [ "$(vg uqctl theme get)" = "user-graphite dark" ] || bad "uqctl theme get did not say user-graphite dark"
}

c_T2() {
    ipc open quicksettings >/dev/null
    wait_layer 8 uq-quick || { bad "quick settings did not open"; return; }
    sleep 2
    snap chips
    note "check the screenshot: a chip named Graphite appears twice (a system theme and yours)"
    ipc close quicksettings >/dev/null
}

c_T3() {
    local out
    vg uqctl theme set user-graphite dark >/dev/null 2>&1
    out=$(vg uqctl theme delete user-graphite 2>&1) || { bad "uqctl theme delete failed: $out"; return; }
    note "delete printed: $out"
    printf '%s' "$out" | grep -q 'Switched to the default theme first' || bad "delete did not say it switched first"
    [ "$(vg uqctl theme get | awk '{print $1}')" = graphite ] || bad "the default theme (graphite) is not in use after the delete"
    vg uqctl theme list | grep -q '^user-graphite$' && bad "user-graphite is still listed"
    vg uqctl theme set graphite dark >/dev/null 2>&1
}

check T1 "Themes: export, import and use a user theme" c_T1
check T2 "Themes: quick settings chips show names" c_T2
check T3 "Themes: deleting the active user theme switches first" c_T3
