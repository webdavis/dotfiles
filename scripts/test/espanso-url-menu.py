import json
import subprocess
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent.parent / "Library/Application Support/espanso/scripts/executable_url-menu"
NO_BREAK_SPACE = "\u00a0"
THIN_SPACE = "\u2009"

MATCHES = [
    {"triggers": [",,uw"], "label": "mw", "replace": "https://www.wide.example/"},
    {"triggers": [",,ui"], "label": "il", "replace": "https://narrow.example/a"},
    {"triggers": [",,ua"], "label": "a b", "replace": "https://spaced.example/"},
    {"triggers": [";;ty"], "label": None, "replace": "Thank you"},
]


def run(*args):
    out = subprocess.run(["python3", SCRIPT, *args], input=json.dumps(MATCHES), capture_output=True, text=True, check=True)
    return out.stdout


def shown_address(row):
    return row.split(NO_BREAK_SPACE)[-1].lstrip(THIN_SPACE)


class UrlMenuRows(unittest.TestCase):
    def test_lists_short_addresses_sorted_by_label(self):
        shown = [shown_address(r) for r in run("rows").splitlines()]
        self.assertEqual(shown, ["spaced.example", "narrow.example/a", "wide.example"])

    def test_pads_narrow_labels_more_than_wide_ones(self):
        _, narrow, wide = run("rows").splitlines()
        self.assertGreater(narrow.count(NO_BREAK_SPACE), wide.count(NO_BREAK_SPACE))

    def test_rows_hold_no_breaking_space(self):
        for row in run("rows").splitlines():
            self.assertNotIn(" ", row)

    def test_pick_resolves_to_the_full_address(self):
        spaced, narrow, wide = run("rows").splitlines()
        self.assertEqual(run("url", spaced), "https://spaced.example/")
        self.assertEqual(run("url", narrow), "https://narrow.example/a")
        self.assertEqual(run("url", wide), "https://www.wide.example/")


if __name__ == "__main__":
    unittest.main()
