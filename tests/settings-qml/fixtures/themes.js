.pragma library
// What the theme editor tests feed ThemeEditor: `editor` is the open session (what the Themes page
// state holds as `editor`) for a new theme made from Gruvbox, and `preview` is the live preview of
// that draft. `fixture_matches_the_editor_session_and_preview` (apps/uq-settings) checks that they
// still match; paste the output of `cargo test print_fixture -- --ignored --nocapture` here when
// the editor logic changes.
var editor = {
  "author": "",
  "base": "gruvbox",
  "colors": {
    "dark": {
      "accent": "#d79921",
      "bad": "#cc241d",
      "bg": "#282828",
      "good": "#98971a",
      "surface": "#343330",
      "text": "#ebdbb2",
      "warn": "#d79921"
    },
    "light": {
      "accent": "#946110",
      "bad": "#9d0006",
      "bg": "#fbf1c7",
      "good": "#79740e",
      "surface": "#f0e6be",
      "text": "#3c3836",
      "warn": "#b57614"
    }
  },
  "edit": false,
  "id": "",
  "keys": [
    {
      "key": "bg",
      "label": "Background"
    },
    {
      "key": "surface",
      "label": "Panels"
    },
    {
      "key": "text",
      "label": "Text"
    },
    {
      "key": "accent",
      "label": "Accent"
    },
    {
      "key": "good",
      "label": "Good"
    },
    {
      "key": "warn",
      "label": "Warning"
    },
    {
      "key": "bad",
      "label": "Bad"
    }
  ],
  "name": "Gruvbox copy"
}
;
var preview = {
  "authorProblem": "",
  "dark": {
    "problems": [],
    "tokens": {
      "accent": "#d79921",
      "accent_text": "#282828",
      "bg": "#282828",
      "border": "#676254",
      "error": "#cc241d",
      "overlay": "#514e45",
      "success": "#98971a",
      "surface": "#343330",
      "surface_alt": "#43403a",
      "text": "#ebdbb2",
      "text_muted": "#a79c82",
      "warning": "#d79921"
    }
  },
  "light": {
    "problems": [],
    "tokens": {
      "accent": "#946110",
      "accent_text": "#fbf1c7",
      "bg": "#fbf1c7",
      "border": "#beb598",
      "error": "#9d0006",
      "overlay": "#d3caa8",
      "success": "#79740e",
      "surface": "#f0e6be",
      "surface_alt": "#e2d8b3",
      "text": "#3c3836",
      "text_muted": "#6c665a",
      "warning": "#b57614"
    }
  },
  "nameProblem": "",
  "ok": true
}
;
