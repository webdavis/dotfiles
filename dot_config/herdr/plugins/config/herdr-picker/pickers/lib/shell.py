import os
import re

from data import source_dir
from rows import HOME, edit, emit, home

ALIAS = re.compile(r"^\s*alias\s+([^=\s]+)=(.*)$")
FUNCTION = re.compile(r"^(?:function\s+)?([A-Za-z_][\w:-]*)\s*\(\)\s*\{?")
ASSIGNMENT = re.compile(r"^\s*(?:export\s+)?([A-Za-z_]\w*)=")
SECRET = re.compile(
    r"TOKEN|SECRET|PASSW|API_?KEY|_KEY$|AUTH|CREDENTIAL|COOKIE|PRIVATE", re.IGNORECASE
)


def lines(name):
    path = source_dir() / name
    return path, path.read_text(errors="replace").splitlines()


def comment_above(text, index):
    start = index
    while start > 0 and text[start - 1].lstrip().startswith("#"):
        start -= 1
    return [line for line in text[start:index] if not set(line) & set("┏┃┗")]


def aliases():
    path, text = lines("dot_bash_aliases.tmpl")
    for index, line in enumerate(text):
        if match := ALIAS.match(line):
            name, value = match.groups()
            preview = "\n".join([*comment_above(text, index), line.strip()])
            emit(f"{name}  {value}", edit(path, index + 1), preview)


def functions():
    path, text = lines("dot_bash_functions")
    for index, line in enumerate(text):
        if not (match := FUNCTION.match(line)):
            continue
        end = index
        if not line.rstrip().endswith("}"):
            end = next((n for n in range(index, len(text)) if text[n] == "}"), len(text) - 1)
        body = text[index : end + 1]
        emit(match[1], edit(path, index + 1), "\n".join([*comment_above(text, index), *body]))


def shell_files():
    for path in sorted(source_dir().glob("dot_*")):
        if path.is_file() and re.match(r"dot_(bash|profile)", path.name):
            yield path, path.read_text(errors="replace").splitlines()


def environment():
    defined = {}
    for path, text in shell_files():
        for index, line in enumerate(text):
            if match := ASSIGNMENT.match(line):
                defined.setdefault(match[1], (path, index + 1))
    for name in sorted(os.environ):
        hidden = bool(SECRET.search(name))
        value = "(hidden)" if hidden else os.environ[name]
        where = defined.get(name)
        origin = f"Set in {home(where[0])}:{where[1]}" if where else "Not set by the dotfiles."
        enter = edit(*where) if where else {"insert": f"${name}"}
        group = "dotfiles" if where else "inherited"
        emit(f"{name}={value}", enter, f"{name}\n{origin}\n\n{value}", group)


def expanded(line):
    prefix = os.environ.get("HOMEBREW_PREFIX", "/opt/homebrew")
    for variable, value in (("HOMEBREW_PREFIX", prefix), ("HOME", HOME)):
        line = line.replace("${" + variable + "}", value).replace("$" + variable, value)
    return line.replace("~/", HOME + "/")


def path():
    places = [
        (file, index + 1, expanded(line))
        for file, text in shell_files()
        for index, line in enumerate(text)
        if not line.lstrip().startswith("#") and re.search(r"PATH|path_prepend", line)
    ]
    seen = set()
    for entry in os.environ.get("PATH", "").split(":"):
        folder = entry.rstrip("/") or "/"
        pattern = re.compile(r"(^|[\s\"'=:])" + re.escape(folder) + r"/?([\"':\s]|$)")
        where = next(((f, n) for f, n, line in places if pattern.search(line)), None)
        try:
            commands, exists = sorted(os.listdir(folder)), True
        except OSError:
            commands, exists = [], os.path.isdir(folder)
        flags = [flag for flag, on in (("missing", not exists), ("again", folder in seen)) if on]
        seen.add(folder)
        origin = f"Added in {home(where[0])}:{where[1]}" if where else "Not added by the dotfiles."
        preview = f"{entry}\n{origin}\n\n{len(commands)} entries:\n" + "\n".join(commands[:200])
        enter = edit(*where) if where else {"cd": folder} if exists else "none"
        label = home(entry) + ("  [" + ", ".join(flags) + "]" if flags else "")
        emit(label, enter, preview, "dotfiles" if where else "inherited")
