from pathlib import Path

import developer.projects as project_paths
import developer.rows as row_model


def excerpt_rows(paths, root):
    rows = []
    for path in paths:
        if not path.is_file() or path.is_symlink():
            continue
        raw = path.read_bytes()
        if b"\0" in raw:
            continue
        lines = raw.decode(errors="replace").splitlines()
        for start in range(0, len(lines), 60):
            excerpt = "\n".join(lines[start : start + 60])
            name = str(path.relative_to(root)) if path.is_relative_to(root) else str(path)
            text = f"{name}:{start + 1}\n{excerpt}"
            rows.append(
                row_model.row(
                    ("context", str(path), start),
                    f"{name}:{start + 1}-{min(start + 60, len(lines))}",
                    text,
                    text,
                    action="copy",
                    path=str(path),
                    line=start + 1,
                )
            )
    return rows


def context_rows(cwd):
    root = project_paths.project_root(cwd)
    extensions = {
        ".md",
        ".rs",
        ".py",
        ".sh",
        ".swift",
        ".js",
        ".ts",
        ".tsx",
        ".jsx",
        ".lua",
        ".c",
        ".h",
        ".cpp",
        ".go",
        ".toml",
        ".yaml",
        ".yml",
        ".tmpl",
    }
    paths = [path for path in project_paths.files(root) if path.suffix.lower() in extensions]
    return [
        row_model.following(
            "files", "Choose any file for context excerpts", return_kind="dev-excerpts"
        )
    ] + excerpt_rows(paths, root)


def instruction_rows(cwd):
    paths = set()
    for directory in project_paths.ancestors(cwd):
        paths.update(
            path
            for name in ("AGENTS.md", "CLAUDE.md", "GEMINI.md")
            if (path := directory / name).is_file()
        )
    for path in project_paths.files(cwd):
        if path.name in ("AGENTS.md", "CLAUDE.md", "SKILL.md") or "handoff" in path.name.lower():
            paths.add(path)
    paths.update((Path.home() / ".agents/skills").glob("*/SKILL.md"))
    return [
        row_model.row(
            ("instructions", str(path)),
            str(path),
            project_paths.read_text(path),
            f"{path}\n{project_paths.read_text(path)}",
            action="copy",
            path=str(path),
            line=1,
        )
        for path in sorted(paths)
        if path.is_file()
    ]
