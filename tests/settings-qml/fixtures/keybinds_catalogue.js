.pragma library
// A few rows of the Keyboard shortcuts page's `catalogue` as `keybinds::list::list` makes it, for the
// shortcut editor tests. `fixture_is_a_slice_of_the_catalogue` (apps/uq-settings) checks that every
// row here is still in the real catalogue; paste the new row from its output when one changes.
var catalogue = {
  "actions": {
    "shell": [
      {
        "action": {
          "kind": "shell",
          "name": "spotlight"
        },
        "label": "Opens Spotlight",
        "may_lock": false,
        "may_repeat": true
      },
      {
        "action": {
          "kind": "shell",
          "name": "vol-up"
        },
        "label": "Turns the volume up",
        "may_lock": true,
        "may_repeat": true
      }
    ],
    "uq": [
      {
        "action": {
          "kind": "uq",
          "name": "terminal"
        },
        "label": "Opens a terminal",
        "may_lock": false,
        "may_repeat": false
      },
      {
        "action": {
          "kind": "uq",
          "media": "play-pause",
          "name": "media"
        },
        "label": "Plays or pauses",
        "may_lock": true,
        "may_repeat": true
      }
    ],
    "window": [
      {
        "action": {
          "kind": "window",
          "name": "close"
        },
        "label": "Closes the window",
        "may_lock": false,
        "may_repeat": true
      }
    ],
    "workspace": [
      {
        "action": {
          "kind": "workspace",
          "name": "focus",
          "workspace": 1
        },
        "label": "Goes to workspace 1",
        "may_lock": false,
        "may_repeat": true
      }
    ]
  },
  "keys": [
    {
      "group": "mouse",
      "kind": "button",
      "label": "Left mouse button",
      "spelling": "mouse:272"
    },
    {
      "group": "letter",
      "kind": "key",
      "label": "K",
      "spelling": "K"
    },
    {
      "group": "letter",
      "kind": "key",
      "label": "P",
      "spelling": "P"
    },
    {
      "group": "letter",
      "kind": "key",
      "label": "Q",
      "spelling": "Q"
    },
    {
      "group": "letter",
      "kind": "key",
      "label": "T",
      "spelling": "T"
    },
    {
      "group": "function",
      "kind": "key",
      "label": "F9",
      "spelling": "F9"
    },
    {
      "group": "editing",
      "kind": "key",
      "label": "Enter",
      "spelling": "Return"
    },
    {
      "group": "editing",
      "kind": "key",
      "label": "Space",
      "spelling": "Space"
    },
    {
      "group": "navigation",
      "kind": "key",
      "label": "Page up",
      "spelling": "Prior"
    },
    {
      "group": "media",
      "kind": "key",
      "label": "Volume up key",
      "spelling": "XF86AudioRaiseVolume"
    },
    {
      "group": "mouse",
      "kind": "wheel",
      "label": "Scroll down",
      "spelling": "mouse_down"
    }
  ]
}
;
