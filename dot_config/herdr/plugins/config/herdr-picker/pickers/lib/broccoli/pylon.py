import json
import shlex
import urllib.parse
import urllib.request
from collections import Counter
from datetime import UTC, datetime, timedelta

from broccoli.access import fixtures, secret
from rows import row

SERVICE = "Pylon API Token (Broccoli)"
RECORD = "Pylon :: API Token (Broccoli)"
API = "https://api.usepylon.com/issues"


def issues(days=30):
    """Issues created in the last days, read only: this module never writes to Pylon."""
    folder = fixtures()
    if folder:
        return json.loads((folder / "pylon-issues.json").read_text())["data"]
    token, now, found, cursor = secret(SERVICE, RECORD), datetime.now(UTC), [], None
    while True:
        asked = {
            "start_time": (now - timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "end_time": now.strftime("%Y-%m-%dT%H:%M:%SZ"),
            **({"cursor": cursor} if cursor else {}),
        }
        request = urllib.request.Request(
            f"{API}?{urllib.parse.urlencode(asked)}", headers={"Authorization": f"Bearer {token}"}
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            page = json.load(response)
        found += page.get("data", [])
        cursor = (page.get("pagination") or {}).get("cursor")
        if not (page.get("pagination") or {}).get("has_next_page") or not cursor:
            return found


def when(text):
    return datetime.fromisoformat(text).astimezone() if text else None


def deadline(issue):
    """The nearest SLA breach time Pylon reports for an issue, if any."""
    times = [
        issue.get("first_response_breach_time"),
        issue.get("resolution_breach_time"),
        (issue.get("next_reply") or {}).get("breach_time"),
        (issue.get("follow_up") or {}).get("breach_time"),
    ]
    return min((when(time) for time in times if time), default=None)


def is_open(issue):
    return issue.get("state") != "closed"


def person(issue):
    return (issue.get("assignee") or {}).get("email", "unassigned").split("@")[0]


def title(issue):
    return f"#{issue.get('number')} {issue.get('title', '')}"


def stats_rows(found, now):
    today = now.date()
    opened = [i for i in found if when(i.get("created_at")).date() == today]
    closed = [
        i
        for i in found
        if not is_open(i)
        and i.get("resolution_time")
        and when(i["resolution_time"]).date() == today
    ]
    still = sorted((i for i in found if is_open(i)), key=lambda i: i.get("created_at"))
    missed = [i for i in still if deadline(i) and deadline(i) < now]
    lines = [
        f"Opened today: {len(opened)}",
        f"Closed today: {len(closed)}",
        *(
            f"Open, {name}: {count}"
            for name, count in sorted(
                Counter(map(person, still)).items(), key=lambda pair: (-pair[1], pair[0])
            )
        ),
        f"Missed deadlines: {len(missed)}",
        *(f"  {title(i)} ({person(i)})" for i in missed),
    ]
    if still:
        age = (now - when(still[0]["created_at"])).days
        lines.append(f"Oldest open: {title(still[0])}, {age} days, {person(still[0])}")
    summary = "\n".join(lines)
    yield {"header": f"Team stats for {today.isoformat()}; Enter copies them all"}
    for line in lines:
        yield row(line, {"copy": summary}, summary)


def tab_for(issue, tabs):
    mark = f"#{issue.get('number')}"
    return next((tab for tab in tabs if mark in tab.get("label", "").split()), None)


def ticket_rows(found, tabs, workspace, now):
    """The open queue, nearest deadline first; Enter goes to the ticket's herdr tab or makes one."""
    queue = sorted(
        (i for i in found if is_open(i)),
        key=lambda i: deadline(i) or datetime.max.replace(tzinfo=UTC),
    )
    for issue in queue:
        due = deadline(issue)
        left = "no deadline" if not due else ("overdue" if due < now else f"due {due:%a %H:%M}")
        tab = tab_for(issue, tabs)
        if tab:
            enter = {"herdr": {"focus_tab": tab["tab_id"]}}
        else:
            place = f"--workspace {shlex.quote(workspace)} " if workspace else ""
            label = shlex.quote(" ".join(title(issue).split())[:40])
            enter = {"run": f"herdr tab create {place}--label {label} --focus"}
        preview = (
            f"{title(issue)}\nstate: {issue.get('state')}\nassignee: {person(issue)}\n"
            f"{left}\n{issue.get('link', '')}\n\n"
            + ("Enter focuses its herdr tab." if tab else "Enter opens a herdr tab for it.")
        )
        yield row(
            f"{left:>13}  {title(issue)}  {person(issue)}", enter, preview, issue.get("state")
        )
