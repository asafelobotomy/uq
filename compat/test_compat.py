"""Tests for compat.py: python3 -m unittest discover -s compat"""

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import compat  # noqa: E402

M = {
    "schema": 1,
    "contract": {
        "uq-api": {"provider": "uq", "levels": {"2": "0.3.0", "3": "0.3.10"}},
        "oyst-scan": {"provider": "oysterav", "levels": {"1": "0.4.0", "2": "0.6.0"}, "dropped": {"1": "0.7.0"}},
    },
    "package": {
        "uq": {"repo": "o/uq", "aur": ["uq"], "pkgbuild": ["PKGBUILD"]},
        "oysterav": {"repo": "o/oysterAV", "aur": ["oysterav"], "pkgbuild": ["PKGBUILD"]},
        "hauler": {"repo": "o/hauler", "aur": ["hauler"], "pkgbuild": ["PKGBUILD"],
                   "needs": {"uq-api": 3, "oyst-scan": 2}, "since": {"oyst-scan": "0.3.0"}},
        "romforge": {"repo": "o/romforge", "aur": ["romforge", "romforge-bin"], "pkgbuild": ["PKGBUILD"],
                     "needs": {"oyst-scan": 1, "uq-api": 2}, "min": {"uq": "0.3.20"}},
    },
}


class Manifest(unittest.TestCase):
    def test_the_real_manifest_is_valid(self):
        self.assertEqual(compat.problems(compat.load()), [])

    def test_mistakes_are_named(self):
        bad = {**M, "package": {**M["package"], "x": {"repo": "o/x", "aur": ["x"], "pkgbuild": ["P"],
                                                       "needs": {"uq-api": 9, "nope": 1}}}}
        errs = compat.problems(bad)
        self.assertIn("package x: uq-api has no level 9", errs)
        self.assertIn("package x: needs unknown contract nope", errs)


class Bounds(unittest.TestCase):
    def test_floors_take_the_highest_and_name_every_aur_package(self):
        self.assertEqual(compat.bounds(M, "romforge"), ["oysterav<0.4.0", "oysterav>=0.7.0", "uq<0.3.20"])
        self.assertEqual(compat.bounds(M, "hauler"), ["oysterav<0.6.0", "uq<0.3.10"])

    def test_a_provider_that_dropped_a_level_keeps_out_old_users(self):
        self.assertEqual(compat.bounds(M, "oysterav"), ["hauler<0.3.0"])
        self.assertEqual(compat.bounds(M, "uq"), [])

    def test_the_line(self):
        self.assertEqual(compat.line(M, "hauler"),
                         "conflicts+=('oysterav<0.6.0' 'uq<0.3.10')  " + compat.MARK)


class Sync(unittest.TestCase):
    def test_goes_before_the_first_function_then_replaces_itself(self):
        text = "pkgname=x\nconflicts=('y')\n\nbuild() {\n  make\n}\n"
        once = compat.sync_text(text, "conflicts+=('a<1')  " + compat.MARK)
        self.assertEqual(once, "pkgname=x\nconflicts=('y')\n\nconflicts+=('a<1')  " + compat.MARK + "\n\nbuild() {\n  make\n}\n")
        twice = compat.sync_text(once, "conflicts+=('a<2')  " + compat.MARK)
        self.assertEqual(twice.count(compat.MARK), 1)
        self.assertIn("'a<2'", twice)

    def test_keeps_a_heredoc_indent(self):
        text = "          cat > out/PKGBUILD <<EOF\n          pkgname=x-bin\n          package() {\n"
        out = compat.sync_text(text, "conflicts+=()  " + compat.MARK)
        self.assertIn("\n          conflicts+=()  " + compat.MARK + "\n\n          package() {", out)


class Releases(unittest.TestCase):
    def test_needs_the_latest_releases_dont_meet(self):
        self.assertEqual(compat.unmet(M, {"uq": "0.3.30", "oysterav": "0.6.1"}), [])
        errs = compat.unmet(M, {"uq": "0.3.5", "oysterav": "0.7.0"})
        self.assertIn("hauler needs uq-api 3 from uq 0.3.10; its latest release is 0.3.5", errs)
        self.assertIn("oysterav 0.7.0 dropped oyst-scan 1, which romforge's latest release needs", errs)

    def test_reads_what_a_checkout_uses(self):
        with tempfile.TemporaryDirectory() as d:
            r = pathlib.Path(d)
            (r / "uq/pages/x").mkdir(parents=True)
            (r / "uq/pages/x/page.toml").write_text("uq_api = 3\nid = \"x\"\n")
            (r / "uq/widgets/x").mkdir(parents=True)
            (r / "uq/widgets/x/widget.toml").write_text("uq_api = 1\n")
            (r / "uq/native.toml").write_text('[uq-theme]\nversion = "1.1.0"\nfiles = []\n')
            self.assertEqual(compat.uses(r), {"uq-api": 3, "uq-theme": 1})
            (r / "VERSION").write_text("0.2.5\n")
            self.assertEqual(compat.release_of(r), "0.2.5")


class Aur(unittest.TestCase):
    def test_clashes_among_the_aurs_versions(self):
        info = {
            "uq": {"Version": "0.3.41-1"},
            "oysterav": {"Version": "0.5.0-1"},
            "hauler": {"Version": "0.2.6-1", "Conflicts": ["oysterav<0.6.0", "uq<0.3.10"]},
            "romforge": {"Version": "1-1", "Conflicts": ["romforge-bin"]},
            "romforge-bin": {"Version": "1-1"},
        }
        errs, have = compat.aur_problems(M, info)
        self.assertEqual(errs, ["hauler 0.2.6-1 conflicts with oysterav<0.6.0, and the AUR has oysterav 0.5.0-1"])
        self.assertEqual(have["uq"], "0.3.41-1")


if __name__ == "__main__":
    unittest.main()
