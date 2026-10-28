from data import annotated, fields, scalar
from rows import edit, emit

IDENTITIES = ("repo", "slug", "skill", "agent", "marketplace")


def grouped(node):
    """A harness's entries with what they sit under: an installer, a profile, or nothing."""
    if isinstance(node, list):
        return [("", item) for item in node]
    return [(under, item) for under, items in node.items() for item in items]


def skills():
    path, tree = annotated("agent-skills.yaml")
    for harness, node in tree["skills"]["harnesses"].items():
        for under, item in grouped(node):
            if scalar(item):
                name, line, detail = item["v"], item["l"], f"profile: {under}"
            else:
                key = next(key for key in IDENTITIES if key in item)
                name, line, detail = item[key]["v"], item[key]["l"], fields(item)
                detail = (f"installer: {under}\n" if under else "") + detail
            label = f"{name}  {under}" if under else name
            emit(label, edit(path, line), f"{name}\nharness: {harness}\n{detail}", harness)


def plugins():
    path, tree = annotated("agent-plugins.yaml")
    for harness, node in tree["plugins"]["harnesses"].items():
        for item in node:
            if scalar(item):
                emit(item["v"], edit(path, item["l"]), f"{item['v']}\nharness: {harness}", harness)
                continue
            for plugin in item["plugins"]:
                preview = f"{plugin['v']}\nharness: {harness}\n{fields(item)}"
                emit(plugin["v"], edit(path, plugin["l"]), preview, harness)
