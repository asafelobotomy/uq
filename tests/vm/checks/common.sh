# shellcheck shell=bash
# uQ VM harness: the check framework and the helpers the checks share. Sourced by run.sh after
# lib.sh; each checks/NN-*.sh registers its checks with `check ID "title" function`.
#
# A check function calls:
#   note TEXT        a line of evidence (also: any output of the function lands in the evidence)
#   bad TEXT         a failed expectation ("expected ...; observed ..."); the check FAILs
#   na TEXT          the check cannot run in a VM (reason); the check is N/A
#   snap NAME        a screenshot, recorded as evidence
# A check with neither bad nor na PASSes. After every check, settle() puts the session back in a
# known state (unlocked, no surfaces open, idle stages off).

CHECK_IDS=()
declare -A CHECK_TITLE CHECK_FN
check() { CHECK_IDS+=("$1"); CHECK_TITLE[$1]=$2; CHECK_FN[$1]=$3; }

EVID=$RUN_DIR/evidence
mkdir -p "$EVID"
note() { printf '  %s\n' "$*" >>"$EVID/$CUR.txt"; }
bad() { CUR_FAIL+=("$*"); printf '  FAIL: %s\n' "$*" >>"$EVID/$CUR.txt"; log "$CUR: FAIL: $*"; }
na() { CUR_NA=$*; printf '  N/A: %s\n' "$*" >>"$EVID/$CUR.txt"; }
snap() { # snap NAME [HEAD]
    local f; f=$(shot "$CUR-$1" ${2:+"$2"}) && { CUR_SHOTS+=("$(basename "$f")"); note "screenshot: $f"; }
}
# ev CMD...: run a guest command (vgsh string) and keep its output as evidence.
ev() { { printf '  $ %s\n' "$1"; vgsh "$1" 2>&1 | sed 's/^/    /'; } >>"$EVID/$CUR.txt"; }

run_checks() {
    local pat=${1:-} id
    for id in "${CHECK_IDS[@]}"; do
        [ -n "$pat" ] && ! [[ $id =~ $pat ]] && continue
        CUR=$id CUR_FAIL=() CUR_NA="" CUR_SHOTS=()
        printf '== %s: %s\n' "$id" "${CHECK_TITLE[$id]}" >"$EVID/$id.txt"
        log "check $id: ${CHECK_TITLE[$id]}"
        if ! vm_running; then
            bad "the VM is not running"
        else
            "${CHECK_FN[$id]}" >>"$EVID/$id.txt" 2>&1
        fi
        local status reason
        if [ -n "$CUR_NA" ]; then status=N/A reason=$CUR_NA
        elif [ "${#CUR_FAIL[@]}" -gt 0 ]; then status=FAIL reason=$(IFS='|'; echo "${CUR_FAIL[*]}")
        else status=PASS reason=
        fi
        printf '%s\t%s\t%s\t%s\t%s\n' "$id" "$status" "${CHECK_TITLE[$id]}" "$reason" "${CUR_SHOTS[*]}" >>"$RESULTS"
        log "check $id: $status${reason:+ ($reason)}"
        vm_running && settle >>"$EVID/$id.txt" 2>&1
    done
}

print_summary() {
    local p f n
    p=$(awk -F'\t' '$2=="PASS"' "$RESULTS" | wc -l)
    f=$(awk -F'\t' '$2=="FAIL"' "$RESULTS" | wc -l)
    n=$(awk -F'\t' '$2=="N/A"' "$RESULTS" | wc -l)
    echo
    printf '%-5s %-5s %s\n' ID RESULT CHECK
    awk -F'\t' '{ printf "%-5s %-5s %s\n", $1, $2, $3; if ($4 != "") { gsub(/\|/, "\n               ", $4); printf "            %s\n", $4 } }' "$RESULTS"
    echo
    echo "PASS $p  FAIL $f  N/A $n   (evidence: $EVID, screenshots: $RUN_DIR)"
    [ "$f" -eq 0 ]
}

# --- session helpers -----------------------------------------------------------------------------
SHELL_QS=/usr/share/uq/shell
ipc() { vgh qs ipc -p "$SHELL_QS" call uq "$@" 2>/dev/null; }
is_locked() { [ "$(locked_hint 2>/dev/null)" = yes ]; }
is_unlocked() { [ "$(locked_hint 2>/dev/null)" = no ]; }
dpms_on() { [ "$(vgh hyprctl monitors -j 2>/dev/null | jq -r '.[0].dpmsStatus')" = true ]; }
dpms_off() { [ "$(vgh hyprctl monitors -j 2>/dev/null | jq -r '.[0].dpmsStatus')" = false ]; }
unit_prop() { vg systemctl --user show "$1" -p "$2" --value 2>/dev/null; }
gnow() { vg date +%s.%N; }
# Seconds since a gnow/hnow mark, one decimal.
since() { python3 -c "import sys,time; print(f'{time.time()-float(sys.argv[1]):.1f}')" "$1"; }
hnow() { date +%s.%N; }
# Real pointer motion (resets the idle timers), alternating so it always moves.
POKE_X=600
poke() { POKE_X=$((POKE_X == 600 ? 610 : 600)); qmp move "$POKE_X" 700 >/dev/null; }
# Journal lines of the user units since a guest epoch mark.
ujournal() { # ujournal SINCE_EPOCH [UNIT...]
    local s=$1 u=() x; shift
    for x in "$@"; do u+=(-u "$x"); done
    vg journalctl --user --no-pager -o cat --since "@${s%.*}" "${u[@]}" 2>/dev/null | grep -v '^MESA'
}

# hexec CMD: run CMD from Hyprland (so it is in the graphical logind session, as from a keybind).
hexec() {
    local f=/tmp/uqvm-exec-$RANDOM.sh
    printf '#!/bin/sh\n%s\n' "$1" | vgin tee "$f" >/dev/null
    vg chmod +x "$f"
    vgh hyprctl dispatch "hl.dsp.exec_cmd(\"$f\")" >/dev/null
}

# The lock surface is on screen (it can take seconds to load under software rendering; keys typed
# before it has focus are lost).
lock_visible() { local f=$RUN_DIR/.lockvis.png; qmp screendump "$f" >/dev/null 2>&1 && shows_lock "$f"; }
unlock_pw() { # type the password on the lock; true when LockedHint goes to no
    for _ in 1 2; do
        wait_for 20 lock_visible || true
        # Clear whatever is in the field first (keys pressed over the lock land there).
        qmp key ctrl-a backspace
        qmp type "$UQVM_PASS" && qmp key ret
        wait_for 10 is_unlocked && return 0
        is_locked || return 1
        sleep 3
    done
    return 1
}

# settle: back to a known state after a check (unlocked, nothing open, idle stages off).
settle() {
    if is_locked; then
        unlock_pw || { vroot faillock --user "$UQVM_USER" --reset; sleep 1; unlock_pw; } ||
            { vroot fp-virtual SCAN finger-1 >/dev/null 2>&1; wait_for 5 is_unlocked; } ||
            log "settle: could not unlock"
    fi
    dpms_on || { qmp key shift; wait_for 3 dpms_on; }
    local s
    for s in spotlight appmenu quicksettings powermenu; do ipc close "$s" >/dev/null; done
    qmp key esc >/dev/null
    # Headless outputs added by the noti-fi checks go, so every check starts on one monitor.
    local m
    for m in $(vgh hyprctl monitors -j 2>/dev/null | jq -r '.[].name' | grep '^HEADLESS-'); do
        vgh hyprctl output remove "$m" >/dev/null 2>&1
    done
    idle_off
    vroot faillock --user "$UQVM_USER" --reset >/dev/null 2>&1
    poke
}

idle_set() { vg uqctl idle set "$1" "$2"; }
# All stages off (any order works when every stage is off).
idle_off() {
    local s
    for s in suspend screen-off lock dim locked-screen-off; do idle_set "$s" off >/dev/null 2>&1; done
}

# PNG analysis on the host (ImageMagick). region_sd FILE WxH+X+Y: standard deviation of the
# region (0 = flat colour); region_mean: mean brightness 0..1; red_px: count of red-ish pixels.
have_magick() { command -v magick >/dev/null 2>&1; }
region_sd() { magick "$1" -crop "$2" -colorspace gray -format '%[fx:standard_deviation]' info: 2>/dev/null; }
region_mean() { magick "$1" -crop "$2" -colorspace gray -format '%[fx:mean]' info: 2>/dev/null; }
red_px() { magick "$1" -crop "$2" -fx '(r>0.6 && g<0.5 && b<0.5) ? 1 : 0' -format '%[fx:round(mean*w*h)]' info: 2>/dev/null; }
# The desktop has the bar; the lock screen, Hyprland's lock-died screen and a blank screen don't.
# shows_bar FILE: true when the top-left of FILE matches the bar in $RUN_DIR/ref-desktop.png
# (taken by guest_prepare while unlocked).
BAR_CROP=170x50+5+5
shows_bar() {
    have_magick && [ -f "$RUN_DIR/ref-desktop.png" ] || return 2
    local d
    d=$(magick compare -metric RMSE <(magick "$1" -crop "$BAR_CROP" +repage png:-) \
        <(magick "$RUN_DIR/ref-desktop.png" -crop "$BAR_CROP" +repage png:-) null: 2>&1 | sed -n 's/.*(\([0-9.e-]*\)).*/\1/p')
    lt "${d:-1}" 0.05
}
# shows_lock FILE: the uQ lock screen (its user label) is on screen in FILE.
shows_lock() { find_template lock-user 400x300+440+350 "$1" >/dev/null; }
lt() { awk -v a="$1" -v b="$2" 'BEGIN { exit !(a < b) }'; }

# guest_prepare: per-run guest setup the checks need (after the uq packages are in and the session
# is up). Fingerprint: libfprint's virtual_device driver through fprintd, with one finger enrolled.
guest_prepare() {
    local f
    for f in fp-virtual notifi-call; do
        vput "$UQVM_DIR/guest/$f" "/tmp/$f" && vroot install -m755 "/tmp/$f" "/usr/local/bin/$f"
    done
    vroot sh -c 'mkdir -p /etc/systemd/system/fprintd.service.d &&
        printf "[Service]\nEnvironment=FP_VIRTUAL_DEVICE=/run/fp-virtual.sock\n" >/etc/systemd/system/fprintd.service.d/uqvm-virtual.conf &&
        systemctl daemon-reload && systemctl restart fprintd'
    if vroot timeout 90 fp-virtual enroll finger-1 >"$RUN_DIR/fprint-enroll.log" 2>&1; then
        log "fingerprint: virtual finger enrolled"
    else
        log "fingerprint: enrolling on the virtual device failed (see $RUN_DIR/fprint-enroll.log)"
    fi
    if [ -n "${NOTIFI_BIN:-}" ] && [ -x "$NOTIFI_BIN" ]; then
        vput "$NOTIFI_BIN" /tmp/notify && vg install -Dm755 /tmp/notify .local/bin/notify &&
            log "noti-fi: notify copied to ~/.local/bin"
    fi
    poke
    sleep 1
    shot ref-desktop >/dev/null
    vroot sync
}

# find_template NAME WxH+X+Y [FILE]: "x y" (screen pixels) of the centre of templates/NAME.png
# inside that region of FILE (default: a fresh screenshot), when it matches closely.
find_template() {
    have_magick || return 1
    local t=$UQVM_DIR/templates/$1.png region=$2 f=${3:-} r score xy x y w h ox oy
    if [ -z "$f" ]; then
        f=$RUN_DIR/.find.png
        qmp screendump "$f" >/dev/null 2>&1 || return 1
    fi
    r=$(magick compare -metric RMSE -subimage-search <(magick "$f" -crop "$region" +repage png:-) "$t" null: 2>&1)
    score=${r#*(}; score=${score%%)*}
    xy=${r#*@ }; xy=${xy%% *}; x=${xy%,*}; y=${xy#*,}
    w=$(magick identify -format %w "$t"); h=$(magick identify -format %h "$t")
    ox=${region#*+}; oy=${ox#*+}; ox=${ox%+*}
    [ -n "$x" ] && [ -n "$y" ] && lt "${score:-1}" "${FIND_MAX_RMSE:-0.03}" || return 1
    echo "$((ox + x + w / 2)) $((oy + y + h / 2))"
}
# Labels in quick settings move with the content (an audio device adds rows), so rows are found
# by their label (a template), not by fixed positions. Text at another row can render with
# different subpixel offsets, so some labels need a looser match (measured: the Schedule label
# scores 0.07 at another row and nothing else in the panel scores under 0.09 against it).
find_in_panel() {
    local thr=0.03
    case "$1" in schedule) thr=0.085 ;; esac
    FIND_MAX_RMSE=${FIND_MAX_RMSE:-$thr} find_template "$1" 400x760+880+40
}
# qs_toggle NAME: click the switch on the quick-settings row labelled by templates/NAME.png
# (scrolling the panel down to find it). Quick settings must be open.
QS_SWITCH_X=1230
qs_toggle() {
    local pos
    for _ in 0 1 2 3; do
        if pos=$(find_in_panel "$1"); then
            note "quick settings: '$1' row at ${pos#* } (y)"
            qmp click "$QS_SWITCH_X" "${pos#* }"
            return 0
        fi
        qmp scroll 1078 400 5
        sleep 0.5
    done
    note "quick settings: no '$1' row found"
    return 1
}
