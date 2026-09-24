import json

from .. import projects, rows


def collect(cwd, state):
    if not state.get("provenance_file"):
        return [rows.notice("Parent shell command inventory unavailable")]
    data = json.loads(projects.read_text(state["provenance_file"]))
    return [
        rows.row(
            ("provenance", item["name"]),
            f"{item['name']}  {item.get('kind', '')}",
            item.get("detail", ""),
            item.get("value", item["name"]),
            action="insert",
        )
        for item in data
    ]
