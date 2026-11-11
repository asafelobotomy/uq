# Settings page template

A starting point for a uQ Settings page shipped by another app.

1. Copy this folder to your repository as `uq/pages/<id>/` and set `id` in `page.toml` to the
   same `<id>`.
2. Change `name`, `icon`, `keywords`, the `[[setting]]` rows, `[requires]` and the
   `[[command]]` tables to fit your app. Remove what you don't need.
3. Edit `Page.qml`; keep parsing and other pure logic in `logic.js` beside it (imported as
   `import "logic.js" as Logic`), and split big pages into component files in the same folder. Show all text with `UqLabel` (from `import org.uq.settings`), never `Text`
   or `Label`.
4. Check the folder: `uqctl integrations check uq/`. Run it in your CI too.
5. Install the folder to `/usr/share/uq/settings/pages/<id>/` from your package. Settings picks
   it up within a few seconds, without restarting.

To try it without installing: `uq-settings --plugin-dir <a folder holding <id>/>`.

The full reference, including what `ctx` offers and what is checked, is
`docs/settings-pages.md` in the uQ repository.
