# Using and developing uQ on one machine

`uq-dev` (installed with uQ; `scripts/uq-dev` in a checkout) lets you use the installed uQ
apps every day and work on their checkouts at the same time. It never changes the running
desktop unless you `promote` a build. bindle's "uQ developer" module clones the checkouts into
`~/Projects` and installs the build tools on a fresh install.

Why three commands: each piece of uQ can run once per app, once per desktop or once per machine.

| Kind | Examples | How to work on it |
|---|---|---|
| Plain app | crumbs, romforge, scale_tail, appOFholding, oysterAV, uq-settings | `uq-dev run <repo>` beside the installed copy |
| One per desktop | uqd (`org.uq.Desktop1`), the shell and lock, notifd (`org.freedesktop.Notifications`), the scdr agent, haulerd | `uq-dev session`, a nested desktop |
| One per machine | scdrd (`org.scdr.Daemon1`), xzramd (`io.github.XZram1`), bindle | `uq-dev promote`, and `uq-dev rollback` |

## `uq-dev run <repo> [--real-config] [-- cmd...]`

Runs `cargo run` (or `uv run <repo>`, or the command after `--`) in `~/Projects/<repo>` with
`XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `XDG_CACHE_HOME` under `~/.local/share/uq-dev/run/<repo>`,
so a dev build's config changes never reach the installed copy. `--real-config` uses your own.
A single-instance app (uq-settings, hauler-desktop, emobie) only raises the installed window
while that is running; quit it first, or use the dev session. Apps that change the system
(crumbs uninstalling, oysterAV's firewall) still do it for real.

## `uq-dev session [--no-build]`

Builds `uqd` and `uqctl` in `~/Projects/uq` and opens Hyprland as a window inside your desktop,
running the checkout's `base.lua`, `uqd` and `shell/` (and `~/Projects/noti-fi/notifd` when that
checkout exists). Shell edits reload live. Inside it:

- a private D-Bus session bus (`dbus-run-session`): every bus name is free;
- a private runtime dir that shares only PipeWire, PulseAudio and the systemd user manager (so
  launched apps work and have sound), not the real bus, `notifd.sock` or other app sockets;
- `XDG_CONFIG_HOME` and `XDG_STATE_HOME` under `~/.local/share/uq-dev/session`, so theme
  files, `generated.lua` and gsettings stay there. Apps you launch inside also get fresh config;
- `UQ_DEV_SESSION=1`, which makes `base.lua`'s start and shutdown hooks do nothing, and a
  `systemctl` shim that refuses to restart, stop or start `uqd`, `uq-shell`, `uq-lock` or
  `scdr-session` in the real user manager;
- no autostart apps: the runner starts nothing and the Startup page says "Not started in a test
  session". `XDG_DATA_HOME` isn't sandboxed here, so uqd neither writes nor deletes launcher
  overrides, and the "Also when started from other launchers" switch is refused;
- the real system bus: scdrd, logind, NetworkManager, UPower are the machine's own.

Logs are in `~/.local/share/uq-dev/session/log`. Your real desktop sees SUPER combinations
before the nested one does, so open the dev shell's panels with
`qs ipc -p ~/Projects/uq/shell call uq open spotlight` (or `appmenu`, `quicksettings`,
`powermenu`) from a terminal inside the session. The lock screen is not run in the session.

## `uq-dev promote <repo> [makepkg args]` and `uq-dev rollback <package>`

`promote` builds the checkout's HEAD (not uncommitted changes) with the repo's own PKGBUILD and
installs it (`makepkg -si`; pass other makepkg arguments to change that). Release PKGBUILDs (uq,
noti-fi, crumbs, emobie, oysterAV) get HEAD packed the way the release tarball is, in place of
the download; local PKGBUILDs (scdr, appofholding, hauler, XZram) already build the checkout.
`pkgrel` becomes `<pkgrel>.<commit count>`, so a normal update keeps the promoted build until the
AUR ships a newer pkgrel or version. Built packages are kept in `~/.local/share/uq-dev/pkg` and
listed in `~/.local/share/uq-dev/promoted`.

`rollback <package>` reinstalls the newest build of that package that did not come from
`promote`, from pacman's cache or paru's or yay's build folders, and otherwise reinstalls it from
the AUR. `uq-dev status` lists the promoted packages still installed.

For quick daemon iteration without packaging, stop the installed unit and run the dev binary in
a terminal, for example `sudo systemctl stop scdrd; sudo target/debug/scdrd --state-dir
/tmp/scdr-dev`, then `sudo systemctl start scdrd`. Test bindle only in its VM (`tests/vm`).
