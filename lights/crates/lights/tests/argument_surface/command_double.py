# Private OpenHue/osascript boundary; never delegates to installed commands.
import json
import os
import sys
from pathlib import Path

argv = sys.argv[1:]
tool = Path(sys.argv[0]).name
with open(os.environ["LIGHTS_LEGACY_CAPTURE"], "a") as stream:
    stream.write(json.dumps([tool, *argv]) + "\n")
if tool == "osascript":
    if len(argv) == 2 and argv[0] == "-e":
        sys.exit(0)
    sys.exit(98)
if tool != "openhue":
    sys.exit(98)
resources = json.loads(Path(os.environ["LIGHTS_TEST_RESOURCES"]).read_text())["data"]
rooms = [r for r in resources if r["type"] == "room"]
scenes = [r for r in resources if r["type"] == "scene"]
by_id = {r["id"]: r for r in resources}


def room(name):
    return next((r for r in rooms if name in (r["id"], r["metadata"]["name"])), None)


if argv == ["get", "room", "--json"]:
    print(json.dumps([{"Id": r["id"], "Name": r["metadata"]["name"]} for r in rooms]))
elif len(argv) == 4 and argv[:2] == ["get", "room"] and argv[3] == "--json":
    found = room(argv[2])
    if found is None:
        sys.exit(1)
    group = by_id[found["services"][0]["rid"]]
    print(
        json.dumps(
            {
                "GroupedLight": {"HueData": group},
                "Scenes": [s for s in scenes if s["group"]["rid"] == found["id"]],
            }
        )
    )
elif len(argv) == 5 and argv[:3] == ["get", "scene", "--room"] and argv[-1] == "--json":
    if os.environ.get("LIGHTS_LEGACY_SCENE_ERROR"):
        sys.exit(1)
    found = room(argv[3])
    print(
        json.dumps(
            [
                {"Name": s["metadata"]["name"], "HueData": s}
                for s in scenes
                if found and s["group"]["rid"] == found["id"]
            ]
        )
    )
elif len(argv) in (4, 5) and argv[:2] == ["set", "room"] and room(argv[2]):
    if not (
        argv[3:] in (["--on"], ["--off"])
        or (len(argv) == 5 and argv[3] == "-b" and argv[4].isdigit())
    ):
        sys.exit(98)
elif len(argv) == 3 and argv[:2] == ["set", "scene"] and argv[2] in by_id:
    pass
elif len(argv) == 5 and argv[:2] == ["set", "scene"] and argv[3] == "-r":
    found = room(argv[4])
    if not any(
        s["metadata"]["name"] == argv[2] and found and s["group"]["rid"] == found["id"]
        for s in scenes
    ):
        sys.exit(98)
else:
    print("unexpected private command: " + repr(argv), file=sys.stderr)
    sys.exit(98)
