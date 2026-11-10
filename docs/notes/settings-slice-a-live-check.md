# Settings slice A (trust): manual live check

Run on a real Hyprland session with uQ installed (not CI, not a private bus). Tick each item.
`uqctl` here means the installed one; the app is `uq-settings` (`uqctl settings`).

- [ ] **1. Overridden in user.lua.** Put `general.gaps_in = 2` (or the equivalent in your own
  config style) in `~/.config/hypr/uq/user.lua`. Open **Look**. The gaps row says "Overridden in
  user.lua, line N" (N is the line you wrote it on). Remove the line and run `hyprctl reload`.
  Switch focus away from the app and back: the warning is gone.
- [ ] **2. Changed while running.** Run `hyprctl keyword general:gaps_out 30`. The gaps row shows
  "Changed while running (not by uQ)". Press **Re-apply**: the value goes back to the one in the
  page and the note disappears.
- [ ] **3. Undo and redo.** Change three settings (on any pages), then press Ctrl+Z three times.
  Each one reverts live, newest first. Press Ctrl+Shift+Z: they come back, oldest undone first.
- [ ] **4. Changes from the terminal.** With the app open, run `uqctl theme set rosewood light`.
  Open "Changed this session": it lists the theme change as "from the terminal". Ctrl+Z in the
  app does not undo it. `uqctl history` shows it with origin `uqctl`, and `uqctl undo` reverts it.
- [ ] **5. Restore points.** On the **History** page, save a restore point with a name. Change
  settings on several pages, then press **Restore** on that point and confirm. Everything comes
  back. The message says how to go back. A new point appears at the top of the restore points,
  marked "Saved automatically before a restore". Press **Restore** on that one and confirm: the
  changes from every page return in one step. (Ctrl+Z does not undo a restore, and **Undo** on
  one of the restore's rows under Changes reverts only that page's part.)
  The same from the terminal: `uqctl restore save mine`, `uqctl restore list`,
  `uqctl restore <id>`, then `uqctl restore list` again and `uqctl restore <id>` of the newest
  `auto:before_restore` point.
- [ ] **6. New version.** After a package update that bumps the uQ version (or by hand: stop
  `uqd`, remove the `*.toml` whose kind is `new_version` from `~/.local/state/uq/restore/`, then
  start `uqd` again), a restore point named after the version (for example `0.2.2`) appears on
  the History page as "Saved automatically when uQ was updated". `uqctl restore list` shows it
  as `auto:new_version`. Restarting `uqd` again on the same version adds no second one.
