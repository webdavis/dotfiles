import os
import shlex
import subprocess
from pathlib import Path

from ._git import git, root


def preview(row):
    project = row.get("project")
    if row.get("changed"):
        relative = os.path.relpath(row["path"], project)
        sections = []
        for label, extra in [("Staged", ["--cached"]), ("Unstaged", [])]:
            diff = git(
                project,
                "diff",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                *extra,
                "--",
                relative,
            ).decode(errors="replace")
            if diff:
                sections.append(label + "\n" + diff)
        if sections:
            return "\n".join(sections)
    if row.get("blob"):
        return git(project, "cat-file", "blob", row["blob"]).decode(errors="replace")
    if row.get("commit"):
        return git(
            project,
            "show",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--stat",
            "--patch",
            row["commit"],
        ).decode(errors="replace")
    if row.get("stages"):
        parts = []
        for number, label in [("1", "Base"), ("2", "Ours"), ("3", "Theirs")]:
            blob = row["stages"].get(number)
            text = (
                git(project, "cat-file", "blob", blob).decode(errors="replace")
                if blob
                else "(stage absent)"
            )
            parts.append(f"{label}\n{text}")
        return "\n".join(parts)
    return None


def bindings(kind, state):
    keys = {}
    if kind in ("files", "directories", "contents"):
        keys.update({"alt-h": "Show/hide hidden files", "alt-s": "Path/modification time"})
    if kind == "contents":
        keys["alt-x"] = "Literal/regular expression"
        if state.get("worktree") or root(state["cwd"]):
            keys.update({"alt-u": "Tracked only/untracked only", "alt-l": "Committed line history"})
    if kind in ("commits", "reflog", "log-search", "historical"):
        keys["alt-s"] = "Newest/oldest first"
    if kind in ("commits", "reflog", "log-search", "stashes", "refs"):
        keys["alt-i"] = "Inspect in pager"
    return keys


def accept(kind, rows, key, state):
    toggles = {"alt-h": "hidden", "alt-u": "untracked", "alt-x": "regex", "alt-s": "sort"}
    if key in toggles:
        name = toggles[key]
        return {"type": "reload", "state": {name: not state.get(name)}}
    if not rows:
        return None
    row = rows[0]
    if key == "alt-l":
        project, path = row["project"], row["path"]
        relative = os.path.relpath(path, project)
        current = Path(path).read_bytes()
        committed = git(project, "show", "HEAD:" + relative, allowed=(0, 128))
        if committed != current:
            raise RuntimeError(
                "The file differs from HEAD. Select a committed version before tracing line history."
            )
        number = row["line"]
        return {
            "type": "command",
            "argv": ["git", "-C", project, "log", "-L", f"{number},{number}:{relative}"],
        }
    if key == "alt-i":
        with open("/dev/tty", "r+") as tty:
            subprocess.run(
                ["git", "--paginate", "-C", row["project"], "show", row["commit"]],
                stdin=tty,
                stdout=tty,
                stderr=tty,
                check=True,
            )
        return {"type": "reload"}
    if kind == "historical-files" and key == "enter":
        return {
            "type": "command",
            "text": shlex.join(["git", "-C", row["project"], "cat-file", "blob", row["blob"]])
            + " | nvim -R -",
        }
    if kind == "changed" and key == "enter":
        return {
            "type": "insert",
            "values": [os.path.relpath(r["path"], state["cwd"]) for r in rows],
        }
    if state.get("checkout") and key == "enter":
        target = row["value"]
        if target.startswith("refs/heads/"):
            target = target[len("refs/heads/") :]
        return {"type": "command", "argv": ["git", "-C", row["project"], "checkout", target]}
    if kind == "files" and state.get("return_kind") and key == "enter":
        return {
            "type": "next",
            "kind": state["return_kind"],
            "state": {"file": row["path"], "return_kind": None},
        }
    return None
