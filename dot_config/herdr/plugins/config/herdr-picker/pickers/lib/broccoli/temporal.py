import json
import os
import subprocess
from datetime import UTC, datetime, timedelta

from broccoli.access import all_set, fixtures, required, secret
from broccoli.history import execution, workflow

SERVICE = "Temporal API Key (Broccoli prod)"
RECORD = "Temporal :: API Key (Broccoli prod)"


def since(days=1):
    return datetime.now(UTC) - timedelta(days=days)


def query(prefix=None, after=None, status=None):
    if prefix and "'" in prefix:
        raise ValueError("A workflow ID holds no quote.")
    parts = [
        f"WorkflowId STARTS_WITH '{prefix}'" if prefix else "",
        f"StartTime > '{after.isoformat()}'" if after else "",
        f"ExecutionStatus = '{status}'" if status else "",
    ]
    return " AND ".join(part for part in parts if part)


def executions(prefix=None, after=None, status=None, limit=200):
    """Workflow runs, newest first, as {id, run, type, status, start}."""
    folder = fixtures()
    if folder:
        listed = json.loads((folder / "list.json").read_text())
        # Fixtures are a frozen day, so `after` is not applied to them.
        found = [
            item
            for item in map(execution, listed)
            if item["id"].startswith(prefix or "") and status in (None, item["status"])
        ]
        return found[:limit]
    listing = cli(
        "workflow", "list", "--query", query(prefix, after, status), "--limit", str(limit)
    )
    return [execution(item) for item in listing or []]


def history(workflow_id, run=""):
    folder = fixtures()
    if folder:
        name = (
            f"{workflow_id}@{run}"
            if (folder / "history" / f"{workflow_id}@{run}.json").exists()
            else workflow_id
        )
        shown = json.loads((folder / "history" / f"{name}.json").read_text())
    else:
        shown = cli(
            "workflow", "show", "--workflow-id", workflow_id, *(["--run-id", run] if run else [])
        )
    events = shown["events"] if isinstance(shown, dict) else shown
    return workflow(events, workflow_id, run)


def cli(*arguments):
    key, found = all_set(
        lambda: secret(SERVICE, RECORD), lambda: required("temporal", "address", "namespace")
    )
    environment = {
        **os.environ,
        "TEMPORAL_API_KEY": key,
        "TEMPORAL_ADDRESS": found["address"],
        "TEMPORAL_NAMESPACE": found["namespace"],
    }
    if found.get("codec_endpoint"):
        environment["TEMPORAL_CODEC_ENDPOINT"] = found["codec_endpoint"]
    try:
        done = subprocess.run(
            ["temporal", *arguments, "--output", "json", "--time-format", "iso"],
            capture_output=True,
            text=True,
            env=environment,
            timeout=60,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as failure:
        raise RuntimeError("The temporal CLI is unavailable or timed out.") from failure
    if done.returncode:
        lines = done.stderr.strip().splitlines()
        raise RuntimeError(f"temporal: {lines[-1] if lines else 'failed'}")
    return json.loads(done.stdout or "null")
