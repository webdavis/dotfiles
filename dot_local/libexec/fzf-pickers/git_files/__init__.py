from functools import partial

from . import filesystem, history, repository
from ._git import root
from .interactions import accept, bindings, preview

__all__ = ["KINDS", "collect", "accept", "bindings", "root", "preview"]

COLLECTORS = {
    "files": filesystem.files,
    "directories": partial(filesystem.files, directories=True),
    "parents": partial(filesystem.locations, parents=True),
    "bookmarks": filesystem.locations,
    "contents": filesystem.search,
    "tracked": repository.tracked,
    "git-files": partial(repository.tracked, untracked=True),
    "changed": repository.changed,
    "commits": history.commits,
    "historical": partial(history.commits, kind="historical"),
    "reflog": partial(history.commits, kind="reflog"),
    "log-search": partial(history.commits, kind="log-search"),
    "historical-files": history.historical_files,
    "refs": history.refs,
    "stashes": history.stashes,
    "worktrees": repository.worktrees,
    "ignored": repository.ignored,
    "conflicts": repository.conflicts,
    "git-config": repository.configuration,
}
KINDS = set(COLLECTORS)


def collect(kind, state):
    project = state.get("worktree") or root(state["cwd"])
    if not project and kind not in ("files", "directories", "parents", "bookmarks", "contents"):
        raise RuntimeError("This picker needs a Git repository.")
    collector = COLLECTORS.get(kind)
    if collector is None:
        raise RuntimeError(f"Unknown Git picker: {kind}")
    return collector(project, state)
