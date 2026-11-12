# shellcheck shell=bash
# Settings slice G (your own themes): docs/notes/settings-slice-g-live-check.md. Run through the
# same uqd calls the Appearance page and `uqctl theme` make, against a .uqtheme fixture
# (tests/live/fixtures/uqtheme).
#
# What stays manual, and why:
#   1   the two-machine round trip, and "Use it now" re-theming the bar, a new kitty and GTK/Qt
#       apps (needs a person's eyes); SG3 checks the files uQ renders instead
#   4   the theme editor (Qt dialog: the contrast warning, the greyed Save, live editing, the
#       colour boxes); SG4 checks the same contrast rule through Import
#   5   a refusal "shown on the page" (SG4 checks the sentence, not where it is shown)
#   11  in the app, Import and Delete in `uq-dev session` (SG9 checks the daemon's side)
#   12  the VM's own T1 to T3 (tests/vm/checks/60-themes.sh)
# Item 10 (user.lua untouched) is checked after every check by run.sh.
#
# Everything the checks add is named uqlive-*; theme_cleanup (lib.sh) removes any that a failed
# check left behind.

GT_ID=user-uqlive-forest
GT_FIX=$LIVE_DIR/fixtures/uqtheme
GT_DIR=${XDG_DATA_HOME:-$HOME/.local/share}/uq/themes
tinfo() { bus ListThemeInfo; }
# trow ID: the theme's row in ListThemeInfo.
trow() { tinfo | jq -c --arg i "$1" '.themes[] | select(.id == $i)'; }
theme_ids() { bus ListThemes | jq -r '.[]' | sort | tr '\n' ' '; }
tokens_accent() { jq -r '.colors.accent // .accent // empty' "$STATE_DIR/theme/tokens.json" 2>/dev/null; }
# tokens_has HEX: the rendered tokens.json mentions this colour.
tokens_has() { grep -qi -- "$1" "$STATE_DIR/theme/tokens.json" 2>/dev/null; }

check SG1 "Import a .uqtheme: listed as yours, not switched to, a restore point, a duplicate needs --replace" sg1
sg1() {
    local r before_theme
    before_theme=$("$UQCTL" theme get)
    r=$("$UQCTL" theme import "$GT_FIX/uqlive-forest.uqtheme" 2>&1) || { bad "import: $r"; return; }
    note "import: $r"
    expect_eq "what import said" "Added theme: $GT_ID" "$r"
    [ -f "$GT_DIR/$GT_ID/dark.toml" ] && [ -f "$GT_DIR/$GT_ID/light.toml" ] ||
        bad "expected $GT_DIR/$GT_ID/{dark,light}.toml; observed $(ls "$GT_DIR/$GT_ID" 2>&1 | tr '\n' ' ')"
    note "row: $(trow "$GT_ID")"
    expect_eq "the theme's name" "uqlive Forest" "$(trow "$GT_ID" | jq -r .name)"
    expect_eq "yours" true "$(trow "$GT_ID" | jq -r .user)"
    expect_eq "in use" false "$(trow "$GT_ID" | jq -r .active)"
    [[ " $(theme_ids) " == *" $GT_ID "* ]] || bad "ListThemes doesn't list $GT_ID: $(theme_ids)"
    "$UQCTL" theme list --long | grep -q "$GT_ID  uqlive Forest  (yours)" || bad "theme list --long has no '$GT_ID  uqlive Forest  (yours)'"
    expect_eq "the theme in use after importing" "$before_theme" "$("$UQCTL" theme get)"
    points | jq -r '.[].name' | grep -qx 'Before adding the uqlive Forest theme' ||
        bad "no restore point 'Before adding the uqlive Forest theme': $(points | jq -r '[.[].name] | join(", ")' | cut -c1-200)"
    r=$("$UQCTL" theme import "$GT_FIX/uqlive-forest.uqtheme" 2>&1) && bad "importing the same id again was accepted"
    note "again: $r"
    [[ $r == *"--replace"* ]] || bad "expected the refusal to point at --replace; observed $r"
    r=$("$UQCTL" theme import --replace "$GT_FIX/uqlive-forest.uqtheme" 2>&1) || bad "import --replace: $r"
    # Restoring that point brings the settings back but takes nothing out of the themes folder.
    local p; p=$(points | jq -r '[.[] | select(.name == "Before adding the uqlive Forest theme")][0].id // empty')
    if [ -n "$p" ]; then
        r=$(bus Restore s "$p") || bad "restore: $r"
        [ -f "$GT_DIR/$GT_ID/dark.toml" ] || bad "restoring the point removed the theme's files"
        [[ " $(theme_ids) " == *" $GT_ID "* ]] || bad "$GT_ID left the list after the restore"
        expect_eq "the theme in use after the restore" "$before_theme" "$("$UQCTL" theme get)"
    fi
    theme_cleanup
}

check SG2 "Export: the file is the imported one, --with-look adds the look, and it imports again" sg2
sg2() {
    local r gaps f=$WORK/sg2.uqtheme
    r=$("$UQCTL" theme import "$GT_FIX/uqlive-forest.uqtheme" 2>&1) || { bad "import: $r"; return; }
    "$UQCTL" theme export "$GT_ID" "$f" || { bad "export to a file failed"; theme_cleanup; return; }
    [ "$(cat "$GT_FIX/uqlive-forest.uqtheme")" = "$(cat "$f")" ] ||
        bad "the exported file differs from the imported one: $(grep -vxFf "$GT_FIX/uqlive-forest.uqtheme" "$f" | head -n 4 | tr '\n' '|') (and the other way: $(grep -vxFf "$f" "$GT_FIX/uqlive-forest.uqtheme" | head -n 4 | tr '\n' '|'))"
    r=$("$UQCTL" theme export "$GT_ID") || bad "export to the screen failed"
    [ "$r" = "$(cat "$f")" ] || bad "export to the screen isn't the file's text"
    grep -q '^\[look\]' "$f" && bad "a plain export has a [look] section"
    gaps=$(section look | jq -c .gaps_in)
    merge look '.gaps_in = 9' >/dev/null || bad "setting a look to export"
    "$UQCTL" theme export "$GT_ID" "$WORK/sg2-look.uqtheme" --with-look || bad "export --with-look failed"
    merge look ".gaps_in = $gaps" >/dev/null
    grep -q '^\[look\]' "$WORK/sg2-look.uqtheme" || bad "export --with-look has no [look] section"
    note "[look] in the export: $(sed -n '/^\[look\]/,$p' "$WORK/sg2-look.uqtheme" | head -n 4 | tr '\n' ' ')"
    grep -qx 'gaps_in = 9' "$WORK/sg2-look.uqtheme" || bad "the exported look doesn't say gaps_in = 9"
    r=$("$UQCTL" theme import --replace "$WORK/sg2-look.uqtheme" 2>&1) || bad "importing the --with-look export: $r"
    expect_eq "gaps_in after importing without --with-look" "$gaps" "$(section look | jq -c .gaps_in)"
    r=$("$UQCTL" theme export "$GT_ID" 2>&1) || bad "export: $r"
    [ "$r" = "$(cat "$f")" ] || bad "importing the --with-look export changed the theme itself"
    r=$("$UQCTL" theme export user-uqlive-nothere 2>&1) && bad "exporting a theme that isn't there was accepted"
    note "unknown theme: $r"
    theme_cleanup
}

check SG3 "With look: gaps change, one History row says so, Undo puts them back" sg3
sg3() {
    local r gaps e
    gaps=$(section look | jq -c .gaps_in)
    { cat "$GT_FIX/uqlive-forest.uqtheme"; printf '\n[look]\ngaps_in = 17\n'; } >"$WORK/sg3.uqtheme"
    r=$("$UQCTL" theme import --with-look "$WORK/sg3.uqtheme" 2>&1) || { bad "import --with-look: $r"; return; }
    expect_eq "gaps_in after importing with the look" 17 "$(section look | jq -c .gaps_in)"
    e=$(history_json 5 | jq -c '[.[] | select(.origin == "uqctl")] | first')
    note "History: $(jq -r .summary <<<"$e")"
    [[ $(jq -r .summary <<<"$e") == *[Ll]ook* || $(jq -r .summary <<<"$e") == *[Gg]aps* ]] ||
        bad "expected the newest History row to name the window look; observed $(jq -c . <<<"$e" | cut -c1-200)"
    expect_eq "History rows from the import that change a setting" 1 "$(history_json 5 | jq '[.[] | select(.origin == "uqctl" and (.summary | test("look|Look|gaps|Gaps")))] | length')"
    r=$(bus Undo sb "$(jq -r .id <<<"$e")" false) || bad "Undo: $r"
    expect_eq "gaps_in after Undo" "$gaps" "$(section look | jq -c .gaps_in)"
    theme_cleanup
}

check SG4 "Apply: the colours reach the rendered files, and the old theme comes back" sg4
sg4() {
    local r was id var
    was=$("$UQCTL" theme get); id=${was% *}; var=${was#* }
    r=$("$UQCTL" theme import "$GT_FIX/uqlive-forest.uqtheme" 2>&1) || { bad "import: $r"; return; }
    r=$("$UQCTL" theme set "$GT_ID" dark 2>&1) || bad "theme set dark: $r"
    expect_eq "the theme in use" "$GT_ID dark" "$("$UQCTL" theme get)"
    tokens_has '#7fb069' || bad "tokens.json has no dark accent #7fb069 (accent: $(tokens_accent))"
    [ -n "$(grep -rli '7fb069' "$STATE_DIR/theme" 2>/dev/null | grep -v tokens.json | head -n1)" ] ||
        bad "no GTK, Qt or terminal file under $STATE_DIR/theme has the accent #7fb069"
    r=$("$UQCTL" theme set "$GT_ID" light 2>&1) || bad "theme set light: $r"
    tokens_has '#3f6b2e' || bad "tokens.json has no light accent #3f6b2e (accent: $(tokens_accent))"
    r=$("$UQCTL" theme set "$id" "$var" 2>&1) || bad "putting $was back: $r"
    expect_eq "the theme in use after putting it back" "$was" "$("$UQCTL" theme get)"
    # (Forest's own colours are the right ones when Forest is what was in use.)
    case $id in forest | user-uqlive-forest) ;; *) ! tokens_has '#7fb069' || bad "tokens.json still has Forest's accent after putting $was back" ;; esac
    note "light and dark switched, then $was back"
    theme_cleanup
}

check SG5 "Hostile files: each refused in one plain sentence, nothing added; a FIFO and a huge file don't hang" sg5
sg5() {
    local f r rc listed folders n=0
    r=$("$UQCTL" theme import "$GT_FIX/uqlive-forest.uqtheme" 2>&1) || { bad "import: $r"; return; }
    "$UQCTL" theme delete "$GT_ID" >/dev/null
    listed=$(theme_ids) folders=$(ls -A "$GT_DIR" 2>/dev/null | tr '\n' ' ')
    for f in "$GT_FIX"/bad/*.uqtheme; do
        n=$((n + 1))
        r=$(timeout 15 "$UQCTL" theme import "$f" 2>&1); rc=$?
        [ "$rc" -ne 0 ] && [ "$rc" -ne 124 ] || { bad "${f##*/}: expected a refusal; exit $rc: $r"; continue; }
        [ "$(wc -l <<<"$r")" = 1 ] || bad "${f##*/}: expected one line; observed $(wc -l <<<"$r"): $(tr '\n' ' ' <<<"$r" | cut -c1-120)"
        case "$r" in *[Pp]anic* | *backtrace* | *org.uq* | *uqd*) bad "${f##*/}: the refusal isn't plain: $r" ;; esac
        note "${f##*/}: $r"
    done
    expect_eq "themes listed after the refused imports" "$listed" "$(theme_ids)"
    expect_eq "the themes folder after the refused imports" "$folders" "$(ls -A "$GT_DIR" 2>/dev/null | tr '\n' ' ')"
    mkfifo "$WORK/x.uqtheme"
    r=$(timeout 10 "$UQCTL" theme import "$WORK/x.uqtheme" 2>&1 </dev/null); rc=$?
    [ "$rc" = 124 ] && bad "importing a FIFO hung"
    [ "$rc" -ne 0 ] || bad "importing a FIFO was accepted"
    note "FIFO: $r"
    rm -f -- "$WORK/x.uqtheme"
    head -c 65537 /dev/zero | tr '\0' 'a' >"$WORK/big.uqtheme"
    r=$(timeout 10 "$UQCTL" theme import "$WORK/big.uqtheme" 2>&1); rc=$?
    [ "$rc" -ne 0 ] && [ "$rc" -ne 124 ] || bad "a 65 537-byte file: expected a refusal; exit $rc: $r"
    note "65 537 bytes: $r"
    r=$("$UQCTL" theme import "$WORK/not-here.uqtheme" 2>&1) && bad "a file that isn't there was accepted"
    note "no such file: $r"
    note "$n hostile files tried"
    expect_eq "themes folder after everything" "$folders" "$(ls -A "$GT_DIR" 2>/dev/null | tr '\n' ' ')"
}

check SG6 "Links: a symlinked theme folder is skipped with a reason; a symlinked themes folder refuses imports" sg6
sg6() {
    local r aside=$WORK/themes.aside empty=$WORK/themes.empty had=0
    mkdir -p "$GT_DIR"
    ln -s /etc "$GT_DIR/user-uqlive-etc"
    r=$(tinfo)
    [[ " $(theme_ids) " == *" user-uqlive-etc "* ]] && bad "a linked theme folder is listed"
    note "skipped: $(jq -c '.skipped' <<<"$r" | cut -c1-300)"
    jq -e '[.skipped[] | select(.reason | test("link"))] | length >= 1' <<<"$r" >/dev/null ||
        bad "expected a skipped entry whose reason says it is a link"
    rm -f -- "$GT_DIR/user-uqlive-etc"
    # The whole themes folder as a link.
    if [ -e "$GT_DIR" ]; then mv "$GT_DIR" "$aside" && had=1; fi
    mkdir -p "$empty"
    ln -s "$empty" "$GT_DIR"
    r=$(tinfo | jq -r .root_is_link)
    expect_eq "root_is_link" true "$r"
    r=$("$UQCTL" theme import "$GT_FIX/uqlive-forest.uqtheme" 2>&1) && bad "an import through a linked themes folder was accepted"
    note "import through a link: $r"
    [[ $r == *"link"* ]] || bad "expected the refusal to say the folder is a link; observed $r"
    [ -z "$(ls -A "$empty")" ] || bad "something was written through the link: $(ls -A "$empty" | tr '\n' ' ')"
    rm -f -- "$GT_DIR"
    [ "$had" = 1 ] && mv "$aside" "$GT_DIR"
    expect_eq "root_is_link after putting the folder back" false "$(tinfo | jq -r .root_is_link)"
}

check SG7 "By hand: a folder copied in shows up at once, and a broken one is skipped" sg7
sg7() {
    local r hand=user-uqlive-hand f
    r=$("$UQCTL" theme import "$GT_FIX/uqlive-forest.uqtheme" 2>&1) || { bad "import: $r"; return; }
    cp -r "$GT_DIR/$GT_ID" "$GT_DIR/$hand"
    for f in "$GT_DIR/$hand"/*.toml; do sed -i "s/$GT_ID/$hand/g" "$f"; done
    [[ " $(theme_ids) " == *" $hand "* ]] || bad "a folder copied in with cp -r isn't listed: $(theme_ids)"
    expect_eq "yours" true "$(trow "$hand" | jq -r .user)"
    # A folder with only one variant is skipped, with a reason, and doesn't hide the rest.
    mkdir "$GT_DIR/user-uqlive-wrong"
    cp "$GT_DIR/$GT_ID/dark.toml" "$GT_DIR/user-uqlive-wrong/dark.toml"
    r=$(tinfo)
    [[ " $(theme_ids) " == *" user-uqlive-wrong "* ]] && bad "a folder with only one variant is listed"
    note "skipped: $(jq -c '.skipped' <<<"$r" | cut -c1-300)"
    [ "$(jq '[.skipped[] | select(.folder == "user-uqlive-wrong")] | length' <<<"$r")" = 1 ] || bad "the one-variant folder isn't in the skipped list"
    [[ " $(theme_ids) " == *" $GT_ID "* ]] || bad "$GT_ID dropped out of the list beside a broken folder"
    rm -rf -- "${GT_DIR:?}/user-uqlive-wrong"
    "$UQCTL" theme delete "$hand" >/dev/null || bad "deleting the copied-in theme failed"
    theme_cleanup
}

check SG8 "Delete: the theme in use goes to the default first; a profile that used it is named and can't apply it" sg8
sg8() {
    local r was id var e out
    was=$("$UQCTL" theme get); id=${was% *}; var=${was#* }
    r=$("$UQCTL" theme import "$GT_FIX/uqlive-forest.uqtheme" 2>&1) || { bad "import: $r"; return; }
    r=$(bus SetProfile sb "$(printf 'format = 1\nid = "%s-th"\nname = "uqlive th"\n\n[appearance]\ntheme = "%s"\n' "$LIVE_PREFIX" "$GT_ID")" false) || { bad "SetProfile: $r"; theme_cleanup; return; }
    "$UQCTL" theme set "$GT_ID" "$var" >/dev/null || bad "switching to the theme"
    out=$("$UQCTL" theme delete "$GT_ID" 2>&1) || bad "delete: $out"
    note "delete: $(tr '\n' '|' <<<"$out")"
    [[ $out == *"Switched to the default theme first."* ]] || bad "expected 'Switched to the default theme first.'; observed $out"
    [[ $out == *"uqlive th profile uses this theme"* ]] || bad "expected the uqlive th profile to be named; observed $out"
    expect_eq "the theme in use after deleting the active one" "graphite $var" "$("$UQCTL" theme get)"
    [ -e "$GT_DIR/$GT_ID" ] && bad "the theme's folder is still there"
    [[ " $(theme_ids) " == *" $GT_ID "* ]] && bad "$GT_ID is still listed"
    e=$(history_json 8 | jq -c '[.[] | select(.summary | test("[Tt]heme"))] | first')
    note "History: $(jq -r .summary <<<"$e")"
    [ "$e" != null ] && [ -n "$e" ] || bad "no History row for the switch to the default theme"
    r=$(bus ApplyProfile s "${LIVE_PREFIX}-th") && bad "applying a profile whose theme is gone was accepted: $r"
    note "apply: $r"
    [[ $r == *"That theme isn't installed any more."* ]] || bad "expected \"That theme isn't installed any more.\"; observed $r"
    r=$("$UQCTL" theme delete "$GT_ID" 2>&1) && bad "deleting a theme that is gone was accepted"
    r=$("$UQCTL" theme delete graphite 2>&1) && bad "deleting a theme that comes with uQ was accepted"
    note "a built-in theme: $r"
    [[ $r == *"come with uQ"* ]] || bad "expected 'Themes that come with uQ can't be deleted.'; observed $r"
    "$UQCTL" theme set "$id" "$var" >/dev/null
    expect_eq "the theme in use at the end" "$was" "$("$UQCTL" theme get)"
}

check SG9 "Gone while active: the list says so, settings still save, a restart falls back to the default" sg9 restart
sg9() {
    local r was id var
    was=$("$UQCTL" theme get); id=${was% *}; var=${was#* }
    r=$("$UQCTL" theme import "$GT_FIX/uqlive-forest.uqtheme" 2>&1) || { bad "import: $r"; return; }
    "$UQCTL" theme set "$GT_ID" "$var" >/dev/null || bad "switching to the theme"
    rm -rf -- "${GT_DIR:?}/$GT_ID"
    note "row: $(trow "$GT_ID")"
    expect_eq "missing" true "$(trow "$GT_ID" | jq -r .missing)"
    expect_eq "still in use" true "$(trow "$GT_ID" | jq -r .active)"
    "$UQCTL" theme list --long | grep -q "$GT_ID.*(its files are gone)" || bad "theme list --long doesn't say its files are gone"
    r=$(merge compat '.steam_scale = true') || bad "another setting didn't save while the theme's files are gone: $r"
    merge compat '.steam_scale = false' >/dev/null
    uqd_restart || { bad "uqd didn't come back"; return; }
    wait_for 10 uqd_up
    expect_eq "the theme in use after a restart" "graphite $var" "$("$UQCTL" theme get)"
    [[ " $(theme_ids) " == *" $GT_ID "* ]] && bad "$GT_ID is still listed after the restart"
    "$UQCTL" theme set "$id" "$var" >/dev/null
    expect_eq "the theme in use at the end" "$was" "$("$UQCTL" theme get)"
}

check SG10 "Dev session: Import and Delete say 'Not available in a test session'" sg10 sandbox
sg10() {
    local r
    r=$("$UQCTL" theme import "$GT_FIX/uqlive-forest.uqtheme" 2>&1) || { bad "import: $r"; return; }
    UQ_DEV_SESSION=1 uqd_restart || { bad "uqd didn't restart as a dev session"; theme_cleanup; return; }
    r=$("$UQCTL" theme import --replace "$GT_FIX/uqlive-forest.uqtheme" 2>&1) && bad "an import was accepted in a dev session"
    note "import: $r"
    [[ $r == *"Not available in a test session"* ]] || bad "expected 'Not available in a test session' for Import; observed $r"
    r=$("$UQCTL" theme delete "$GT_ID" 2>&1) && bad "a delete was accepted in a dev session"
    note "delete: $r"
    [[ $r == *"Not available in a test session"* ]] || bad "expected 'Not available in a test session' for Delete; observed $r"
    [ -d "$GT_DIR/$GT_ID" ] || bad "the dev session removed the theme's folder"
    uqd_restart || bad "uqd didn't restart normally"
    theme_cleanup
}
