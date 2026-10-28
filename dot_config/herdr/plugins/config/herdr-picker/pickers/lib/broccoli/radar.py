from collections import defaultdict

from broccoli.history import RERUN, call_parts, troubled
from broccoli.payloads import found, readable
from broccoli.views import open_workflow
from rows import row


def trouble(flow):
    first = next((step for step in flow["steps"] if troubled(step) and step["failure"]), None)
    message = flow["failure"] or (first and first["failure"]) or "no message"
    return message.split(" <- ")[-1]


def failure_rows(failed, history_of):
    """Failed call workflows grouped by their innermost error, the biggest group first."""
    grouped = defaultdict(list)
    for run in failed:
        flow = history_of(run["id"], run["run"])
        grouped[trouble(flow)].append((run, flow))
    for error, items in sorted(grouped.items(), key=lambda pair: -len(pair[1])):
        orgs = sorted({call_parts(run["id"])[1] for run, _ in items})
        group = f"{len(items)}× {error[:60]}"
        for run, flow in items:
            preview = (
                f"{error}\n\n{len(items)} workflows, orgs: {', '.join(orgs)}\n\n"
                f"{run['id']}\nstarted {run['start']}\n\nTroubled steps:\n"
                + "\n".join(
                    f"  {s['name']} ×{s['attempts']}: {s['failure']}"
                    for s in flow["steps"]
                    if troubled(s)
                )
            )
            label = f"{call_parts(run['id'])[1]}  {run['start'][11:16]}  {run['id']}"
            yield row(label, open_workflow(run), preview, group)


def missed_rows(runs, history_of):
    """Today's calls with a rerun-missed-action child, with what was missed and why."""
    for run in runs:
        if not run["id"].endswith(RERUN):
            continue
        flow = history_of(run["id"], run["run"])
        action = found(flow["input"], "missedAction", "missedActions", "action")
        reasoning = found(flow["input"], "reasoning")
        summary = action if isinstance(action, str) else readable(action)
        label = f"{call_parts(run['id'])[0]}  {' '.join(str(summary).split())[:70]}"
        preview = (
            f"Missed: {summary}\n\nWhy: {reasoning}\n\n{run['id']}\n\n{readable(flow['input'])}"
        )
        yield row(label, open_workflow(run), preview)
