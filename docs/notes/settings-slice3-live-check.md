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

## VM run (2026-11-11)

Run in the `tests/vm` guest with two outputs (`UQVM_GPU2="bochs-display,xres=1280,yres=800"`,
uq at this branch, Hyprland v0.57.2, Quickshell 0.3.2), by the new `checks/40-displays.sh` and
`checks/50-input.sh` plus two manual reboots:

```
UQVM_GPU2="bochs-display,xres=1280,yres=800" tests/vm/run.sh --only '^(D|I)'
```

**Result: 18 PASS of 18, plus both reboot checks.** The 15 s is timed inside the guest
(`guest/uq-revert-timer`): every automatic revert came 15.1 to 15.3 s after the change went live.

- **Bug found and fixed here:** the window started when `ApplyMonitors` arrived, before the
  config check and the eval. In the VM an apply took up to 18.6 s, which left the new layout on
  screen for 1 s. It now starts when the change is live (uqd, `apply_monitors`).
- Blackout (D1), revert (D1b), only output off (D2), crash (D3), uqd SIGKILL (D4), second apply
  (D7), mirror and on/off (D8), scale 1.25/1.5 at 1920x1080 and 1920x1440 (D9), VRR accepted
  (D10), gaps during the window (D11), Keep then History/Undo (D14), `eval` without a save (D15):
  PASS. Reboot during the window came back on the old layout; Keep then reboot came back on the
  new one.
- Keyboard layouts, combo, reorder and remove (I1), repeat values (I2), `uqctl mouse speed -0.5`
  on the open Input page (I3), touchpad options (I4), per-device override (I5), and the layout
  code field without `evdev.lst` (I6, screenshot): PASS.
- **Hyprland snaps scales:** 1.5 on a 1280x800 output runs at 1.6 (Hyprland only takes scales
  that divide the mode evenly). uQ stores 1.5.
- **Not covered in the VM, still for real hardware:** a mode the panel truly can't show (the VM
  falls back to a mode it has), VRR actually on, a 4K panel, how the 250 ms repeat feels, real
  touchpad gestures, and dragging in the arrangement and Escape/Enter in the live dialog (both
  covered offscreen by `tst_display_arrangement.qml` and `tst_revert_dialog.qml`).
