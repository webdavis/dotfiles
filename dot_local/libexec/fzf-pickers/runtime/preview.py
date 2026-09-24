import json
import shlex
from pathlib import Path

import git_files

from .controls import controls
from .records import clean, load


def preview(session, identity):
    state = load(session)
    if state.get("help"):
        return "\n".join(
            f"{key:25} {description}" for key, description in controls(state["kind"], state).items()
        )
    path = session / (identity + ".json")
    if not path.is_file():
        return "No selection"
    row = json.loads(path.read_text())
    content = row.get("detail", "")
    if not content and state["kind"] in git_files.KINDS:
        content = git_files.preview(row)
    if not content and row.get("path") and Path(row["path"]).is_file():
        try:
            with Path(row["path"]).open("rb") as stream:
                raw = stream.read(200000)
            if b"\0" in raw:
                content = "[Binary file]"
            else:
                lines = raw.decode(errors="replace").splitlines()
                number = row.get("line", 1)
                content = "\n".join(
                    f"{i + 1:5} {line}"
                    for i, line in enumerate(lines)
                    if max(0, number - 15) <= i < number + 150
                )
        except OSError as exc:
            content = str(exc)
    if not content:
        content = shlex.join(row["argv"]) if row.get("argv") else str(row.get("value", ""))
    return "\n".join(
        [
            clean(row.get("label", "")),
            "",
            *[clean(line) for line in str(content).splitlines()[:500]],
        ]
    )
