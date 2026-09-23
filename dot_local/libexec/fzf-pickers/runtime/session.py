import shlex
import subprocess
import sys
import tempfile
from pathlib import Path

from .actions import choose_action, copy_text
from .records import clean, load, row_ids, save
from .window import arguments, select


def picker(state, entrypoint):
    with tempfile.TemporaryDirectory(prefix="fzf-picker-") as directory:
        session = Path(directory)
        save(session / "state.json", state)
        command = shlex.join([sys.executable, str(entrypoint)])
        argument = shlex.quote(directory)
        while True:
            state = load(session)
            args = arguments(state, session, command, argument)
            result = select(args, state, session, entrypoint)
            if result.returncode != 0:
                return None
            output = result.stdout.decode(errors="surrogateescape").split("\0")
            if len(output) < 3:
                return None
            state = load(session)
            state["query"] = output[0]
            save(session / "state.json", state)
            key = output[1] or "enter"
            rows = row_ids(session, [record.split("\t", 1)[0] for record in output[2:] if record])
            try:
                action = choose_action(session, rows, key)
                if not action:
                    continue
                action_type = action["type"]
                if action_type == "done":
                    return None
                if action_type == "copy":
                    copy_text(action["text"])
                    return None
                if action_type == "open":
                    subprocess.run(["open", action["url"]], check=True)
                    return None
                if action_type in ("next", "reload"):
                    if action_type == "next":
                        state.update(kind=action["kind"], query="")
                    state.update(action.get("state", {}))
                    save(session / "state.json", state)
                    continue
                return action
            except (
                RuntimeError,
                OSError,
                ValueError,
                subprocess.SubprocessError,
            ) as exc:
                state["notice"] = clean(exc)
                save(session / "state.json", state)
                with open("/dev/tty", "w") as tty:
                    print(clean(exc), file=tty)
