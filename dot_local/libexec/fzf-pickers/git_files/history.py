import os

from ._git import git, strings


def commits(project, state, *, kind="commits"):
    args = ["reflog"] if kind == "reflog" else ["log"]
    args += ["-500", "--format=%H%x00%h %s (%cr, %an)%x00"]
    if kind == "log-search":
        if not state.get("query"):
            return []
        args += ["--no-ext-diff", "--no-textconv", "-G" + state["query"]]
    values = strings(git(project, *args))
    rows = []
    for i in range(0, len(values) - 1, 2):
        commit, label = values[i].strip(), values[i + 1]
        row = {
            "id": commit,
            "label": label,
            "value": commit,
            "commit": commit,
            "project": project,
            "action": "insert",
        }
        if kind == "historical":
            row.update(action="next", next="historical-files", state={"commit": commit})
        rows.append(row)
    return list(reversed(rows)) if state.get("sort") else rows


def historical_files(project, state):
    commit = state["commit"]
    rows = []
    for item in git(project, "ls-tree", "-rz", commit).split(b"\0"):
        if not item:
            continue
        metadata, name = item.split(b"\t", 1)
        mode, kind_name, blob = metadata.decode().split()
        if kind_name != "blob":
            continue
        path = os.fsdecode(name)
        rows.append(
            {
                "id": commit + ":" + path,
                "label": path,
                "value": path,
                "blob": blob,
                "project": project,
                "action": "historical",
            }
        )
    return rows


def refs(project, state):
    records = git(
        project,
        "for-each-ref",
        "--format=%(refname)%09%(refname:short)%09%(subject)",
        "refs/heads",
        "refs/remotes",
        "refs/tags",
    )
    return [
        {
            "id": parts[0],
            "label": f"{parts[1]}  {parts[2]}",
            "value": parts[0],
            "commit": parts[0],
            "project": project,
            "action": "insert",
        }
        for line in records.decode(errors="replace").splitlines()
        if len(parts := line.split("\t", 2)) == 3 and not parts[0].endswith("/HEAD")
    ]


def stashes(project, state):
    values = strings(git(project, "stash", "list", "--format=%gd%x00%gs%x00"))
    return [
        {
            "id": values[i].strip(),
            "label": f"{values[i].strip()} {values[i + 1]}",
            "value": values[i].strip(),
            "commit": values[i].strip(),
            "project": project,
            "action": "insert",
        }
        for i in range(0, len(values) - 1, 2)
    ]
