# shellcheck shell=bash
# uQ VM harness: the driver library. Sourced by tests/vm/run.sh and the check files.
#
# Everything here acts on the QEMU guest only. Nothing touches the host's display, session,
# /dev/dri or XDG_RUNTIME_DIR: the guest has its own virtio-gpu, and input and screenshots go
# through QMP.
#
#   vm_boot DISK SEED          start QEMU (KVM, q35 with S3, virtio-gpu, no display, QMP socket)
#   vm_wait_ssh [SECS]         wait until sshd answers
#   vg CMD...                   run CMD (argv, quoted) in the guest as uqtest over ssh
#   vgsh 'SCRIPT'               run a bash script string in the guest as uqtest
#   vgh CMD...                  as g, with the Hyprland session's env (HYPRLAND_INSTANCE_SIGNATURE,
#                              WAYLAND_DISPLAY, XDG_RUNTIME_DIR, DBUS_SESSION_BUS_ADDRESS)
#   vroot CMD...               as root (sudo inside the guest)
#   vput SRC DEST              copy a host file into the guest
#   qmp OP ARGS...             tests/vm/qmp.py against this VM (key, type, click, screendump, ...)
#   shot NAME                  host-side screenshot (QMP screendump) -> $RUN_DIR/NAME.png
#   layers / has_layer NS      hyprctl layers -j, and whether a layer namespace is mapped
#   vm_suspend / vm_wakeup     S3 in the guest, QMP system_wakeup from the host
#   vm_quit                    QMP quit, then make sure QEMU is gone

UQVM_HOME=${UQVM_HOME:-$HOME/.cache/uq-vm}
UQVM_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
UQVM_MEM=${UQVM_MEM:-4096}
UQVM_CPUS=${UQVM_CPUS:-4}
UQVM_USER=uqtest
# The throwaway VM user's password: made once per machine and kept outside the repo (secret
# scanners flag a committed one). Every run's cloud-init seed sets it, so a new one is harmless.
uqvm_password() {
    local f="$UQVM_HOME/test-password"
    if [ ! -s "$f" ]; then
        mkdir -p "$UQVM_HOME"
        (umask 077; printf 'uqtest-%s\n' "$(od -An -N8 -tx1 /dev/urandom | tr -d ' \n')" >"$f")
    fi
    cat "$f"
}
# shellcheck disable=SC2034 # used by the checks
UQVM_PASS=$(uqvm_password)

log() { printf '[uq-vm %(%H:%M:%S)T] %s\n' -1 "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }

free_port() {
    python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])'
}

# vm_boot DISK SEED_ISO: q35 with S3 enabled. noreboot=on keeps the ICH9 TCO watchdog from
# resetting the guest ~50 s after an S3 resume (iTCO_wdt is not re-armed across it). QEMU runs daemonized; its pid, QMP socket, serial log and ssh port are
# in $VM_DIR (set by the caller, a per-run directory).
vm_boot() {
    local disk=$1 seed=$2
    VM_SSH_PORT=$(free_port)
    VM_QMP=$VM_DIR/qmp.sock
    VM_PID=$VM_DIR/qemu.pid
    rm -f "$VM_QMP" "$VM_PID"
    qemu-system-x86_64 \
        -name uq-vm -nodefaults -machine q35,accel=kvm,vmport=off -cpu host -smp "$UQVM_CPUS" -m "$UQVM_MEM" \
        -global ICH9-LPC.disable_s3=0 -global ICH9-LPC.noreboot=on \
        -drive file="$disk",if=virtio,format=qcow2,discard=unmap \
        -drive file="$seed",if=virtio,format=raw,readonly=on \
        -nic user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:"$VM_SSH_PORT"-:22 \
        -device "$(vm_gpu)" ${UQVM_GPU2:+-device "$UQVM_GPU2"} \
        -device virtio-keyboard-pci -device virtio-tablet-pci \
        -audiodev none,id=snd0 -device ich9-intel-hda -device hda-duplex,audiodev=snd0 \
        -display none -vga none \
        -serial file:"$VM_DIR/serial.log" \
        -qmp unix:"$VM_QMP",server=on,wait=off \
        -pidfile "$VM_PID" -daemonize \
        || die "qemu failed to start"
    log "qemu up (pid $(cat "$VM_PID"), ssh 127.0.0.1:$VM_SSH_PORT, serial $VM_DIR/serial.log)"
}

# The guest GPU: virtio-gpu (2D, no GL) when this QEMU has it (Arch: qemu-hw-display-virtio-gpu),
# else bochs-display (a plain KMS framebuffer; qemu-base has it built in). Either way the guest
# renders with mesa's llvmpipe and QMP screendump sees the scanout. UQVM_GPU overrides; UQVM_GPU2
# adds a second display device (e.g. bochs-display,xres=1280,yres=800) for two-output checks.
vm_gpu() {
    if [ -n "${UQVM_GPU:-}" ]; then echo "$UQVM_GPU"
    elif qemu-system-x86_64 -device help 2>/dev/null | grep -q '"virtio-gpu-pci"'; then
        echo "virtio-gpu-pci,xres=1280,yres=800"
    else echo "bochs-display,xres=1280,yres=800"
    fi
}

vm_running() { [ -f "${VM_PID:-/nonexistent}" ] && kill -0 "$(cat "$VM_PID")" 2>/dev/null; }

SSH_OPTS=(-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR
    -o ConnectTimeout=5 -o ServerAliveInterval=5 -o ServerAliveCountMax=6 -o BatchMode=yes)

# ssh joins its arguments into one string for the remote shell: quote each one so vg runs exactly
# argv. vgsh runs one string as a bash script instead (pipes, $vars expanded in the guest).
# vg never reads the caller's stdin (-n); vgin passes it through.
vg() { ssh -n "${SSH_OPTS[@]}" -i "$VM_KEY" -p "$VM_SSH_PORT" "$UQVM_USER@127.0.0.1" -- "$(printf '%q ' "$@")"; }
vgin() { ssh "${SSH_OPTS[@]}" -i "$VM_KEY" -p "$VM_SSH_PORT" "$UQVM_USER@127.0.0.1" -- "$(printf '%q ' "$@")"; }
vgsh() { vg bash -c "$1"; }
vghsh() { vgh bash -c "$1"; }
vroot() { vg sudo -n "$@"; }
# The guest's /usr/local/bin/uqvm-env (installed by run.sh) exports the Hyprland session env.
vgh() { vg /usr/local/bin/uqvm-env "$@"; }
vput() { scp "${SSH_OPTS[@]}" -i "$VM_KEY" -P "$VM_SSH_PORT" -q "$1" "$UQVM_USER@127.0.0.1:$2"; }

vm_wait_ssh() {
    local t=${1:-300} i
    for ((i = 0; i < t; i += 2)); do
        vm_running || die "qemu exited while waiting for ssh (see $VM_DIR/serial.log)"
        vg true 2>/dev/null && return 0
        sleep 2
    done
    die "no ssh after ${t}s"
}

qmp() { python3 -I "$UQVM_DIR/qmp.py" "$VM_QMP" "$@"; }

# shot NAME [HEAD]: PNG of what the guest's GPU scans out (the lock screen too).
shot() {
    local f=$RUN_DIR/${1}.png
    qmp screendump "$f" ${2:+"$2"} >/dev/null 2>&1 && echo "$f"
}

layers() { vgh hyprctl layers -j 2>/dev/null; }
# Namespaces of every mapped layer, one per line.
layer_names() { layers | jq -r '.. | objects | select(has("namespace")) | .namespace' 2>/dev/null; }
has_layer() { layer_names | grep -qx "$1"; }
# wait_for SECS CMD...: poll CMD until it succeeds or SECS (wall clock) have passed.
wait_for() {
    local end; end=$(($(date +%s%N) + ${1%.*} * 1000000000)); shift
    while :; do
        "$@" && return 0
        [ "$(date +%s%N)" -lt "$end" ] || return 1
        sleep 0.3
    done
}
wait_layer() { wait_for "$1" has_layer "$2"; }
wait_no_layer() { wait_for "$1" not_layer "$2"; }
not_layer() { ! has_layer "$1"; }

# The graphical (tty1) session's id, and its LockedHint.
gsession() { vgsh 'loginctl list-sessions --no-legend | awk '"'"'$0 ~ /seat0/ {print $1; exit}'"'"; }
locked_hint() { vg loginctl show-session "$(gsession)" -p LockedHint --value; }
lock_active() { vg systemctl --user is-active uq-lock.service 2>/dev/null; }

vm_status() { qmp status 2>/dev/null; }

vm_suspend() { # vm_suspend [how]: how = "systemctl" (default) or a command already sent
    [ "${1:-systemctl}" = systemctl ] && { vroot systemctl suspend >/dev/null 2>&1 & }
    wait_for 30 vm_is_suspended
}
vm_is_suspended() { [ "$(vm_status)" = suspended ]; }
vm_wakeup() {
    qmp wakeup
    wait_for 30 vm_not_suspended && vm_wait_ssh 60
}
vm_not_suspended() { [ "$(vm_status)" = running ]; }

vm_quit() {
    vm_running || return 0
    log "shutting the VM down (QMP quit)"
    qmp quit >/dev/null 2>&1 || true
    local pid i
    pid=$(cat "$VM_PID" 2>/dev/null) || return 0
    for ((i = 0; i < 20; i++)); do kill -0 "$pid" 2>/dev/null || return 0; sleep 0.5; done
    kill "$pid" 2>/dev/null
    sleep 1
    kill -9 "$pid" 2>/dev/null || true
}

# Seed image for cloud-init's NoCloud datasource (an ISO labelled CIDATA).
make_seed() { # make_seed OUT USER_DATA_FILE INSTANCE_ID
    local out=$1 ud=$2 iid=$3 d
    d=$(mktemp -d "$VM_DIR/seed.XXXXXX")
    cp "$ud" "$d/user-data"
    printf 'instance-id: %s\nlocal-hostname: uq-vm\n' "$iid" >"$d/meta-data"
    xorriso -as mkisofs -quiet -o "$out" -V CIDATA -J -r "$d" 2>/dev/null || die "xorriso failed"
    rm -rf "$d"
}
