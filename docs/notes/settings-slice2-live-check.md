# Settings slice 2: manual live check

Run on a real Hyprland session with uQ installed (not CI, not a private bus). Tick each item.

## Look

- [ ] `uqctl settings look`: change gaps in/out, border width and rounding; open windows change at
  once.
- [ ] Blur off and on, shadow off and on: windows follow live.
- [ ] Animation speed 0.25 (slow) and 4 (fast), also 0.5 and 2: the change is visible; animations
  off stops them.
- [ ] After upgrading from 0.2.x: animations are default until uqd re-renders, then follow the
  page. An `hl.animation` line in `user.lua` still wins.
- [ ] A value the installed Hyprland rejects shows its message in plain text and the control
  snaps back to the last good value.

## Layouts

- [ ] A workspace rule added from the page applies on that workspace; removing it reverts after
  the reload.
- [ ] `SUPER+ALT+M` changes the layout and the page's rule list updates without reopening.
- [ ] Rules set from the page and from the keybind stay in step (no stale rows either way).
- [ ] Default layout and dwindle/master/scrolling options take effect.

## Shell

- [ ] Hide each island in turn (Status, and the others): its space is freed; show it again.
- [ ] 12-hour clock: the bar clock switches; 24-hour switches back.
- [ ] Spotlight: switch calc off, calculator results stop; switch other providers off, their
  sections disappear and old rows cannot be activated.
- [ ] Change a provider's order: the Spotlight section order follows.
- [ ] Choose foot as the terminal: `htop.desktop` (Terminal=true) opens in foot. Uninstall foot:
  it falls back to kitty.
- [ ] Idle settings round trip: change idle timings and notification/privacy options, restart
  uqd and the page, values persist. Dim above lock is refused with a message and both controls
  show the saved values.
- [ ] Clear launch history empties the ranking.
