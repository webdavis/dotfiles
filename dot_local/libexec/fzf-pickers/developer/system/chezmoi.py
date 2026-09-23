import json

import developer.execution as execution
import developer.rows as row_model


def chezmoi_rows():
    data = json.loads(
        execution.run(
            [
                "chezmoi",
                "managed",
                "--include=files,symlinks",
                "--path-style=all",
                "--format=json",
                "--no-tty",
                "--skip-secrets",
            ]
        )
    )
    rows = []
    for item in data.values():
        target, source = item["absolute"], item["sourceAbsolute"]
        rows.append(
            row_model.command(
                ("chezmoi", target),
                target,
                ["nvim", "--", source],
                f"Target: {target}\nSource: {source}",
                value=source,
                path=source,
            )
        )
    return rows


def accept(rows, key, state):
    if rows and key == "alt-t":
        return {"type": "copy", "text": "\n".join(item["label"] for item in rows)}
    return None
