import os
import re
import subprocess
from pathlib import Path

from ._git import git, strings
from .filesystem import file_row


def tracked(project, state, *, untracked=False):
    cwd = state["cwd"]
    args = ["ls-files", "-z", "--cached"]
    if untracked:
        args += ["--others", "--exclude-standard"]
    return [
        file_row(os.path.join(project, p), cwd)
        for p in dict.fromkeys(strings(git(project, *args)))
        if ".git" not in Path(p).parts
    ]


def changed(project, state):
    cwd = state["cwd"]
    names = strings(git(project, "diff", "--name-only", "--no-renames", "-z"))
    names += strings(git(project, "diff", "--cached", "--name-only", "--no-renames", "-z"))
    names += strings(git(project, "ls-files", "-z", "--others", "--exclude-standard"))
    return [
        file_row(
            os.path.join(project, p), cwd, action="insert", value=p, project=project, changed=True
        )
        for p in dict.fromkeys(names)
    ]


def worktrees(project, state):
    records = strings(git(project, "worktree", "list", "--porcelain", "-z"))
    return [
        {
            "id": p,
            "label": p,
            "value": p,
            "action": "next",
            "next": "contents",
            "state": {"worktree": p, "query": "", "hidden": True},
        }
        for record in records
        if record.startswith("worktree ") and (p := record[9:]) != project
    ]


def ignored(project, state):
    cwd = state["cwd"]
    names = git(project, "ls-files", "--others", "--ignored", "--exclude-standard", "-z")
    result = subprocess.run(
        ["git", "check-ignore", "-z", "-v", "--stdin"],
        cwd=project,
        input=names,
        capture_output=True,
    )
    if result.returncode not in (0, 1):
        raise RuntimeError(result.stderr.decode(errors="replace").strip())
    values = strings(result.stdout)
    return [
        file_row(
            os.path.abspath(os.path.join(project, values[i])),
            cwd,
            label=values[i + 3],
            id=values[i + 3],
            line=int(values[i + 1]),
            detail=f"{values[i + 3]}\nRule: {values[i + 2]}\nSource: {values[i]}:{values[i + 1]}",
        )
        for i in range(0, len(values) - 3, 4)
    ]


def conflicts(project, state):
    cwd = state["cwd"]
    stages = {}
    for record in git(project, "ls-files", "-u", "-z").split(b"\0"):
        if record:
            metadata, name = record.split(b"\t", 1)
            mode, blob, stage = metadata.decode().split()
            stages.setdefault(os.fsdecode(name), {})[stage] = blob
    return [
        file_row(os.path.join(project, p), cwd, stages=s, project=project)
        for p, s in stages.items()
    ]


def configuration(project, state):
    values = strings(git(project, "config", "--null", "--list", "--show-origin", "--show-scope"))
    rows = []
    for i in range(0, len(values) - 2, 3):
        scope, origin, entry = values[i : i + 3]
        name, _, value = entry.partition("\n")
        sensitive = any(
            word in name.lower()
            for word in ("password", "token", "secret", "extraheader", "credential")
        )

        def redact(text):
            text = re.sub(r"(?i)(https?://)[^/\s@]+@", r"\1[redacted]@", text)
            return re.sub(
                r"(?i)([?&](?:token|access_token|password|api_key|apikey)=)[^&\s]+",
                r"\1[redacted]",
                text,
            )

        safe_name = redact(name)
        shown = "[redacted]" if sensitive else redact(value)
        row = {
            "id": f"{scope}:{origin}:{name}",
            "label": f"{scope}  {safe_name} = {shown}",
            "detail": f"{origin}\n{safe_name} = {shown}",
            "value": safe_name,
            "action": "insert",
        }
        if origin.startswith("file:"):
            row.update(path=os.path.abspath(os.path.join(project, origin[5:])), action="edit")
        rows.append(row)
    return rows
