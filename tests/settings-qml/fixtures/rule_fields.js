.pragma library
// The Window rules page's `fields` catalogue as `pages::rules::fields()` returns it, for the
// rule editor tests. `fixture_matches_the_fields_catalogue` (apps/uq-settings) checks that it
// still does; paste the new output here when a table in `rules::hypr` changes.
var fields = {
  "layer_effects": [
    {
      "choices": [],
      "key": "blur",
      "kind": "bool",
      "label": "Blur",
      "lua": "blur",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "ignore_alpha",
      "kind": "float",
      "label": "Blur ignores faint pixels below",
      "lua": "ignore_alpha",
      "max": 100,
      "min": 0,
      "static": false
    },
    {
      "choices": [],
      "key": "no_anim",
      "kind": "bool",
      "label": "No animations",
      "lua": "no_anim",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [
        "fade",
        "slide",
        "popin"
      ],
      "key": "animation",
      "kind": "choice",
      "label": "Animation style",
      "lua": "animation",
      "max": null,
      "min": null,
      "static": false
    }
  ],
  "layer_match": [
    {
      "choices": [],
      "key": "namespace",
      "kind": "pattern",
      "label": "Panel name",
      "lua": "namespace",
      "max": null,
      "min": null,
      "static": false
    }
  ],
  "match": [
    {
      "choices": [],
      "key": "class",
      "kind": "pattern",
      "label": "App ID",
      "lua": "class",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "title",
      "kind": "pattern",
      "label": "Window title",
      "lua": "title",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "initial_class",
      "kind": "pattern",
      "label": "App ID when it opened",
      "lua": "initial_class",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "initial_title",
      "kind": "pattern",
      "label": "Title when it opened",
      "lua": "initial_title",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "tag",
      "kind": "text",
      "label": "Tag",
      "lua": "tag",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "xwayland",
      "kind": "bool",
      "label": "Older X11 app",
      "lua": "xwayland",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "floating",
      "kind": "bool",
      "label": "Floating",
      "lua": "float",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "fullscreen",
      "kind": "bool",
      "label": "Fullscreen",
      "lua": "fullscreen",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "pinned",
      "kind": "bool",
      "label": "Pinned",
      "lua": "pin",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "modal",
      "kind": "bool",
      "label": "Dialog",
      "lua": "modal",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "workspace",
      "kind": "text",
      "label": "Workspace",
      "lua": "workspace",
      "max": null,
      "min": null,
      "static": false
    }
  ],
  "window_effects": [
    {
      "choices": [],
      "key": "float",
      "kind": "bool",
      "label": "Float",
      "lua": "float",
      "max": null,
      "min": null,
      "static": true
    },
    {
      "choices": [],
      "key": "tile",
      "kind": "bool",
      "label": "Tile",
      "lua": "tile",
      "max": null,
      "min": null,
      "static": true
    },
    {
      "choices": [],
      "key": "fullscreen",
      "kind": "bool",
      "label": "Fullscreen",
      "lua": "fullscreen",
      "max": null,
      "min": null,
      "static": true
    },
    {
      "choices": [],
      "key": "maximize",
      "kind": "bool",
      "label": "Maximize",
      "lua": "maximize",
      "max": null,
      "min": null,
      "static": true
    },
    {
      "choices": [],
      "key": "workspace",
      "kind": "text",
      "label": "Open on workspace",
      "lua": "workspace",
      "max": null,
      "min": null,
      "static": true
    },
    {
      "choices": [],
      "key": "monitor",
      "kind": "text",
      "label": "Open on screen",
      "lua": "monitor",
      "max": null,
      "min": null,
      "static": true
    },
    {
      "choices": [],
      "key": "size",
      "kind": "pair",
      "label": "Size",
      "lua": "size",
      "max": 16384,
      "min": 1,
      "static": true
    },
    {
      "choices": [],
      "key": "move",
      "kind": "pair",
      "label": "Position",
      "lua": "move",
      "max": 16384,
      "min": -16384,
      "static": true
    },
    {
      "choices": [],
      "key": "center",
      "kind": "bool",
      "label": "Center",
      "lua": "center",
      "max": null,
      "min": null,
      "static": true
    },
    {
      "choices": [],
      "key": "pin",
      "kind": "bool",
      "label": "Keep on top of every workspace",
      "lua": "pin",
      "max": null,
      "min": null,
      "static": true
    },
    {
      "choices": [],
      "key": "no_initial_focus",
      "kind": "bool",
      "label": "Don't focus when it opens",
      "lua": "no_initial_focus",
      "max": null,
      "min": null,
      "static": true
    },
    {
      "choices": [],
      "key": "opacity",
      "kind": "float",
      "label": "Opacity",
      "lua": "opacity",
      "max": 100,
      "min": 10,
      "static": false
    },
    {
      "choices": [],
      "key": "no_blur",
      "kind": "bool",
      "label": "No blur",
      "lua": "no_blur",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "no_shadow",
      "kind": "bool",
      "label": "No shadow",
      "lua": "no_shadow",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "no_anim",
      "kind": "bool",
      "label": "No animations",
      "lua": "no_anim",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "rounding",
      "kind": "int",
      "label": "Corner rounding",
      "lua": "rounding",
      "max": 20,
      "min": 0,
      "static": false
    },
    {
      "choices": [],
      "key": "border_size",
      "kind": "int",
      "label": "Border size",
      "lua": "border_size",
      "max": 20,
      "min": 0,
      "static": false
    },
    {
      "choices": [
        "none",
        "always",
        "focus",
        "fullscreen"
      ],
      "key": "idle_inhibit",
      "kind": "choice",
      "label": "Keep the screen awake",
      "lua": "idle_inhibit",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "keep_aspect_ratio",
      "kind": "bool",
      "label": "Keep the shape when resized",
      "lua": "keep_aspect_ratio",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "tag",
      "kind": "text",
      "label": "Add a tag",
      "lua": "tag",
      "max": null,
      "min": null,
      "static": false
    }
  ],
  "workspace": [
    {
      "choices": [],
      "key": "monitor",
      "kind": "text",
      "label": "Screen",
      "lua": "monitor",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "default",
      "kind": "bool",
      "label": "Default workspace",
      "lua": "default",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "persistent",
      "kind": "bool",
      "label": "Keep when empty",
      "lua": "persistent",
      "max": null,
      "min": null,
      "static": false
    },
    {
      "choices": [],
      "key": "gaps_in",
      "kind": "int",
      "label": "Gap between windows",
      "lua": "gaps_in",
      "max": 100,
      "min": 0,
      "static": false
    },
    {
      "choices": [],
      "key": "gaps_out",
      "kind": "int",
      "label": "Gap at the screen edge",
      "lua": "gaps_out",
      "max": 100,
      "min": 0,
      "static": false
    },
    {
      "choices": [],
      "key": "border_size",
      "kind": "int",
      "label": "Border size",
      "lua": "border_size",
      "max": 20,
      "min": 0,
      "static": false
    }
  ]
};
