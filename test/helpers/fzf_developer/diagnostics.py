import io
import json
import os
import subprocess
import sys
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

import developer as dev
from developer import capture, execution, selection
from developer import rows as row_model
from developer.diagnostics import locations as diag_locations
from developer.tasks import just


def test_compiler_output_and_quickfix_locations(cwd, state):
    diagnostic = {
        "reason": "compiler-message",
        "message": {
            "level": "error",
            "message": "bad type",
            "spans": [
                {
                    "file_name": "src/a.rs",
                    "line_start": 4,
                    "column_start": 2,
                    "is_primary": True,
                }
            ],
        },
    }
    rows = diag_locations.diagnostic_rows(
        json.dumps(diagnostic) + "\nfoo.swift:8:3: warning: unused\nfatal: build failed",
        cwd,
    )
    assert rows[0]["line"] == 4 and rows[0]["path"] == str(cwd / "src/a.rs")
    assert dev.accept("diagnostics", rows[:2], "enter", state) is None
    assert all(item["action"] == "edit" for item in rows[:2])
    locations = diag_locations.quickfix_rows(
        [{"filename": "a", "lnum": 2}, {"filename": "b", "lnum": 4}], cwd
    )
    assert dev.accept("quickfix", locations, "enter", state) is None
    assert all(item["action"] == "edit" for item in locations)
    assert rows[1]["line"] == 8 and "unused" in rows[1]["detail"]
    assert "build failed" in rows[-1]["detail"] and not rows[-1].get("path")


def test_explicit_quickfix_file(cwd, state):
    quickfix = cwd / "quickfix.json"
    quickfix.write_text(
        json.dumps([{"filename": "src/a.rs", "lnum": 10, "col": 2, "text": "problem"}])
    )
    assert dev.collect("quickfix", dict(state, quickfix_file=str(quickfix)))[0]["line"] == 10


def test_editor_relative_locations(cwd, state):
    editor_root = cwd / "editor-project"
    shell_cwd = editor_root / "subdir"
    shell_cwd.mkdir(parents=True)
    original_run = execution.run

    def remote_expression(argv, _cwd):
        return original_run(
            [
                "nvim",
                "--headless",
                "--clean",
                "-n",
                "-i",
                "NONE",
                "-c",
                "call setqflist([{'filename': 'src/a.swift', 'lnum': 4}, {'text': 'unlocated warning'}])",
                "-c",
                "lua io.write(vim.fn.eval(" + json.dumps(argv[-1]) + "))",
                "-c",
                "qa!",
            ],
            editor_root,
        )

    with (
        patch.dict(os.environ, {"NVIM": "test-editor"}),
        patch.object(execution, "run", side_effect=remote_expression),
    ):
        editor_rows = diag_locations.current_quickfix(shell_cwd)
    assert editor_rows[0]["path"] == str(editor_root / "src/a.swift")
    assert "path" not in editor_rows[1]


def test_prepared_check_parameters(cwd, state):
    (cwd / "justfile").write_text("check thing:\n    echo {{thing}}\n")
    check_recipe = row_model.command(
        "check",
        "check",
        ["just", "--justfile", str(cwd / "justfile"), "check"],
        parameters=[{"name": "thing"}],
    )
    with (
        patch.object(just, "just_recipes", return_value=[check_recipe]),
        patch.object(diag_locations, "current_quickfix", return_value=[]),
    ):
        check = next(
            item
            for item in dev.collect("diagnostics", state)
            if item["label"] == "check (save diagnostics)"
        )
    assert check["parameters"][0]["name"] == "thing"
    with patch.object(selection, "prompt", return_value="a b"):
        accepted = dev.accept("diagnostics", [check], "enter", state)
    assert accepted["argv"][-2:] == ["check", "a b"]


def test_capture_failed_command(cwd, state):
    output = cwd / "check.json"
    terminal_output = io.StringIO()
    with redirect_stdout(terminal_output), redirect_stderr(terminal_output):
        status = capture.capture(
            "diagnostics",
            str(output),
            str(cwd),
            "rust",
            [sys.executable, "-c", 'print("fatal: broken"); raise SystemExit(3)'],
        )
    assert "fatal: broken" in terminal_output.getvalue()
    assert status == 3 and json.loads(output.read_text())["exit_code"] == 3
    assert output.stat().st_mode & 0o777 == 0o600


def test_prepared_capture_command_runs(cwd, state):
    command = capture.capture_row(
        "Save fixture diagnostics",
        [sys.executable, "-c", 'print("fixture output"); raise SystemExit(3)'],
        cwd,
        state,
        "fixture",
        "diagnostics",
    )
    result = subprocess.run(
        command["argv"],
        capture_output=True,
        text=True,
        timeout=3,
        env=dict(os.environ, PYTHONDONTWRITEBYTECODE="1"),
    )
    record = json.loads(Path(command["output"]).read_text())
    assert result.returncode == 3 and result.stdout == "fixture output\n"
    assert record["exit_code"] == 3 and record["stdout"] == result.stdout
