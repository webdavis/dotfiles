import json

import developer.projects as project_paths
import developer.rows as row_model


def json_rows(path):
    root = json.loads(project_paths.read_text(path))
    rows = []

    def visit(value, keys):
        expression = "getpath(" + json.dumps(keys, ensure_ascii=False, separators=(",", ":")) + ")"
        text = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False, indent=2)
        rows.append(
            row_model.row(
                ("json", str(path), keys),
                expression,
                f"{expression}\n\n{text}",
                text,
                action="copy",
                json_path=keys,
                jq=expression,
                path=str(path),
            )
        )
        children = (
            value.items()
            if isinstance(value, dict)
            else enumerate(value)
            if isinstance(value, list)
            else []
        )
        for key, child in children:
            visit(child, keys + [key])

    visit(root, [])
    return rows


def accept(rows, key, state):
    if rows and key == "alt-j":
        return {"type": "copy", "text": "\n".join(item["jq"] for item in rows if "jq" in item)}
    return None
