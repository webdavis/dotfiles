import re
from urllib.parse import quote

from . import bridge

MODES = {
    "alt-0": "all",
    "alt-1": "name",
    "alt-2": "phone",
    "alt-3": "email",
    "alt-4": "address",
    "alt-5": "work",
    "alt-6": "link",
}


BINDINGS = {
    **{key: "Copy " + mode for key, mode in MODES.items()},
    "alt-7": "Choose populated fields",
    "ctrl-t": "Call phone number",
    "alt-m": "Find conversations",
}


def contact_row(person, state):
    fields = [field for field in person["fields"] if str(field.get("value", "")).strip()]
    mode = state.get("contact_mode", "all")
    chosen = fields if mode == "all" else [field for field in fields if field["group"] == mode]
    card = "\n".join(f"{field['label']}: {field['value']}" for field in fields)
    copied = card if mode == "all" else "\n".join(field["value"] for field in chosen)
    return {
        "id": person["id"],
        "label": person["name"] or "(unnamed contact)",
        "detail": card + f"\n\nCopy: {mode}. Enter copies; Alt-Y copies and stays.",
        "value": copied,
        "action": "copy",
        "contact": person,
    }


def contact_fields(person, group=None):
    for field in person["fields"]:
        if not str(field.get("value", "")).strip() or (group and field["group"] != group):
            continue
        yield {
            "id": person["id"] + ":" + field["id"],
            "label": f"{field['label']}: {field['value']}",
            "detail": f"{person['name']}\n{field['label']}: {field['value']}",
            "value": field["value"],
            "action": "copy",
            "group": field["group"],
        }


def collect(state):
    for person in bridge.native("contacts"):
        yield contact_row(person, state)


def fields(state, group=None):
    yield from contact_fields(state["contact"], group)


def accept(kind, rows, key, state):
    if kind == "contacts" and key in MODES:
        return {"type": "reload", "state": {"contact_mode": MODES[key]}}
    if not rows:
        return None
    row = rows[0]
    if kind == "contacts":
        person = row["contact"]
        if key == "alt-7":
            return {"type": "next", "kind": "contact-fields", "state": {"contact": person}}
        if key == "ctrl-t":
            if len(rows) != 1:
                raise RuntimeError("Select one contact to call.")
            phones = list(contact_fields(person, "phone"))
            if not phones:
                raise RuntimeError("This contact has no phone number.")
            if len(phones) == 1:
                return accept("macos-contact-call", phones, "enter", state)
            return {"type": "next", "kind": "macos-contact-call", "state": {"contact": person}}
        if key == "alt-m":
            addresses = [
                field["value"]
                for field in person["fields"]
                if field["group"] in ("phone", "email") and field["value"]
            ]
            if not addresses:
                raise RuntimeError("This contact has no phone number or email address to match.")
            return {"type": "next", "kind": "chats", "state": {"contact_addresses": addresses}}
    if kind == "macos-contact-call" and key == "enter":
        number = re.sub(r"[\s().-]", "", row["value"])
        if not re.fullmatch(r"\+?[0-9]+(?:[;,][0-9]+)*", number):
            raise RuntimeError("The selected phone number cannot be used as a telephone URL.")
        return {"type": "open", "url": "tel:" + quote(number, safe="+;,")}
    if key == "enter" and kind in ("contacts", "contact-fields"):
        return {"type": "copy", "text": "\n".join(str(item["value"]) for item in rows)}
    return None
