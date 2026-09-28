import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

import developer.projects as project_paths
import developer.rows as row_model


def saved_dir(cwd, state):
    home = Path(
        state.get(
            "state_home",
            os.environ.get("XDG_STATE_HOME", str(Path.home() / ".local/state")),
        )
    )
    manifest = project_paths.nearest(
        cwd,
        [
            "Cargo.toml",
            "Package.swift",
            "package.json",
            "go.mod",
            "pytest.ini",
            "pyproject.toml",
        ],
    )
    scope = manifest.parent if manifest else project_paths.project_root(cwd)
    return home / "fzf-pickers" / hashlib.sha256(str(scope).encode()).hexdigest()[:20]


def capture_row(label, argv, cwd, state, framework, category="tests"):
    output = saved_dir(cwd, state) / f"{category}-{framework}.json"
    capture_argv = [
        "python3",
        str(Path(__file__).resolve().parent),
        "capture",
        "--category",
        category,
        "--output",
        str(output),
        "--cwd",
        str(cwd),
        "--framework",
        framework,
        "--",
        *argv,
    ]
    detail = "Run explicitly to save results for this picker. This can build code, run project scripts and access the network."
    return row_model.command(
        ("capture", str(cwd), framework, argv),
        label,
        capture_argv,
        detail,
        source_argv=argv,
        framework=framework,
        capture=True,
        output=str(output),
    )


def capture(category, output, cwd, framework, argv):
    destination = Path(output)
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    result = subprocess.run(
        argv, cwd=cwd, capture_output=True, text=True, errors="replace", check=False
    )
    record = {
        "category": category,
        "cwd": cwd,
        "framework": framework,
        "argv": argv,
        "exit_code": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "recorded_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.fchmod(descriptor, 0o600)
    with os.fdopen(descriptor, "w") as handle:
        json.dump(record, handle, ensure_ascii=False)
    print(result.stdout, end="")
    print(result.stderr, end="", file=sys.stderr)
    print(
        f"\nSaved {category} results. Reopen the picker to inspect them. Exit: {result.returncode}",
        file=sys.stderr,
    )
    return result.returncode
