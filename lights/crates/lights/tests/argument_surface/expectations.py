# Observable expectations derive from the approved behavior changes.
import json

HELP = "Usage: lights"


def writes(result, fixture, legacy=False):
    by_id = {r["id"]: r for r in fixture["data"]}
    actions = []
    if legacy:
        for argv in result["legacy"]:
            if argv[:2] != ["openhue", "set"]:
                continue
            if argv[2] == "room":
                name = by_id[argv[3]]["metadata"]["name"]
                body = (
                    {"on": {"on": argv[4] == "--on"}}
                    if argv[4] != "-b"
                    else {"dimming": {"brightness": int(argv[5])}}
                )
                actions.append([name, body])
            else:
                if len(argv) == 6:
                    actions.append([argv[5], {"scene": argv[3]}])
                else:
                    scene = by_id[argv[3]]
                    actions.append(
                        [
                            by_id[scene["group"]["rid"]]["metadata"]["name"],
                            {"scene": scene["metadata"]["name"]},
                        ]
                    )
        return actions
    for raw in result["requests"]:
        header, body = raw.split("\r\n\r\n", 1)
        method, path, _ = header.splitlines()[0].split(" ")
        assert method in ("GET", "PUT"), method
        if method == "GET":
            assert path == "/clip/v2/resource", path
            continue
        target = by_id[path.rsplit("/", 1)[1]]
        value = json.loads(body)
        if target["type"] == "scene":
            assert value == {"recall": {"action": "active"}}, value
            actions.append(
                [
                    by_id[target["group"]["rid"]]["metadata"]["name"],
                    {"scene": target["metadata"]["name"]},
                ]
            )
        else:
            actions.append([by_id[target["owner"]["rid"]]["metadata"]["name"], value])
    assert len(result["requests"]) <= 2, "duplicate reads or writes"
    return actions


def expected(case, legacy=False):
    action, room, value = case["action"], case["room"], case["value"]
    if action in ("power", "scene"):
        body = {"on": {"on": value}} if action == "power" else {"scene": value}
        output = (
            f"{room}: {'on' if value else 'off'}\n"
            if action == "power"
            else f"Room: {room} | Scene: {value}\n"
        )
        if legacy and case.get("direct"):
            output = ""
        return 0, output, [[room, body]]
    if action == "step":
        if legacy:
            level = min(
                100,
                max(
                    0,
                    int(case.get("brightness", 42.75)) + (15 if value == "up" else -15),
                ),
            )
            return (
                0,
                f"{'🔆' if value == 'up' else '🔅'} Room: {room} | Current brightness: {level}%\n",
                [[room, {"dimming": {"brightness": level}}]],
            )
        return (
            0,
            f"{room}: brightness {value}\n",
            [[room, {"dimming_delta": {"action": value, "brightness_delta": 15}}]],
        )
    if action == "absolute":
        return (
            0,
            f"{room}: brightness requested {value}%\n",
            [[room, {"dimming": {"brightness": value}}]],
        )
    if action == "status":
        scene = case.get("active", "Read") or ("" if legacy else "unknown")
        return (
            0,
            f"{room}: ON | brightness: {42 if legacy else 42.75}% | scene: {scene}\n",
            [],
        )
    if action == "unknown-room":
        return (
            (
                1,
                f"Error: Room ID for '{room}' not found. Please ensure the room name is correct.\n",
                [],
            )
            if legacy
            else (2, "", [])
        )
    if action == "unknown-scene":
        return (
            (
                1,
                f"Error: Unable to find the Scene ID for scene '{value}' in room '{room}'.\n",
                [],
            )
            if legacy
            else (3, "", [])
        )
    if action == "usage":
        output = ""
        if legacy:
            argv = case["legacy"]
            if argv[0] in ["--next", "--last", "-n", "-l"]:
                output = f"Invalid: Unknown option: {argv[0]}\n"
            if argv in [["-b", "UP"], ["-b", "-1"]]:
                output = "Error: Direction must be 'up' or 'down'.\n"
        return 1, output, []
    if action == "none":
        return 0, "", []
    if action == "help":
        return 0, None, []
    raise AssertionError(case)


def check(case, result, fixture, legacy=False, capture_notifications=True):
    code, output, changes = expected(case, legacy)
    assert result["exit"] == code, ("exit", code, result["exit"])
    if output is not None:
        assert result["stdout"] == output, ("stdout", output, result["stdout"])
    else:
        assert ("DESCRIPTION" if legacy else HELP) in result["stdout"]
    assert writes(result, fixture, legacy) == changes, (
        "writes",
        changes,
        writes(result, fixture, legacy),
    )
    if legacy or code == 0:
        assert result["stderr"] == "", ("stderr", result["stderr"])
    else:
        assert result["stderr"].startswith("lights: "), result["stderr"]
        if code == 1:
            assert HELP in result["stderr"]
        elif code == 2:
            assert case["room"] in result["stderr"]
        elif code == 3:
            assert (
                case["room"] in result["stderr"] and case["value"] in result["stderr"]
            )
    if not capture_notifications:
        return
    notes = (
        [a for a in result["legacy"] if a[0] == "osascript"]
        if legacy
        else result["notifications"]
    )
    assert len(notes) == int(case["notify"]), ("notifications", notes)
    if notes and not legacy:
        want = [
            result["home"] + "/.cargo/bin/pns",
            "send",
            "--producer",
            "lights",
            "--state",
            "done",
            "--project",
            case["room"],
            "--detail",
            result["stdout"].strip(),
            "--scope",
            "local_only",
        ]
        assert notes[0] == want, ("notification arguments", want, notes[0])
    if notes and legacy:
        action, value = case["action"], case["value"]
        detail = (
            f"{case['room']} lights {'on' if value else 'off'}"
            if action == "power"
            else f"{case['room']} scene: {value}"
        )
        if action == "step":
            detail = f"{case['room']} brightness {value} to {min(100, max(0, 42 + (15 if value == 'up' else -15)))}%"
        assert notes[0] == [
            "osascript",
            "-e",
            f'display notification "{detail}" with title "Smart Lights"',
        ], notes
