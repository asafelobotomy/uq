# Settings slice F (display arrangements, main display, span wallpaper, workspace planner, bar per screen, touchpad gestures): manual live check

> Automated part: `tests/live/run.sh --only '^SF'` (`tests/live/checks/f-displays.sh`; also CI's settings-live job). The header of that file lists the items below that it leaves to a person, and why.

Run on a real Hyprland session (0.56.2 or newer, up to 0.57.x) with uQ installed. Not CI. You need
a laptop plus one or two external screens (ideally two of the same model), a touchpad, and
`xrandr` installed (`xorg-xrandr`). The pages are **Displays**, **Input** and **Wallpaper** in
`uq-settings` (`uqctl settings displays`, `input`, `wallpaper`). Before you start, note
`user.lua`'s modification time: `stat -c %y ~/.config/hypr/uq/user.lua`. It must be the same at
the end.

Where a step says "check Hyprland's errors", run `hyprctl configerrors`. Empty output means no
errors.

- [ ] **1. Arrangements switch on their own (spec §1, criterion 3).** Undocked, arrange the laptop
  screen, Keep, and Remember it as "Laptop". Docked, arrange the three screens, Keep, Remember as
  "Desk". Unplug: within 2 s the laptop layout is back with no dialog. Plug in: "Desk" is back
  within 2 s. **History** shows the two Remembers and no hotplug entries.
- [ ] **1b. A kept layout stays.** With "Desk" docked, Keep a different layout (for example turn one
  screen off), wait 5 s: it stays, and so it does after undoing it in History. Log out and in with
  the same screens: the kept layout is still there (uqd switches on login only when the stored
  layout doesn't list every connected screen). Plug or unplug a screen: the saved arrangement for
  the new set applies.
- [ ] **2. Port swap.** Swap the two external cables (or dock ports). "Desk" still applies and each
  screen keeps its own position and scale. With two identical screens, record which way round they
  came back.
- [ ] **3. Flapping.** Unplug and replug quickly three times: one switch at the end, no dialog,
  `journalctl --user -u uqd` shows one apply.
- [ ] **4. Bad arrangement.** Hand-edit a saved arrangement's `mode` to one the screen can't do (or
  one Hyprland refuses) and replug: the layout stays, one notification appears, the Displays page
  says it couldn't be used; replug again without changing the set: no second notification.
- [ ] **5. Auto switch off.** Turn off "Switch arrangements when screens change" and replug: nothing
  changes, and the page offers to arrange and remember.
- [ ] **6. Main display.** Choose the external screen. Notifications, the volume OSD, Spotlight
  (SUPER+Space) and the emoji picker (SUPER+period) appear there while you work on the laptop
  screen; the app menu from the laptop's bar stays on the laptop. `xrandr --listmonitors` (from an
  X11 terminal) shows it as primary (`*`). Log out and in: the cursor and workspace 1 start there.
  Unplug it: pop-ups follow the focused screen again. Record whether `default_monitor` changed
  anything on reload. Also record the XWayland connector names `xrandr` lists against
  `hyprctl monitors` (they must match for `--primary` to work; check `journalctl --user -u uqd`
  for a logged failure).
- [ ] **7. Bar per screen.** Main display only: one bar. Choose: only the ticked screens. Choose a
  screen and unplug it: bars on every screen. Plug a screen in and out with "All": the other
  screens' bars aren't rebuilt (Quickshell reuses the delegates for the same screen; if every bar
  flickers, record it).
- [ ] **8. Screens with no bar.** With the bar on one screen only, open quick settings and the
  notification centre by their keys while the focus is on a screen with no bar: they now open there
  (they used to need a bar). Record whether that's acceptable. With a main display chosen on a
  screen with no bar, pop-ups still keep the bar's top space on that screen (the exclusive zone):
  record whether windows or pop-ups sit under or too far from the screen's edge.
- [ ] **9. Span.** Choose "Span across displays" for All displays with a wide image: the image runs
  on across the bezel at the screens' real positions; a screen with its own wallpaper is left out.
  With a folder, both screens change slide together. With screens of different scale (for example
  one at 1 and one at 1.5) record whether the crop lines up across the bezel (the box uses logical
  coordinates).
- [ ] **10. Workspaces per screen.** Sequential with 10: `hyprctl workspaces` puts 1 to 5 on the left
  screen and 6 to 10 on the right. Open windows on 7, unplug the right screen, plug it back:
  workspace 7 moves back (this confirms the `hl.dispatch(hl.dsp.workspace.move(...))` call moves a
  workspace that has windows; note how long after the screen came back, to check the event
  timing). Turn Move back off: it stays. Interleave and Choose for each behave as the preview said.
  A workspace rule of your own on the Rules page keeps its own screen.
- [ ] **11. Gesture defaults.** On a fresh login, three fingers sideways switches workspace and four
  fingers up opens Spotlight once per swipe (a real touchpad: confirms the shell action runs once
  at the gesture's end). `grep -n gesture /usr/share/uq/hypr/base.lua` finds nothing;
  `generated.lua` has both lines. On a first login after upgrading, check the swipe works before
  uqd's first render has finished (it should after a second or two).
- [ ] **12. Gesture editor.** Add three fingers down → Show or hide the scratch workspace: it works.
  Try three fingers left: the editor warns that sideways already uses it and Save stays off. Add
  SUPER + four fingers pinch in → Float the window: it works. Turn the default sideways swipe off:
  it stops. `hyprctl configerrors` is empty after each change, and a config reload doesn't report
  "will be overshadowed" against uQ's own earlier lines.
- [ ] **13. Swipe options.** Distance 600, Reverse on, Make a new one off, ratio 0.3: each takes
  effect at once; `hyprctl getoption gestures:workspace_swipe_distance` is 600. Add
  `gestures = { workspace_swipe_distance = 200 }` to `user.lua` and reload: the row says
  "Overridden in user.lua, line N".
- [ ] **14. `user.lua` gesture.** Add `hl.gesture({ fingers = 3, direction = "left", action =
  "close" })` to `user.lua` and reload: the Input page's gestures row says user.lua sets gestures
  at that line, and the next uQ change shows Hyprland's message plainly. Remove it.
- [ ] **15. History and restore.** Undo the main display change, a bar change, a gesture change and a
  Remember from **History**: each row reads plainly and the page follows. Restore yesterday's
  point: the saved arrangements, the planner and the gestures match it.
- [ ] **16. Dev session.** In `uq-dev session`, plugging a screen into the host doesn't run `xrandr`
  (check `journalctl --user -u uqd`).
- [ ] **17. `user.lua`'s modification time** is unchanged.

## Not confirmed from source

These were not checked against a running Hyprland when the slice was built; items 1, 6, 10, 11 and
12 above are where they are confirmed:

- a C function from `hl.dsp.global(...)` as a gesture's action runs the shell's global shortcut once,
  on the gesture's end;
- a config reload clears earlier gestures;
- `cursor:default_monitor` takes a connector name at startup only and does nothing harmful on reload;
- XWayland names its RandR outputs after the Wayland connectors, so `xrandr --output DP-1 --primary`
  works;
- `hl.dispatch(hl.dsp.workspace.move(...))` through `eval` moves a workspace that has windows;
- `monitoradded` and `monitorremoved` events arrive soon enough that the 1 s debounce covers a dock;
- Quickshell's `Variants` keeps delegates by `ShellScreen` identity when its model is a fresh array.
