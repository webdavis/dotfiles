import ipaddress
import shlex
from pathlib import Path

import tomllib

from . import inputs
from .rows import command_row


def catalog(state):
    deployed = Path.home() / ".config/fzf-pickers/network-actions.toml"
    source = Path(__file__).resolve().parents[4] / "dot_config/fzf-pickers/network-actions.toml"
    path = Path(state.get("network_catalog", source if source.is_file() else deployed))
    with path.open("rb") as stream:
        return tomllib.load(stream)["action"]


def network_actions(state):
    for item in catalog(state):
        title = f"{item['title']}  [{item['tool']}]"
        if item.get("next"):
            yield {
                "id": item["id"],
                "label": title,
                "detail": item["purpose"] + "\nEnter opens the selector.",
                "value": item["title"],
                "search": item["purpose"],
                "action": "next",
                "next": item["next"],
            }
        else:
            row = command_row(
                item["id"],
                title,
                text=item["command"],
                purpose=item["purpose"],
                settings="Choose " + ", ".join(item["params"]) + " on Enter."
                if item.get("params")
                else "",
            )
            row["params"] = item.get("params", [])
            row["search"] = item["purpose"] + " " + item["command"]
            yield row


def prepare(row, state):
    text = row.get("text")
    if text is not None:
        for parameter in row.get("params", []):
            value = inputs.validate(parameter, inputs.prompt(parameter.capitalize()))
            text = text.replace("<" + parameter + ">", shlex.quote(value))
            if parameter == "cidr" and ipaddress.ip_network(value).version == 6:
                text = text.replace("nmap ", "nmap -6 ", 1)
        return {"type": "command", "text": text}
    return {"type": "command", "argv": row["argv"]}
