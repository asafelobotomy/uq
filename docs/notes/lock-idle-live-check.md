# Lock, idle and power: first-run checklist

The lock, idle and power code (Phase 2 slice 5) is tested only with qmllint, pure JS tests,
offscreen PAM runs and a private D-Bus daemon. Some of it has never run for real: the logind
sleep inhibitor fd, Hyprland's `LockState` from `hyprland-lock-notify-v1`, and every window. Run
this once on a real uQ Hyprland session. Fill in each result with pass or fail and a note. A
failure that leaves the session unlocked is a security bug: stop and fix it before anything else.

Before you start: keep a TTY login handy (`CTRL+ALT+F3`) in case the lock won't come back, enroll
a finger (`fprintd-enroll`), and have a laptop on battery for the suspend check. Watch
`journalctl --user -fu uq-shell -u uq-lock` in a second session; no password or PAM message may
ever appear in it.

| # | Check | How | Expected | Result |
|---|-------|-----|----------|--------|
| 1 | Lock and unlock with a password | `SUPER+L`, type the password, Enter | The lock covers every monitor; the field is cleared after each try; a wrong password shows PAM's message; the right one unlocks. About 250 ms after unlocking the lock process exits 0 and stays stopped (`systemctl --user status uq-lock` shows it inactive, not failed or restarting). | |
| 2 | Unlock with a fingerprint | `SUPER+L`, touch the reader | It unlocks without the password. A wrong finger shows a message and keeps polling. | |
| 3 | LockState from Hyprland | `SUPER+L` with Spotlight and quick settings open, then try `SUPER+Space` and `qs ipc -p /usr/share/uq/shell call uq open spotlight` from the TTY | The shell sees locked within a second (spotlight, app menu, quick settings, the centre and the power menu close and won't open); `loginctl show-session` shows `LockedHint=yes`, and `no` after unlocking. | |
| 4 | `kill -9` the lock | Lock, then from the TTY `pkill -9 -f /usr/share/uq/lock` | The session stays locked (Hyprland's red screen at worst) and the lock is back within about 1 s. | |
| 5 | `kill -9` the shell | Lock, then from the TTY `pkill -9 -f /usr/share/uq/shell` | The session stays locked; the shell restarts; after unlocking, the bar and idle stages work. | |
| 6 | Suspend from the power menu | Unlocked, `SUPER+Escape`, Suspend, then wake | The lock is already up when the screen comes back; nothing of the desktop flashes. | |
| 7 | Suspend from logind | Unlocked, `systemctl suspend` from a terminal, then wake | As 6. Before suspending, `systemd-inhibit --list` shows uQ's "Lock the screen before sleep" delay lock; after waking it shows a fresh one. | |
| 8 | Sleep fallback | Unlocked, `systemctl --user mask --runtime uq-lock`, `systemctl suspend`, wake | The machine still suspends within about 2 s (the inhibitor's fallback). On waking, Hyprland keeps the session locked with no lock client; unmask and start `uq-lock` from the TTY (or press `SUPER+CTRL+L`). | |
| 9 | Dim | `uqctl idle set dim 10`, wait 10 s | A 60% dark overlay on every screen. The first click or key press removes it and also reaches the app under it. | |
| 10 | Lock on idle | `uqctl idle set lock 20`, wait | Locks after 20 s. | |
| 11 | Screens off | `uqctl idle set screen-off 30`, wait | Screens turn off after 30 s; input turns them back on. | |
| 12 | Screens off after locking | `uqctl idle set locked-screen-off 15`, lock with `SUPER+L` straight after some input | Screens go off 15 s after the lock, counted from locking, not from your last input before it. | |
| 13 | Suspend on battery | Unplug, `uqctl idle set suspend 60`, wait | It suspends only after the lock is up and secure. Plugged in, it never suspends. With `uqctl idle set lock off`, it doesn't suspend either. | |
| 14 | Suspend counts from last input | With 13's settings, keep using the machine past 60 s, then stop | It suspends 60 s after your last input, not 60 s after the dim or lock. | |
| 15 | Media playing | Play a video, `uqctl idle set dim 10`, `lock 20`, `screen-off 30`, wait | No dim and no screen-off; it still locks at 20 s. With `idle.media_blocks_lock` on, it doesn't lock either. | |
| 16 | Stay awake | Turn on Stay awake in quick settings, wait past every stage | Nothing happens. | |
| 17 | Restore the defaults | `uqctl idle set dim 240`, `lock 300`, `screen-off 360`, `suspend 1200`, `locked-screen-off 30` | Done. | |
| 18 | Power menu countdown | `SUPER+Escape`, Restart | It counts down from 30 s. Esc, Cancel, a click outside and `SUPER+L` each abort it (try each); nothing restarts later. Let it reach 0 once: the machine restarts. | |
| 19 | App menu restart | Open the app menu, click restart (and shut down) | The power menu opens at that action's countdown. | |
| 20 | Polkit prompt | Stop any other polkit agent (e.g. KDE's `polkit-kde-authentication-agent-1`), restart the shell, run `pkexec true` | A dialog on the focused monitor above every window, with the message and action id as plain text; a wrong password shows an error; the right one succeeds; Esc or Cancel makes `pkexec` fail. Lock while it's open: it hides, and it's back after unlocking. | |
| 21 | Faillock lockout | Enter a wrong password on the lock until faillock locks the account (3 tries by default) | The lock shows the faillock message ("The account is locked due to ...") as plain text and doesn't unlock until the lockout ends. The fingerprint still works if enrolled. | |

Notes:
