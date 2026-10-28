import json
import re
import subprocess
import tempfile
from datetime import datetime
from pathlib import Path

from broccoli import pylon
from broccoli.explain import page_rows
from broccoli.history import RERUN
from broccoli.radar import failure_rows, missed_rows
from broccoli.temporal import executions, history, since
from broccoli.views import call_id, call_rows, workflow_rows
from rows import row

MENU = [
    (
        "Temporal: one call's post-call workflow",
        "calls",
        (
            "Steps, transcript, settings, missed actions and failures of one call.\n"
            "Starts from the call link or ID on the clipboard, or search the last day's calls."
        ),
    ),
    (
        "Explain this call",
        "explain-calls",
        "A readable timeline of one call, the failure marked where it happened.",
    ),
    (
        "Failure radar",
        "failures",
        "Failed call workflows in the last day, grouped by error, with their orgs.",
    ),
    (
        "Missed-action radar",
        "missed",
        "Today's calls that needed a rerun-missed-action, with what and why.",
    ),
    (
        "Team stats",
        "stats",
        "For the 4 PM meeting: opened and closed today, open by person, missed deadlines.",
    ),
    (
        "Ticket switcher",
        "tickets",
        "The open Pylon queue by deadline; Enter goes to the ticket's herdr tab.",
    ),
]


def menu():
    for label, step, about in MENU:
        yield row(label, {"next": {"picker": step}}, about)
    yield row(
        "Reply edits (to be designed with you)",
        "none",
        "Tickets whose sent reply differs from /ticket-reply's draft.\n"
        "The interface is mocked and reviewed with you before it is built.",
    )


def clipboard():
    done = subprocess.run(["pbpaste"], capture_output=True, text=True, check=False)
    return done.stdout


def calls(step="workflow"):
    recent = executions(prefix="call-", after=since(1))
    found = call_id(clipboard())
    if found:
        yield from call_rows(executions(prefix=f"call-{found}"), "clipboard", step)
    else:
        yield {"header": "No call link on the clipboard: type to search the last day's calls"}
    yield from call_rows(recent, "last day", step)


def scratch(workflow_id, run):
    """A private folder for the files the data picker opens; payloads are customer data."""
    root = Path(tempfile.gettempdir()) / "broccoli-picker"
    root.mkdir(mode=0o700, exist_ok=True)
    return root / re.sub(r"[^A-Za-z0-9_.-]", "_", f"{workflow_id}@{run}")


def workflow(workflow_id, run=""):
    runs = executions(prefix=workflow_id.removesuffix(RERUN))
    yield from workflow_rows(history(workflow_id, run), runs, scratch(workflow_id, run))


def explain(workflow_id, run=""):
    yield from page_rows(history(workflow_id, run))


def failures():
    failed = executions(prefix="call-", after=since(1), status="Failed", limit=50)
    if not failed:
        yield row("No call workflow failed in the last day", "none")
    yield from failure_rows(failed, history)


def missed():
    start = datetime.now().astimezone().replace(hour=0, minute=0, second=0, microsecond=0)
    yield from missed_rows(executions(prefix="call-", after=start, limit=1000), history)


def herdr(*arguments):
    try:
        done = subprocess.run(
            ["herdr", *arguments], capture_output=True, text=True, timeout=5, check=False
        )
        return json.loads(done.stdout)["result"]
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired):
        return {}


def stats():
    yield from pylon.stats_rows(pylon.issues(), datetime.now().astimezone())


def tickets():
    workspaces = herdr("workspace", "list").get("workspaces", [])
    broccoli = next(
        (w["workspace_id"] for w in workspaces if "broccoli" in w.get("label", "").lower()), None
    )
    tabs = [
        tab for tab in herdr("tab", "list").get("tabs", []) if tab.get("workspace_id") == broccoli
    ]
    yield from pylon.ticket_rows(pylon.issues(), tabs, broccoli, datetime.now().astimezone())


STEPS = {
    "menu": menu,
    "calls": calls,
    "explain-calls": lambda: calls("explain"),
    "workflow": workflow,
    "explain": explain,
    "failures": failures,
    "missed": missed,
    "stats": stats,
    "tickets": tickets,
}
