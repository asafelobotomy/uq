# Settings slice C (window and workspace rules): manual live check

Run on a real Hyprland session (0.56.2 or newer, up to 0.57.x) with uQ installed. Not CI, not a
private bus. Tick each item. The app is `uq-settings` (`uqctl settings rules`). Have Pavucontrol,
a terminal, Firefox and mpv installed, and ideally two monitors. Before you start, note
`user.lua`'s modification time: `stat -c %y ~/.config/hypr/uq/user.lua`. It must be the same at
the end.

Where a step says "check Hyprland's errors", run `hyprctl configerrors`. Empty output means no
errors.

- [ ] **1. Float a window that was already open.** Open Pavucontrol. In the app, go to **Window
  rules** and press **Pick a window**. The app hides itself and a dark overlay covers every
  screen. Click Pavucontrol. The app comes back with a new rule open: its App ID is filled in,
  and **Matches now** lists Pavucontrol. Tick **Float** and save. The page says "Close and reopen
  Pavucontrol to apply." Close Pavucontrol and open it again: it floats.
- [ ] **2. Dynamic effects apply at once.** Open a terminal. Add a rule for it (use Pick a window)
  with only **See-through 80 %**. The terminal turns see-through right away, without closing it,
  and the page shows no "Close and reopen" message.
- [ ] **3. Why does this window…?** Make three rules: two that match your terminal (one by App
  ID, one by its title) and one that doesn't (some other App ID). Press **Why does this
  window…?** and click the terminal. It lists the two matching rules, in list order, and not the
  third. The page says rules written in `user.lua` aren't listed.
- [ ] **4. `user.lua` override.** In the editor, open "Advanced details" on one of the two rules
  and note its id (it looks like `r-7f2c`). Add this line to `~/.config/hypr/uq/user.lua`, using
  that id, and reload (`hyprctl reload`):
  ```lua
  hl.window_rule({ name = "uq:r-7f2c", enabled = false })
  ```
  Click away from the app and back. The rule's row and its row in the Why list say "Overridden in
  user.lua, line N", and **Open user.lua** opens the file. Put `--` at the start of that line,
  reload, click away and back: the flag is gone. `user.lua`'s modification time is the same as
  before you started, after every step of this checklist (uQ never writes it).
- [ ] **5. Pick mode edges.** In each case the app must come back.
  - Press **Pick a window**, then Esc: the page says you cancelled.
  - Press it and wait 20 seconds without clicking: "Took too long".
  - Run `systemctl --user stop uq-shell`, press it: within about 2 seconds the page says "The
    desktop shell didn't answer". Start it again with `systemctl --user start uq-shell`.
  - Lock the screen while a pick is on: no overlay shows over the lock screen, and after
    unlocking the app is back.
  - Run `systemctl --user stop uqd`, press **Pick a window**: the app does not stay hidden and
    shows a message. Start uqd again afterwards.
  - Click on empty desktop (no window): the page says no window was there.
- [ ] **6. The overlay across screens.** With two or more monitors, press **Pick a window**:
  every screen is covered. Move the mouse to the second screen and click a window there. It is
  picked, and Esc also works with the mouse on any screen (keyboard focus lands on the overlay on
  whichever screen you are using). While the pick is on, the settings window is hidden and
  reappears afterwards on the same workspace, focused. Note it if it comes back minimised, on
  another workspace or without focus.
- [ ] **7. Templates.** Each of the five builds, saves and works:
  - "Float an app's dialogs": a dialog of the app (for example its Open file box) floats, centred;
  - "Open an app on a workspace": the app opens on workspace 3 and, with "Quietly" ticked, you
    are not taken there;
  - "No blur for an app": the app is not blurred;
  - "Picture-in-picture floats and stays on top": Firefox's picture-in-picture window floats and
    stays on top of other windows;
  - "Keep the screen awake while an app is fullscreen": play a video in mpv fullscreen and
    confirm the screen doesn't dim or lock (`systemd-inhibit --list` shows an idle block).
- [ ] **8. Panel and workspace rules.**
  - Add a panel rule "Blur notifications" for the App ID `^noti-fi$` (see the shell's panel
    names under Advanced details): trigger a notification. Its popup is blurred.
  - Add a workspace rule for workspace 3: on your second monitor, stays when empty, gaps 0.
    Workspace 3 moves to that monitor and is still there when empty, with no gaps.
  - Run `uqctl layout set 3 master`. The rule is still on the page, and
    `hyprctl workspacerules` shows one merged rule for 3 (monitor, persistent, gaps and layout).
  - Make a window rule that matches workspace `name:web` and have a window open on a workspace
    named `web`: **Matches now** lists it.
- [ ] **9. Hyprland names (the unconfirmed ones).** These were read from Hyprland's source but not
  run. Note any failure in `crates/uq-core/src/rules/hypr.rs` and in the PR.
  - Import a test state with one rule of every effect and one panel rule of every effect (the CI
    fixture), reload, and check Hyprland's errors. They must be empty.
  - **Panel animations:** make a panel rule with each animation (`fade`, `slide`, `popin`) in
    turn. Check Hyprland's errors each time. Each should be accepted, and the panel should
    animate differently.
  - **Tags:** add a rule with the tag effect "mytag" to a terminal. `hyprctl clients` lists
    "mytag" in that window's tags. If Hyprland's errors complain, or the tag toggles on and off
    on each reload, the `+name` form is wrong.
  - **Workspace gaps:** a workspace rule with gaps in set to 0 takes effect (gaps between
    windows on that workspace vanish) and Hyprland's errors are empty. A non-zero number such
    as 12 works too.
  - **Reload:** run `hyprctl reload` a few times. Every rule still works afterwards, and
    nothing doubles up (a see-through rule doesn't get more see-through).
- [ ] **10. History, undo and restore.** Open **History**: the rule you added reads "Window rule
  “…” added". Press **Undo** on it: the rule is gone (undo puts back the whole window-rules list
  as it was). Restore the restore point from yesterday (or one you saved at the start): the rules
  list matches it.
- [ ] **11. Refusals in plain words.** Each of these is refused with one plain line (no word
  "regex", no pasted error) and nothing changes:
  - the patterns `negative:foo`, `[a&&b]`, `(?x)a` and `a{1001}`;
  - a 201st window rule (add them with `uqctl` or the app until you hit the limit);
  - a rule with both Float and Tile.
- [ ] **12. Profiles (rules on and off).** Make a float rule and a panel rule. On **Profiles**,
  create a profile and use **Add settings**: the two rules' switches are listed by rule name.
  Set the float rule's switch to off in the profile. Turn the profile on:
  - the float rule's row says "From the <name> profile" and is off;
  - **History** has a "Window rule “…” turned off" row from the profile;
  - reopen Pavucontrol: it no longer floats.

  Now flip the panel rule's switch by hand and press **Back to normal**: the float rule is on
  again and the message names the panel rule as kept. Export the profile
  (`uqctl profile export <id> /tmp/r.toml`), change one rule id in the file, import it as a copy
  (change its `id` and `name` too, or use `--replace` on a copy), and turn it on: it applies and
  says "1 rule in this profile isn't on this computer, so it was left out." A profile with a
  `[keybinds]` table is still refused with "…which this version of uQ can't do yet."
