import json
from unittest.mock import patch

import developer as dev
from developer import capture, execution, projects
from developer.discovery import bash, xcode
from developer.discovery import saved as saved_results


def test_discovery_names_and_bash_locations(cwd, state):
    assert saved_results.discovery_names(
        "swift",
        "Build complete!\nDemoTests.TestClass/testOne\nDemoTests.Other/testTwo()",
    ) == ["DemoTests.TestClass/testOne", "DemoTests.Other/testTwo()"]
    assert saved_results.discovery_names(
        "rust", "a::b: test\nc::d: benchmark\n2 tests, 0 benchmarks"
    ) == ["a::b"]
    assert saved_results.discovery_names(
        "xcode", json.dumps({"values": [{"testIdentifier": "AppTests/MyCase/testOne"}]})
    ) == ["AppTests/MyCase/testOne"]
    script = cwd / "things.test.sh"
    script.write_text(
        "function test_safe() { :; }\n# function test_no() { :; }\ntest_second() { :; }\n"
    )
    assert bash.bash_tests(script) == [("test_safe", 1), ("test_second", 3)]


def test_swift_discovery_is_explicit(cwd, state):
    manifest = cwd / "Package.swift"
    manifest.write_text("// swift-tools-version: 6.0\n")
    with patch.object(
        execution,
        "run",
        side_effect=AssertionError("discovery must not run during collection"),
    ):
        rows = dev.collect("tests", state)
    swift = next(row for row in rows if row.get("framework") == "swift" and row.get("capture"))
    assert swift["source_argv"][:3] == ["swift", "test", "--package-path"]
    assert swift["argv"][-1] == "list"
    result = dev.accept("tests", [swift], "enter", state)
    assert result is None or result["type"] == "command"


def test_nested_projects_are_passive(cwd, state):
    module_dir = cwd / "subproject"
    module_dir.mkdir()
    (module_dir / "Cargo.toml").write_text('[package]\nname="nested"\nversion="0.1.0"\n')
    with patch.object(
        execution,
        "run",
        side_effect=AssertionError("project selection must not execute discovery"),
    ):
        nested = projects.nested_projects(cwd, "tests")
    assert nested[0]["state"]["cwd"] == str(module_dir) and nested[0]["next"] == "tests"


def test_saved_results_are_project_scoped(cwd, state):
    module_dir = cwd / "subproject"
    module_dir.mkdir()
    (module_dir / "Cargo.toml").write_text('[package]\nname="nested"\nversion="0.1.0"\n')
    second_module = cwd / "other-project"
    second_module.mkdir()
    (second_module / "Cargo.toml").write_text('[package]\nname="other"\nversion="0.1.0"\n')
    deeper = module_dir / "src"
    deeper.mkdir()
    assert capture.saved_dir(module_dir, state) != capture.saved_dir(second_module, state)
    assert capture.saved_dir(module_dir, state) == capture.saved_dir(deeper, state)


def test_xcode_schemes_and_test_identifiers(cwd, state):
    xproject = cwd / "Native.xcodeproj"
    xproject.mkdir()
    (xproject / "project.pbxproj").write_text("{}")
    project_rows = xcode.xcode_projects(cwd, state, "tests")
    assert project_rows[0]["source_argv"] == [
        "xcodebuild",
        "-project",
        str(xproject),
        "-list",
        "-json",
    ]
    assert saved_results.discovery_names(
        "xcode-abc", '{"testIdentifiers":["AppTests/Suite/testOne"]}'
    ) == ["AppTests/Suite/testOne"]
    listing = {
        "framework": "xcode-list-abc",
        "argv": ["xcodebuild", "-project", str(xproject), "-list", "-json"],
        "cwd": str(cwd),
        "stdout": '{"project":{"schemes":["Native"]}}',
        "stderr": "",
        "exit_code": 0,
    }
    recorded = capture.saved_dir(cwd, state)
    recorded.mkdir(parents=True)
    (recorded / "tests-xcode-list-abc.json").write_text(json.dumps(listing))
    saved = saved_results.saved_tests(cwd, state)
    scheme = next(item for item in saved if item.get("next") == "dev-xcode-scheme")
    assert scheme["state"]["xcode_base"][-2:] == ["-scheme", "Native"]
    rows = dev.collect("dev-xcode-scheme", dict(state, **scheme["state"]))
    assert any("-showTestPlans" in item.get("source_argv", []) for item in rows)
    assert any("-enumerate-tests" in item.get("source_argv", []) for item in rows)
