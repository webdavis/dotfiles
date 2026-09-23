import subprocess
import sys

from .controls import LOCATION_KINDS, SEARCH_DETAILS, TOGGLES, controls, header
from .providers import module


def arguments(state, session, command, argument):
    kind = state["kind"]
    keys = controls(kind, state)
    reload_command = f"{command} rows {argument} {{q}}"
    args = [
        "fzf",
        "--read0",
        "--print0",
        "--print-query",
        "--delimiter=\t",
        "--with-nth=2",
        "--nth=1",
        "--id-nth=1",
        "--track",
        "--multi",
        "--no-sort",
        "--query=" + state.get("query", ""),
        "--header=" + header(session),
        "--footer= ",
        "--footer-border=rounded",
        "--footer-label= Ctrl-/ Alt-/: help | Alt-Y: copy | Enter: accept | Esc: cancel ",
        "--footer-label-pos=0",
        "--preview=" + f"{command} preview {argument} {{1}} {{q}}",
        "--bind=" + f"load:transform-header({command} header {argument})",
        "--bind=" + f"alt-r:reload-sync({reload_command})",
        "--bind="
        + f"alt-y:execute-silent({command} copy {argument} {{+1}})+transform-header({command} header {argument})",
        "--bind=" + f"ctrl-/:execute-silent({command} help {argument})+refresh-preview",
        "--bind=" + f"ctrl-_:execute-silent({command} help {argument})+refresh-preview",
        "--bind=" + f"alt-/:execute-silent({command} help {argument})+refresh-preview",
    ]
    for key in TOGGLES & keys.keys():
        args += ["--bind=" + f"{key}:reload-sync({reload_command} {key})"]
    if state.get("checkout") or kind in (
        "directories",
        "parents",
        "bookmarks",
        "worktrees",
        "historical",
        "macos-contact-call",
    ):
        args.append("--no-multi")
    custom = set(module(kind).bindings(kind, state)) - TOGGLES - {"alt-y"}
    if kind in LOCATION_KINDS:
        custom |= {"ctrl-o", "ctrl-s", "alt-c", "alt-q"}
    args += ["--expect=" + ",".join(sorted(custom | {"enter"}))]
    if kind in SEARCH_DETAILS:
        source_keys = ["alt-r", *sorted(TOGGLES & keys.keys())]
        paused = ",".join(["change", "ctrl-r", *source_keys])
        gate = f"unbind({paused})+rebind(load)"
        args += [
            "--disabled",
            "--bind=start:" + gate,
            "--bind="
            + f"load:transform-header({command} header {argument})+unbind(load)+rebind({paused})+trigger(change)",
            "--bind=" + f"change:reload-sync({command} filter {argument} {{q}})",
            "--bind=" + f"ctrl-r:reload-sync({command} filter {argument} {{q}} toggle)",
        ]
        for key in source_keys:
            suffix = "" if key == "alt-r" else " " + key
            args += ["--bind=" + f"{key}:{gate}+reload-sync({reload_command}{suffix})"]
    if kind in ("contents", "log-search"):
        args += ["--disabled", "--bind=" + f"change:reload({reload_command})"]
    return args


def select(args, state, session, entrypoint):
    producer = subprocess.Popen(
        [sys.executable, str(entrypoint), "rows", str(session), state.get("query", "")],
        stdout=subprocess.PIPE,
    )
    try:
        return subprocess.run(args, stdin=producer.stdout, stdout=subprocess.PIPE)
    finally:
        producer.stdout.close()
        if producer.poll() is None:
            producer.terminate()
        producer.wait()
