import importlib.util
import io
import json
import sqlite3
import sys
import tempfile
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

root = Path(__file__).resolve().parents[2]
module = root / "dot_local/libexec/fzf-pickers/developer.py"
assert module.exists(), "developer picker provider is not implemented"
spec = importlib.util.spec_from_file_location("developer", module)
dev = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dev)

with tempfile.TemporaryDirectory(prefix="fzf-developer-") as scratch:
    cwd = Path(scratch).resolve()
    state = {"cwd": str(cwd), "state_home": str(cwd / "state")}
    data = cwd / "odd name.json"
    data.write_text(json.dumps({"a.b": {'quo"te': [None, False, "hello\nworld"]}}))
    rows = dev.collect("json", dict(state, file=str(data)))
    leaf = next(row for row in rows if row.get("json_path") == ["a.b", 'quo"te', 2])
    assert leaf["value"] == "hello\nworld"
    assert dev.accept("json", [leaf], "alt-j", state)["text"] == 'getpath(["a.b","quo\\"te",2])'
    assert any(row["value"] == "null" for row in rows)
    db = cwd / "space.sqlite"
    with sqlite3.connect(db) as connection:
        connection.execute('CREATE TABLE "say""hi" ("a b" TEXT, n INTEGER)')
        connection.execute('CREATE INDEX named_idx ON "say""hi" (n)')
    before = db.read_bytes()
    rows = dev.collect("sqlite", dict(state, file=str(db)))
    table = next(row for row in rows if row.get("sql_name") == 'say"hi')
    assert "a b" in table["detail"] and "named_idx" in table["detail"]
    assert table["argv"][-1] == 'SELECT * FROM "say""hi" LIMIT 100;'
    assert db.read_bytes() == before
    assert dev.collect("sqlite", state)[0]["next"] == "files"
    diagnostic = {
        "reason": "compiler-message",
        "message": {
            "level": "error",
            "message": "bad type",
            "spans": [
                {
                    "file_name": "src/a.rs",
                    "line_start": 4,
                    "column_start": 2,
                    "is_primary": True,
                }
            ],
        },
    }
    rows = dev.diagnostic_rows(
        json.dumps(diagnostic) + "\nfoo.swift:8:3: warning: unused\nfatal: build failed",
        cwd,
    )
    assert rows[0]["line"] == 4 and rows[0]["path"] == str(cwd / "src/a.rs")
    assert dev.accept("diagnostics", rows[:2], "enter", state) is None
    assert all(item["action"] == "edit" for item in rows[:2])
    locations = dev.quickfix_rows([{"filename": "a", "lnum": 2}, {"filename": "b", "lnum": 4}], cwd)
    assert dev.accept("quickfix", locations, "enter", state) is None
    assert all(item["action"] == "edit" for item in locations)
    assert rows[1]["line"] == 8 and "unused" in rows[1]["detail"]
    assert "build failed" in rows[-1]["detail"] and not rows[-1].get("path")
    assert dev.discovery_names(
        "swift",
        "Build complete!\nDemoTests.TestClass/testOne\nDemoTests.Other/testTwo()",
    ) == ["DemoTests.TestClass/testOne", "DemoTests.Other/testTwo()"]
    assert dev.discovery_names("rust", "a::b: test\nc::d: benchmark\n2 tests, 0 benchmarks") == ["a::b"]
    assert dev.discovery_names(
        "xcode", json.dumps({"values": [{"testIdentifier": "AppTests/MyCase/testOne"}]})
    ) == ["AppTests/MyCase/testOne"]
    script = cwd / "things.test.sh"
    script.write_text("function test_safe() { :; }\n# function test_no() { :; }\ntest_second() { :; }\n")
    assert dev.bash_tests(script) == [("test_safe", 1), ("test_second", 3)]
    manifest = cwd / "Package.swift"
    manifest.write_text("// swift-tools-version: 6.0\n")
    with patch.object(
        dev,
        "run",
        side_effect=AssertionError("discovery must not run during collection"),
    ):
        rows = dev.collect("tests", state)
    swift = next(row for row in rows if row.get("framework") == "swift" and row.get("capture"))
    assert swift["source_argv"][:3] == ["swift", "test", "--package-path"]
    assert swift["argv"][-1] == "list"
    result = dev.accept("tests", [swift], "enter", state)
    assert result is None or result["type"] == "command"
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
    rows = dev.project_tasks(cwd, "tests")
    task = next(row for row in rows if row["label"] == "unit tests")
    assert "'a b'" in task["text"] and str(cwd / "sub") in task["text"]
    assert any("invalid" in row["label"].lower() for row in rows)
    process = dev.process_rows(
        " 12 1 me /usr/bin/server\n 13 1 me /bin/worker",
        "p12\ncserver\nf4\nn*:8080\nTST=LISTEN\n",
    )
    assert process[0]["value"] == "12" and "8080" in process[0]["detail"]
    assert len(process) == 2
    card = dev.container_rows(
        [
            {
                "ID": "abc",
                "Names": "web",
                "State": "running",
                "Image": "app:1",
                "Environment": {"PASSWORD": "never show"},
            }
        ],
        cwd,
    )[0]
    assert "PASSWORD" not in card["detail"] and "never show" not in card["detail"]
    assert dev.accept("services", [card], "alt-e", state)["argv"] == [
        "docker",
        "exec",
        "-it",
        "abc",
        "sh",
    ]
    provenance = cwd / "provenance.json"
    provenance.write_text(
        json.dumps(
            [
                {
                    "name": "ll",
                    "kind": "alias",
                    "detail": "ll is aliased to `ls -l'",
                    "value": "ll",
                }
            ]
        )
    )
    assert dev.collect("provenance", dict(state, provenance_file=str(provenance)))[0]["value"] == "ll"
    quickfix = cwd / "quickfix.json"
    quickfix.write_text(json.dumps([{"filename": "src/a.rs", "lnum": 10, "col": 2, "text": "problem"}]))
    assert dev.collect("quickfix", dict(state, quickfix_file=str(quickfix)))[0]["line"] == 10
    editor_root = cwd / "editor-project"
    shell_cwd = editor_root / "subdir"
    shell_cwd.mkdir(parents=True)
    original_run = dev.run

    def remote_expression(argv, _cwd):
        return original_run(
            [
                "nvim",
                "--headless",
                "--clean",
                "-n",
                "-i",
                "NONE",
                "-c",
                "call setqflist([{'filename': 'src/a.swift', 'lnum': 4}, {'text': 'unlocated warning'}])",
                "-c",
                "lua io.write(vim.fn.eval(" + json.dumps(argv[-1]) + "))",
                "-c",
                "qa!",
            ],
            editor_root,
        )

    with (
        patch.dict(dev.os.environ, {"NVIM": "test-editor"}),
        patch.object(dev, "run", side_effect=remote_expression),
    ):
        editor_rows = dev.current_quickfix(shell_cwd)
    assert editor_rows[0]["path"] == str(editor_root / "src/a.swift")
    assert "path" not in editor_rows[1]
    (cwd / "justfile").write_text("hello who:\n    echo {{who}}\n")
    just_dump = {
        "source": str(cwd / "justfile"),
        "recipes": {"hello": {"parameters": [{"name": "who", "default": None}], "private": False}},
        "modules": {},
    }
    with patch.object(dev, "run", side_effect=[json.dumps(just_dump), "hello who:\n    echo {{who}}\n"]):
        recipe = dev.just_recipes(cwd)[0]
    assert recipe["label"] == "hello" and recipe["parameters"][0]["name"] == "who"
    assert recipe["argv"][-1] == "hello" and "echo" in recipe["detail"]
    with patch.object(dev, "prompt", return_value="a b"):
        assert dev.accept("just", [recipe], "enter", state)["argv"][-1] == "a b"
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
    assert any(item["label"] == "test good" for item in dev.project_tasks(cwd, "tests"))
    bad_jsonc = '{"tasks": [/* comment */ {"label": "test one", "command": "echo test",},],}'
    assert dev.parse_jsonc(bad_jsonc)["tasks"][0]["label"] == "test one"
    assert dev.parse_jsonc('{"text":"// literal,}"}')["text"] == "// literal,}"
    assert dev.bash_test_command(script, 1) == ["bashunit", str(script) + ":1"]
    module_dir = cwd / "subproject"
    module_dir.mkdir()
    (module_dir / "Cargo.toml").write_text('[package]\nname="nested"\nversion="0.1.0"\n')
    with patch.object(
        dev,
        "run",
        side_effect=AssertionError("project selection must not execute discovery"),
    ):
        nested = dev.nested_projects(cwd, "tests")
    assert nested[0]["state"]["cwd"] == str(module_dir) and nested[0]["next"] == "tests"
    source = cwd / "source.py"
    source.write_text("one\ntwo\n")
    excerpts = dev.excerpt_rows([source], cwd)
    assert excerpts[0]["value"] == "source.py:1\none\ntwo"
    assert excerpts[0]["line"] == 1
    second_module = cwd / "other-project"
    second_module.mkdir()
    (second_module / "Cargo.toml").write_text('[package]\nname="other"\nversion="0.1.0"\n')
    deeper = module_dir / "src"
    deeper.mkdir()
    assert dev.saved_dir(module_dir, state) != dev.saved_dir(second_module, state)
    assert dev.saved_dir(module_dir, state) == dev.saved_dir(deeper, state)
    (cwd / "justfile").write_text("check thing:\n    echo {{thing}}\n")
    check_recipe = dev.command(
        "check",
        "check",
        ["just", "--justfile", str(cwd / "justfile"), "check"],
        parameters=[{"name": "thing"}],
    )
    with (
        patch.object(dev, "just_recipes", return_value=[check_recipe]),
        patch.object(dev, "current_quickfix", return_value=[]),
    ):
        check = next(
            item for item in dev.diagnostics(cwd, state) if item["label"] == "check (save diagnostics)"
        )
    assert check["parameters"][0]["name"] == "thing"
    with patch.object(dev, "prompt", return_value="a b"):
        accepted = dev.accept("diagnostics", [check], "enter", state)
    assert accepted["argv"][-2:] == ["check", "a b"]
    xproject = cwd / "Native.xcodeproj"
    xproject.mkdir()
    (xproject / "project.pbxproj").write_text("{}")
    project_rows = dev.xcode_projects(cwd, state, "tests")
    assert project_rows[0]["source_argv"] == ["xcodebuild", "-project", str(xproject), "-list", "-json"]
    assert dev.discovery_names("xcode-abc", '{"testIdentifiers":["AppTests/Suite/testOne"]}') == [
        "AppTests/Suite/testOne"
    ]
    listing = {
        "framework": "xcode-list-abc",
        "argv": ["xcodebuild", "-project", str(xproject), "-list", "-json"],
        "cwd": str(cwd),
        "stdout": '{"project":{"schemes":["Native"]}}',
        "stderr": "",
        "exit_code": 0,
    }
    recorded = dev.saved_dir(cwd, state)
    recorded.mkdir(parents=True)
    (recorded / "tests-xcode-list-abc.json").write_text(json.dumps(listing))
    saved = dev.saved_tests(cwd, state)
    scheme = next(item for item in saved if item.get("next") == "dev-xcode-scheme")
    assert scheme["state"]["xcode_base"][-2:] == ["-scheme", "Native"]
    rows = dev.collect("dev-xcode-scheme", dict(state, **scheme["state"]))
    assert any("-showTestPlans" in item.get("source_argv", []) for item in rows)
    assert any("-enumerate-tests" in item.get("source_argv", []) for item in rows)
    output = cwd / "check.json"
    terminal_output = io.StringIO()
    with redirect_stdout(terminal_output), redirect_stderr(terminal_output):
        status = dev.capture(
            "diagnostics",
            str(output),
            str(cwd),
            "rust",
            [sys.executable, "-c", 'print("fatal: broken"); raise SystemExit(3)'],
        )
    assert "fatal: broken" in terminal_output.getvalue()
    assert status == 3 and json.loads(output.read_text())["exit_code"] == 3
    assert output.stat().st_mode & 0o777 == 0o600

print("PASS: developer selections, read-only data, discovery, diagnostics and prepared commands")
