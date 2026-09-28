import json

import git_files

from .providers import module
from .records import clean, load

COMMON = {
    "ctrl-/": "Help",
    "alt-/": "Help",
    "alt-p": "Preview",
    "alt-w": "Wrap preview",
    "alt-W": "Wrap results",
    "ctrl-r": "Relevance/source order",
    "alt-r": "Refresh",
    "alt-y": "Copy selected/current values",
    "ctrl-n/ctrl-p": "Next/previous",
    "ctrl-alt-n/ctrl-alt-p": "Page down/up",
    "tab/shift-tab": "Select/unselect",
    "enter": "Accept",
    "esc/ctrl-c": "Cancel",
}
LOCATION_KINDS = {
    "files",
    "directories",
    "parents",
    "bookmarks",
    "contents",
    "tracked",
    "git-files",
    "changed",
    "ignored",
    "conflicts",
    "git-config",
    "quickfix",
    "diagnostics",
    "chezmoi",
    "recent",
    "crashes",
    "dev-context",
    "dev-instructions",
}
SEARCH_DETAILS = {"network", "devices", "network-hosts", "contacts"}
TOGGLES = {
    "alt-h",
    "alt-u",
    "alt-x",
    "alt-s",
    "alt-B",
    "alt-0",
    "alt-1",
    "alt-2",
    "alt-3",
    "alt-4",
    "alt-5",
    "alt-6",
}


def controls(kind, state):
    custom = module(kind).bindings(kind, state)
    result = dict(COMMON)
    if kind in ("contents", "log-search"):
        result.pop("ctrl-r")
    if kind in LOCATION_KINDS:
        result.update(
            {
                "ctrl-o": "Prepare opening",
                "ctrl-s": "Prepare sudo edit",
                "alt-c": "Prepare containing directory",
                "alt-q": "Export selected locations",
            }
        )
    result.update(custom)
    return result


def header(session):
    state = load(session)
    modes = [state["kind"].replace("-", " ").title()]
    if state["kind"] == "contents":
        project = state.get("worktree") or git_files.root(state["cwd"])
        modes += (
            ["Untracked only" if state.get("untracked") else "Tracked only"]
            if project
            else ["Current directory"]
        )
        modes += [
            "Regex" if state.get("regex") else "Literal",
            "Hidden" if state.get("hidden") else "Visible",
        ]
    for field in ("browser", "contact_mode"):
        if state.get(field):
            modes.append(str(state[field]))
    if state.get("notice"):
        modes.append(state["notice"])
    status_file = session / "status.json"
    status = json.loads(status_file.read_text()) if status_file.exists() else {}
    modes.append(status.get("status", "Loading"))
    return clean(" | ".join(modes))
