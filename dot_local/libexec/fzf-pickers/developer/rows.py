import hashlib
import shlex


def row(identity, label, detail="", value=None, **fields):
    return dict(
        id=hashlib.sha256(str(identity).encode()).hexdigest()[:24],
        label=str(label),
        detail=str(detail),
        value=label if value is None else value,
        **fields,
    )


def notice(label, detail=""):
    return row(("notice", label, detail), label, detail, action="none")


def command(identity, label, argv, detail="", cwd=None, **fields):
    text = shlex.join(argv)
    if cwd:
        text = f"cd {shlex.quote(str(cwd))} && {text}"
    result = row(
        identity,
        label,
        f"{text}\n\n{detail}\nEnter prepares this command.",
        action="command",
        argv=argv,
        **fields,
    )
    if cwd:
        result["text"] = text
    return result


def following(kind, label, detail="", **state):
    return row((kind, state), label, detail, action="next", next=kind, state=state)
