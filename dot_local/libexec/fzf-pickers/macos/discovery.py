import hashlib
import json
import os
import plistlib
import re
from pathlib import Path

from . import bridge


def recent_files(state):
    scopes = state.get(
        "recent_scopes", [str(Path.home() / "Documents"), str(Path.home() / "Downloads")]
    )
    found = set()
    records = []
    query = 'kMDItemContentTypeTree == "public.data"'
    for scope in scopes:
        raw = bridge.run(
            ["mdfind", "-0", "-onlyin", str(Path(scope).expanduser()), query], binary=True
        )
        found.update(os.fsdecode(path) for path in raw.split(b"\0") if path)
    excluded = [Path(path).expanduser().resolve() for path in state.get("recent_exclude", [])]
    for name in found:
        path = Path(name)
        if not path.is_file() or any(path.is_relative_to(exclusion) for exclusion in excluded):
            continue
        try:
            metadata = plistlib.loads(bridge.run(["mdls", "-plist", "-", str(path)], binary=True))
            last = metadata.get("kMDItemLastUsedDate")
            order = last.timestamp() if hasattr(last, "timestamp") else path.stat().st_mtime
            detail = f"{path}\nKind: {metadata.get('kMDItemKind', 'unknown')}\nLast used: {last or 'not indexed'}"
            if metadata.get("kMDItemWhereFroms"):
                detail += "\nOrigin: " + "\n".join(metadata["kMDItemWhereFroms"])
            records.append(
                (
                    order,
                    {
                        "id": str(path),
                        "label": str(path),
                        "detail": detail,
                        "path": str(path),
                        "value": str(path),
                        "action": "command",
                        "argv": ["open", str(path)],
                    },
                )
            )
        except (OSError, plistlib.InvalidFileException):
            continue
    yield from (row for _, row in sorted(records, key=lambda pair: pair[0], reverse=True))


def shortcuts(state):
    for line in bridge.run(["shortcuts", "list", "--show-identifiers"]).splitlines():
        match = re.fullmatch(r"(.*) \(([0-9A-Fa-f-]{36})\)", line)
        if match:
            name, identifier = match.groups()
            yield {
                "id": identifier,
                "label": name,
                "detail": f"{name}\n{identifier}\n\nEnter prepares shortcuts run. Ctrl-O opens its editor.",
                "value": identifier,
                "action": "command",
                "argv": ["shortcuts", "run", identifier],
            }


def logs(state):
    yield {
        "id": "unified",
        "label": "Unified logs from the last hour",
        "detail": "Read the local unified log. May require additional permissions.",
        "value": "unified",
        "action": "next",
        "next": "macos-unified-logs",
    }
    yield {
        "id": "crashes",
        "label": "Crash and hang reports",
        "detail": "Browse accessible DiagnosticReports and prepare opening the original file in Console.",
        "value": "crashes",
        "action": "next",
        "next": "crashes",
    }


def unified_logs(state):
    argv = [
        "/usr/bin/log",
        "show",
        "--style",
        "ndjson",
        "--last",
        state.get("log_window", "1h"),
        "--no-pager",
    ]
    if state.get("log_predicate"):
        argv.extend(["--predicate", state["log_predicate"]])
    for index, line in enumerate(bridge.run(argv, timeout=60).splitlines()):
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        text = event.get("eventMessage", "")
        timestamp = event.get("timestamp", "")
        process = event.get("processImagePath", "")
        identity = hashlib.sha256(line.encode()).hexdigest()
        yield {
            "id": f"{identity}:{index}",
            "label": f"{timestamp} {Path(process).name} {text}",
            "detail": json.dumps(event, indent=2, ensure_ascii=False),
            "value": text,
            "action": "copy",
        }


def crashes(state):
    reports = []
    for folder in (
        Path.home() / "Library/Logs/DiagnosticReports",
        Path("/Library/Logs/DiagnosticReports"),
    ):
        if folder.is_dir():
            reports.extend(
                path
                for path in folder.rglob("*")
                if path.suffix in (".ips", ".spin", ".crash", ".hang")
            )
    for path in sorted(reports, key=lambda item: item.stat().st_mtime, reverse=True):
        try:
            detail = path.read_text(errors="replace")
        except OSError:
            detail = "Report unavailable. Check file permissions."
        yield {
            "id": str(path),
            "label": path.name,
            "detail": detail,
            "value": str(path),
            "path": str(path),
            "action": "command",
            "argv": ["open", "-a", "Console", str(path)],
        }


def accept(kind, rows, key, state):
    if rows and key == "ctrl-o":
        return {"type": "command", "argv": ["shortcuts", "view", rows[0]["id"]]}
    return None
