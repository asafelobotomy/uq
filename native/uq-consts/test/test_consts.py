"""Tests for the generated Python, JS, shell and the generator: python3 -m unittest discover -s native/uq-consts/test"""

import importlib.util
import json
import subprocess
import tomllib
import unittest
from pathlib import Path

KIT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("uq_consts", KIT / "uq_consts.py")
uq_consts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(uq_consts)
SRC = tomllib.loads((KIT / "consts.toml").read_text())


class Python(unittest.TestCase):
    def test_absolute_state_home_wins(self):
        self.assertEqual(uq_consts.state_home({"HOME": "/h", "XDG_STATE_HOME": "/st"}), Path("/st"))

    def test_relative_state_home_is_ignored(self):
        for v in ("", "rel"):
            self.assertEqual(uq_consts.state_home({"HOME": "/h", "XDG_STATE_HOME": v}), Path("/h/.local/state"))

    def test_no_home_is_none(self):
        self.assertIsNone(uq_consts.tokens({}))

    def test_tokens_path(self):
        self.assertEqual(uq_consts.tokens({"HOME": "/h"}), Path("/h/.local/state/uq/theme/tokens.json"))
        self.assertEqual(uq_consts.quickshell_crashes_dir({"HOME": "/h"}), Path("/h/.cache/quickshell/crashes"))

    def test_values_match_the_source(self):
        self.assertEqual(uq_consts.VERSION, SRC["version"])
        self.assertEqual(uq_consts.DBUS_DESKTOP_NAME, SRC["dbus"]["desktop_name"])
        self.assertEqual(uq_consts.MOTION_POP_SCALE, SRC["motion"]["pop_scale"])
        self.assertEqual(uq_consts.PALETTE_DARK, SRC["palette"]["dark"])
        self.assertEqual(list(uq_consts.PALETTE_LIGHT), list(uq_consts.PALETTE_KEYS))


class Js(unittest.TestCase):
    def test_module_exports_the_source(self):
        script = (
            f"import * as c from {json.dumps(str(KIT / 'uq-consts.js'))};"
            "console.log(JSON.stringify({v: c.VERSION, m: c.motion, p: c.palette.light, t: c.statePaths.tokens,"
            " frozen: Object.isFrozen(c.palette.dark)}))"
        )
        out = json.loads(subprocess.run(["node", "--input-type=module", "-e", script],
                                        check=True, capture_output=True, text=True).stdout)
        self.assertEqual(out["v"], SRC["version"])
        self.assertEqual(out["m"]["easingExit"], SRC["motion"]["easing_exit"])
        self.assertEqual(out["p"], SRC["palette"]["light"])
        self.assertEqual(out["t"], SRC["paths"]["state"]["tokens"])
        self.assertTrue(out["frozen"])


class Shell(unittest.TestCase):
    def run_sh(self, body, env):
        return subprocess.run(["sh", "-c", f". {KIT / 'uq-consts.sh'}; {body}"], check=True,
                              capture_output=True, text=True, env=env).stdout

    def test_paths_and_values(self):
        self.assertEqual(self.run_sh("uq_tokens", {"HOME": "/h"}), "/h/.local/state/uq/theme/tokens.json")
        self.assertEqual(self.run_sh("uq_tokens", {"HOME": "/h", "XDG_STATE_HOME": "/st"}), "/st/uq/theme/tokens.json")
        self.assertEqual(self.run_sh("uq_tokens", {"HOME": "/h", "XDG_STATE_HOME": "rel"}), "/h/.local/state/uq/theme/tokens.json")
        self.assertEqual(self.run_sh('printf %s "$UQ_INSTALL_PAGES_DIR $UQ_PALETTE_DARK_BG"', {}),
                         f"{SRC['install']['pages_dir']} {SRC['palette']['dark']['bg']}")


class Generator(unittest.TestCase):
    def test_outputs_are_up_to_date(self):
        subprocess.run(["python3", str(KIT / "gen.py"), "--check"], check=True)

    def test_every_output_starts_with_its_version(self):
        for name in ("uq_consts.rs", "uqconsts.h", "uq_consts.py", "uq-consts.js", "uq-consts.sh", "UqThemeBase.qml"):
            head = (KIT / name).read_text().splitlines()[0]
            self.assertIn(f"uq-consts {SRC['version']}", head, name)


if __name__ == "__main__":
    unittest.main()
