import json
import os
import re
from pathlib import Path

import developer.capture as recording
import developer.diagnostics.locations as diagnostics_locations
import developer.discovery.saved as discovery_saved
import developer.discovery.xcode as discovery_xcode
import developer.execution as execution
import developer.projects as project_paths
import developer.rows as row_model
import developer.tasks.just as tasks_just
import developer.tasks.project as tasks_project


def diagnostics(cwd, state, checks=True):
    result = []
    explicit = state.get("diagnostics_file") or state.get("quickfix_file")
    exported = (
        Path(os.environ.get("XDG_STATE_HOME", str(Path.home() / ".local/state")))
        / "fzf-pickers/quickfix.json"
    )
    if not checks and not explicit and not os.environ.get("NVIM") and exported.is_file():
        explicit = str(exported)
    if explicit:
        text = project_paths.read_text(explicit)
        try:
            data = json.loads(text)
        except json.JSONDecodeError:
            result.extend(diagnostics_locations.diagnostic_rows(text, cwd))
        else:
            if isinstance(data, dict) and "stdout" in data:
                result.extend(
                    diagnostics_locations.diagnostic_rows(
                        data["stdout"] + "\n" + data.get("stderr", ""),
                        data.get("cwd", cwd),
                    )
                )
                result.insert(
                    0,
                    row_model.notice(
                        f"Saved check exited {data.get('exit_code', 'unknown')}",
                        data.get("recorded_at", ""),
                    ),
                )
            else:
                result.extend(diagnostics_locations.quickfix_rows(data, cwd))
    else:
        try:
            result.extend(diagnostics_locations.current_quickfix(cwd))
        except execution.ProviderError as error:
            result.append(row_model.notice("Neovim quickfix unavailable", str(error)))
        for path in sorted(recording.saved_dir(cwd, state).glob("diagnostics-*.json")):
            data = json.loads(project_paths.read_text(path))
            if data.get("framework", "").startswith("xcode-list-") and not data.get("exit_code"):
                result.extend(discovery_saved.xcode_list_rows(data, "diagnostics"))
                continue
            result.append(
                row_model.notice(
                    f"{data.get('framework', 'check')}: exit {data.get('exit_code', 'unknown')}",
                    data.get("recorded_at", ""),
                )
            )
            result.extend(
                diagnostics_locations.diagnostic_rows(
                    data.get("stdout", "") + "\n" + data.get("stderr", ""),
                    data.get("cwd", cwd),
                )
            )
    if checks:
        result.extend(project_paths.nested_projects(cwd, "diagnostics"))
        swift = project_paths.nearest(cwd, ["Package.swift"])
        rust = project_paths.nearest(cwd, ["Cargo.toml"])
        if swift:
            result.append(
                recording.capture_row(
                    "Run Swift build and save diagnostics",
                    ["swift", "build", "--package-path", str(swift.parent)],
                    swift.parent,
                    state,
                    "swift",
                    "diagnostics",
                )
            )
        if rust:
            result.append(
                recording.capture_row(
                    "Run Cargo check and save diagnostics",
                    [
                        "cargo",
                        "check",
                        "--manifest-path",
                        str(rust),
                        "--message-format=json",
                    ],
                    rust.parent,
                    state,
                    "rust",
                    "diagnostics",
                )
            )
        for item in tasks_project.project_tasks(cwd, "diagnostics"):
            if item.get("text"):
                result.append(
                    recording.capture_row(
                        item["label"] + " (save diagnostics)",
                        ["bash", "-c", item["text"]],
                        cwd,
                        state,
                        item["id"],
                        "diagnostics",
                    )
                )
            else:
                result.append(item)
        if project_paths.nearest(cwd, ["justfile", "Justfile", ".justfile"]):
            for item in tasks_just.just_recipes(cwd):
                if re.search(r"lint|check|build", item["label"], re.IGNORECASE):
                    result.append(
                        recording.capture_row(
                            item["label"] + " (save diagnostics)",
                            item["argv"],
                            cwd,
                            state,
                            item["id"],
                            "diagnostics",
                        )
                        | {"parameters": item.get("parameters", [])}
                    )
        result.extend(discovery_xcode.xcode_projects(cwd, state, "diagnostics"))
    return result or [
        row_model.notice(
            "No saved results",
            "Use the repository diagnostics picker to prepare a check, or populate the current Neovim quickfix list.",
        )
    ]
