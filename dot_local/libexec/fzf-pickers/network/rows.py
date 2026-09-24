import datetime
import shlex


def command_row(identity, title, argv=None, text=None, purpose="", settings=""):
    command = text if text is not None else shlex.join(argv)
    row = {
        "id": identity,
        "label": title,
        "detail": "\n".join(
            filter(None, [title, command, purpose, settings, "Enter prepares the command."])
        ),
        "value": command,
        "action": "command",
        "search": " ".join([command, purpose, settings]),
    }
    if argv is not None:
        row["argv"] = argv
    else:
        row["text"] = command
    return row


def note(identity, message):
    return {"id": identity, "label": message, "detail": message, "value": "", "action": "none"}


def device_row(source, identity, name, address, fields):
    now = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")
    details = [f"Source: {source}", f"Observed: {now}"]
    details.extend(
        f"{key}: {value}" for key, value in fields.items() if value not in ("", None, [], {})
    )
    return {
        "id": f"{source}:{identity}",
        "label": f"{name or address or identity}  {address}",
        "detail": "\n".join(details),
        "search": " ".join(details),
        "value": address or name or identity,
        "action": "insert",
    }


def ready(title, argv):
    return {
        "type": "next",
        "kind": "network-ready",
        "state": {"command_title": title, "command_argv": argv, "command_text": None},
    }


def prepared(state):
    yield command_row(
        "ready",
        state.get("command_title", "Prepared network command"),
        state.get("command_argv"),
        state.get("command_text"),
    )
