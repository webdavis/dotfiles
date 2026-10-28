from broccoli.payloads import decoded

RERUN = "-rerun-missed-action"
CLOSING = {
    "activitytaskcompleted": "Completed",
    "activitytaskfailed": "Failed",
    "activitytasktimedout": "Timed Out",
    "activitytaskcanceled": "Canceled",
    "childworkflowexecutioncompleted": "Completed",
    "childworkflowexecutionfailed": "Failed",
    "childworkflowexecutiontimedout": "Timed Out",
    "childworkflowexecutioncanceled": "Canceled",
    "childworkflowexecutionterminated": "Terminated",
}


def kind(event):
    """The event type in one spelling, whether the CLI wrote EVENT_TYPE_X_Y or EventTypeXY."""
    name = event.get("eventType", "").removeprefix("EVENT_TYPE_").removeprefix("EventType")
    return name.replace("_", "").lower()


def attributes(event):
    return next((value for key, value in event.items() if key.endswith("EventAttributes")), {})


def status_word(status):
    return str(status or "").removeprefix("WORKFLOW_EXECUTION_STATUS_").replace("_", " ").title()


def failure_text(failure):
    messages = []
    while isinstance(failure, dict):
        messages.append(failure.get("message", ""))
        failure = failure.get("cause")
    return " <- ".join(message for message in messages if message) or None


def call_parts(workflow_id):
    """The call and org IDs in call-<callId>-<orgId>; assumes the call ID holds no hyphen."""
    rest = workflow_id.removeprefix("call-").removesuffix(RERUN)
    call, _, org = rest.partition("-")
    return call, org


def execution(item):
    return {
        "id": item["execution"]["workflowId"],
        "run": item["execution"].get("runId", ""),
        "type": item.get("type", {}).get("name", ""),
        "status": status_word(item.get("status")),
        "start": item.get("startTime", ""),
    }


def workflow(events, workflow_id, run=""):
    """A workflow history as its input, result, and steps in the order they were scheduled."""
    made = {
        "id": workflow_id,
        "run": run,
        "type": "",
        "status": "Running",
        "input": None,
        "result": None,
        "failure": None,
        "steps": [],
    }
    steps = {}
    for event in events:
        name, attrs = kind(event), attributes(event)
        origin = str(attrs.get("scheduledEventId") or attrs.get("initiatedEventId") or "")
        step = steps.get(origin)
        if name == "workflowexecutionstarted":
            made["type"] = attrs.get("workflowType", {}).get("name", "")
            made["input"] = decoded(attrs.get("input"))
        elif name == "workflowexecutioncompleted":
            made["status"], made["result"] = "Completed", decoded(attrs.get("result"))
        elif name in ("workflowexecutionfailed", "workflowexecutiontimedout"):
            made["status"] = "Failed" if name.endswith("failed") else "Timed Out"
            made["failure"] = failure_text(attrs.get("failure"))
        elif name in ("activitytaskscheduled", "startchildworkflowexecutioninitiated"):
            child = name.startswith("startchild")
            step = {
                "kind": "child" if child else "activity",
                "name": (attrs.get("workflowType") if child else attrs.get("activityType", {})).get(
                    "name", ""
                ),
                "status": "Started" if child else "Scheduled",
                "attempts": 1,
                "time": event.get("eventTime", ""),
                "input": decoded(attrs.get("input")),
                "output": None,
                "failure": None,
                "child": [attrs.get("workflowId", ""), ""] if child else None,
            }
            steps[str(event.get("eventId"))] = step
            made["steps"].append(step)
        elif step and name == "activitytaskstarted":
            step["status"], step["attempts"] = "Running", int(attrs.get("attempt", 1))
            step["failure"] = failure_text(attrs.get("lastFailure"))
        elif step and name == "childworkflowexecutionstarted":
            step["child"][1] = attrs.get("workflowExecution", {}).get("runId", "")
        elif step and name in CLOSING:
            step["status"] = CLOSING[name]
            step["output"] = decoded(attrs.get("result"))
            step["failure"] = failure_text(attrs.get("failure")) or step["failure"]
    return made


def troubled(step):
    return step["status"] in ("Failed", "Timed Out", "Terminated") or step["attempts"] > 1
