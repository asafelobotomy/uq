#!/usr/bin/env bash
# uQ live checks in a QEMU/KVM VM.
#
#   tests/vm/run.sh [--keep] [--refresh] [--no-build] [--only PATTERN] [--boot-only]
#
#   --keep        leave the VM running at the end (prints how to reach it); default: QMP quit
#   --refresh     rebuild the cached provisioned image (pacman -Syu and the guest packages)
#   --no-build    reuse the packages already in $UQVM_HOME/pkg instead of building HEAD
#   --only PAT    run only the checks whose id matches the extended regex PAT (e.g. 'L(1|3)$|N2')
#   --boot-only   boot and start the session, run no checks (implies --keep)
#
# What it does: builds uq and uq-hyprland-pin from this checkout (scripts/build-local-pkg.sh,
# HEAD only), boots the official Arch cloud image (sha256 and signature checked) with a qcow2
# overlay per run, installs the packages over ssh, runs `uqctl setup` as uqtest, autologs uqtest
# in on tty1 into Hyprland, then runs every check in tests/vm/checks/ and prints a table. Exit 1
# if any check FAILs.
#
# State lives in $UQVM_HOME (default ~/.cache/uq-vm); each run gets runs/<timestamp>/ with its
# screenshots, logs and results.tsv. The host's display and session are never touched: the guest
# has its own virtio-gpu, and input and screenshots go through QMP. Needs /dev/kvm, qemu, xorriso,
# makepkg, cargo, python3, gpg, jq.
set -uo pipefail

UQVM_DIR=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$UQVM_DIR/../.." && pwd)
# shellcheck source=lib.sh
. "$UQVM_DIR/lib.sh"
# shellcheck source=image.sh
. "$UQVM_DIR/image.sh"

KEEP=0 BUILD=1 ONLY='' BOOT_ONLY=0
while [ $# -gt 0 ]; do
    case "$1" in
        --keep) KEEP=1 ;;
        --refresh) export UQVM_REFRESH=1 ;;
        --no-build) BUILD=0 ;;
        --only) ONLY=${2:?--only needs a pattern}; shift ;;
        --boot-only) BOOT_ONLY=1 KEEP=1 ;;
        -h|--help) sed -n '2,/^set -uo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
        *) die "unknown option $1" ;;
    esac
    shift
done

for t in qemu-system-x86_64 qemu-img xorriso makepkg cargo python3 gpg jq ssh ssh-keygen curl; do
    command -v "$t" >/dev/null || die "$t is not installed"
done
[ -r /dev/kvm ] && [ -w /dev/kvm ] || die "/dev/kvm is not usable"

RUN_DIR=$UQVM_HOME/runs/$(date +%Y%m%d-%H%M%S)
VM_DIR=$RUN_DIR
mkdir -p "$RUN_DIR"
RESULTS=$RUN_DIR/results.tsv
: >"$RESULTS"
export RUN_DIR VM_DIR RESULTS
log "run directory: $RUN_DIR"

on_exit() {
    local rc=$?
    if [ "$KEEP" = 1 ] && vm_running; then
        log "VM kept running (--keep). ssh: ssh -i $VM_KEY -p $VM_SSH_PORT -o StrictHostKeyChecking=no uqtest@127.0.0.1"
        log "QMP: python3 $UQVM_DIR/qmp.py $VM_QMP status|screendump F.png|key meta_l-l|quit"
        echo "VM_DIR=$VM_DIR VM_SSH_PORT=$VM_SSH_PORT VM_KEY=$VM_KEY VM_QMP=$VM_QMP VM_PID=$VM_PID" >"$UQVM_HOME/last-run.env"
    else
        vm_quit
    fi
    exit "$rc"
}
trap on_exit EXIT
trap 'exit 130' INT TERM

# --- per-run ssh key and cloud-init seed ----------------------------------------------------------
VM_KEY=$RUN_DIR/id_ed25519
ssh-keygen -q -t ed25519 -N '' -C "uq-vm-$(basename "$RUN_DIR")" -f "$VM_KEY"
sed -e "s|@SSH_KEY@|$(cat "$VM_KEY.pub")|" -e "s|@PASSWORD@|$UQVM_PASS|" \
    "$UQVM_DIR/user-data.in" >"$RUN_DIR/user-data"

# --- packages from this checkout ------------------------------------------------------------------
PKG_DIR=$UQVM_HOME/pkg
if [ "$BUILD" = 1 ]; then
    log "building uq-hyprland-pin and uq from HEAD ($(git -C "$ROOT" rev-parse --short HEAD))"
    rm -rf "$PKG_DIR"
    mkdir -p "$PKG_DIR" "$UQVM_HOME/build"
    # -d: the host is not the target (no Hyprland here); --nocheck: CI runs the tests.
    for p in uq-hyprland-pin uq; do
        TMPDIR=$UQVM_HOME/build PKGDEST=$PKG_DIR "$ROOT/scripts/build-local-pkg.sh" --package "$p" \
            -f -d --nocheck >"$RUN_DIR/build-$p.log" 2>&1 || die "building $p failed (see $RUN_DIR/build-$p.log)"
    done
fi
# noti-fi's `notify` for the noti-fi checks (built from NOTIFI_SRC; skipped if it isn't there).
NOTIFI_SRC=${NOTIFI_SRC:-$HOME/Projects/noti-fi}
NOTIFI_BIN=$UQVM_HOME/notifi-target/release/notify
if [ "$BUILD" = 1 ] && [ -f "$NOTIFI_SRC/Cargo.toml" ]; then
    log "building noti-fi's notify from $NOTIFI_SRC ($(git -C "$NOTIFI_SRC" rev-parse --short HEAD 2>/dev/null))"
    CARGO_TARGET_DIR=$UQVM_HOME/notifi-target cargo build --release --locked -p notify-cli \
        --manifest-path "$NOTIFI_SRC/Cargo.toml" >"$RUN_DIR/build-notifi.log" 2>&1 ||
        log "noti-fi build failed (see $RUN_DIR/build-notifi.log); the noti-fi checks will fail"
fi
export NOTIFI_BIN
mapfile -t PKGS < <(find "$PKG_DIR" -maxdepth 1 -name 'uq-*.pkg.tar.zst' ! -name '*-debug-*' | sort)
[ "${#PKGS[@]}" -eq 2 ] || die "expected the uq and uq-hyprland-pin packages in $PKG_DIR"

# --- images -----------------------------------------------------------------------------------------
fetch_base
ensure_provisioned "$BASE"
qemu-img create -q -f qcow2 -F qcow2 -b "$PROV" "$RUN_DIR/disk.qcow2" || die "qemu-img"
make_seed "$RUN_DIR/seed.iso" "$RUN_DIR/user-data" "run-$(basename "$RUN_DIR")"

# --- boot, install, start the session ---------------------------------------------------------------
UQVM_OUTPUTS=${UQVM_OUTPUTS:-1}
vm_boot "$RUN_DIR/disk.qcow2" "$RUN_DIR/seed.iso"
vm_wait_ssh 300
vg cloud-init status --wait >/dev/null 2>&1 || true
log "installing ${PKGS[*]##*/}"
vg mkdir -p /tmp/uqpkg
for p in "${PKGS[@]}"; do vput "$p" /tmp/uqpkg/ || die "scp $p"; done
vroot bash -c 'pacman -U --noconfirm /tmp/uqpkg/*.pkg.tar.zst' >"$RUN_DIR/install.log" 2>&1 ||
    die "pacman -U failed (see $RUN_DIR/install.log)"
vput "$UQVM_DIR/guest/uqvm-env" /tmp/uqvm-env
vput "$UQVM_DIR/guest/autologin.conf" /tmp/autologin.conf
vput "$UQVM_DIR/guest/bash_profile" .bash_profile
vroot install -Dm755 /tmp/uqvm-env /usr/local/bin/uqvm-env
vroot install -Dm644 /tmp/autologin.conf /etc/systemd/system/getty@tty1.service.d/autologin.conf
vg uqctl setup --json >"$RUN_DIR/uqctl-setup.json" 2>&1
log "uqctl setup: exit $? $(cat "$RUN_DIR/uqctl-setup.json")"
vg bash -c 'pacman -Q hyprland quickshell uq uq-hyprland-pin' | tee "$RUN_DIR/versions.txt" >&2
# Keep the journal across the run for the report.
vroot systemctl daemon-reload
# The test user's own Hyprland settings (uQ loads user.lua last): no update-news or donation
# dialogs, which would take focus from the checks.
printf '%s\n' 'hl.config({ ecosystem = { no_update_news = true, no_donation_nag = true } })' |
    vgin sh -c 'mkdir -p ~/.config/hypr/uq && cat >>~/.config/hypr/uq/user.lua'
vroot systemctl restart getty@tty1.service
log "waiting for Hyprland and the uQ bar"
session_up() { [ -n "$(vgh sh -c 'echo $HYPRLAND_INSTANCE_SIGNATURE')" ] && has_layer uq-bar; }
if ! wait_for 120 session_up; then
    shot boot-failed >/dev/null
    vgsh 'tail -n 40 ~/.hyprland-tty1.log; journalctl --user -n 60 --no-pager' >"$RUN_DIR/boot-failed.log" 2>&1
    die "no uq-bar layer after 120 s (see $RUN_DIR/boot-failed.log, $RUN_DIR/boot-failed.png)"
fi
log "session up: $(layer_names | sort | uniq -c | tr '\n' ' ')"
shot session-up >/dev/null

[ "$BOOT_ONLY" = 1 ] && exit 0

# --- checks -------------------------------------------------------------------------------------------
# shellcheck source=checks/common.sh
. "$UQVM_DIR/checks/common.sh"
for f in "$UQVM_DIR"/checks/[0-9]*.sh; do
    # shellcheck disable=SC1090
    . "$f"
done
guest_prepare
run_checks "$ONLY"
print_summary
