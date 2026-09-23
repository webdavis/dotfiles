import os
from pathlib import Path

import developer.execution as execution
import developer.rows as row_model


def ancestors(cwd):
    path = Path(cwd).resolve()
    return [path, *path.parents]


def nearest(cwd, names):
    for directory in ancestors(cwd):
        for name in names:
            path = directory / name
            if path.is_file():
                return path
    return None


def project_root(cwd):
    for directory in ancestors(cwd):
        if (directory / ".git").exists():
            return directory
    return Path(cwd).resolve()


def files(cwd):
    root = project_root(cwd)
    if (root / ".git").exists():
        output = execution.run(
            ["git", "-C", str(root), "ls-files", "-co", "--exclude-standard", "-z"]
        )
        return [root / path for path in dict.fromkeys(output.split("\0")) if path]
    result = []
    for directory, dirs, names in os.walk(root):
        dirs[:] = sorted(
            name
            for name in dirs
            if name
            not in {
                ".git",
                "node_modules",
                ".build",
                "target",
                ".venv",
                "venv",
                "__pycache__",
            }
        )
        result.extend(Path(directory) / name for name in sorted(names))
    return result


def read_text(path):
    return Path(path).read_text(errors="replace")


def nested_projects(cwd, kind):
    rows = []
    for directory, dirs, names in os.walk(cwd):
        dirs[:] = sorted(
            name
            for name in dirs
            if name
            not in {
                ".git",
                "node_modules",
                ".build",
                "target",
                ".venv",
                "venv",
                "__pycache__",
            }
        )
        path = Path(directory)
        markers = set(names) & {
            "Cargo.toml",
            "Package.swift",
            "package.json",
            "go.mod",
            "pytest.ini",
            "pyproject.toml",
        }
        if path != Path(cwd) and markers:
            rows.append(
                row_model.following(
                    kind,
                    f"Project: {path.relative_to(cwd)}",
                    ", ".join(sorted(markers)),
                    cwd=str(path),
                )
            )
            dirs[:] = []
    return rows
