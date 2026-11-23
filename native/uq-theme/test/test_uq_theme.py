"""Tests for uq_theme.py: python3 -m unittest discover -s native/uq-theme/test

The shared cases are the Rust tests' (uq_theme.rs), so the three languages give the same object."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

KIT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("uq_theme", KIT / "uq_theme.py")
uq_theme = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(uq_theme)


def tokens(accent="#d4737f", **extra):
    colors = {k: "#202020" for k in uq_theme.COLORS}
    colors["accent"] = accent
    t = {"name": "Test", "variant": "light", "colors": colors,
         "ui": {"radius": 10, "gap_in": 4}, "fonts": {"ui": "Inter", "size": 11}}
    t.update(extra)
    return json.dumps(t)


class Parse(unittest.TestCase):
    def test_a_full_theme_is_read(self):
        t = uq_theme.parse(tokens())
        self.assertTrue(t["present"])
        self.assertEqual(t["variant"], "light")
        self.assertEqual(t["colors"]["accent"], "#d4737f")
        self.assertEqual((t["radius"], t["gapIn"], t["gapOut"], t["borderWidth"]), (10, 4, 12, 1))
        self.assertEqual(t["fontUi"], "Inter")
        self.assertEqual(t["motion"], {"fast": 100, "base": 140, "slow": 220, "reduce": False})

    def test_a_partial_or_broken_file_is_no_theme(self):
        for text in ("not json", "[]", '{"colors":{"bg":"#000000"}}', tokens("red")):
            self.assertEqual(uq_theme.parse(text), {"present": False}, text)

    def test_silly_numbers_are_clamped(self):
        self.assertEqual(uq_theme.parse(tokens(ui={"radius": 500}))["radius"], 32)
        self.assertEqual(uq_theme.parse(tokens(ui={"radius": True}))["radius"], 8)

    def test_motion_follows_and_reduces(self):
        m = uq_theme.parse(tokens(motion={"fast": 90, "base": 120, "slow": 99999}))["motion"]
        self.assertEqual(m, {"fast": 90, "base": 120, "slow": 1000, "reduce": False})
        m = uq_theme.parse(tokens(motion={"fast": 100, "base": 140, "slow": 220, "reduce": True}))["motion"]
        self.assertEqual(m, {"fast": 0, "base": 0, "slow": 0, "reduce": True})


class Paths(unittest.TestCase):
    def test_the_path_follows_xdg(self):
        self.assertEqual(uq_theme.tokens_path(env={"HOME": "/h", "XDG_STATE_HOME": "/st"}),
                         Path("/st/uq/theme/tokens.json"))
        self.assertEqual(uq_theme.tokens_path(env={"HOME": "/h", "XDG_STATE_HOME": "rel"}),
                         Path("/h/.local/state/uq/theme/tokens.json"))
        self.assertEqual(uq_theme.tokens_path("APP_TOKENS", env={"APP_TOKENS": "/f.json"}), Path("/f.json"))
        self.assertIsNone(uq_theme.tokens_path(env={}))

    def test_load_and_watch_folders(self):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "uq/theme/tokens.json"
            self.assertEqual(uq_theme.load(f), {"present": False})
            self.assertEqual(uq_theme.theme_dirs(f)[0], d)
            f.parent.mkdir(parents=True)
            f.write_text(tokens())
            self.assertTrue(uq_theme.load(f)["present"])
            self.assertEqual(uq_theme.theme_dirs(f), [str(f.parent), str(f.parent.parent)])


if __name__ == "__main__":
    unittest.main()
