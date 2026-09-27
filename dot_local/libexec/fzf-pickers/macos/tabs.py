from . import bridge

BINDINGS = {"alt-B": "Switch Arc / Chrome"}


def collect(state):
    browser = state.get("browser", "arc")
    for tab in bridge.native("tabs", browser=browser):
        space = tab.get("space") or {}
        yield {
            "id": browser + ":" + tab["id"],
            "label": f"{tab['title']}  {tab['url']}",
            "detail": f"{browser.title()}\n{tab['title']}\n{tab['url']}\n{space.get('title', '')} {tab.get('location', '')}\n\nEnter focuses this tab. Alt-Y copies its URL.",
            "value": tab["url"],
            "tab": tab,
            "browser": browser,
        }


def accept(kind, rows, key, state):
    if kind == "tabs" and key == "alt-B":
        return {
            "type": "reload",
            "state": {"browser": "arc" if state.get("browser", "arc") == "chrome" else "chrome"},
        }
    if not rows:
        return None
    row = rows[0]
    if kind == "tabs" and key == "enter":
        bridge.native("focus", browser=row["browser"], item=row["tab"])
        return {"type": "done"}
    return None
