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

RESULT_FILE_PREFIXES = ("fzf-result.", "fzf-provenance.")
ACTION_ERRORS = (RuntimeError, OSError, ValueError, subprocess.SubprocessError)
HIDDEN_BY_DEFAULT = ("contents", "directories")


def remove_result_file(path):
    path = Path(path)
    if path.name.startswith(RESULT_FILE_PREFIXES) and path.is_file():
        path.unlink()


def show_rows(session, query, key=None):
    emit_rows(Path(session), query, key)


def show_filtered_rows(session, query, toggle=None):
    filter_rows(Path(session), query, toggle is not None)


def show_header(session):
    print(header(Path(session)))


def show_preview(session, identity, query=None):
    print(preview(Path(session), identity))


def toggle_help(session):
    session = Path(session)
    state = load(session)
    state["help"] = not state.get("help")
    save(session / "state.json", state)


def copy_selection(session, *identities):
    session = Path(session)
    state = load(session)
    try:
        action = choose_action(session, row_ids(session, identities), "alt-y")
        if action and action["type"] == "copy":
            copy_text(action["text"])
            state["notice"] = "Copied selection"
    except ACTION_ERRORS as exc:
        state["notice"] = clean(exc)
    save(session / "state.json", state)


def parse_run_options(arguments):
    parser = argparse.ArgumentParser()
    parser.add_argument("kind")
    parser.add_argument("--root")
    parser.add_argument("--hidden", action="store_true", default=None)
    parser.add_argument("--checkout", action="store_true")
    parser.add_argument("--paths", nargs="*")
    parser.add_argument("--provenance")
    options = vars(parser.parse_args(arguments))
    return {name: value for name, value in options.items() if value is not None}


def provenance_records(path):
    raw = path.read_bytes().decode(errors="replace").split("\0")
    return [
        {"name": raw[i], "kind": raw[i + 1].strip(), "detail": raw[i + 2], "value": raw[i]}
        for i in range(0, len(raw) - 2, 3)
    ]


def initial_state(arguments):
    state = parse_run_options(arguments)
    state.update(
        cwd=os.getcwd(),
        config=str(Path.home() / ".config/fzf-pickers/config.toml"),
        query="",
    )
    state.setdefault("hidden", state["kind"] in HIDDEN_BY_DEFAULT)
    state["browser"] = "arc" if state["kind"] == "tabs" else None
    if state.get("provenance"):
        path = Path(state["provenance"])
        path.write_text(json.dumps(provenance_records(path)))
        state["provenance_file"] = state["provenance"]
    return state


def write_result(action):
    if not action or action["type"] not in ("insert", "command"):
        return
    if action["type"] == "insert":
        fields = action["values"]
    else:
        fields = [action.get("text") or shlex.join(action["argv"])]
    record = "\0".join([action["type"], *fields]) + "\0"
    sys.stdout.buffer.write(record.encode(errors="surrogateescape"))


def run_picker(*arguments):
    write_result(picker(initial_state(list(arguments)), Path(__file__).resolve()))


OPERATIONS = {
    "run": ("<kind> [options]", 1, None, run_picker),
    "rows": ("<session> <query> [key]", 2, 3, show_rows),
    "filter": ("<session> <query> [toggle]", 2, 3, show_filtered_rows),
    "header": ("<session>", 1, 1, show_header),
    "preview": ("<session> <row> [query]", 2, 3, show_preview),
    "help": ("<session>", 1, 1, toggle_help),
    "copy": ("<session> [row...]", 1, None, copy_selection),
    "cleanup": ("<file>", 1, 1, remove_result_file),
}


def exit_with_usage(operation=None):
    if operation in OPERATIONS:
        forms = [f"{operation} {OPERATIONS[operation][0]}"]
    else:
        forms = [f"{name} {arguments}" for name, (arguments, *_) in OPERATIONS.items()]
    print(f"usage: {Path(sys.argv[0]).name} " + " | ".join(forms), file=sys.stderr)
    sys.exit(2)


def main(arguments):
    if not arguments or arguments[0] not in OPERATIONS:
        exit_with_usage()
    operation, *rest = arguments
    _, fewest, most, perform = OPERATIONS[operation]
    if len(rest) < fewest or (most is not None and len(rest) > most):
        exit_with_usage(operation)
    perform(*rest)


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
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
