# Fresh install: the desktop plan

What to do on the real machine from the moment it is wiped, through bindle and the first uQ login,
to every check that needs real hardware, and then how the rest of the coding lands on it. Work
through it top to bottom; each step says who does it.

| Tag | Meaning |
|---|---|
| **[You]** | Needs your hands or eyes: a password, a reboot, a cable, a look at the screen, a feel. |
| **[Claude]** | Claude can do it alone through a Remote Control (RC) session on the machine: commands, logs, `hyprctl`, `busctl`, `grim` screenshots it reads back, git and PRs. |
| **[Both]** | Claude runs the commands and reads the output; you confirm what the screen shows or do the physical part. |

An RC session lives inside your login session, so a log out, a reboot or a suspend ends it. Steps
that need one say so; start a new RC session from the project thread when you are back.

Results go in the matching `docs/notes/*-live-check.md` file (the Result column or the tick box).
Claude fills them in from what you report and opens one PR per checklist.

## 0. Before you wipe

Nothing here can be undone once the disk is gone, so check each item.

1. **[You]** Back up what isn't in git: `~/.ssh`, `~/.gnupg`, browser profiles, `~/Documents` and
   anything else you keep, and `~/.claude/settings.json` (it was rewritten on 2026-11-10 and you
   want that version back).
2. **[You]** Make sure Proton Pass holds the AUR SSH key and that you can log in to `pass-cli` from
   another device; after the reinstall it is the only copy you need.
3. **[Claude]** Check every repo in `~/Projects` has no unpushed branches or stashes
   (`git status`, `git stash list`, `git log --branches --not --remotes`) and list any that do.
   bindle's developer module re-clones the twelve uQ repos, but not the others (CirrusUX, kyko,
   noveleditor and so on) and not local-only work.
4. **[You]** Note the machine's hardware so the checks can be planned: the GPU, every panel you can
   connect (size, resolution, refresh, VRR), a fingerprint reader, a battery, a touchpad, Bluetooth
   devices. The laptop alone is one 1920x1080 panel with no VRR; the multi-monitor, 4K and VRR
   checks need an external monitor.
5. **[Claude]** Get a uQ release onto the AUR that has the Settings app and `uq-dev`. The AUR has
   0.2.1, which has neither, so bindle would install a uQ without any of slices 1–3 or A–D. Cut
   `v0.3.0-rc.1` (a pre-release; the slice checklists are what it is waiting for) following
   [releasing.md](releasing.md). The tag and the AUR push need your go-ahead, and the push needs
   `pass-cli login` on whatever machine does it. If you'd rather not release yet, the install still
   works: step 3.4 builds uQ from the checkout instead.
6. **[You]** Write the Arch ISO to a USB stick.

## 1. Install day

1. **[You]** Boot the ISO, connect (`iwctl station wlan0 connect "<network>"` for Wi-Fi), and run
   bindle's installer:

   ```bash
   bash <(curl -fsSL https://raw.githubusercontent.com/asafelobotomy/bindle/main/install.sh)
   ```

2. **[You]** On bindle's pre-install screen pick **uQ** and the **developer** profile.
3. **[You]** In archinstall, choose **Btrfs** for the root filesystem (with the default
   subvolumes). That lets bindle's `snapper` module snapshot `/` before and after every pacman
   transaction, which is the rollback the rest of this plan leans on. Set disk, user, hostname,
   locale and timezone; leave the rest as bindle pre-filled it. Reboot.
4. **[You]** Log in through SDDM to the Hyprland session. Bindle opens with the developer profile
   selected and uQ, scdr, crumBS and noti-fi ticked. Also tick:
   - **snapper** (and **Btrfs scrub**);
   - **uQ developer setup** (clones the twelve repos into `~/Projects`, installs the build tools);
   - **appOFholding** and **scale_tail** (with Tailscale) if you use them;
   - one scheduler module at most: `scdr-sched-ext` or `scdr-devbox`, not both (they conflict).
5. **[You]** Review bindle's plan screen and apply. Note the snapshot id it prints; `bindle undo`
   reverts the whole apply.
6. **[You]** Log out and back in. You should land in a themed uQ desktop.

If something fails here, `bindle undo --list` and `bindle undo` revert the apply, and `CTRL+ALT+F3`
gives a TTY. Bindle's own logs are in `/var/log/bindle`.

## 2. First boot: get Claude onto the machine

1. **[You]** Open a terminal (`SUPER+Return`) and:
   - `pass-cli login` (needed after every reboot before an AUR push);
   - `gh auth login`;
   - install Claude Code the way you did before, sign in, and copy your saved
     `~/.claude/settings.json` back into place;
   - connect this machine to the project: approve `~/Projects` for folder access and start a
     Remote Control session in `~/Projects/uq` from the project thread.
2. **[You]** Cap build parallelism before anything compiles. earlyoom has killed big parallel Rust
   builds and restarted the machine, so add to `~/.config/environment.d/90-build.conf`:

   ```
   CARGO_BUILD_JOBS=4
   MAKEFLAGS=-j4
   ```

   (half your cores is a safe start), then log out and in.

## 3. Check the install

1. **[Claude]** Inventory: `pacman -Q uq uq-hyprland-pin hyprland quickshell noti-fi scdr
   crum-bs-bin appofholding`, `uqctl --version`, `hyprctl version`, `systemctl --user --failed`,
   `systemctl --failed`, `journalctl --user -b -p warning -u uqd -u uq-shell`. Every unit
   active, no failed unit, Hyprland inside the pin (0.56.2 to 0.57.x).
2. **[Claude]** noti-fi owns notifications: `busctl --user status org.freedesktop.Notifications`
   names notifd, and `notify-send hello` shows a popup. **[You]** confirm you saw it.
3. **[Claude]** scdr runs: `systemctl status scdrd`, `systemctl --user status scdr-session`,
   `scdrctl status`.
4. **[Claude]** If step 0.5 didn't happen (the AUR still has 0.2.1): run
   `~/Projects/uq/scripts/uq-dev promote uq`. That builds `main` with the release PKGBUILD and
   installs it with pacman, so it needs your sudo password: **[You]** type it when asked. Then
   **[You]** log out and in.
5. **[Claude]** `ls ~/Projects` has the twelve repos; `uq-dev status` runs.
6. **[You]** Restore point zero. Save a baseline before you test anything:
   - **[Claude]** `uqctl restore save fresh-install`;
   - **[Claude]** `sudo snapper -c root create -d "uQ fresh install"` (password: **[You]**);
   - **[Claude]** copy `~/.config/hypr/uq/user.lua` and `~/.local/state/uq/` to
     `~/uq-baseline/`.

## 4. The live checklists

Run them in this order: the lock first, because a failure there is a security bug and stops
everything else. Keep a TTY ready (`CTRL+ALT+F3`) and, for the lock, run
`journalctl --user -fu uq-shell -u uq-lock _TRANSPORT=stdout` in a second terminal that Claude
reads. Most rows have already passed in the VM ([vm-live-check-results.md](notes/vm-live-check-results.md));
on the desktop the point is real hardware, real timing and real eyes, so the rows the VM couldn't
do are marked below.

### 4.1 Lock, idle and power ([lock-idle-live-check.md](notes/lock-idle-live-check.md))

| Rows | Who | Notes |
|---|---|---|
| 1, 3, 22, 23 | [Both] | You type the password; Claude checks `LockedHint`, `uq-lock`'s exit status and the journal. |
| 2 | [You] | Real reader: `fprintd-enroll` first. The VM used a virtual reader. |
| 4, 5, 24 | [Both] | Claude runs the `pkill -9` from a TTY or ssh; you watch that the desktop never shows. |
| 6, 7, 8 | [You] | Suspend ends the RC session; Claude reads the journal afterwards. Real S3/s2idle is the point. |
| 9–12, 15, 16, 17 | [Both] | Claude sets the timings and times them; you stay off the keyboard and say what you see. |
| 13, 14 | [You] | On battery. Not possible in the VM (L13b, L14 were N/A). |
| 18, 19 | [Both] | Let row 18 reach 0 once: the machine restarts and the RC session ends. |
| 20, 21 | [Both] | Real polkit and faillock. |

### 4.2 noti-fi on Hyprland ([notifi-hyprland-check.md](notes/notifi-hyprland-check.md))

[Both] for all nine rows. Rows 3–5 need a second monitor (row 5 is a real unplug). Row 8 needs a
real screen share. Row 7 (theme tokens) is the known open failure N7; record it again and report
it to noti-fi.

### 4.3 Quick settings (the VM's Q rows)

- **Q3 Bluetooth** [You]: pair and connect a real device from quick settings. N/A in the VM.
- **Q4 scroll jitter** [You]: scroll the quick-settings lists with a real touchpad and mouse wheel.
- Q1, Q2, Q5, Q6 [Both]: repeat on real Wi-Fi.

### 4.4 Settings slices 1–3

- [settings-slice1-live-check.md](notes/settings-slice1-live-check.md): theme, fonts, cursor,
  Kvantum, wallpaper, single instance, broken plugin page. [Both]. The 4K wallpaper row needs a
  4K output.
- [settings-slice2-live-check.md](notes/settings-slice2-live-check.md): Look, layouts, Shell.
  [Both]; "the change is visible" rows are yours.
- [settings-slice3-live-check.md](notes/settings-slice3-live-check.md): **Displays on a real
  panel.** The VM passed 18 of 18; what's left is hardware only:
  - **Blackout** [You]: a mode the panel truly can't show, hands off, back within 15 s;
  - **VRR** [You]: on an adaptive-sync monitor, turn VRR on and confirm it is really on
    (`hyprctl monitors` shows `vrr: true`; the monitor's OSD shows a varying rate in a game or
    `vrrtest`), then off;
  - **4K and scale** [Both]: 1.25 and 1.5 on the 1080p panel and on a 4K one; note any snapped
    scale (Hyprland only takes scales that divide the mode evenly);
  - **Multi-monitor** [Both]: arrangement by dragging, mirror, one of two off, Escape and Enter in
    the live dialog;
  - **Reboot** rows [You]: reboot during the window, and after Keep;
  - **Input** [You]: the 250 ms repeat feel, real touchpad tap and natural scroll, per-device speed.

### 4.5 Settings slices A–D

All four are written for a real session and have never been run. Do each in full.

- [A, trust](notes/settings-slice-a-live-check.md): [Both]. Claude edits `user.lua`, runs
  `hyprctl` and `uqctl history`; you confirm the page text. Item 6 (new version) happens for real
  on the first uQ package update after install.
- [B, profiles](notes/settings-slice-b-live-check.md): [Both]. Item 4 masks
  `power-profiles-daemon` (sudo: you). Item 6 needs `SUPER+ALT+P` pressed: you.
- [C, window rules](notes/settings-slice-c-live-check.md): [Both]. Pick mode is clicks: you click,
  Claude reads the result. Install Pavucontrol, Firefox and mpv first. Item 6 needs two monitors.
  Item 9 (Hyprland names) Claude can do alone and should do first, because a wrong name there is a
  code fix.
- [D, startup and apps](notes/settings-slice-d-live-check.md): [Both]. Items 1, 3 and 11 need a
  log out and in. Items 6–8 are "is it sharp": yours, ideally at 125 %. Item 15 needs a Flatpak
  Chromium-based browser.

## 5. The first real `uq-dev session`

This has never run on real Hyprland. [docs/developing.md](developing.md) has the details.

1. **[Claude]** `uq-dev session` from a terminal in the real desktop. It builds `uqd` and `uqctl`
   (capped by step 2.2) and opens a nested Hyprland window.
2. **[Both]** Inside it, check what the doc promises:
   - a private bus: `busctl --user list | grep org.uq` inside shows the dev uqd, outside still the
     installed one;
   - a theme change inside (`uqctl theme set rosewood light`) doesn't touch the real desktop, and
     `~/.config/hypr/uq` is unchanged (`stat` before and after);
   - `systemctl --user restart uqd` inside is refused by the shim;
   - an edit to a file in `~/Projects/uq/shell` reloads live;
   - `qs ipc -p ~/Projects/uq/shell call uq open spotlight` opens the dev Spotlight;
   - an app launched inside has sound;
   - slice D item 14: no autostart, the Startup page's notice, the refused launcher switch, the
     real `~/.local/share/applications` untouched.
3. **[Claude]** Close it and check the logs in `~/.local/share/uq-dev/session/log`.
4. **[Both]** `uq-dev run crumbs -- crumbs apps` and `uq-dev run uq` for uq-settings (quit the
   installed one first): the dev copy's config lands under `~/.local/share/uq-dev/run/`.
5. **[Both]** Promote and roll back once, on something low-risk: `uq-dev promote noti-fi`,
   `uq-dev status`, check notifications still work, then `uq-dev rollback noti-fi`. You type sudo.
   Then the same for `scdr`, which is a system daemon: `systemctl status scdrd` after each.

Anything that fails here is a bug in `scripts/uq-dev`; Claude fixes it in a PR before relying on
it for section 7.

## 6. Companion apps

| App | What to check | Who |
|---|---|---|
| noti-fi | Covered by 4.2. Also `notify restore` hands notifications back to uQ, and `notify takeover --plain --now` takes them again. | [Both] |
| scdr | `scdrctl status` lists the focused app. The Hyprland focus path has only been tested against Hyprland's documented events, never on Hyprland: focus three windows in turn and check `scdrctl status` follows each within a second; lock the screen and check nothing counts as focused. Start a `cargo build` and check it is demoted while a browser stays responsive. If a scheduler module is on, `scxctl get` shows the expected one per power profile. | [Both] |
| crumBS | `crumbs apps` lists uQ's packages as "Part of: the uQ desktop"; `crumbs uninstall uq --dry-run` asks twice. `crumbs scan` and a `--dry-run` clean. It leaves appOFholding's AppImages alone. | [Claude] |
| appOFholding | Add an AppImage; it shows in Spotlight; an update check runs; crumBS doesn't list it as removable. | [Both] |
| scale_tail | The tray icon appears in the Status island of uQ's bar, connect and disconnect, switch an exit node. | [You] |
| XZram, hauler, emobie, oysterAV, romforge | Only if bindle installed them: each starts, and its window follows the uQ theme where it claims to. | [Both] |

Report a failure in the app's own repo; anything uQ draws (bar, tray, theme) is a uQ issue.

## 7. Finishing the code, and how each piece reaches the desktop

The remaining work, in the order the specs set it:

1. Settings slice E, companion pages: Scheduling (scdr), Storage and cleanup (crumBS),
   Notifications (noti-fi), Memory and swap (XZram), Tailscale (scale_tail), with the plugin-page
   SDK and `command` helper pulled forward.
2. Settings slice F: monitor profiles, multi-monitor extras, touchpad gestures.
3. Settings slice G: theme export, import and custom themes.
4. Phase 3 slice 4: keybinds in uqd with an editor (`keybinds` and `rules` are reserved profile
   domains; add `uq.profile.next` unbound).
5. Phase 3 slices 5 and 6: Sound, Bluetooth, Network; Power with `uq-settings-helper`, Users,
   Date and time, Region, Default apps, About. Then tag `v0.3.0`.
6. Phase 4: integration foundation (`v0.4.0`), then one slice per repo.
7. Phase 5: session entry, `uq-greeter`, Login screen page, Welcome, bindle hand-off
   (`v0.5.0`).
8. Hardening, release candidates and the daily-driver period, then `v1.0.0`.

Every slice goes through the same loop, and only the last two steps need the desktop:

| Step | Where | Who |
|---|---|---|
| Plan, code, unit, QML and shell tests, review | Claude in the cloud | [Claude] |
| CI green, PR merged | GitHub | [Claude] (merge on green is pre-approved) |
| VM run of the slice's automated checks (`tests/vm/run.sh --only …`), extended with new `checks/` scripts for the slice | On the desktop (needs `/dev/kvm`), started over RC | [Claude] |
| Try it nested: `uq-dev session` on `main` | Desktop | [Both] |
| Promote, then the slice's live checklist on the real session | Desktop | [Both] |
| Release tag and AUR push | Desktop (needs `pass-cli login`) | [Both]: your go-ahead, Claude does the rest |

For the VM runs on the desktop, install the harness's needs once: `qemu-base` (or `qemu-full`
for virtio-gpu), `xorriso`, `jq`, `imagemagick`, plus what the developer module already brought.
The harness keeps everything in `~/.cache/uq-vm`; a full run takes about 35 minutes and uses 4 GB
and 4 vCPUs, so don't run it next to a big build.

## 8. Rollback and restore points

From the smallest step back to the biggest:

| What went wrong | Go back with |
|---|---|
| A setting | Ctrl+Z in uq-settings, `uqctl undo`, or Undo on the History page. |
| Several settings, or a test day | `uqctl restore list`, then `uqctl restore <id>` (the `fresh-install` point from 3.6 is always there). |
| A promoted dev build | `uq-dev rollback <package>`. |
| `user.lua` or uQ's state | Copy back from `~/uq-baseline/`. |
| A bad package update | `sudo snapper list`, then roll back to the pre-transaction snapshot (snap-pac takes one before each pacman run), or downgrade from `/var/cache/pacman/pkg`. |
| bindle's apply | `bindle undo --list`, `bindle undo <id>`. |
| The session won't come up | `CTRL+ALT+F3`, log in, `journalctl --user -b -u uqd -u uq-shell`; the lock's recovery bind is `SUPER+CTRL+L`. Comment out the `require` line in `~/.config/hypr/hyprland.lua` to start plain Hyprland. |
| The system won't boot | Boot an older snapper snapshot from the boot menu (if set up), or the Arch ISO and `arch-chroot`. |

Take a new restore point and snapper snapshot before each checklist that changes a lot of state
(slices C and D, every promote of `uq` itself, every release candidate).

## 9. Done means

- Every row in `docs/notes/` has a result from the real machine, with failures fixed or filed.
- `uq-dev session`, `run`, `promote` and `rollback` each worked at least once.
- Each companion app in section 6 passed or has an issue in its repo.
- `v0.3.0` is tagged and on the AUR, and the plan in section 7 is running slice by slice.
