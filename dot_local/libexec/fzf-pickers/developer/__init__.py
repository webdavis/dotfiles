import os
import sqlite3
from pathlib import Path

from . import execution, projects, rows, selection
from .agents import changes, context
from .data import json, sqlite
from .data import selection as data_selection
from .diagnostics import collection as diagnostics
from .discovery import tests, xcode
from .system import chezmoi, processes, provenance, services
from .tasks import just

__all__ = ["KINDS", "collect", "accept", "bindings"]

_COLLECTORS = {
    "just": lambda cwd, state: just.just_recipes(cwd),
    "processes": processes.collect,
    "chezmoi": lambda cwd, state: chezmoi.chezmoi_rows(),
    "provenance": provenance.collect,
    "tests": tests.test_rows,
    "dev-xcode-scheme": xcode.xcode_scheme,
    "diagnostics": diagnostics.diagnostics,
    "quickfix": lambda cwd, state: diagnostics.diagnostics(cwd, state, checks=False),
    "sqlite": lambda cwd, state: data_selection.collect(
        "sqlite", state, [".sqlite", ".sqlite3", ".db"], sqlite.sqlite_rows
    ),
    "json": lambda cwd, state: data_selection.collect("json", state, [".json"], json.json_rows),
    "services": lambda cwd, state: services.services(cwd),
    "agents": lambda cwd, state: [
        rows.following("dev-context", "Copy source excerpts with paths and lines"),
        rows.following("dev-diffs", "Copy staged or unstaged diff hunks"),
        rows.following("dev-failures", "Copy saved failures and diagnostics"),
        rows.following("dev-overlaps", "Inspect overlapping worktree changes"),
        rows.following("dev-instructions", "Find instructions, skills and handoffs"),
        rows.following("quickfix", "Navigate current quickfix results"),
    ],
    "dev-context": lambda cwd, state: context.context_rows(cwd),
    "dev-excerpts": lambda cwd, state: context.excerpt_rows(
        [Path(state["file"]).resolve()], projects.project_root(cwd)
    ),
    "dev-diffs": lambda cwd, state: changes.diff_rows(cwd),
    "dev-overlaps": lambda cwd, state: changes.worktree_rows(cwd),
    "dev-instructions": lambda cwd, state: context.instruction_rows(cwd),
    "dev-failures": lambda cwd, state: [
        dict(item, action="copy", value=item["detail"])
        for item in diagnostics.diagnostics(cwd, state, checks=False)
    ],
}
KINDS = frozenset(_COLLECTORS)

_ACTIONS = {
    "dev-xcode-scheme": xcode.accept,
    "json": json.accept,
    "services": services.accept,
    "processes": processes.accept,
    "chezmoi": chezmoi.accept,
}
_BINDINGS = {
    "dev-xcode-scheme": {"alt-d": "Choose Xcode destination"},
    "json": {"alt-j": "Copy jq expression"},
    "services": {"alt-d": "Prepare service logs", "alt-e": "Prepare container shell"},
    "processes": {"alt-d": "Prepare process connection details"},
    "chezmoi": {"alt-t": "Copy target path"},
}


def collect(kind, state):
    cwd = Path(state.get("cwd", os.getcwd())).resolve()
    collector = _COLLECTORS.get(kind)
    if collector is None:
        return [rows.notice(f"Unknown developer picker: {kind}")]
    try:
        return collector(cwd, state)
    except (OSError, ValueError, sqlite3.Error, execution.ProviderError) as error:
        return [rows.notice(f"{kind} unavailable", str(error))]


def bindings(kind, state):
    return dict(_BINDINGS.get(kind, {}))


def accept(kind, rows, key, state):
    action = _ACTIONS.get(kind)
    result = action(rows, key, state) if action else None
    return result if result is not None else selection.accept(rows, key, state)
