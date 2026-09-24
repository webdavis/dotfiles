from functools import partial

from . import contacts, discovery, tabs

_PICKERS = {
    "contacts": (contacts.collect, contacts.accept, contacts.BINDINGS),
    "contact-fields": (contacts.fields, contacts.accept, {}),
    "macos-contact-call": (partial(contacts.fields, group="phone"), contacts.accept, {}),
    "tabs": (tabs.collect, tabs.accept, tabs.BINDINGS),
    "recent": (discovery.recent_files, None, {}),
    "shortcuts": (
        discovery.shortcuts,
        discovery.accept,
        {"ctrl-o": "Prepare opening Shortcut editor"},
    ),
    "logs": (discovery.logs, None, {}),
    "macos-unified-logs": (discovery.unified_logs, None, {}),
    "crashes": (discovery.crashes, None, {}),
}
KINDS = set(_PICKERS)


def collect(kind, state):
    if kind not in _PICKERS:
        raise RuntimeError(f"Unknown macOS picker: {kind}")
    yield from _PICKERS[kind][0](state)


def accept(kind, rows, key, state):
    picker = _PICKERS.get(kind)
    if picker and picker[1]:
        return picker[1](kind, rows, key, state)
    return None


def bindings(kind, state):
    picker = _PICKERS.get(kind)
    return dict(picker[2]) if picker else {}
