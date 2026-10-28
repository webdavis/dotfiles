from broccoli.history import call_parts, troubled
from broccoli.payloads import found
from broccoli.views import ICONS, MISSED_STEPS, outputs
from rows import row


def page(flow):
    """The call as a timeline with the failure marked where it happened, then why."""
    call, org = call_parts(flow["id"])
    lines = [f"Call {call}  org {org}  {flow['status']}", "", "Timeline"]
    first_failure = None
    for step in flow["steps"]:
        mark = ICONS.get(step["status"], "…")
        arrow = "↳ " if step["kind"] == "child" else ""
        retried = f" after {step['attempts']} attempts" if step["attempts"] > 1 else ""
        lines.append(f"  {step['time'][11:19]}  {mark} {arrow}{step['name']}{retried}")
        if troubled(step) and step["failure"]:
            lines.append(f"            <- {step['failure']}")
            first_failure = first_failure or step
    lines += ["", "Cause"]
    if flow["failure"]:
        lines.append(f"  The workflow {flow['status'].lower()}: {flow['failure']}")
    if first_failure:
        lines.append(f"  First trouble at {first_failure['name']}: {first_failure['failure']}")
    if not (flow["failure"] or first_failure):
        lines.append("  Nothing failed in this run.")
    lines += ["", "Missed actions"]
    missed = outputs(flow, *MISSED_STEPS)
    actions = found(missed, "missedActions", "missedAction")
    reasoning = found(missed, "reasoning")
    if actions:
        listed = actions if isinstance(actions, list) else [actions]
        lines += [
            f"  - {item if isinstance(item, str) else found(item, 'action', 'name') or item}"
            for item in listed
        ]
        if reasoning:
            lines.append(f"  Why: {reasoning}")
    else:
        lines.append("  None found.")
    lines += ["", "Similar tickets", "  Needs Pylon; search there for the cause above."]
    return lines


def page_rows(flow):
    lines = page(flow)
    text = "\n".join(lines)
    yield {"header": f"Explain {flow['id']}"}
    for line in lines:
        if line.strip():
            yield row(line, {"copy": text}, text)
