# Settings slice E (pages from other apps): manual live check

Run on a real uQ session with each app installed from its repo (`makepkg -si` in each). Not CI.
Tick each item. Before you start, note `user.lua`'s modification time:
`stat ~/.config/hypr/uq/user.lua`. It must be the same at the end.

- [ ] **1. Discovery.** Install scdr's package while Settings is open. **Scheduling** appears
  under Apps within a few seconds. Remove the package and it goes. Same for crumBS, noti-fi,
  XZram and scale_tail. `uqctl integrations list` shows each page as visible.
- [ ] **2. Search.** Settings search for "boost", "cache", "do not disturb" and "swap" each opens
  the right page with the row highlighted.
- [ ] **3. Scheduling (scdr).**
  - Pause asks first. After you confirm, a password prompt appears from uQ's agent. After the
    password the page says paused, and `scdrctl status` agrees. Resume.
  - Turn Games off: the prompt appears and the switch stays off. Cancel the password prompt
    instead: the switch returns on with "You need to enter your password...".
  - Add a rule for `xeyes` with profile `background`, start `xeyes`, and `scdrctl explain xeyes`
    names the rule. Remove it (confirm).
  - Pick a scheduler, if `scx-scheds` is installed.
  - Turn the focused-window agent off, then log out and in: it stays off. Turn it on.
  - `sudo systemctl stop scdrd`: within 5 s the page says scdr isn't running. **Start scdr**
    starts it after the password.
- [ ] **4. Storage and cleanup (crumBS).**
  - Open the page: one scan runs (`pgrep -a crumbs` shows one).
  - Clean up **Thumbnails**: the confirm says "go to the trash". After it,
    `crumbs history --limit 1` shows the batch, and **Put back** restores it.
  - Clean up **Trash** says "for good".
  - **Journal** prompts for the password. Cancel it: the page says "Couldn't get administrator
    rights", with the commands under Advanced details.
  - Uninstall a running app: refused, with crumBS's message.
  - Uninstalling `uq` is not offered ("Part of uQ").
  - An appOFholding AppImage never appears.
- [ ] **5. Notifications (noti-fi).**
  - Do not disturb on: the bar island shows the mark within 5 s. Turn it off again.
  - Mute a group: `notify-send` into that group shows nothing.
  - Set history to 50: noti-fi's own panel shows 50.
  - **Clear history...** asks first.
  - Stop notifd: the page says noti-fi isn't running.
- [ ] **6. Plain text.** Add an scdr rule named `<b>bold</b>`. It's refused by the pattern (an
  inline reason) and nothing is sent.
- [ ] **7. Dev session.** In `uq-dev session`, the pages act on the real services. Note anything
  surprising.
- [ ] **8. Nothing in user.lua.** `stat ~/.config/hypr/uq/user.lua` is unchanged after every step.
- [ ] **9. Memory and swap (XZram).** Change the zram size: it asks first and prompts for the
  password, then the page shows the new size. Change the swappiness: it applies.
- [ ] **10. Tailscale (scale_tail).** Connect, disconnect, pick an exit node and turn it off,
  turn accept routes on and off. Sign out asks first. With no operator set, the page shows the
  one-time `sudo tailscale set --operator=$USER` line. When signed out, the sign-in link can be
  selected and copied.
