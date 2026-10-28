import json
import tomllib
from pathlib import Path

from rows import run


def source_dir():
    try:
        with (Path.home() / ".config/chezmoi/chezmoi.toml").open("rb") as stream:
            return Path(tomllib.load(stream)["sourceDir"]).expanduser()
    except (OSError, KeyError, ValueError):
        return Path.home() / ".local/share/chezmoi"


def annotated(name):
    """A .chezmoidata YAML file as JSON with every scalar turned into {"v": value, "l": line}."""
    path = source_dir() / ".chezmoidata" / name
    query = '(.. | select(kind == "scalar")) |= {"v": ., "l": line}'
    return path, json.loads(run(["yq", "-o", "json", query, str(path)]))


def scalar(node):
    return isinstance(node, dict) and set(node) == {"v", "l"}


def fields(item):
    """An annotated mapping's scalar and list fields as `key: value` lines."""
    lines = []
    for key, value in item.items():
        if scalar(value):
            lines.append(f"{key}: {value['v']}")
        elif isinstance(value, list):
            lines.append(f"{key}: " + ", ".join(str(v["v"]) for v in value if scalar(v)))
    return "\n".join(lines)
