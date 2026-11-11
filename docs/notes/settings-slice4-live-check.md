# Settings slice 4 (keyboard shortcuts): manual live check

Run on a real Hyprland session, once on 0.56.2 and once on 0.57.x, with uQ installed. Not CI, not
a private bus. Tick each item. The app is `uq-settings` (`uqctl settings keybinds`). Before you
start, note `user.lua`'s modification time: `stat -c %y ~/.config/hypr/uq/user.lua`. It must be the
same at the end.

Where a step says "check Hyprland's errors", run `hyprctl configerrors`. Empty output means none.

- [ ] **1. Upgrade keeps every shortcut.** Install the package over the previous one while logged
  in. Every default still works before and after `systemctl --user restart uqd`, and
  `hyprctl binds -j | grep -c '"uq:'` is 54. Move `generated.lua` aside and run `hyprctl reload`:
  the fallback shortcuts still work. Put it back.
- [ ] **2. Rebind.** Change Open Spotlight to Super + K. Super + K opens it at once, and
  Super + Space does nothing. **Reset** brings Super + Space back. History shows both lines
  plainly.
- [ ] **3. Conflict.** Add a shortcut on Super + Q. The editor names "Close the window" before
  Save, and Save stays disabled. `busctl --user call org.uq.Desktop1 ... SetSection` with the same
  JSON is refused with both names.
- [ ] **4. Your own shortcuts.** Add Super + Shift + P, Start an app (Firefox): it starts in its
  own scope, with its Apps and scaling fix applied. Add Ctrl + Alt + T, Run a command (`foot`):
  `systemctl --user status 'run-*.scope'` shows it under `app.slice`. A command holding `'` and
  `$(...)` runs literally.
- [ ] **5. Key capture.** Press **Change** on any shortcut and press Super + Space. The editor
  shows "Super + Space" and Spotlight doesn't open (R11). Wait 15 s without pressing anything:
  shortcuts work again. Record whether the empty submap passed the keys on to the app on 0.56.2
  and on 0.57.x. Stop uqd's compositor link (for example `hyprctl` unavailable) mid-capture: no
  shortcut stays paused afterwards, and `systemctl --user restart uqd` always leaves the normal
  submap.
- [ ] **6. Lock screen.** Lock with Super + L. The volume keys work over the lock (OSD shows), and
  Super + Space doesn't. The recovery shortcut's switch is disabled with its reason. Kill the lock
  client: Super + Ctrl + L brings it back.
- [ ] **7. Switch to the next profile.** Give it Super + Alt + P. With two profiles, each press
  switches profile.
- [ ] **8. `user.lua`.** Add `hl.unbind("SUPER + Q")` to `user.lua` and reload. The Close row says
  "Overridden in user.lua, line N". Add
  `hl.bind("SUPER + T", hl.dsp.exec_cmd("foot")) -- uq:bind Terminal (foot)`: it's listed
  read-only, and a uQ shortcut on Super + T warns but saves.
- [ ] **9. Profiles.** Make a profile that turns off Close the window. Apply it: Super + Q does
  nothing, and the row says "From the ... profile". Back to normal brings it back. Import a
  hand-edited profile with `"uq.spotlight" = "SUPER + K"` or `[keybinds."k-1234"]` and `command`:
  it's refused plainly.
- [ ] **10. Terminal.** `uqctl keys reset --all` puts every override back and keeps your own
  shortcuts. `uqctl keys list` shows every row.
- [ ] **11. App menu tap.** Tapping Super alone still opens the app menu, now rendered from
  `generated.lua`.
- [ ] **12. Undo and restore.** Undo a rebind and an added shortcut from History, and restore
  yesterday's point. Each works live with no relog.
- [ ] **13. Key names.** In the capture, press each of: a letter, an arrow, F5, Print, a volume
  key, a brightness key, Launch/mail/search/home keys, and a keypad key. Note any the editor shows
  as "other keys" or refuses (the Qt key code table was written without a Qt build here, so
  brightness, Launch1, mail, search and homepage codes especially need checking).

At the end, `stat -c %y ~/.config/hypr/uq/user.lua` matches the start.

## Questions for the PR

1. Should **Open a terminal** follow the Shell page's terminal (`shell.terminal`, via
   `uqctl launch`) instead of `kitty`? R4 keeps `kitty` so this slice changes no default.
2. Does an empty submap pass keys through to the app on 0.56.2 and 0.57.x (R11)? If it doesn't,
   capture falls back to the manual pickers, and the button is hidden.
3. Should uQ also offer `long_press` or `non_consuming` for your own shortcuts? v1 offers only
   `locked` and `repeating`.
