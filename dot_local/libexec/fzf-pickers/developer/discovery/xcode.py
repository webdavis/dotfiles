import hashlib
import json
import os
import shlex
from pathlib import Path

import developer.capture as recording
import developer.discovery.saved as discovery_saved
import developer.projects as project_paths
import developer.rows as row_model

from .. import selection


def xcode_projects(cwd, state, category):
    rows = []
    for directory, dirs, _ in os.walk(cwd):
        dirs[:] = sorted(
            name
            for name in dirs
            if name
            not in {".git", "node_modules", ".build", "target", ".venv", "venv", "__pycache__"}
        )
        for name in list(dirs):
            project = Path(directory) / name
            if project.suffix not in (".xcodeproj", ".xcworkspace"):
                continue
            dirs.remove(name)
            flag = "-workspace" if project.suffix == ".xcworkspace" else "-project"
            base = ["xcodebuild", flag, str(project)]
            identity = hashlib.sha256(str(project).encode()).hexdigest()[:12]
            rows.append(
                recording.capture_row(
                    f"Discover Xcode schemes: {name}",
                    base + ["-list", "-json"],
                    cwd,
                    state,
                    "xcode-list-" + identity,
                    category,
                )
            )
            for scheme in sorted(project.glob("**/*.xcscheme")):
                rows.append(
                    row_model.following(
                        "dev-xcode-scheme",
                        f"Xcode scheme: {scheme.stem}",
                        cwd=str(cwd),
                        xcode_base=base + ["-scheme", scheme.stem],
                        xcode_category=category,
                    )
                )
    return rows


def xcode_scheme(cwd, state):
    base = state["xcode_base"]
    identity = hashlib.sha256(shlex.join(base).encode()).hexdigest()[:12]
    category = state.get("xcode_category", "tests")
    destination = (
        ["-destination", state["xcode_destination"]] if state.get("xcode_destination") else []
    )
    if category == "diagnostics":
        return [
            recording.capture_row(
                "Build selected Xcode scheme and save diagnostics",
                base + destination + ["build"],
                cwd,
                state,
                "xcode-" + identity,
                "diagnostics",
            )
        ]
    plans = []
    plan_file = recording.saved_dir(cwd, state) / ("tests-xcode-plans-" + identity + ".json")
    if plan_file.is_file():
        data = json.loads(project_paths.read_text(plan_file))
        output = json.loads(data.get("stdout", "{}"))
        for plan in output.get("testPlans", []):
            name = plan.get("name") if isinstance(plan, dict) else plan
            if isinstance(name, str):
                plans.append(name)
    rows = [
        recording.capture_row(
            "Discover test plans for this scheme",
            base + ["-showTestPlans", "-json"],
            cwd,
            state,
            "xcode-plans-" + identity,
        )
    ]
    for plan in [None, *plans]:
        argv = base + destination + (["-testPlan", plan] if plan else []) + ["test"]
        title = plan or "scheme default plan"
        rows.append(row_model.command(("xcode-run", base, plan), "Run Xcode tests: " + title, argv))
        rows.append(
            recording.capture_row(
                "Discover Xcode test identifiers: " + title,
                argv
                + [
                    "-enumerate-tests",
                    "-test-enumeration-style",
                    "flat",
                    "-test-enumeration-format",
                    "json",
                    "-test-enumeration-output-path",
                    "-",
                ],
                cwd,
                state,
                "xcode-" + hashlib.sha256(shlex.join(argv).encode()).hexdigest()[:12],
            )
        )
    rows.extend(
        item
        for item in discovery_saved.saved_tests(cwd, state)
        if item.get("argv", [])[: len(base)] == base
    )
    return rows


def accept(rows, key, state):
    if key == "alt-d":
        destination = selection.prompt(
            "Destination, for example platform=macOS (blank for default)"
        )
        return {
            "type": "reload",
            "state": {} if destination is None else {"xcode_destination": destination},
        }
    return None
