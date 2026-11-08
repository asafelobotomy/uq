# Theme credits

| Theme | Source | Licence |
|---|---|---|
| Gruvbox | Palette by Pavel Pertsev (morhetz), https://github.com/morhetz/gruvbox | MIT |
| Dracula (dark) | Dracula Theme, https://draculatheme.com | MIT |
| Dracula (light) | "Alucard", the Dracula Theme light palette | MIT |
| Matcha, Forest, Graphite, Rosewood, Velvet | Original palettes authored for uQ | MIT (this repository) |

Where a published accent failed WCAG AA (4.5:1) against its own background, uQ darkens it
slightly: Gruvbox light uses `#946110` instead of `#b57614`, Matcha light `#197c69` instead of
`#1a7f6b`.

Each theme file holds semantic tokens; see `crates/uq-core/src/theme.rs` for the schema.
