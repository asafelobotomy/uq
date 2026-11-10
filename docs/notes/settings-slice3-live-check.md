# Settings slice 3: manual live check

Run on a real Hyprland session with uQ installed (not CI, not a private bus), on hardware with two
outputs where possible. Tick each item.

## Displays

- [ ] **Blackout:** choose a mode the panel can't show. Leave the keyboard and mouse alone. The old
  layout is back within 15 s.
- [ ] **Only output off:** turning off the only output is refused with "at least one display must
  stay on".
- [ ] **Crash:** kill `uq-settings` during the window; the layout reverts on time.
- [ ] **uqd restart:** `systemctl --user kill -s KILL uqd` during the window. After the restart the
  old layout is back.
- [ ] **Reboot:** reboot during the window; it comes back on the old layout.
- [ ] **Keep:** keep, then reboot; it comes back on the new layout.
- [ ] **Second apply:** apply twice within 15 s, then wait; it returns to the layout from before the
  first apply.
- [ ] **Mirror and on/off:** mirror one output onto another, and turn one of two outputs off.
- [ ] **Scale:** 1.25 and 1.5 on a 1080p and a 4K panel.
- [ ] **VRR:** on and off.
- [ ] **Gaps during the window:** change gaps on the Look page while the dialog counts down. The
  displays stay on the candidate.
- [ ] **Arrangement:** drag one display next to the other; it snaps to the edge and never leaves a
  gap or an overlap.
- [ ] **Escape:** pressing Escape in the dialog reverts; Enter does not keep.
- [ ] **History:** after Keep, the History page shows one "Displays" entry from the Settings app;
  Undo on it puts the old layout back without a countdown.
- [ ] **`eval`:** Hyprland's `eval` of `hl.monitor` applied without a reload. Record the version:
  ______

## Input

- [ ] **Keyboard:** two layouts with a switch combo; the combo switches layout. Reorder them and
  remove one.
- [ ] **Repeat:** a repeat delay of 250 ms and a rate of 50 are noticeable.
- [ ] **Mouse:** `uqctl mouse speed -0.5` shows on the Input page at once.
- [ ] **Touchpad:** tap-to-click off works. Natural scroll on and off.
- [ ] **Per device:** a per-device speed for the touchpad differs from the mouse; removing the
  override goes back to the general value.
- [ ] **No xkeyboard-config list:** with `/usr/share/X11/xkb/rules/evdev.lst` missing, the page
  offers a layout code field instead of the list.
