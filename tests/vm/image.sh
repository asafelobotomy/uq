# shellcheck shell=bash
# shellcheck disable=SC2034 # BASE and PROV are results read by run.sh
# uQ VM harness: images. Sourced by run.sh after lib.sh.
#
#   $UQVM_HOME/download/     the Arch cloud image, its .SHA256 and .sig (untrusted until verified)
#   $UQVM_HOME/images/       provisioned-<key>.qcow2: the cloud image plus Hyprland, Quickshell and
#                            uQ's optdepends (cached; rebuilt when the package list changes or with
#                            --refresh)
#   $UQVM_HOME/runs/<ts>/    one run: its overlay disk, seed, key, QMP socket, logs, screenshots

ARCH_IMG_BASE=https://geo.mirror.pkgbuild.com/images/latest
# arch-boxes' CI release key (its README), pinned by primary fingerprint.
ARCH_BOXES_FPR=1B9A16984A4E8CB448712D2AE0B78BF4326C6F8F

# Bump to rebuild the cached provisioned image after changing the provisioning steps below.
PROVISION_REV=2

# Everything uQ's checks need besides the uq packages (whose deps pacman resolves on -U).
GUEST_PACKAGES=(
    hyprland quickshell qt6-declarative qt6ct kitty foot
    wireplumber pipewire pipewire-pulse networkmanager bluez bluez-utils
    power-profiles-daemon upower brightnessctl playerctl fprintd libfprint hyprlock
    libnotify wtype grim jq polkit mesa ttf-dejavu inter-font ttf-jetbrains-mono
    papirus-icon-theme noto-fonts-emoji mpv mpv-mpris xdg-desktop-portal-hyprland wf-recorder
    hostapd iw python
)

# fetch_base: download (once) and verify the newest cloud image; sets BASE to its path.
fetch_base() {
    local dl=$UQVM_HOME/download name f gnupghome
    mkdir -p "$dl"
    name=$(curl -fsSL "$ARCH_IMG_BASE/" | grep -o 'Arch-Linux-x86_64-cloudimg-[0-9.]*\.qcow2"' | tr -d '"' | sort -V | tail -n1)
    [ -n "$name" ] || die "could not find the cloud image on $ARCH_IMG_BASE"
    f=$dl/$name
    if [ ! -f "$f.verified" ]; then
        log "downloading $name"
        local x
        for x in .SHA256 .sig; do curl -fsSL -o "$f$x" "$ARCH_IMG_BASE/$name$x" || die "download $name$x"; done
        if [ ! -f "$f" ]; then
            curl -fsSL -o "$f.part" "$ARCH_IMG_BASE/$name" || die "download $name"
            mv "$f.part" "$f"
        fi
        (cd "$dl" && sha256sum -c "$name.SHA256" >&2) || { rm -f "$f"; die "sha256 mismatch for $name"; }
        gnupghome=$(mktemp -d)
        gpg --homedir "$gnupghome" --batch --quiet --import "$UQVM_DIR/arch-boxes.asc" 2>/dev/null
        if ! gpg --homedir "$gnupghome" --batch --status-fd 1 --verify "$f.sig" "$f" 2>/dev/null |
            grep -q "^\[GNUPG:\] VALIDSIG .* $ARCH_BOXES_FPR\$"; then
            rm -rf "$gnupghome"
            die "bad or missing arch-boxes signature on $name"
        fi
        rm -rf "$gnupghome"
        touch "$f.verified"
        log "verified $name (sha256 and arch-boxes signature)"
    fi
    BASE=$f
}

# ensure_provisioned BASE: sets PROV to the cached provisioned image, building it if needed.
# (Not called in $(...): the VM it boots must stay visible to run.sh's exit trap.)
ensure_provisioned() {
    local base=$1 key img
    key=$(printf '%s\n' "$(basename "$base")" "${GUEST_PACKAGES[@]}" "$PROVISION_REV" | sha256sum | cut -c1-12)
    img=$UQVM_HOME/images/provisioned-$key.qcow2
    mkdir -p "$UQVM_HOME/images"
    if [ -f "$img" ] && [ "${UQVM_REFRESH:-0}" != 1 ]; then
        PROV=$img
        return 0
    fi
    log "provisioning $img (pacman -Syu plus ${#GUEST_PACKAGES[@]} packages; a few minutes)"
    rm -f "$img"
    qemu-img create -q -f qcow2 -F qcow2 -b "$base" "$img.part" 24G || die "qemu-img"
    make_seed "$VM_DIR/seed-provision.iso" "$VM_DIR/user-data" "provision-$key-$(date +%s)"
    vm_boot "$img.part" "$VM_DIR/seed-provision.iso"
    vm_wait_ssh 300
    vg cloud-init status --wait >/dev/null 2>&1 || true
    local pkgs="${GUEST_PACKAGES[*]}"
    # shellcheck disable=SC2016
    vroot bash -c "set -e
        systemctl is-active -q pacman-init.service 2>/dev/null || true
        pacman-key --init >/dev/null 2>&1; pacman-key --populate archlinux >/dev/null 2>&1
        pacman -Syu --noconfirm --needed $pkgs >/var/log/uqvm-provision.log 2>&1 ||
            { tail -n 30 /var/log/uqvm-provision.log; exit 1; }
        # The cloud image's systemd-networkd keeps the wired link (and ssh); NetworkManager runs
        # for the shell's network status but leaves wired devices alone.
        printf '[keyfile]\\nunmanaged-devices=type:ethernet\\n' >/etc/NetworkManager/conf.d/uqvm.conf
        systemctl enable NetworkManager bluetooth power-profiles-daemon >/dev/null 2>&1 || true
        pacman -Scc --noconfirm >/dev/null 2>&1 || true
        pacman -Q hyprland quickshell" >&2 || { vm_quit; die "provisioning failed"; }
    vroot systemctl poweroff >/dev/null 2>&1 || true
    local i
    for ((i = 0; i < 120; i++)); do vm_running || break; sleep 1; done
    vm_quit
    mv "$img.part" "$img"
    log "provisioned image ready"
    PROV=$img
}
