.pragma library
// The Apps and scaling page's `fields` catalogue as `pages::compat::fields()` returns it, for
// the fix dialog tests. `fixture_matches_the_fields_catalogue` (apps/uq-settings, pages::compat)
// checks that it still does; paste the new output here when a table in `compat` changes.
var fields = {
  "limits": {
    "apps": 100,
    "arg_chars": 1024,
    "args": 20,
    "env": 20
  },
  "platforms": [
    {
      "key": "auto",
      "label": "Automatic"
    },
    {
      "key": "wayland",
      "label": "Wayland"
    },
    {
      "key": "x11",
      "label": "X11"
    }
  ],
  "scales": [
    1.0,
    1.25,
    1.5,
    1.75,
    2.0,
    2.25,
    2.5,
    2.75,
    3.0
  ],
  "switches": [
    {
      "description": "Older X11 apps draw at your screen's full sharpness instead of being stretched and blurry. Applies to apps started after the change.",
      "key": "sharp_x11",
      "label": "Sharp X11 apps"
    },
    {
      "description": "Apps built on Electron, such as many chat apps and code editors, run natively for sharp text. Applies to apps started after the change.",
      "key": "electron_wayland",
      "label": "Run Electron apps on Wayland"
    },
    {
      "description": "Chromium, Chrome, Brave, Vivaldi and Edge run natively for sharp text and smooth scaling. Applies to apps started after the change.",
      "key": "chromium_wayland",
      "label": "Run Chromium-based browsers on Wayland"
    },
    {
      "description": "Qt apps run natively when they can, and as X11 apps when they can't. Applies to apps started after the change.",
      "key": "qt_wayland",
      "label": "Qt apps prefer Wayland"
    },
    {
      "description": "Steam's own windows follow your main screen's scale. Applies to apps started after the change.",
      "key": "steam_scale",
      "label": "Steam UI scale"
    }
  ]
};
