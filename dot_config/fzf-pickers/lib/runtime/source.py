import hashlib
import subprocess
import sys

from .controls import SEARCH_DETAILS
from .providers import module
from .records import clean, filter_rows, load, record, save


def emit_rows(session, query, key=None):
    state = load(session)
    kind = state["kind"]
    state["query"] = query
    if key:
        action = module(kind).accept(kind, [], key, state)
        if action and action.get("type") == "reload":
            state.update(action.get("state", {}))
    if key:
        state.pop("notice", None)
        save(session / "state.json", state)
    save(session / "status.json", {"status": "Loading"})
    count = 0
    seen = set()
    (session / "index").write_text("")
    try:
        for row in module(kind).collect(kind, state):
            identity = hashlib.sha256(
                str(row.get("id", row.get("value", row.get("label")))).encode(
                    errors="surrogateescape"
                )
            ).hexdigest()
            if identity in seen:
                continue
            seen.add(identity)
            save(session / (identity + ".json"), row)
            with (session / "index").open("a") as index:
                index.write(identity + "\n")
            if kind not in SEARCH_DETAILS or not query:
                sys.stdout.buffer.write(record(identity, row))
                sys.stdout.buffer.flush()
            count += 1
        state["status"] = f"{count} items | Complete"
    except (
        RuntimeError,
        OSError,
        ValueError,
        KeyError,
        subprocess.SubprocessError,
    ) as exc:
        state["status"] = f"{count} items | Incomplete: {clean(exc)}"
        identity = "0" * 64
        save(
            session / (identity + ".json"),
            {"error": True, "label": "Unavailable", "detail": clean(exc)},
        )
        sys.stdout.buffer.write((identity + "\t" + clean(exc) + "\t\0").encode(errors="replace"))
        sys.stdout.buffer.flush()
    save(session / "status.json", {"status": state["status"]})
    if kind in SEARCH_DETAILS and query:
        filter_rows(session, query)
