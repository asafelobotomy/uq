# noti-fi on Hyprland: first-run checklist

noti-fi's Hyprland mode hasn't run on a real Hyprland session yet (spec §2.6, open item). Run
this once on a uQ session with `notif.backend` set to `auto` or `notifi`. Fill in each result
with pass or fail and a note. Report every failure to noti-fi (its issue tracker), with the
noti-fi version, the Hyprland version and the step.

Before you start: stop any other notification daemon (swaync, mako, dunst) and log in to
Hyprland with at least two monitors connected.

| # | Check | How | Expected | Result | On failure |
|---|-------|-----|----------|--------|------------|
| 1 | Takeover | `notify takeover --plain` | It installs and starts noti-fi's plain user service; `busctl --user status org.freedesktop.Notifications` names notifd. | | Report to noti-fi |
| 2 | Hello and subscribe | Restart the shell (`systemctl --user restart uq-shell.service`) | Within 5 s the island shows noti-fi's count, and quick settings shows Do Not Disturb; `journalctl --user -u uq-shell` has no "noti-fi is not running" warning. | | Report to noti-fi |
| 3 | Popups on the focused monitor | Add `"screen": "focused"` to noti-fi's `rules.json`, focus the second monitor, run `notify-send test hello` | The popup appears on the focused monitor. | | Report to noti-fi |
| 4 | OSD on the focused monitor | Focus each monitor in turn and press a volume key | noti-fi's OSD appears on the focused monitor each time. | | Report to noti-fi |
| 5 | Unplugging a monitor | Show a popup and the OSD on the second monitor, then unplug it | Every noti-fi window moves to a remaining monitor; nothing is lost or stuck. | | Report to noti-fi |
| 6 | Centre from `SUPER+N` | Press `SUPER+N`, then again; also left-click the island | noti-fi's list opens on the first press and closes on the second. | | Report to noti-fi |
| 7 | Theme tokens | `uqctl theme set rosewood light`, then `uqctl theme set dracula dark` | Within about 2 s noti-fi's popups and list switch colours with each theme. | | Report to noti-fi |
| 8 | Sharing hides summaries | Start a screen share (e.g. a browser's "share screen"), then `notify-send secret body` | Popups show only the app name and icon; the island shows no summary even on hover. | | Report to noti-fi (an island summary is a uQ bug) |
| 9 | Nothing above the lock | Lock (`loginctl lock-session`), then `notify-send locked hello` and press a volume key | Nothing from noti-fi shows over the lock screen. | | Report to noti-fi |

Notes:
