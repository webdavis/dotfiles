import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


def clean(text):
    return "".join(
        c if c >= " " and c != "\x7f" else {"\n": " ↵ ", "\t": "  "}.get(c, "") for c in str(text)
    )


def save(path, value):
    with tempfile.NamedTemporaryFile(
        mode="w", dir=path.parent, prefix=".write-", delete=False
    ) as stream:
        json.dump(value, stream, ensure_ascii=True)
        temporary = Path(stream.name)
    temporary.replace(path)


def load(session):
    return json.loads((session / "state.json").read_text())


def row_ids(session, ids):
    rows = []
    for identity in ids:
        if len(identity) == 64 and all(c in "0123456789abcdef" for c in identity):
            path = session / (identity + ".json")
            if path.is_file():
                row = json.loads(path.read_text())
                if not row.get("error"):
                    rows.append(row)
    return rows


def filter_rows(session, query, toggle=False):
    state = load(session)
    if toggle:
        state["source_order"] = not state.get("source_order")
        save(session / "state.json", state)
    index = session / "index"
    identities = index.read_text().splitlines() if index.exists() else []
    data = bytearray()
    for identity in identities:
        for row in row_ids(session, [identity]):
            data.extend(record(identity, row))
    args = [
        "fzf",
        "--read0",
        "--print0",
        "--delimiter=\t",
        "--nth=2..",
        "--filter=" + query,
    ]
    if state.get("source_order"):
        args.append("--no-sort")
    result = subprocess.run(
        args,
        input=data,
        stdout=subprocess.PIPE,
        env={**os.environ, "FZF_DEFAULT_OPTS": "", "FZF_DEFAULT_OPTS_FILE": ""},
    )
    if result.returncode not in (0, 1):
        raise RuntimeError("Could not filter picker records.")
    sys.stdout.buffer.write(result.stdout)


def record(identity, row):
    label = clean(row.get("label", row.get("value", "")))
    search = clean(row.get("search", row.get("detail", "")))
    return (identity + "\t" + label + "\t" + search + "\0").encode(errors="replace")
