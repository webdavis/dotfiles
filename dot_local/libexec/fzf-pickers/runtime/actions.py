import json
import os
import shlex
import subprocess
import sys
from pathlib import Path

from .providers import module
from .records import load, save


def copy_text(text):
    if not text.strip():
        raise RuntimeError("No populated value to copy.")
    subprocess.run(["pbcopy"], input=text.encode(), check=True)


def values(rows):
    return [
        str(value)
        for row in rows
        for value in (
            row.get("value") if isinstance(row.get("value"), list) else [row.get("value", "")]
        )
    ]


def default_action(rows, key, state):
    if not rows:
        return None
    row = rows[0]
    if key == "alt-y":
        return {"type": "copy", "text": "\n".join(values(rows))}
    if key == "alt-q":
        located = [
            {
                "filename": r["path"],
                "lnum": r.get("line", 1),
                "text": r.get("text", r["label"]),
            }
            for r in rows
            if r.get("path")
        ]
        if not located:
            raise RuntimeError("No file locations in the selection.")
        target = Path.home() / ".local/state/fzf-pickers/quickfix.json"
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        save(target, located)
        target.chmod(0o600)
        expression = "setqflist(json_decode('" + json.dumps(located).replace("'", "''") + "'))"
        socket = os.environ.get("NVIM")
        if socket:
            subprocess.run(
                ["nvim", "--server", socket, "--remote-expr", expression],
                check=True,
                stdout=subprocess.DEVNULL,
            )
            return {"type": "reload"}
        return {
            "type": "command",
            "argv": ["nvim", "-c", "call " + expression, "-c", "copen"],
        }
    if key in ("ctrl-o", "ctrl-s", "alt-c") or row.get("action") in ("edit", "cd"):
        path = row.get("path")
        if not path:
            raise RuntimeError("This selection has no local path.")
        if key == "alt-c" or row.get("action") == "cd" and key == "enter":
            if len(rows) != 1:
                raise RuntimeError("Select one directory for this action.")
            directory = path if Path(path).is_dir() else str(Path(path).parent)
            return {"type": "command", "argv": ["cd", "--", directory]}
        if key == "ctrl-o":
            return {
                "type": "command",
                "argv": [
                    "open" if sys.platform == "darwin" else "xdg-open",
                    *dict.fromkeys(r["path"] for r in rows if r.get("path")),
                ],
            }
        argv = shlex.split(os.environ.get("EDITOR_CMD") or os.environ.get("EDITOR") or "nvim")
        if row.get("line"):
            argv += ["+" + str(int(row["line"]))]
        argv += ["--", *dict.fromkeys(r["path"] for r in rows if r.get("path"))]
        return {"type": "command", "argv": (["sudo"] if key == "ctrl-s" else []) + argv}
    action = row.get("action", "insert")
    if action == "none":
        return {"type": "reload"}
    if action == "next":
        return {"type": "next", "kind": row["next"], "state": row.get("state", {})}
    if action == "command":
        return {
            "type": "command",
            **{name: row[name] for name in ("argv", "text") if name in row},
        }
    if action == "copy":
        return {"type": "copy", "text": "\n".join(values(rows))}
    if action == "insert":
        return {"type": "insert", "values": values(rows)}
    raise RuntimeError("This item has no selectable action.")


def choose_action(session, rows, key):
    state = load(session)
    return module(state["kind"]).accept(state["kind"], rows, key, state) or default_action(
        rows, key, state
    )
