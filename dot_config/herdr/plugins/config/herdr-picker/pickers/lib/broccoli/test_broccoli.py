import base64
import json
import os
import sys
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
os.environ["BROCCOLI_FIXTURES"] = str(HERE / "fixtures")

from broccoli import pylon
from broccoli.explain import page
from broccoli.payloads import decoded
from broccoli.radar import failure_rows
from broccoli.temporal import executions, history
from broccoli.views import call_id, workflow_rows

MAIN = "call-cm7q2x9k1-org_4f8a2c"


def b64(text):
    return base64.b64encode(text.encode()).decode()


class Payloads(unittest.TestCase):
    def test_base64_payload_holding_json_in_a_string_decodes_twice(self):
        inner = json.dumps(json.dumps({"callId": "c1", "notes": "a\nb"}))
        wrapped = {"payloads": [{"metadata": {"encoding": b64("json/plain")}, "data": b64(inner)}]}
        self.assertEqual(decoded(wrapped), {"callId": "c1", "notes": "a\nb"})

    def test_null_payload_is_none_and_plain_text_stays_text(self):
        null = {"payloads": [{"metadata": {"encoding": b64("binary/null")}}]}
        self.assertIsNone(decoded(null))
        self.assertEqual(decoded({"note": "{not json"}), {"note": "{not json"})


class Calls(unittest.TestCase):
    def test_call_id_comes_from_a_link_a_bare_id_or_a_workflow_id(self):
        self.assertEqual(call_id("https://app.broccoli.com/dashboard/call/cm7q2x9k1"), "cm7q2x9k1")
        self.assertEqual(call_id(" cm7q2x9k1\n"), "cm7q2x9k1")
        self.assertEqual(call_id(MAIN + "-rerun-missed-action"), "cm7q2x9k1")
        self.assertIsNone(call_id("hello there"))


class History(unittest.TestCase):
    def test_steps_follow_schedule_order_with_attempts_and_children(self):
        steps = history(MAIN)["steps"]
        self.assertEqual(
            [(s["kind"], s["name"], s["status"], s["attempts"]) for s in steps],
            [
                ("activity", "getOrgFromDB", "Completed", 1),
                ("activity", "getCallDetailsFromVoiceAgent", "Completed", 1),
                ("activity", "judgeMissedActions", "Completed", 1),
                ("activity", "syncCallToCrm", "Failed", 3),
                ("child", "customerSyncInteraction", "Completed", 1),
                ("child", "rerunMissedAction", "Started", 1),
            ],
        )
        self.assertEqual(steps[5]["child"], [MAIN + "-rerun-missed-action", "run-rerun-1"])

    def test_workflow_rows_pin_the_useful_parts_then_runs_then_steps(self):
        with tempfile.TemporaryDirectory() as folder:
            made = list(
                workflow_rows(history(MAIN, "run-main-1"), executions(prefix=MAIN), Path(folder))
            )
            labels = [(r.get("group"), r.get("label")) for r in made]
            settings = json.loads((Path(folder) / "settings.json").read_text())
        self.assertEqual(
            labels[:8],
            [
                (None, None),
                ("pinned", "transcript with tool calls"),
                ("pinned", "settings at call time"),
                ("pinned", "missed actions"),
                ("pinned", "failures (2)"),
                ("pinned", "explain this call"),
                ("runs", "rerun-missed-action Completed"),
                ("runs", "other run 2026-10-18T09:00 Completed"),
            ],
        )
        self.assertEqual(settings["getOrgFromDB"]["settings"]["followUpHours"], 24)
        self.assertEqual(
            made[-1]["enter"],
            {
                "next": {
                    "picker": "workflow",
                    "args": [MAIN + "-rerun-missed-action", "run-rerun-1"],
                }
            },
        )

    def test_explain_page_marks_the_failure_on_the_timeline(self):
        lines = page(history(MAIN))
        failed = lines.index("  15:00:12  ✗ syncCallToCrm after 3 attempts")
        self.assertEqual(
            lines[failed + 1], "            <- CRM rejected the record: 422 missing contact email"
        )
        self.assertIn("  - Send the price list by email", lines)


class Radar(unittest.TestCase):
    def test_failures_group_by_the_innermost_error(self):
        failed = executions(prefix="call-", status="Failed")
        groups = [r["group"] for r in failure_rows(failed, history)]
        self.assertEqual(
            groups,
            ["2× CRM rejected the record: 422 missing contact email"] * 2
            + ["1× voice agent returned no transcript"],
        )


class TeamStats(unittest.TestCase):
    def test_stats_for_a_fixed_afternoon(self):
        now = datetime(2026, 10, 19, 16, 0, tzinfo=timezone(timedelta(hours=-6)))
        labels = [r["label"] for r in pylon.stats_rows(pylon.issues(), now) if "label" in r]
        self.assertEqual(
            labels,
            [
                "Opened today: 2",
                "Closed today: 1",
                "Open, alex: 1",
                "Open, sam: 1",
                "Open, unassigned: 1",
                "Missed deadlines: 1",
                "#1042 Price list email never sent (alex)",
                "Oldest open: #1039 Agent mispronounces street names, 7 days, unassigned",
            ],
        )


if __name__ == "__main__":
    unittest.main()
