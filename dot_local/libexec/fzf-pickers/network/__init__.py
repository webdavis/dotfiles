from . import captures, catalog, devices, diagnostics, hosts, rows, url

_COLLECTORS = {
    "network": catalog.network_actions,
    "devices": devices.devices,
    "network-hosts": hosts.hosts,
    "network-capture": captures.capture_builder,
    "captures": captures.saved_captures,
    "network-capture-analysis": captures.capture_analysis,
    "url": url.collect,
    "network-dns": diagnostics.dns_records,
    "network-iperf": diagnostics.iperf_modes,
    "network-ready": rows.prepared,
}
KINDS = frozenset(_COLLECTORS)

_KEYS = {
    "devices": {"alt-d": ("Prepare discovery of a chosen subnet", devices.discover)},
    "captures": {
        "alt-f": ("Choose capture search directory", captures.choose_root),
        "alt-h": ("Show/hide hidden captures", lambda state: captures.toggle(state, "hidden")),
        "alt-s": ("Path/modification time", lambda state: captures.toggle(state, "sort")),
    },
    "url": {"alt-e": ("Choose another URL", url.edit)},
}
_ACTIONS = {
    "url": url.choose,
    "capture": captures.prepare_capture,
    "display-filter": captures.prepare_filter,
    "dns": diagnostics.prepare_dns,
    "iperf": diagnostics.prepare_iperf,
    "command": catalog.prepare,
}
_ROW_ACTIONS = {"captures": captures.open_capture}


def collect(kind, state):
    collector = _COLLECTORS.get(kind)
    if collector:
        yield from collector(state)


def accept(kind, rows, key, state):
    binding = _KEYS.get(kind, {}).get(key)
    if binding:
        return binding[1](state)
    if key != "enter" or not rows:
        return None
    row = rows[0]
    action = row.get("action")
    if action == "none":
        return {"type": "reload", "state": {}}
    handler = _ROW_ACTIONS.get(kind) or _ACTIONS.get(action)
    if handler:
        return handler(row, state)
    return None


def bindings(kind, state):
    return {key: binding[0] for key, binding in _KEYS.get(kind, {}).items()}
