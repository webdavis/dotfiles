#!/usr/bin/env python3
import argparse
import json
import os
import shlex
import subprocess
import sys
from pathlib import Path

sys.dont_write_bytecode = True

from runtime.actions import choose_action, copy_text
from runtime.controls import header
from runtime.preview import preview
from runtime.records import clean, filter_rows, load, row_ids, save
from runtime.session import picker
from runtime.source import emit_rows


def main():
    operation = sys.argv[1]
    if operation == "cleanup":
        path = Path(sys.argv[2])
        if path.name.startswith(("fzf-result.", "fzf-provenance.")) and path.is_file():
            path.unlink()
        return
    if operation != "run":
        session = Path(sys.argv[2])
        if operation == "rows":
            emit_rows(session, sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else None)
        elif operation == "filter":
            filter_rows(session, sys.argv[3], len(sys.argv) > 4)
        elif operation == "header":
            print(header(session))
        elif operation == "preview":
            print(preview(session, sys.argv[3]))
        elif operation == "help":
            state = load(session)
            state["help"] = not state.get("help")
            save(session / "state.json", state)
        elif operation == "copy":
            state = load(session)
            try:
                action = choose_action(session, row_ids(session, sys.argv[3:]), "alt-y")
                if action and action["type"] == "copy":
                    copy_text(action["text"])
                    state["notice"] = "Copied selection"
            except (
                RuntimeError,
                OSError,
                ValueError,
                subprocess.SubprocessError,
            ) as exc:
                state["notice"] = clean(exc)
            save(session / "state.json", state)
        return
    parser = argparse.ArgumentParser()
    parser.add_argument("kind")
    parser.add_argument("--root")
    parser.add_argument("--hidden", action="store_true", default=None)
    parser.add_argument("--checkout", action="store_true")
    parser.add_argument("--paths", nargs="*")
    parser.add_argument("--provenance")
    options = vars(parser.parse_args(sys.argv[2:]))
    state = {k: v for k, v in options.items() if v is not None}
    state.update(
        cwd=os.getcwd(),
        config=str(Path.home() / ".config/fzf-pickers/config.toml"),
        query="",
    )
    state.setdefault("hidden", state["kind"] in ("contents", "directories"))
    state["browser"] = "arc" if state["kind"] == "tabs" else None
    if state.get("provenance"):
        raw = Path(state["provenance"]).read_bytes().decode(errors="replace").split("\0")
        records = [
            {
                "name": raw[i],
                "kind": raw[i + 1].strip(),
                "detail": raw[i + 2],
                "value": raw[i],
            }
            for i in range(0, len(raw) - 2, 3)
        ]
        Path(state["provenance"]).write_text(json.dumps(records))
        state["provenance_file"] = state["provenance"]
    action = picker(state, Path(__file__).resolve())
    if action and action["type"] in ("insert", "command"):
        fields = (
            action["values"]
            if action["type"] == "insert"
            else [action.get("text") or shlex.join(action["argv"])]
        )
        sys.stdout.buffer.write(
            ("\0".join([action["type"], *fields]) + "\0").encode(errors="surrogateescape")
        )


if __name__ == "__main__":
    try:
        main()
    except (BrokenPipeError, KeyboardInterrupt):
        sys.exit(130)
    except (
        RuntimeError,
        OSError,
        ValueError,
        KeyError,
        subprocess.SubprocessError,
    ) as error:
        print("fzf picker: " + clean(error), file=sys.stderr)
        sys.exit(1)
