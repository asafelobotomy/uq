# The uQ login screen

uQ ships its own login screen (the *greeter*) for [greetd](https://sr.ht/~kennylevinsen/greetd/),
and a **uQ** session entry that sets itself up on first login. This page covers how they work, how
to turn them on and off, fingerprint login, and what to do when something goes wrong.

## The uQ session

`pacman -S uq` installs `/usr/share/wayland-sessions/uq.desktop` ("uQ"), which runs
`/usr/bin/uq-session`. Any display manager that lists Wayland sessions shows it; uQ's greeter
lists it first and selects it.

`uq-session` runs as you:

1. If `~/.config/hypr/hyprland.lua` doesn't load uQ's config yet, it runs `uqctl setup --quiet`.
   Setup never overwrites a file of yours. If it reports a problem, the problem is written to
   `~/.local/state/uq/session.log` and the session starts anyway.
2. It starts Hyprland (through `start-hyprland` when that's installed).

So a new user gets the themed uQ desktop on their first login, without running `uqctl setup`.

## Turning the login screen on

The greeter needs `greetd` (it also provides `agreety`, the text fallback):

```bash
run0 pacman -S greetd
run0 uqctl greeter enable
```

`uqctl greeter enable` must run as root, through `run0` (not sudo). It refuses when greetd isn't
installed, tells you which display manager it replaces (for example `sddm.service`), records that
one in `/etc/uq/greeter-previous-dm`, disables it and enables `greetd.service`. The login screen
appears at the next boot.

`run0 uqctl greeter disable` switches back to the recorded display manager. If none was recorded,
it says so; enable one with `run0 systemctl enable <name>.service`.

## How it works

- **greetd's config.** uQ ships `/etc/uq/greetd.toml` and a drop-in,
  `/usr/lib/systemd/system/greetd.service.d/uq.conf`, that starts greetd with it. greetd's own
  `/etc/greetd/config.toml` is never edited. While `/etc/uq/greetd.toml` exists greetd uses it;
  delete it and greetd goes back to its stock config.
- **The greeter session.** greetd runs `/usr/lib/uq/uq-greeter-session` as the `greeter` user on
  VT 1. It starts Hyprland with `/usr/share/uq/greeter/hyprland.lua`: no key bindings (switching
  VTs with Ctrl+Alt+F1…F12 still works), no animations, and your system keyboard layout (the X11
  layout from `localectl status`, read once). Hyprland runs the greeter,
  `quickshell -p /usr/share/uq/greeter`, and exits when it does.
- **What it shows.** On the monitor with the cursor: the clock, the users, the password field,
  the session to start and Suspend, Restart and Shut down (each needs a second click). Other
  monitors show the wallpaper and the clock. Tab goes user → password → session → power; Enter
  logs in; Esc cancels.
- **What it reads.** The greeter never reads a home directory. It reads only:
  - `/etc/uq/greeter.toml`: theme, variant, `hide_users` and `reduce_motion` (Settings writes it);
  - `/usr/share/uq/themes/rendered/<theme>-<variant>/tokens.json`: every theme pre-rendered by the
    package (the default theme, dark, if the chosen one is missing);
  - `/var/lib/uq-greeter/wallpaper`: the login wallpaper (the theme's background colour if it's
    missing or unreadable);
  - the users AccountsService lists (system and locked accounts are hidden; pictures only from
    `/var/lib/AccountsService/icons/`), read with `busctl` over the system bus;
  - the `Name` and `Exec` of each `/usr/share/wayland-sessions/*.desktop`;
  - its own `/var/cache/uq-greeter/state.toml`: the last user and each user's last session.
- **Passwords and messages.** PAM runs inside greetd with `/etc/pam.d/greetd` (from the greetd
  package; uQ doesn't ship or edit it). Password fields are cleared after every attempt and
  nothing the greeter logs contains a password or a PAM message. A wrong password, a locked
  account ("The account is locked due to 3 failed logins.") and every other PAM message are shown
  as plain text. When your password has expired, the greeter asks for a new one twice and checks
  they match before PAM sees them.

## Fingerprint login

Fingerprint login is **off** at the greeter. greetd runs one PAM conversation at a time, so the
greeter can't wait for a password and a finger at once the way the lock screen does, and a
fingerprint login can't unlock a password-protected keyring.

If you want it anyway, add `pam_fprintd` to greetd's own PAM file, above its first
`auth include` line (Arch's file includes `system-local-login`):

```
# /etc/pam.d/greetd
auth       required     pam_securetty.so
auth       requisite    pam_nologin.so
auth       sufficient   pam_fprintd.so max-tries=1 timeout=10
auth       include      system-local-login
...
```

The greeter then shows fprintd's messages ("Place your finger on the reader", "Failed to match
fingerprint") as plain text, and falls through to the password prompt after the timeout. Leave
`timeout` short: until it passes, the password field waits. Expect your keyring to stay locked
after a fingerprint login. The lock screen's fingerprint unlock is separate and unaffected.

## Recovery

- **The greeter crashes or never appears.** `uq-greeter-session` gives Quickshell 30 seconds to
  show the greeter. When the greeter exits without starting a session, the script exits with an
  error and greetd starts it again. After 3 failures within 60 seconds it runs `agreety`, greetd's
  text login, on the same VT: log in there with your user name and password, and the uQ session
  starts as usual. Failures are counted in `/var/cache/uq-greeter/failures`; the graphical greeter
  is tried again once they are older than 60 seconds.
- **"The session ended straight away".** The session you chose exited within seconds. The
  greeter keeps you selected; pick another session, or log in on a text console (Ctrl+Alt+F2) and
  look at `~/.local/state/uq/session.log` and `journalctl --user -b`.
- **Back to the old display manager.** From a text console: `run0 uqctl greeter disable`. Or
  remove the drop-in's effect by deleting `/etc/uq/greetd.toml` (greetd then uses its stock
  config), or `run0 systemctl disable greetd && run0 systemctl enable <your-dm>`.
- **Nothing logs in at all.** Switch to another VT (Ctrl+Alt+F2), log in on the text console and
  run `run0 uqctl greeter disable`.
