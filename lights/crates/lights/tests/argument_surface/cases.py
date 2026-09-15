# Hand-selected argument partitions and approved legacy-to-Rust spellings.
from itertools import combinations

STUDIO = "3F - Studio"
BEDROOM = "3F - MBedroom"
KITCHEN = "2F - Kitchen"
CASES = []


def add(
    name,
    legacy,
    candidate,
    action=None,
    *,
    room=STUDIO,
    value=None,
    notify=False,
    change="parity",
    **state,
):
    CASES.append(
        dict(
            name=name,
            legacy=legacy,
            candidate=candidate,
            action=action,
            room=room,
            value=value,
            notify=notify,
            change=change,
            **state,
        )
    )


add("default", [], [], "power", value=False)
for args in [["-p"], ["--power"], ["--pow"], ["-pp"], ["--power", "--power"]]:
    add("power-" + "-".join(args), args, ["toggle"], "power", value=False)
# The bedroom row spells the room out. The `bedroom` alias is a deliberate
# divergence: legacy resolves it to `3F - Master Bedroom`, a name the bridge's
# own inventory does not list, and the Rust tool resolves it to `3F - MBedroom`.
for alias, name, on in [
    ("studio", STUDIO, False),
    (BEDROOM, BEDROOM, True),
    ("kitchen", KITCHEN, True),
    ("Custom Room", "Custom Room", True),
]:
    for legacy, rust in [
        (["-r", alias, "-p"], ["--room", alias, "toggle"]),
        (["--power", "--room=" + alias], ["toggle", "--room", alias]),
    ]:
        add("room-" + "-".join(legacy), legacy, rust, "power", room=name, value=on)
add(
    "last-room-wins",
    ["-p", "-r", "kitchen", "-r", "studio"],
    ["toggle", "--room", "studio"],
    "power",
    value=False,
)
for direction in ["up", "down"]:
    for flags in [["-b", direction], ["--brightness=" + direction]]:
        add(
            "brightness-" + "-".join(flags),
            flags,
            ["brightness", direction],
            "step",
            value=direction,
            change="L022-L023",
        )
    for level in [0, 1, 2, 99, 100]:
        add(
            f"brightness-{direction}-at-{level}",
            ["-b", direction],
            ["brightness", direction],
            "step",
            value=direction,
            brightness=level,
            change="L022-L023",
        )
for scene in [
    "Read",
    "Dimmed",
    "Energize",
    "Concentrate",
    "CC Halo Daylight",
    "CC Halo Amber",
    "next",
    "previous",
]:
    expected = {"next": "Energize", "previous": "Dimmed"}.get(scene, scene)
    add("scene-" + scene, ["--scene", scene], ["scene", scene], "scene", value=expected)
for current, next_scene, previous in [
    ("Dimmed", "Read", "Concentrate"),
    ("Read", "Energize", "Dimmed"),
    ("Energize", "Concentrate", "Read"),
    ("Concentrate", "Dimmed", "Energize"),
    ("CC Halo Daylight", "Read", "Read"),
    ("CC Halo Amber", "Read", "Read"),
    (None, "Read", "Read"),
]:
    for direction, expected in [("next", next_scene), ("previous", previous)]:
        add(
            f"rotation-{current}-{direction}",
            ["-s", direction],
            ["scene", direction],
            "scene",
            value=expected,
            active=current,
        )
for short in [["-S"], ["--status"]]:
    add("status-" + short[0], short, ["status"], "status", change="L015")
add("no-static-scene", ["-S"], ["status"], "status", active=None, change="L016")
for legacy, candidate, action, value in [
    (["-pN"], ["toggle", "--notify"], "power", False),
    (["-N", "-b", "up"], ["--notify", "brightness", "up"], "step", "up"),
    (["-s", "Read", "--notify"], ["scene", "Read", "--notify"], "scene", "Read"),
    (["-NS"], ["status", "--notify"], "status", None),
    (["-N", "-N", "-p"], ["toggle", "--notify"], "power", False),
]:
    add(
        "notify-" + "-".join(legacy),
        legacy,
        candidate,
        action,
        value=value,
        notify=action != "status",
        change="L021/L023" if action == "step" else "L021",
    )
# Every subset of conflicting actions, in both orders: Bash selects status, power,
# brightness, then scene. The new grammar spells out that one selected action.
choices = [
    (["-S"], ["status"], "status", None),
    (["-p"], ["toggle"], "power", False),
    (["-b", "up"], ["brightness", "up"], "step", "up"),
    (["-s", "Read"], ["scene", "Read"], "scene", "Read"),
]
for count in range(2, 5):
    for group in combinations(choices, count):
        for reverse in [False, True]:
            ordered = list(reversed(group)) if reverse else group
            argv = [arg for item in ordered for arg in item[0]]
            selected = group[0]
            add(
                "precedence-" + "-".join(argv),
                argv,
                selected[1],
                selected[2],
                value=selected[3],
                change="subcommand mapping/L015/L023",
            )
add(
    "last-brightness-wins",
    ["-b", "down", "-b", "up"],
    ["brightness", "up"],
    "step",
    value="up",
    change="L023",
)
add(
    "last-scene-wins",
    ["-s", "Dimmed", "-s", "Read"],
    ["scene", "Read"],
    "scene",
    value="Read",
)
for suffix in [["ignored"], ["--", "ignored"], ["--", "--status"]]:
    add(
        "ignored-operands-" + "-".join(suffix),
        ["-p", *suffix],
        ["toggle"],
        "power",
        value=False,
        change="subcommand mapping",
    )
for flag in ["--next", "--last", "-n", "-l", "--bogus", "-z", "--s"]:
    add(
        "dropped-or-invalid-" + flag,
        [flag],
        [flag],
        "usage",
        change="dropped flags/usage diagnostic",
    )
for legacy, rust in [
    (["-b"], ["brightness"]),
    (["-s"], ["scene"]),
    (["-r"], ["--room"]),
    (["-b", "UP"], ["brightness", "UP"]),
    (["-b", "-1"], ["brightness", "-1"]),
]:
    add("invalid-" + "-".join(legacy), legacy, rust, "usage", change="usage diagnostic")
add("unknown-command", ["--bogus"], ["bogus"], "usage", change="usage diagnostic")
add(
    "unknown-room",
    ["-S", "-r", "Missing"],
    ["status", "--room", "Missing"],
    "unknown-room",
    room="Missing",
    change="L017",
)
add(
    "unknown-scene",
    ["-s", "Missing"],
    ["scene", "Missing"],
    "unknown-scene",
    value="Missing",
    change="L018",
)
for key, name in [("f4", "CC Halo Daylight"), ("f7", "CC Halo Amber")]:
    add(
        key,
        ["set", "scene", name, "-r", STUDIO],
        ["scene", name],
        "scene",
        value=name,
        direct=True,
        change="direct OpenHue key migrated through lights",
    )
for key, legacy, rust, action, value in [
    ("f5", ["-s", "previous"], ["scene", "previous"], "scene", "Dimmed"),
    ("f6", ["-s", "next"], ["scene", "next"], "scene", "Energize"),
    ("f8", ["-b", "down"], ["brightness", "down"], "step", "down"),
    ("f9", ["-p"], ["toggle"], "power", False),
    ("f10", ["-b", "up"], ["brightness", "up"], "step", "up"),
]:
    add(
        key,
        legacy,
        rust,
        action,
        value=value,
        change="L023" if action == "step" else "parity",
    )
# No corresponding Bash command exists: these have explicit approved expectations.
for command in ["on", "off"]:
    for on in [False, True]:
        add(
            f"new-{command}-{on}",
            None,
            [command],
            "power",
            value=command == "on",
            power=on,
            change="L005",
        )
for text, level in [
    ("0", 1),
    ("1", 1),
    ("2", 2),
    ("99", 99),
    ("100", 100),
    ("101", 100),
    ("18446744073709551615", 100),
    ("0002", 2),
]:
    add(
        "absolute-" + text,
        None,
        ["brightness", text],
        "absolute",
        value=level,
        change="L009/L022",
    )
for argv in [
    ["brightness", "18446744073709551616"],
    ["brightness", "1.2"],
    ["brightness", "+1"],
    ["brightness", "1e2"],
    ["brightness", ""],
    ["toggle", "extra"],
    ["on", "off"],
    ["--room", ""],
    ["--room", " "],
    ["--room", "--notify"],
    ["--room", "studio", "--room", "kitchen"],
    ["--notify", "--notify"],
    ["scene", ""],
    ["scene", "--bogus"],
    ["status", "--bogus"],
    ["--help", "--bogus"],
    ["--room=studio", "toggle"],
    ["-p"],
]:
    add("new-refusal-" + repr(argv), None, argv, "usage", change="new grammar")
for argv in [[], ["--help"], ["--notify", "--help"]]:
    if argv:
        add("help-" + repr(argv), ["--help"], argv, "help", change="new help")
for value in ["", "Read", "next"]:
    if value:
        add(
            "short-attached-scene-" + value,
            ["-s" + value],
            ["scene", value],
            "scene",
            value="Energize" if value == "next" else value,
        )
for legacy in [
    ["--brightness", "up", "-p"],
    ["--scene", "Read", "-S"],
    ["-prstudio"],
    ["--power", "operand", "--room", "studio"],
]:
    status = "-S" in legacy
    add(
        "parser-form-" + repr(legacy),
        legacy,
        ["status"] if status else ["toggle"],
        "status" if status else "power",
        value=None if status else False,
        change="subcommand mapping/L015",
    )
for legacy in [
    ["--help", "-p"],
    ["-p", "--help"],
    ["-hN"],
    ["--help", "-r", "kitchen"],
]:
    add("help-order-" + repr(legacy), legacy, ["--help"], "help", change="help grammar")
for legacy in [
    ["-r", "studio"],
    ["--notify"],
    ["--"],
    ["operand"],
    ["-b", ""],
    ["-s", ""],
]:
    # These legacy forms accept no action. No Rust no-op command was approved:
    # record them as dropped forms, with no candidate spelling rather than invent one.
    add(
        "legacy-no-action-" + repr(legacy),
        legacy,
        None,
        "none",
        change="dropped actionless legacy form; new command grammar",
    )
for legacy in [["--r", "studio", "--p"], ["-bup"], ["--bright", "down"], ["--stat"]]:
    action = (
        "step"
        if legacy[0] in ["-bup", "--bright"]
        else "status"
        if legacy == ["--stat"]
        else "power"
    )
    value = ("up" if legacy == ["-bup"] else "down") if action == "step" else False
    rust = (
        ["brightness", value]
        if action == "step"
        else ["status"]
        if action == "status"
        else ["toggle"]
    )
    add(
        "getopt-prefix-" + repr(legacy),
        legacy,
        rust,
        action,
        value=value,
        change="subcommand mapping/L015/L023",
    )

# Global options can precede, interrupt, or follow the command words.
for position in range(3):
    command = ["scene", "Read"]
    add(
        f"room-option-position-{position}",
        ["-s", "Read", "-r", "studio"],
        command[:position] + ["--room", "studio"] + command[position:],
        "scene",
        value="Read",
    )
    add(
        f"notify-option-position-{position}",
        ["-s", "Read", "-N"],
        command[:position] + ["--notify"] + command[position:],
        "scene",
        value="Read",
        notify=True,
        change="L021",
    )
add(
    "new-option-only-room",
    None,
    ["--room", "kitchen"],
    "power",
    room=KITCHEN,
    value=True,
    change="Rust default command with global room option",
)
add(
    "new-option-only-notify",
    None,
    ["--notify"],
    "power",
    value=False,
    notify=True,
    change="Rust default command with global notify option",
)
