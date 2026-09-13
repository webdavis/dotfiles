# Run with --candidate <scripted-lights> --process <lights> --evidence <new directory>.
# The optional --previous is independently built from the pinned main source.
import argparse
import copy
import hashlib
import json
import subprocess
import sys
import time
from pathlib import Path

sys.dont_write_bytecode = True

from cases import CASES
from expectations import check

HERE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare(root):
    root.mkdir(parents=True)
    bins = root / "bin"
    bins.mkdir()
    for name, target in [
        ("bash", "/opt/homebrew/bin/bash"),
        ("jq", "/opt/homebrew/bin/jq"),
        ("date", "/bin/date"),
        ("getopt", "/opt/homebrew/opt/gnu-getopt/bin/getopt"),
    ]:
        (bins / name).symlink_to(target)
    double = (HERE / "command_double.py").read_text()
    for name in ["openhue", "osascript"]:
        target = bins / name
        target.write_text("#!/opt/homebrew/bin/python3\n" + double)
        target.chmod(0o700)
    for name, body in [("which", 'type -P "$1"'), ("tput", "exit 0")]:
        target = bins / name
        target.write_text("#!/opt/homebrew/bin/bash\nset -euo pipefail\n" + body + "\n")
        target.chmod(0o700)
    return bins


def resource(case):
    fixture = json.loads((HERE.parent / "fixtures/resources.json").read_text())
    room = copy.deepcopy(fixture["data"][4])
    group = copy.deepcopy(fixture["data"][5])
    room["id"] = room["id"][:-2] + "30"
    room["metadata"]["name"] = "Custom Room"
    group["id"] = group["id"][:-2] + "31"
    group["owner"]["rid"] = room["id"]
    room["services"][0]["rid"] = group["id"]
    fixture["data"] += [room, group]
    if "brightness" in case:
        fixture["data"][1]["dimming"]["brightness"] = case["brightness"]
    if "power" in case:
        fixture["data"][1]["on"]["on"] = case["power"]
    if "active" in case:
        for entry in fixture["data"]:
            if (
                entry["type"] == "scene"
                and entry["group"]["rid"] == fixture["data"][0]["id"]
            ):
                entry["status"]["active"] = (
                    "static"
                    if entry["metadata"]["name"] == case["active"]
                    else "inactive"
                )
    return fixture


def invoke(executable, argv, root, bins, fixture, *, direct=False, config=None):
    root.mkdir()
    (root / "Library/Logs").mkdir(parents=True)
    (root / "lights").mkdir()
    if config is None:
        config = (
            "[controller]\ntype='hue'\naddress='192.0.2.1'\nkey='private-test-key'\n"
        )
    (root / "lights/config.toml").write_text(config)
    resources = root / "resources.json"
    resources.write_text(json.dumps(fixture))
    env = {
        key: str(root)
        for key in [
            "HOME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "XDG_CACHE_HOME",
            "XDG_RUNTIME_DIR",
            "TMPDIR",
            "TMP",
            "TEMP",
            "CLAUDE_CONFIG_DIR",
        ]
    }
    env.update(
        PATH=str(bins),
        LC_ALL="C",
        GIT_CONFIG_GLOBAL="/dev/null",
        GIT_CONFIG_SYSTEM="/dev/null",
        LIGHTS_TEST_RESOURCES=str(resources),
        LIGHTS_TEST_CAPTURE=str(root / "requests.json"),
        LIGHTS_TEST_NOTIFICATIONS=str(root / "notifications.json"),
        LIGHTS_LEGACY_CAPTURE=str(root / "legacy.jsonl"),
    )
    command = (
        [str(executable), *argv]
        if direct or executable.suffix != ".bash"
        else [str(bins / "bash"), str(executable), *argv]
    )
    before = time.monotonic()
    result = subprocess.run(
        command, env=env, cwd=root, capture_output=True, timeout=0.9, check=False
    )
    elapsed = time.monotonic() - before
    assert elapsed < 1, ("slow invocation", argv, elapsed)
    requests = (
        json.loads((root / "requests.json").read_text())
        if (root / "requests.json").exists()
        else []
    )
    notifications = (
        json.loads((root / "notifications.json").read_text())
        if (root / "notifications.json").exists()
        else []
    )
    legacy = (
        [json.loads(line) for line in (root / "legacy.jsonl").read_text().splitlines()]
        if (root / "legacy.jsonl").exists()
        else []
    )
    return {
        "exit": result.returncode,
        "stdout": result.stdout.decode(),
        "stderr": result.stderr.decode(),
        "requests": requests,
        "notifications": notifications,
        "legacy": legacy,
        "seconds": elapsed,
        "home": str(root),
    }


def argument_surface_matches_reference(
    candidate, previous, root, cases=CASES, process=None
):
    bins = prepare(root)
    reference = HERE / "legacy.bash"
    assert reference.resolve() != candidate.resolve() and digest(reference) != digest(
        candidate
    )
    if previous:
        assert candidate.resolve() != previous.resolve() and digest(
            candidate
        ) != digest(previous)
    (root / "executables.json").write_text(
        json.dumps(
            {
                "legacy_commit": "770c8c75e34227cb790d6bca743eb4ddd9fc29ac",
                "legacy_source": "dot_local/libexec/executable_control-hue-lights.sh",
                "executables": [
                    {"path": str(p.resolve()), "sha256": digest(p)}
                    for p in [reference, candidate, previous, process]
                    if p
                ],
            },
            indent=2,
        )
    )
    rows = []
    for index, case in enumerate(cases):
        case_started = time.monotonic()
        fixture = resource(case)
        row = {"case": case}
        for label, exe, argv in [
            ("legacy", HERE / "legacy.bash", case["legacy"]),
            ("candidate", candidate, case["candidate"]),
            ("previous", previous, case["candidate"]),
        ]:
            if exe is None or argv is None:
                continue
            direct = label == "legacy" and case.get("direct", False)
            if direct:
                exe = bins / "openhue"
            result = invoke(
                exe, argv, root / f"{index:03}-{label}", bins, fixture, direct=direct
            )
            row[label] = result
            try:
                check(case, result, fixture, label == "legacy", label != "previous")
            except AssertionError as error:
                (root / "failure.json").write_text(json.dumps(row, indent=2))
                raise AssertionError((case["name"], label, error.args)) from error
        if previous and case["candidate"] is not None:
            for field in ["exit", "stdout", "stderr", "requests"]:
                assert row["candidate"][field] == row["previous"][field], (
                    case["name"],
                    field,
                )
        if process and case["candidate"] is not None:
            result = invoke(
                process,
                case["candidate"],
                root / f"{index:03}-process",
                bins,
                fixture,
                config="invalid = [",
            )
            expected_exit = (
                0 if case["action"] == "help" else 1 if case["action"] == "usage" else 5
            )
            assert result["exit"] == expected_exit, (
                case["name"],
                "process exit",
                result,
            )
            if expected_exit:
                assert result["stdout"] == "" and result["stderr"].startswith(
                    "lights: "
                ), result
            else:
                assert (
                    result["stdout"] == row["candidate"]["stdout"]
                    and result["stderr"] == ""
                )
            assert (
                not result["legacy"]
                and not result["requests"]
                and not result["notifications"]
            )
            row["process"] = result
        elapsed = time.monotonic() - case_started
        assert elapsed < 1, (case["name"], "case exceeded one second", elapsed)
        rows.append(row)
        print("PASS", case["name"], flush=True)
    if process:
        invalid = invoke(
            process, [b"\xff"], root / "invalid-unicode", bins, resource(CASES[0])
        )
        assert (invalid["exit"], invalid["stdout"], invalid["stderr"]) == (
            1,
            "",
            "lights: arguments must be valid Unicode\n",
        ), invalid
        rows.append({"case": {"name": "invalid-unicode"}, "process": invalid})
        print("PASS invalid-unicode", flush=True)
    (root / "results.json").write_text(json.dumps(rows, indent=2))
    return rows


def argument_surface_rejects_changed_reference_exit(candidate, mutant, root):
    reference = HERE / "legacy.bash"
    assert len({reference.resolve(), candidate.resolve(), mutant.resolve()}) == 3
    assert len({digest(reference), digest(candidate), digest(mutant)}) == 3
    case = next(c for c in CASES if c["name"] == "unknown-command")
    argument_surface_matches_reference(candidate, None, root / "green", [case])
    try:
        argument_surface_matches_reference(mutant, None, root / "mutant", [case])
    except AssertionError as error:
        assert error.args[0] == ("unknown-command", "candidate", (("exit", 1, 0),)), (
            error
        )
        print("PASS argument_surface_rejects_changed_reference_exit", flush=True)
    else:
        raise AssertionError("changed candidate exit was not detected")
    argument_surface_matches_reference(candidate, None, root / "restored", [case])


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--previous", type=Path)
    parser.add_argument("--process", type=Path)
    parser.add_argument("--mutant", type=Path)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    for path in [args.candidate, args.previous, args.mutant]:
        if path:
            assert path.is_absolute() and path.is_file(), path
    if args.mutant:
        argument_surface_rejects_changed_reference_exit(
            args.candidate, args.mutant, args.evidence
        )
    else:
        rows = argument_surface_matches_reference(
            args.candidate, args.previous, args.evidence, process=args.process
        )
        print(
            "PASS argument_surface_matches_reference",
            len(rows),
            "cases; maximum invocation seconds",
            max(
                r[k]["seconds"]
                for r in rows
                for k in ["legacy", "candidate", "previous", "process"]
                if k in r
            ),
        )
