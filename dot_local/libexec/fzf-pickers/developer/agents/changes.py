import re
from pathlib import Path

import developer.execution as execution
import developer.projects as project_paths
import developer.rows as row_model


def diff_rows(cwd):
    root = project_paths.project_root(cwd)
    rows = []
    for staged in (False, True):
        args = [
            "git",
            "-C",
            str(root),
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--unified=5",
        ]
        if staged:
            args.append("--cached")
        output = execution.run(args)
        file_header = ""
        for part in re.split(r"(?=^diff --git )", output, flags=re.MULTILINE):
            if not part:
                continue
            chunks = re.split(r"(?=^@@ )", part, flags=re.MULTILINE)
            file_header = chunks[0]
            for index, chunk in enumerate(chunks[1:] or [""]):
                text = ("Staged\n" if staged else "Unstaged\n") + file_header + chunk
                label = file_header.splitlines()[0] + " " + (chunk.splitlines()[0] if chunk else "")
                rows.append(
                    row_model.row(
                        ("diff", staged, file_header, index),
                        label,
                        text,
                        text,
                        action="copy",
                    )
                )
    return rows


def worktree_rows(cwd):
    root = project_paths.project_root(cwd)
    current = {
        path
        for path in execution.run(
            ["git", "-C", str(root), "diff", "--name-only", "-z", "HEAD"]
        ).split("\0")
        if path
    }
    current.update(
        path
        for path in execution.run(
            ["git", "-C", str(root), "ls-files", "--others", "--exclude-standard", "-z"]
        ).split("\0")
        if path
    )
    rows = []
    output = execution.run(["git", "-C", str(root), "worktree", "list", "--porcelain", "-z"])
    for record in output.split("\0\0"):
        values = dict(field.split(" ", 1) for field in record.split("\0") if " " in field)
        path = values.get("worktree")
        if not path or Path(path).resolve() == root:
            continue
        try:
            changed = {
                name
                for name in execution.run(
                    ["git", "-C", path, "diff", "--name-only", "-z", "HEAD"]
                ).split("\0")
                if name
            }
            changed.update(
                name
                for name in execution.run(
                    [
                        "git",
                        "-C",
                        path,
                        "ls-files",
                        "--others",
                        "--exclude-standard",
                        "-z",
                    ]
                ).split("\0")
                if name
            )
            overlaps = sorted(current & changed)
            detail = (
                f"Worktree: {path}\nBranch: {values.get('branch', 'detached')}\n\nOverlapping paths ({len(overlaps)}):\n"
                + "\n".join(overlaps)
            )
            detail += "\n\nPath overlap indicates shared work, not a proven merge conflict.\n\n"
            for name in overlaps:
                detail += execution.run(
                    [
                        "git",
                        "-C",
                        path,
                        "diff",
                        "--no-ext-diff",
                        "--no-textconv",
                        "HEAD",
                        "--",
                        name,
                    ]
                )
            rows.append(
                row_model.row(
                    ("worktree", path),
                    f"{len(overlaps):>3} overlaps  {path}",
                    detail,
                    detail,
                    action="copy",
                    path=path,
                )
            )
        except execution.ProviderError as error:
            rows.append(row_model.notice(path, str(error)))
    return rows
