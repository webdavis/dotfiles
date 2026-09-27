def prompt(label):
    try:
        with open("/dev/tty", "r+") as terminal:
            terminal.write(label + ": ")
            terminal.flush()
            value = terminal.readline()
            return value.rstrip("\n") if value else None
    except (OSError, KeyboardInterrupt):
        return None


def accept(rows, key, state):
    if not rows:
        return None
    first = rows[0]
    if key in ("enter", "") and first.get("parameters"):
        argv = list(first["argv"])
        for parameter in first["parameters"]:
            value = prompt(parameter["name"])
            if value is None:
                return {"type": "reload", "state": {}}
            argv.append(value)
        return {"type": "command", "argv": argv}
    if key in ("enter", "") and first.get("text") and first.get("action") == "command":
        return {"type": "command", "text": first["text"]}
    return None
