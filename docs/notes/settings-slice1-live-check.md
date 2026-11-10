# Settings slice 1: manual live check

Run on a real Hyprland session with uQ installed (not CI, not a private bus). Tick each item.

## Theme, fonts, cursor

- [ ] Open `uqctl settings appearance`. Change the theme: the app, the bar and a GTK app (for
  example `gnome-text-editor`) follow without a restart.
- [ ] Switch the variant (dark/light): the same three follow.
- [ ] Change the UI font and size: the app, the bar and the GTK app follow.
- [ ] Change the cursor theme: a **new** kitty window shows the new cursor (running windows keep
  the old one; that is expected).
- [ ] Qt: Kvantum actually styles uq-settings' controls (buttons, sliders, combo boxes look like
  the uQ Kvantum theme, not plain Fusion). Check after `uqctl setup --apps`.
- [ ] Set `kvantum` off: the Qt style falls back and nothing breaks.

## Wallpaper

- [ ] Give each monitor its own wallpaper; each shows its own.
- [ ] Set a folder slideshow with a short interval (10 s): images advance in name order with a
  crossfade.
- [ ] Delete the image that is showing: the layer shows the fallback colour (not black or blank)
  and the Wallpaper page shows the error for that monitor.
- [ ] The wallpaper layer is click-through and sits below windows on real Hyprland: clicks on
  an empty desktop reach nothing odd, no window is covered, and `hyprctl layers` lists
  `uq-wallpaper` on the background layer (`mask: Region {}`).
- [ ] A 4K image fills a 4K output sharply (no blur from downscaling; `sourceSize` is the output
  size).

## Single instance, plugins

- [ ] With the app open, `uqctl settings wallpaper` from a terminal raises the same window on the
  Wallpaper page; no second window.
- [ ] "Settings…" in quick settings opens the app and closes the panel.
- [ ] Put a broken page (a `page.toml` plus a QML file with a syntax error) in
  `/usr/share/uq/settings/pages/<id>/`: the Apps category shows it and opening it shows the
  "This page couldn't load" card with the error text; the rest of the app works.

## Known gaps (not failures)

- [ ] Plugin pages get theme tokens only at startup: a theme change while uq-settings is open
  does not restyle an open plugin page until the app is restarted.
- [ ] `uqctl settings` from a keybind while the app is wedged (frozen, name still owned) hangs:
  the D-Bus call has no timeout yet. Confirm it only affects that call, then kill `uq-settings`.
