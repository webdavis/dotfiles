import shlex
import sys
import tempfile
import tracemalloc
from pathlib import Path

helpers = Path(__file__).resolve().parents[2] / "dot_local/libexec/fzf-pickers"
sys.path.insert(0, str(helpers))
from runtime import actions, preview, records

assert records.clean("a\x1b[31m\n\tb") == "a[31m ↵   b"
row = {"path": "/tmp/a\nb; $(echo bad)", "line": 3, "label": "path", "action": "edit"}
a = actions.default_action([row], "enter", {})
assert shlex.split(shlex.join(a["argv"]))[-1] == row["path"]
assert actions.default_action([{"value": ["a b", "c"]}], "enter", {})["values"] == [
    "a b",
    "c",
]
assert actions.default_action([], "enter", {}) is None
second = {**row, "path": "/tmp/second"}
assert actions.default_action([row, second], "enter", {})["argv"][-2:] == [
    row["path"],
    second["path"],
]
try:
    actions.copy_text("")
    raise AssertionError("Empty copy must be refused")
except RuntimeError:
    pass
with tempfile.TemporaryDirectory() as directory:
    session = Path(directory)
    records.save(session / "state.json", {"kind": "files", "cwd": directory})
    identity = "f" * 64
    records.save(session / (identity + ".json"), row)
    assert records.row_ids(session, ["../../private", identity]) == [row]
    records.save(session / ("0" * 64 + ".json"), {"error": True})
    assert not records.row_ids(session, ["0" * 64])
    assert preview.preview(session, identity).startswith("path")
    large = session / "large.bin"
    with large.open("wb") as stream:
        stream.truncate(32 * 1024 * 1024)
    records.save(session / (identity + ".json"), {**row, "path": str(large)})
    tracemalloc.start()
    assert "[Binary file]" in preview.preview(session, identity)
    _, peak = tracemalloc.get_traced_memory()
    tracemalloc.stop()
    assert peak < 8 * 1024 * 1024, "Preview read the whole large file"
print("Picker transport checks passed")
