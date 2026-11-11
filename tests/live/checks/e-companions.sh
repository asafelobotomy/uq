# shellcheck shell=bash
# Settings slice E (pages from other apps): docs/notes/settings-slice-e-live-check.md. The real
# apps (scdr, crumBS, noti-fi, XZram, scale_tail) need not be installed: a fixture `uq/` folder
# (tests/live/fixtures/companion) stands in for an app's package, and is laid out under a scratch
# prefix the way /usr/share/uq is (`uqctl integrations list --root`), because uQ reads integrations
# only from /usr/share/uq, which a check doesn't write.
#
# What stays manual, and why:
#   1   a package install while Settings is open: Settings' own watcher showing the page "within a
#       few seconds" (the live part is the shell's); SE1 checks what `uqctl integrations list`
#       reports as the folder is added and removed
#   2   Settings search finding "boost", "cache", "do not disturb" and "swap" (the Settings app)
#   3   scdr: the password prompt from uQ's agent, `scdrctl`, a scheduler picker, restarting
#       scdrd (the real app, a polkit agent and root)
#   4   crumBS: scans, the trash, the journal's password prompt, uninstalling an app
#   5   noti-fi: do not disturb on the bar island, muted groups, history size, stopping notifd
#   6   a rule named `<b>bold</b>` refused by the pattern (the page's QML)
#   7   `uq-dev session`'s pages acting on the real services
#   9   XZram's size and swappiness (root and a password)
#   10  Tailscale (scale_tail): connect, exit nodes, sign out
# Item 8 (user.lua untouched) is checked after every check by run.sh.

CP_FIX=$LIVE_DIR/fixtures/companion
cp_list() { "$UQCTL" integrations list --root "$WORK/prefix"; }
cp_json() { "$UQCTL" integrations list --root "$WORK/prefix" --json; }
# cp_row KIND ID: that entry in the JSON listing.
cp_row() { cp_json | jq -c --arg k "$1" --arg i "$2" '.[] | select(.kind == $k and .id == $i)'; }
cp_fresh() { rm -rf -- "${WORK:?}/prefix"; mkdir -p "$WORK/prefix/settings/pages" "$WORK/prefix/integrations"; }
cp_install() {
    cp -r "$CP_FIX/good/uq/pages/uqlive-page" "$WORK/prefix/settings/pages/"
    cp -r "$CP_FIX/good/uq/integrations/uqlive-rules" "$WORK/prefix/integrations/"
}

check SE1 "A companion's page and rules appear in 'integrations list' when installed and go when removed" se1
se1() {
    local r row
    cp_fresh
    expect_eq "'integrations list' with nothing installed" "no integrations installed" "$(cp_list)"
    cp_install
    note "$(cp_list | tr '\n' '|')"
    row=$(cp_row page uqlive-page)
    expect_eq "the page's state" shown "$(cp_list | awk -F'\t' '$1 == "page" && $2 == "uqlive-page" { print $3 }')"
    expect_eq "the page's name" "uqlive Page" "$(jq -r .name <<<"$row")"
    expect_eq "the page is valid" true "$(jq -r .valid <<<"$row")"
    expect_eq "the page is visible" true "$(jq -r .visible <<<"$row")"
    expect_eq "the rules' state" shown "$(cp_list | awk -F'\t' '$1 == "rules" && $2 == "uqlive-rules" { print $3 }')"
    # Removing the folders (what a package removal does) takes them off the list.
    rm -rf -- "${WORK:?}/prefix/settings/pages/uqlive-page"
    [ -z "$(cp_row page uqlive-page)" ] || bad "the page is still listed after its folder was removed"
    [ -n "$(cp_row rules uqlive-rules)" ] || bad "removing the page took the rules off the list"
    rm -rf -- "${WORK:?}/prefix/integrations/uqlive-rules"
    expect_eq "'integrations list' after removing both" "no integrations installed" "$(cp_list)"
    # A page that fails to load is listed with the reason, not hidden.
    mkdir -p "$WORK/prefix/settings/pages/uqlive-page"
    printf 'uq_api = 1\nid = "uqlive-page"\nname = "x"\nqml = "../Page.qml"\n' >"$WORK/prefix/settings/pages/uqlive-page/page.toml"
    row=$(cp_json | jq -c '[.[] | select(.kind == "page")][0] // empty')
    note "a manifest with qml = \"../Page.qml\": $(jq -c '{valid, visible, error}' <<<"${row:-null}")"
    expect_eq "a broken page is valid" false "$(jq -r '.valid | tostring' <<<"${row:-null}")"
    [ -n "$(jq -r '.error // empty' <<<"${row:-null}")" ] || bad "a broken page is listed with no reason"
    r=$(cp_list | awk -F'\t' '$1 == "page" { print $3 }')
    expect_eq "a broken page's state" invalid "$r"
    cp_fresh
}

check SE2 "integrations check: the fixture passes, a missing program is found unless --no-programs, bad QML is refused" se2
se2() {
    local r rc
    r=$("$UQCTL" integrations check "$CP_FIX/good/uq" --no-programs 2>&1); rc=$?
    note "good, --no-programs: $r"
    expect_eq "exit status with --no-programs" 0 "$rc"
    [[ $r == "ok: 0 widget(s), 1 page(s), 0 provider(s), 0 app action(s), 0 service file(s), 1 rule file(s) (programs not checked)" ]] ||
        bad "expected one page and one rule file, programs not checked; observed $r"
    r=$("$UQCTL" integrations check "$CP_FIX/good/uq" 2>&1); rc=$?
    note "good, with programs: $(tr '\n' '|' <<<"$r")"
    expect_eq "exit status when a program isn't installed" 1 "$rc"
    [[ $r == *"/usr/bin/uqlive-not-installed does not exist"* ]] || bad "expected the missing program to be named; observed $r"
    r=$("$UQCTL" integrations check "$CP_FIX/bad/uq" --no-programs 2>&1); rc=$?
    note "bad QML: $(tr '\n' '|' <<<"$r" | cut -c1-300)"
    expect_eq "exit status for a page with a Text object" 1 "$rc"
    [[ $r == *"Page.qml:5:"* && $r == *UqLabel* ]] || bad "expected the refusal at Page.qml:5 pointing at UqLabel; observed $r"
    # Things that don't belong in a uq/ folder, and a folder named for another id.
    mkdir -p "$WORK/cp-odd/uq/pages"
    cp -r "$CP_FIX/good/uq/pages/uqlive-page" "$WORK/cp-odd/uq/pages/other-name"
    : >"$WORK/cp-odd/uq/stray.txt"
    r=$("$UQCTL" integrations check "$WORK/cp-odd/uq" --no-programs 2>&1); rc=$?
    note "odd folder: $(tr '\n' '|' <<<"$r" | cut -c1-300)"
    expect_eq "exit status for a stray file and a mismatched folder name" 1 "$rc"
    [[ $r == *"not part of a uq/ folder"* ]] || bad "expected the stray file to be refused; observed $r"
    r=$("$UQCTL" integrations check "$WORK/cp-nothere" --no-programs 2>&1); rc=$?
    [ "$rc" -ne 0 ] || bad "a folder that isn't there passed the check"
    rm -rf -- "${WORK:?}/cp-odd"
}

check SE3 "A page that waits for a service shows only while that service is on the bus" se3 restart
se3() {
    local r
    cp_fresh
    cp_install
    printf '\n[requires]\nbus_name = "org.uq.Desktop1"\n' >>"$WORK/prefix/settings/pages/uqlive-page/page.toml"
    note "uqd up: $(cp_row page uqlive-page | jq -c '{visible, error, waits_for}')"
    expect_eq "the page's state while its service is up" shown "$(cp_list | awk -F'\t' '$1 == "page" { print $3 }')"
    uqd_stop || { bad "uqd didn't stop"; uqd_start; cp_fresh; return; }
    r=$(cp_row page uqlive-page)
    note "uqd down: $(jq -c '{visible, error, waits_for}' <<<"${r:-null}")"
    expect_eq "the page's state while its service is gone" hidden "$(cp_list | awk -F'\t' '$1 == "page" { print $3 }')"
    [[ $(jq -r '.error // ""' <<<"${r:-null}") == "hidden until org.uq.Desktop1 is running" ]] ||
        bad "expected 'hidden until org.uq.Desktop1 is running'; observed $(jq -r '.error' <<<"${r:-null}")"
    uqd_start || bad "uqd didn't start again"
    expect_eq "the page's state once the service is back" shown "$(cp_list | awk -F'\t' '$1 == "page" { print $3 }')"
    cp_fresh
}

check SE4 "Window rules from apps, on the daemon's side: the list is well formed, and a rule that isn't installed can't be turned on" se4
se4() {
    local r s0
    r=$(bus ListIntegrationRules)
    note "ListIntegrationRules: $(jq -c . <<<"$r" | cut -c1-200)"
    expect_eq "the keys of ListIntegrationRules" '["missing","problems","rules"]' "$(jq -c 'keys' <<<"$r")"
    s0=$(section integration_rules)
    r=$(set_section integration_rules '{"enabled":["uqlive-rules.games"]}') && bad "a rule that isn't installed was turned on"
    note "turning one on: $r"
    [[ $r == *"uqlive-rules.games isn't installed"* ]] || bad "expected \"The rule uqlive-rules.games isn't installed.\"; observed $r"
    expect_eq "the integration_rules section after the refusal" "$s0" "$(section integration_rules)"
}
