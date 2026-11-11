# Widget template

A starting point for a uQ widget shipped by another app: an icon in the bar's Status island
(`slot = "bar"`) or a tile in the Apps group of quick settings (`slot = "quick"`).

1. Copy this folder to your repository as `uq/widgets/<id>/` and set `id` in `widget.toml` to the
   same `<id>`.
2. Change `name`, `slot`, `order`, `[requires]`, `[dbus]` and the `[[command]]` tables to fit
   your app. Remove what you don't need.
3. Edit `Widget.qml`. Show all text through `ctx.UqText`, never `Text` or `Label`.
4. Check the folder: `uqctl integrations check uq/`. Run it in your CI too.
5. Install the folder to `/usr/share/uq/shell/widgets/<id>/` from your package. The bar picks it
   up a second or two later, without logging out.

`uqctl integrations list` shows whether uQ found it, and why it is hidden or refused.

The full reference, including what `ctx` offers and what the bridge enforces, is
`docs/integrating.md` in the uQ repository.
