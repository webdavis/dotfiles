import json
from unittest.mock import patch

import developer as dev
from developer import execution, selection
from developer.discovery import bash
from developer.tasks import project


def test_declared_tasks_preserve_arguments(cwd, state):
    tasks = cwd / ".overseer"
    tasks.mkdir()
    (tasks / "tasks.json").write_text(
        json.dumps(
            {
                "tasks": [
                    {
                        "name": "unit tests",
                        "cmd": ["runner", "a b"],
                        "tags": ["TEST"],
                        "cwd": "sub",
                    },
                    {"name": "bad", "cmd": ["runner", 1]},
                ]
            }
        )
    )
    rows = project.project_tasks(cwd, "tests")
    task = next(row for row in rows if row["label"] == "unit tests")
    assert "'a b'" in task["text"] and str(cwd / "sub") in task["text"]
    assert any("invalid" in row["label"].lower() for row in rows)


def test_just_parameters_are_prompted(cwd, state):
    (cwd / "justfile").write_text("hello who:\n    echo {{who}}\n")
    just_dump = {
        "source": str(cwd / "justfile"),
        "recipes": {"hello": {"parameters": [{"name": "who", "default": None}], "private": False}},
        "modules": {},
    }
    with patch.object(
        execution, "run", side_effect=[json.dumps(just_dump), "hello who:\n    echo {{who}}\n"]
    ):
        recipe = dev.collect("just", state)[0]
    assert recipe["label"] == "hello" and recipe["parameters"][0]["name"] == "who"
    assert recipe["argv"][-1] == "hello" and "echo" in recipe["detail"]
    with patch.object(selection, "prompt", return_value="a b"):
        assert dev.accept("just", [recipe], "enter", state)["argv"][-1] == "a b"


def test_malformed_tasks_and_jsonc(cwd, state):
    tasks = cwd / ".overseer"
    tasks.mkdir()
    script = cwd / "things.test.sh"
    (tasks / "tasks.json").write_text(
        json.dumps(
            {
                "tasks": [
                    {"name": 1, "cmd": "broken", "tags": 4},
                    {"name": "test good", "cmd": ["runner"], "tags": ["TEST"]},
                ]
            }
        )
    )
    assert any(item["label"] == "test good" for item in project.project_tasks(cwd, "tests"))
    bad_jsonc = '{"tasks": [/* comment */ {"label": "test one", "command": "echo test",},],}'
    assert project.parse_jsonc(bad_jsonc)["tasks"][0]["label"] == "test one"
    assert project.parse_jsonc('{"text":"// literal,}"}')["text"] == "// literal,}"
    assert bash.bash_test_command(script, 1) == ["bashunit", str(script) + ":1"]
