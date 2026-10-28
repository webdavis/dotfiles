import json
import re

from broccoli.history import RERUN, call_parts, troubled
from broccoli.payloads import found, readable
from rows import row

CALL = re.compile(r"(?:/call/|^)([A-Za-z0-9_]{6,})/?$")
TRANSCRIPT_STEP = "getCallDetailsFromVoiceAgent"
MISSED_STEPS = ("judgeMissedActions", "missedActionsMinion")
ICONS = {"Completed": "✓", "Failed": "✗", "Timed Out": "✗", "Terminated": "✗", "Canceled": "-"}


def call_id(text):
    """The call ID in a Broccoli call link, a bare call ID, or a call workflow ID."""
    text = (text or "").strip()
    if text.startswith("call-"):
        return call_parts(text)[0]
    hit = CALL.search(text)
    return hit.group(1) if hit else None


def open_workflow(found_run, step="workflow"):
    return {"next": {"picker": step, "args": [found_run["id"], found_run["run"]]}}


def call_rows(runs, group, step="workflow"):
    for run in runs:
        label = f"{run['start'][5:16].replace('T', ' ')}  {run['status']}  {run['id']}"
        preview = f"{run['id']}\nrun: {run['run']}\ntype: {run['type']}\nstarted: {run['start']}"
        yield row(label, open_workflow(run, step), preview, group)


def saved(folder, name, value):
    """A file the data picker opens; Enter there decodes, searches, and converts it."""
    path = folder / name
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False))
    return {"next": {"picker": "data-file", "args": [str(path)]}}


def step_preview(step):
    parts = [f"{step['name']}  {step['status']}  attempts {step['attempts']}  {step['time']}"]
    if step["failure"]:
        parts.append(f"Failure: {step['failure']}")
    parts += ["Input:", readable(step["input"]), "", "Output:", readable(step["output"])]
    return "\n".join(parts)


def outputs(flow, *names):
    return {step["name"]: step["output"] for step in flow["steps"] if step["name"] in names}


def pinned_rows(flow, folder):
    transcript = found(outputs(flow, TRANSCRIPT_STEP), "readableTranscriptWithToolCalls")
    if isinstance(transcript, str):
        path = folder / "transcript.txt"
        path.write_text(transcript)
        yield row("transcript with tool calls", {"edit": {"path": str(path)}}, transcript, "pinned")
    settings = {"workflow input": flow["input"], **outputs(flow, "getOrgFromDB")}
    yield row(
        "settings at call time" if flow["id"].startswith("call-") else "workflow input",
        saved(folder, "settings.json", settings),
        readable(settings),
        "pinned",
    )
    missed = outputs(flow, *MISSED_STEPS)
    if missed:
        yield row(
            "missed actions", saved(folder, "missed.json", missed), readable(missed), "pinned"
        )
    failed = [
        {
            "step": s["name"],
            "status": s["status"],
            "attempts": s["attempts"],
            "failure": s["failure"],
        }
        for s in flow["steps"]
        if troubled(s)
    ]
    if flow["failure"]:
        failed.insert(0, {"step": "workflow", "status": flow["status"], "failure": flow["failure"]})
    label = f"failures ({len(failed)})" if failed else "failures (none)"
    yield row(label, saved(folder, "failures.json", failed), readable(failed), "pinned")
    if flow["id"].startswith("call-"):
        yield row(
            "explain this call",
            open_workflow(flow, "explain"),
            "A readable page: the call as a timeline, the failure marked where it happened.",
            "pinned",
        )


def run_rows(flow, runs):
    parent = flow["id"].removesuffix(RERUN)
    for run in runs:
        if run["id"] == flow["id"] and run["run"] != flow["run"]:
            label = f"other run  {run['start'][:16]}  {run['status']}"
        elif run["id"] == parent + RERUN and flow["id"] == parent:
            label = f"rerun-missed-action  {run['status']}"
        elif run["id"] == parent and flow["id"] != parent:
            label = f"parent call workflow  {run['start'][:16]}  {run['status']}"
        else:
            continue
        yield row(label, open_workflow(run), f"{run['id']}\nrun: {run['run']}", "runs")


def step_rows(flow, folder):
    for number, step in enumerate(flow["steps"], 1):
        icon = ICONS.get(step["status"], "…")
        retried = f"  ×{step['attempts']}" if step["attempts"] > 1 else ""
        if step["kind"] == "child":
            child_id, child_run = step["child"]
            enter = open_workflow({"id": child_id, "run": child_run})
            label = f"{number:>2} ↳ {step['name']}  {icon} {step['status']}  {child_id}"
        else:
            enter = saved(
                folder,
                f"{number:02}-{step['name']}.json",
                {k: step[k] for k in ("name", "status", "attempts", "failure", "input", "output")},
            )
            label = f"{number:>2} {icon} {step['name']}  {step['status']}{retried}"
        yield row(label, enter, step_preview(step), "steps")


def workflow_rows(flow, runs, folder):
    folder.mkdir(parents=True, exist_ok=True, mode=0o700)
    yield {"header": f"{flow['id']}  {flow['status']}"}
    yield from pinned_rows(flow, folder)
    yield from run_rows(flow, runs)
    yield from step_rows(flow, folder)
