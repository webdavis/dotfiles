from pathlib import Path

import developer.rows as row_model


def data_file(state, kind, patterns):
    if state.get("file"):
        return Path(state["file"]).expanduser().resolve()
    return [
        row_model.following(
            "files", f"Choose a {kind} file", return_kind=kind, file_patterns=patterns
        )
    ]


def collect(kind, state, patterns, read):
    path = data_file(state, kind, patterns)
    return path if isinstance(path, list) else read(path)
