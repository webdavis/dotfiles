import json
import os
import subprocess
import sys
from pathlib import Path

HOME = str(Path.home())


def row(label, enter, preview=None, group=None):
    made = {"label": " ".join(str(label).split()) or "(empty)", "enter": enter}
    if preview is not None:
        made["preview"] = preview
    if group:
        made["group"] = group
    return made


def emit(label, enter, preview=None, group=None):
    print(json.dumps(row(label, enter, preview, group), ensure_ascii=False), flush=True)


def notice(text):
    emit(text, "none", text)


def edit(path, line=None):
    return {"edit": {"path": str(path), "line": line} if line else {"path": str(path)}}


def home(path):
    path = str(path)
    return "~" + path[len(HOME) :] if path == HOME or path.startswith(HOME + "/") else path


def run(argv, timeout=30):
    try:
        done = subprocess.run(argv, check=False, capture_output=True, text=True, timeout=timeout)
    except (OSError, subprocess.TimeoutExpired) as failure:
        raise RuntimeError(f"{Path(argv[0]).name} is unavailable or timed out.") from failure
    if done.returncode:
        error = done.stderr.strip().splitlines()
        raise RuntimeError(f"{Path(argv[0]).name}: {error[-1] if error else 'failed'}")
    return done.stdout


def main(sources):
    """Prints the rows of the chosen source; any failure becomes one row saying why."""
    name = os.environ.get("HERDR_PICKER_SOURCE") or next(iter(sources))
    try:
        if name not in sources:
            raise RuntimeError(f"No source is named {name}.")
        sources[name]()
    except Exception as failure:  # noqa: BLE001
        notice(str(failure) or type(failure).__name__)
    sys.exit(0)
