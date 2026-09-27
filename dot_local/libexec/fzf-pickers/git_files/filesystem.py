import base64
import json
import os
import subprocess
from pathlib import Path

from ._git import git, root, strings


def paths(cwd, state, directories=False):
    start = state.get("root", cwd)
    start = str(Path.home()) if start == "home" else start
    start = root(cwd) if start == "git" else start
    if not start:
        raise RuntimeError("This directory is outside a Git repository.")
    start = os.path.abspath(start)
    max_depth = 3 if start == "/" else 2 if start == str(Path.home()) else None
    for directory, dirs, files in os.walk(start):
        dirs[:] = [
            d for d in dirs if d != ".git" and (state.get("hidden") or not d.startswith("."))
        ]
        depth = len(Path(directory).relative_to(start).parts)
        if max_depth is not None and depth >= max_depth:
            dirs[:] = []
        for name in dirs if directories else files:
            if state.get("hidden") or not name.startswith("."):
                path = os.path.join(directory, name)
                patterns = state.get("file_patterns", [])
                if patterns and not any(
                    Path(path).match(pattern if "*" in pattern else "*" + pattern)
                    for pattern in patterns
                ):
                    continue
                yield path


def file_row(path, cwd, label=None, **extra):
    return {
        "id": path,
        "label": label or os.path.relpath(path, cwd),
        "path": path,
        "value": os.path.relpath(path, cwd),
        "action": "edit",
        **extra,
    }


def files(project, state, *, directories=False):
    cwd = state["cwd"]
    rows = [
        file_row(p, cwd, action="cd" if directories else "edit")
        for p in paths(cwd, state, directories)
    ]
    return sorted(
        rows,
        key=lambda r: os.path.getmtime(r["path"]) if state.get("sort") else r["path"],
        reverse=bool(state.get("sort")),
    )


def locations(project, state, *, parents=False):
    cwd = state["cwd"]
    names = [str(p) for p in Path(cwd).parents] if parents else state.get("paths", [])
    return [
        file_row(p, cwd, label=p, action="cd") for p in dict.fromkeys(names) if os.path.isdir(p)
    ]


def search(project, state):
    return contents(project or state["cwd"], state, bool(project))


def contents(project, state, in_git):
    query = state.get("query", "")
    if not query:
        return []
    if in_git:
        args = (
            ["ls-files", "-z", "--others", "--exclude-standard"]
            if state.get("untracked")
            else ["ls-files", "-z", "--cached"]
        )
        names = strings(git(project, *args))
    else:
        names = list(paths(project, {**state, "root": project}))
    names = [
        p
        for p in names
        if os.path.isfile(os.path.join(project, p))
        and ".git" not in Path(p).parts
        and (
            state.get("hidden", True)
            or not any(
                part.startswith(".")
                for part in Path(os.path.relpath(p, project) if os.path.isabs(p) else p).parts
            )
        )
    ]
    rows = []
    for offset in range(0, len(names), 128):
        args = ["rg", "--json", "--smart-case", "--hidden", "--no-ignore"]
        if not state.get("regex"):
            args.append("--fixed-strings")
        args += ["--", query, *names[offset : offset + 128]]
        result = subprocess.run(args, cwd=project, capture_output=True)
        if result.returncode not in (0, 1):
            raise RuntimeError(result.stderr.decode(errors="replace").strip())
        for line in result.stdout.splitlines():
            event = json.loads(line)
            if event["type"] != "match":
                continue
            data = event["data"]

            def content(value):
                return (
                    value["text"]
                    if "text" in value
                    else os.fsdecode(base64.b64decode(value["bytes"]))
                )

            path = os.path.abspath(os.path.join(project, content(data["path"])))
            number = data["line_number"]
            text = content(data["lines"]).rstrip("\n")
            rows.append(
                file_row(
                    path,
                    project,
                    label=f"{os.path.relpath(path, project)}:{number}: {text}",
                    id=f"{path}:{number}",
                    line=number,
                    text=text,
                    project=project,
                )
            )
    if state.get("sort"):
        rows.sort(key=lambda row: (os.path.getmtime(row["path"]), row["line"]), reverse=True)
    return rows
