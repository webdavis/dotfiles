import argparse
import hashlib
import json
import os
import plistlib
import re
import shlex
import sqlite3
import subprocess
import sys
import time
from pathlib import Path


class ProviderError(RuntimeError):
    pass


def run(argv, cwd=None, check=True, timeout=15):
    try:
        result = subprocess.run(
            argv,
            cwd=cwd,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProviderError(f"{argv[0]}: {error}") from error
    if check and result.returncode:
        raise ProviderError(f"{argv[0]} exited {result.returncode}: {result.stderr.strip()}")
    return result.stdout


def row(identity, label, detail="", value=None, **fields):
    return dict(
        id=hashlib.sha256(str(identity).encode()).hexdigest()[:24],
        label=str(label),
        detail=str(detail),
        value=label if value is None else value,
        **fields,
    )


def notice(label, detail=""):
    return row(("notice", label, detail), label, detail, action="none")


def command(identity, label, argv, detail="", cwd=None, **fields):
    text = shlex.join(argv)
    if cwd:
        text = f"cd {shlex.quote(str(cwd))} && {text}"
    result = row(
        identity,
        label,
        f"{text}\n\n{detail}\nEnter prepares this command.",
        action="command",
        argv=argv,
        **fields,
    )
    if cwd:
        result["text"] = text
    return result


def following(kind, label, detail="", **state):
    return row((kind, state), label, detail, action="next", next=kind, state=state)


def ancestors(cwd):
    path = Path(cwd).resolve()
    return [path, *path.parents]


def nearest(cwd, names):
    for directory in ancestors(cwd):
        for name in names:
            path = directory / name
            if path.is_file():
                return path
    return None


def project_root(cwd):
    for directory in ancestors(cwd):
        if (directory / ".git").exists():
            return directory
    return Path(cwd).resolve()


def files(cwd):
    root = project_root(cwd)
    if (root / ".git").exists():
        output = run(["git", "-C", str(root), "ls-files", "-co", "--exclude-standard", "-z"])
        return [root / path for path in dict.fromkeys(output.split("\0")) if path]
    result = []
    for directory, dirs, names in os.walk(root):
        dirs[:] = sorted(
            name
            for name in dirs
            if name
            not in {
                ".git",
                "node_modules",
                ".build",
                "target",
                ".venv",
                "venv",
                "__pycache__",
            }
        )
        result.extend(Path(directory) / name for name in sorted(names))
    return result


def read_text(path):
    return Path(path).read_text(errors="replace")


def data_file(state, kind, patterns):
    if state.get("file"):
        return Path(state["file"]).expanduser().resolve()
    return [following("files", f"Choose a {kind} file", return_kind=kind, file_patterns=patterns)]


def just_recipes(cwd):
    data = json.loads(run(["just", "--dump", "--dump-format", "json"], cwd))
    source = data.get("source")
    result = []

    def visit(module, prefix=""):
        for name, recipe in module.get("recipes", {}).items():
            if recipe.get("private"):
                continue
            namepath = prefix + name
            argv = ["just", "--justfile", source, namepath] if source else ["just", namepath]
            parameters = recipe.get("parameters", [])
            required = [p for p in parameters if p.get("default") is None and p.get("kind") != "star"]
            rendered = run(["just", "--show", namepath], cwd)
            detail = recipe.get("doc") or ""
            if parameters:
                detail += "\nParameters: " + ", ".join(
                    str(p["name"])
                    + (f"={p['default']}" if p.get("default") is not None else " (required)")
                    for p in parameters
                )
            result.append(
                command(
                    ("just", source, namepath),
                    namepath,
                    argv,
                    f"{detail}\n\n{rendered}",
                    parameters=required,
                )
            )
        for name, child in module.get("modules", {}).items():
            if isinstance(child, dict):
                visit(child, prefix + name + "::")

    visit(data)
    return result


def process_rows(processes, sockets):
    connections = {}
    pid = None
    for line in sockets.splitlines():
        if line.startswith("p") and line[1:].isdigit():
            pid = line[1:]
        elif pid and line.startswith("n"):
            connections.setdefault(pid, []).append(line[1:])
        elif pid and line.startswith("TST=") and connections.get(pid):
            connections[pid][-1] += " " + line[4:]
    rows = []
    for line in processes.splitlines():
        parts = line.split(None, 3)
        if len(parts) != 4 or not parts[0].isdigit():
            continue
        pid, parent, user, executable = parts
        ports = "\n".join(connections.get(pid, []))
        detail = f"PID: {pid}\nParent: {parent}\nUser: {user}\nExecutable: {executable}"
        if ports:
            detail += "\n\nConnections and ports:\n" + ports
        rows.append(
            row(
                ("pid", pid, executable),
                f"{pid:>7}  {executable}  {ports.replace(chr(10), '; ')}",
                detail,
                pid,
                action="insert",
            )
        )
    return rows


def chezmoi_rows():
    data = json.loads(
        run(
            [
                "chezmoi",
                "managed",
                "--include=files,symlinks",
                "--path-style=all",
                "--format=json",
                "--no-tty",
                "--skip-secrets",
            ]
        )
    )
    rows = []
    for item in data.values():
        target, source = item["absolute"], item["sourceAbsolute"]
        rows.append(
            command(
                ("chezmoi", target),
                target,
                ["nvim", "--", source],
                f"Target: {target}\nSource: {source}",
                value=source,
                path=source,
            )
        )
    return rows


def json_rows(path):
    root = json.loads(read_text(path))
    rows = []

    def visit(value, keys):
        expression = "getpath(" + json.dumps(keys, ensure_ascii=False, separators=(",", ":")) + ")"
        text = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False, indent=2)
        rows.append(
            row(
                ("json", str(path), keys),
                expression,
                f"{expression}\n\n{text}",
                text,
                action="copy",
                json_path=keys,
                jq=expression,
                path=str(path),
            )
        )
        children = (
            value.items()
            if isinstance(value, dict)
            else enumerate(value)
            if isinstance(value, list)
            else []
        )
        for key, child in children:
            visit(child, keys + [key])

    visit(root, [])
    return rows


def sqlite_rows(path):
    uri = path.as_uri() + "?mode=ro"
    rows = []
    with sqlite3.connect(uri, uri=True, timeout=1) as connection:
        connection.execute("PRAGMA query_only = ON")
        schema = connection.execute(
            "SELECT type, name, tbl_name, sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name"
        ).fetchall()
        for kind, name, table, definition in schema:
            quoted = '"' + name.replace('"', '""') + '"'
            detail = definition or name
            if kind in ("table", "view"):
                columns = connection.execute(
                    'SELECT name, type, "notnull", dflt_value, pk FROM pragma_table_info(?)',
                    (name,),
                ).fetchall()
                detail += "\n\nColumns:\n" + "\n".join(
                    f"{column[0]}  {column[1]}  {'NOT NULL' if column[2] else ''}  {'PRIMARY KEY' if column[4] else ''}"
                    for column in columns
                )
                indexes = [item[1] for item in schema if item[0] == "index" and item[2] == name]
                if indexes:
                    detail += "\n\nIndexes: " + ", ".join(indexes)
                query = f"SELECT * FROM {quoted} LIMIT 100;"
            else:
                query = (
                    "SELECT sql FROM sqlite_schema WHERE name = " + "'" + name.replace("'", "''") + "';"
                )
            rows.append(
                command(
                    ("sqlite", str(path), kind, name),
                    f"{kind}  {name}",
                    ["sqlite3", "-readonly", "-header", "-column", str(path), query],
                    detail,
                    sql_name=name,
                    value=name,
                )
            )
    return rows


def saved_dir(cwd, state):
    home = Path(
        state.get(
            "state_home",
            os.environ.get("XDG_STATE_HOME", str(Path.home() / ".local/state")),
        )
    )
    manifest = nearest(
        cwd,
        [
            "Cargo.toml",
            "Package.swift",
            "package.json",
            "go.mod",
            "pytest.ini",
            "pyproject.toml",
        ],
    )
    scope = manifest.parent if manifest else project_root(cwd)
    return home / "fzf-pickers" / hashlib.sha256(str(scope).encode()).hexdigest()[:20]


def capture_row(label, argv, cwd, state, framework, category="tests"):
    output = saved_dir(cwd, state) / f"{category}-{framework}.json"
    capture_argv = [
        "python3",
        str(Path(__file__).resolve()),
        "capture",
        "--category",
        category,
        "--output",
        str(output),
        "--cwd",
        str(cwd),
        "--framework",
        framework,
        "--",
        *argv,
    ]
    detail = "Run explicitly to save results for this picker. This can build code, run project scripts and access the network."
    return command(
        ("capture", str(cwd), framework, argv),
        label,
        capture_argv,
        detail,
        source_argv=argv,
        framework=framework,
        capture=True,
        output=str(output),
    )


def capture(category, output, cwd, framework, argv):
    destination = Path(output)
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    result = subprocess.run(argv, cwd=cwd, capture_output=True, text=True, errors="replace", check=False)
    record = {
        "category": category,
        "cwd": cwd,
        "framework": framework,
        "argv": argv,
        "exit_code": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "recorded_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.fchmod(descriptor, 0o600)
    with os.fdopen(descriptor, "w") as handle:
        json.dump(record, handle, ensure_ascii=False)
    print(result.stdout, end="")
    print(result.stderr, end="", file=sys.stderr)
    print(
        f"\nSaved {category} results. Reopen the picker to inspect them. Exit: {result.returncode}",
        file=sys.stderr,
    )
    return result.returncode


def discovery_names(framework, output):
    if framework == "swift":
        return list(
            dict.fromkeys(
                line.strip()
                for line in output.splitlines()
                if re.fullmatch(r"\S+\.\S+/\S+", line.strip())
            )
        )
    if framework == "rust":
        return list(dict.fromkeys(line[:-6] for line in output.splitlines() if line.endswith(": test")))
    if framework == "pytest":
        return list(
            dict.fromkeys(
                line.strip()
                for line in output.splitlines()
                if ".py::" in line and not line.startswith((" ", "<"))
            )
        )
    if framework == "go":
        return list(
            dict.fromkeys(
                line for line in output.splitlines() if re.fullmatch(r"Test\w+|Example\w*|Fuzz\w+", line)
            )
        )
    if framework.startswith("xcode"):
        try:
            data = json.loads(output[output.index("{") :])
        except (ValueError, json.JSONDecodeError):
            return []
        found = []

        def visit(value):
            if isinstance(value, dict):
                identifier = value.get("testIdentifier") or value.get("nodeIdentifier")
                if isinstance(identifier, str) and identifier.count("/") >= 2:
                    found.append(identifier)
                identifiers = value.get("testIdentifiers", [])
                if isinstance(identifiers, list):
                    found.extend(
                        item for item in identifiers if isinstance(item, str) and item.count("/") >= 2
                    )
                for child in value.values():
                    visit(child)
            elif isinstance(value, list):
                for child in value:
                    visit(child)

        visit(data)
        return list(dict.fromkeys(found))
    return []


def saved_tests(cwd, state):
    rows = []
    for path in sorted(saved_dir(cwd, state).glob("tests-*.json")):
        data = json.loads(read_text(path))
        framework = data["framework"]
        base = data["argv"]
        content = data.get("stdout", "") + "\n" + data.get("stderr", "")
        if data.get("exit_code"):
            rows.append(notice(f"Discovery failed: {framework}, exit {data['exit_code']}", content))
        if framework.startswith("xcode-list-"):
            if not data.get("exit_code"):
                rows.extend(xcode_list_rows(data, "tests"))
            continue
        if framework.startswith("xcode-plans-"):
            continue
        for name in discovery_names(framework, data.get("stdout", "")):
            if framework == "swift":
                argv = base[:-1] + ["--filter", "^" + re.escape(name) + "$"]
            elif framework == "rust":
                argv = base[: base.index("--")] + [name, "--", "--exact"]
            elif framework.startswith("xcode"):
                argv = base[: base.index("-enumerate-tests")] + [f"-only-testing:{name}"]
            elif framework == "pytest":
                argv = ["python3", "-m", "pytest", name]
            elif framework == "go":
                argv = ["go", "test", "./...", "-run", "^" + re.escape(name) + "$"]
            else:
                continue
            rows.append(
                command(
                    ("test", framework, str(cwd), name),
                    name,
                    argv,
                    f"Discovered: {data.get('recorded_at', '')}\nFramework: {framework}",
                    cwd=data["cwd"],
                )
            )
    return rows


def bash_tests(path):
    result = []
    for number, line in enumerate(read_text(path).splitlines(), 1):
        match = re.match(r"^\s*(?:function\s+)?(test_[A-Za-z0-9_]+)\s*\(\s*\)\s*\{", line)
        if match:
            result.append((match[1], number))
    return result


def parse_jsonc(text):
    text = re.sub(
        r'("(?:\\.|[^"\\])*")|//[^\n]*|/\*[\s\S]*?\*/',
        lambda match: match[1] or "",
        text,
    )
    text = re.sub(r'("(?:\\.|[^"\\])*")|,\s*([}\]])', lambda match: match[1] or match[2], text)
    return json.loads(text)


def bash_test_command(path, line):
    return ["bashunit", f"{path}:{line}"]


def project_tasks(cwd, category):
    rows = []
    task_file = nearest(cwd, [".overseer/tasks.json", "overseer.json", ".vscode/tasks.json"])
    if task_file:
        data = parse_jsonc(read_text(task_file))
        base = (
            task_file.parent.parent
            if task_file.parent.name in (".overseer", ".vscode")
            else task_file.parent
        )
        if not isinstance(data, dict) or not isinstance(data.get("tasks", []), list):
            return [notice("Invalid project task file", str(task_file))]
        for index, task in enumerate(data.get("tasks", [])):
            if not isinstance(task, dict):
                rows.append(notice("Invalid project task", f"{task_file}: entry {index + 1}"))
                continue
            name = task.get("name", task.get("label", ""))
            if not isinstance(name, str) or not isinstance(task.get("tags", []), list):
                rows.append(
                    notice(
                        "Invalid project task",
                        f"{task_file}: entry {index + 1} has invalid name or tags",
                    )
                )
                continue
            cmd = task.get("cmd", task.get("command"))
            tags = task.get("tags", [])
            group = task.get("group", "")
            if isinstance(group, dict):
                group = group.get("kind", "")
            matches = (
                category == "tests" and ("TEST" in tags or group == "test" or "test" in name.lower())
            ) or (
                category == "diagnostics"
                and (
                    "BUILD" in tags
                    or group == "build"
                    or re.search(r"check|lint|build", name, re.IGNORECASE)
                )
            )
            valid = (
                isinstance(name, str)
                and name
                and (
                    isinstance(cmd, str)
                    and cmd
                    or isinstance(cmd, list)
                    and cmd
                    and all(isinstance(v, str) for v in cmd)
                )
            )
            if not valid:
                rows.append(
                    notice(
                        "Invalid project task",
                        f"{task_file}: entry {index + 1} has invalid name or command",
                    )
                )
                continue
            if not matches:
                continue
            text = cmd if isinstance(cmd, str) else shlex.join(cmd)
            args = task.get("args", [])
            if not isinstance(args, list) or not all(isinstance(arg, str) for arg in args):
                rows.append(notice("Invalid project task arguments", f"{task_file}: {name}"))
                continue
            if args:
                text += " " + shlex.join(args)
            options = task.get("options", {})
            if not isinstance(options, dict):
                rows.append(notice("Invalid project task options", f"{task_file}: {name}"))
                continue
            task_cwd = task.get("cwd", options.get("cwd", str(base)))
            if not isinstance(task_cwd, str) or "${" in text or "${" in task_cwd:
                rows.append(notice(f"{name}: variables require editor task runner", str(task_file)))
                continue
            directory = (base / task_cwd).resolve()
            environment = task.get("env", options.get("env", {}))
            if not isinstance(environment, dict) or any(
                not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key) or not isinstance(value, str)
                for key, value in environment.items()
            ):
                rows.append(notice("Invalid project task environment", f"{task_file}: {name}"))
                continue
            if environment:
                text = (
                    "env "
                    + shlex.join([f"{key}={value}" for key, value in environment.items()])
                    + " bash -c "
                    + shlex.quote(text)
                )
            text = f"cd {shlex.quote(str(directory))} && {text}"
            rows.append(
                row(
                    ("task", str(task_file), name),
                    name,
                    f"{text}\n\n{task.get('desc', '')}\nSource: {task_file}\nEnter prepares this task.",
                    text,
                    action="command",
                    text=text,
                    task=True,
                    path=str(task_file),
                )
            )
    package = nearest(cwd, ["package.json"])
    if package:
        scripts = json.loads(read_text(package)).get("scripts", {})
        pattern = r"test" if category == "tests" else r"check|lint|typecheck|build"
        for name, script in scripts.items():
            if re.search(pattern, name, re.IGNORECASE) and isinstance(script, str):
                rows.append(
                    command(
                        ("npm", str(package), name),
                        f"npm: {name}",
                        ["npm", "run", name],
                        script,
                        cwd=package.parent,
                        task=True,
                    )
                )
    return rows


def nested_projects(cwd, kind):
    rows = []
    for directory, dirs, names in os.walk(cwd):
        dirs[:] = sorted(
            name
            for name in dirs
            if name
            not in {
                ".git",
                "node_modules",
                ".build",
                "target",
                ".venv",
                "venv",
                "__pycache__",
            }
        )
        path = Path(directory)
        markers = set(names) & {
            "Cargo.toml",
            "Package.swift",
            "package.json",
            "go.mod",
            "pytest.ini",
            "pyproject.toml",
        }
        if path != Path(cwd) and markers:
            rows.append(
                following(
                    kind,
                    f"Project: {path.relative_to(cwd)}",
                    ", ".join(sorted(markers)),
                    cwd=str(path),
                )
            )
            dirs[:] = []
    return rows


def xcode_list_rows(record, category):
    data = json.loads(record.get("stdout", "{}"))
    listing = data.get("project", data.get("workspace", {}))
    base = record["argv"][: record["argv"].index("-list")]
    return [
        following(
            "dev-xcode-scheme",
            f"Xcode scheme: {scheme}",
            cwd=record["cwd"],
            xcode_base=base + ["-scheme", scheme],
            xcode_category=category,
        )
        for scheme in listing.get("schemes", [])
        if isinstance(scheme, str)
    ]


def xcode_projects(cwd, state, category):
    rows = []
    for directory, dirs, _ in os.walk(cwd):
        dirs[:] = sorted(
            name
            for name in dirs
            if name not in {".git", "node_modules", ".build", "target", ".venv", "venv", "__pycache__"}
        )
        for name in list(dirs):
            project = Path(directory) / name
            if project.suffix not in (".xcodeproj", ".xcworkspace"):
                continue
            dirs.remove(name)
            flag = "-workspace" if project.suffix == ".xcworkspace" else "-project"
            base = ["xcodebuild", flag, str(project)]
            identity = hashlib.sha256(str(project).encode()).hexdigest()[:12]
            rows.append(
                capture_row(
                    f"Discover Xcode schemes: {name}",
                    base + ["-list", "-json"],
                    cwd,
                    state,
                    "xcode-list-" + identity,
                    category,
                )
            )
            for scheme in sorted(project.glob("**/*.xcscheme")):
                rows.append(
                    following(
                        "dev-xcode-scheme",
                        f"Xcode scheme: {scheme.stem}",
                        cwd=str(cwd),
                        xcode_base=base + ["-scheme", scheme.stem],
                        xcode_category=category,
                    )
                )
    return rows


def xcode_scheme(cwd, state):
    base = state["xcode_base"]
    identity = hashlib.sha256(shlex.join(base).encode()).hexdigest()[:12]
    category = state.get("xcode_category", "tests")
    destination = ["-destination", state["xcode_destination"]] if state.get("xcode_destination") else []
    if category == "diagnostics":
        return [
            capture_row(
                "Build selected Xcode scheme and save diagnostics",
                base + destination + ["build"],
                cwd,
                state,
                "xcode-" + identity,
                "diagnostics",
            )
        ]
    plans = []
    plan_file = saved_dir(cwd, state) / ("tests-xcode-plans-" + identity + ".json")
    if plan_file.is_file():
        data = json.loads(read_text(plan_file))
        output = json.loads(data.get("stdout", "{}"))
        for plan in output.get("testPlans", []):
            name = plan.get("name") if isinstance(plan, dict) else plan
            if isinstance(name, str):
                plans.append(name)
    rows = [
        capture_row(
            "Discover test plans for this scheme",
            base + ["-showTestPlans", "-json"],
            cwd,
            state,
            "xcode-plans-" + identity,
        )
    ]
    for plan in [None, *plans]:
        argv = base + destination + (["-testPlan", plan] if plan else []) + ["test"]
        title = plan or "scheme default plan"
        rows.append(command(("xcode-run", base, plan), "Run Xcode tests: " + title, argv))
        rows.append(
            capture_row(
                "Discover Xcode test identifiers: " + title,
                argv
                + [
                    "-enumerate-tests",
                    "-test-enumeration-style",
                    "flat",
                    "-test-enumeration-format",
                    "json",
                    "-test-enumeration-output-path",
                    "-",
                ],
                cwd,
                state,
                "xcode-" + hashlib.sha256(shlex.join(argv).encode()).hexdigest()[:12],
            )
        )
    rows.extend(item for item in saved_tests(cwd, state) if item.get("argv", [])[: len(base)] == base)
    return rows


def test_rows(cwd, state):
    root = project_root(cwd)
    rows = saved_tests(cwd, state) + project_tasks(cwd, "tests") + nested_projects(cwd, "tests")
    swift = nearest(cwd, ["Package.swift"])
    rust = nearest(cwd, ["Cargo.toml"])
    python = nearest(cwd, ["pytest.ini", "pyproject.toml", "setup.cfg"])
    golang = nearest(cwd, ["go.mod"])
    if swift:
        rows.append(
            capture_row(
                "Discover Swift tests (XCTest and Swift Testing)",
                ["swift", "test", "--package-path", str(swift.parent), "list"],
                swift.parent,
                state,
                "swift",
            )
        )
        rows.append(
            command(
                ("swift-all", str(swift)),
                "Run all Swift tests",
                ["swift", "test", "--package-path", str(swift.parent)],
            )
        )
    if rust:
        base = ["cargo", "test", "--manifest-path", str(rust)]
        rows.append(
            capture_row(
                "Discover Rust tests",
                base + ["--", "--list", "--format", "terse"],
                rust.parent,
                state,
                "rust",
            )
        )
        rows.append(command(("rust-all", str(rust)), "Run all Rust tests", base))
    if python:
        rows.append(
            capture_row(
                "Discover pytest tests",
                ["python3", "-m", "pytest", "--collect-only", "-q"],
                python.parent,
                state,
                "pytest",
            )
        )
    if golang:
        rows.append(
            capture_row(
                "Discover Go tests",
                ["go", "test", "./...", "-list", "."],
                golang.parent,
                state,
                "go",
            )
        )
    for path in files(root):
        if not path.is_relative_to(cwd):
            continue
        if (
            path.suffix == ".sh"
            and not path.name.endswith(".test.sh")
            and path.parent.name in ("unit", "integration", "e2e")
        ):
            rows.append(
                command(
                    ("bash-script", str(path)),
                    str(path.relative_to(root)),
                    ["bash", str(path)],
                    path=str(path),
                )
            )
        if path.name.endswith(".test.sh"):
            for name, line in bash_tests(path):
                rows.append(
                    command(
                        ("bash-test", str(path), name),
                        f"{path.relative_to(root)}: {name}",
                        bash_test_command(path, line),
                        path=str(path),
                        line=line,
                    )
                )
    rows.extend(xcode_projects(cwd, state, "tests"))
    if nearest(cwd, ["justfile", "Justfile", ".justfile"]):
        rows.extend(item for item in just_recipes(cwd) if "test" in item["label"].lower())
    return rows or [
        notice(
            "No supported tests found",
            "Declared project test tasks, Swift, Xcode, Cargo, Go, pytest and Bashunit are supported.",
        )
    ]


def diagnostic_rows(output, cwd):
    result = []
    for index, raw in enumerate(output.splitlines()):
        line = re.sub(r"\x1b\[[0-9;]*[mK]", "", raw)
        if not line.strip():
            continue
        try:
            data = json.loads(line)
        except json.JSONDecodeError:
            data = None
        if isinstance(data, dict) and data.get("reason") == "compiler-message":
            message = data.get("message", {})
            if not isinstance(message, dict):
                continue
            spans = [span for span in message.get("spans", []) if span.get("is_primary")]
            detail = message.get("rendered") or message.get("message", line)
            fields = {}
            if spans:
                fields = {
                    "action": "edit",
                    "path": str((Path(cwd) / spans[0]["file_name"]).resolve()),
                    "line": spans[0]["line_start"],
                    "column": spans[0].get("column_start", 1),
                }
            result.append(
                row(
                    ("diagnostic", index, line),
                    f"{message.get('level', '')}: {message.get('message', '')}",
                    detail,
                    **fields,
                )
            )
            continue
        if isinstance(data, dict) and "reason" in data:
            continue
        match = re.match(r"^(.+?):(\d+)(?::(\d+))?:\s*(.*)$", line)
        if match:
            path = str((Path(cwd) / match[1]).resolve())
            result.append(
                row(
                    ("diagnostic", index, line),
                    line,
                    line,
                    action="edit",
                    path=path,
                    line=int(match[2]),
                    column=int(match[3] or 1),
                )
            )
        else:
            result.append(row(("output", index, line), line, line, action="copy", value=line))
    return result


def quickfix_rows(data, cwd):
    if isinstance(data, dict):
        data = data.get("items", data.get("diagnostics", []))
    if not isinstance(data, list):
        raise ProviderError("Quickfix data must be a list of location records.")
    result = []
    for index, item in enumerate(data):
        if not isinstance(item, dict):
            continue
        path = item.get("filename") or item.get("path")
        detail = item.get("text", item.get("message", ""))
        fields = {
            "line": int(item.get("lnum", item.get("line", 1))),
            "column": int(item.get("col", item.get("column", 1))),
        }
        if path:
            fields["path"] = str((Path(cwd) / path).resolve())
            fields["action"] = "edit"
        result.append(
            row(
                ("quickfix", index, path, detail),
                f"{path or 'output'}:{fields['line']}  {detail}",
                detail,
                **fields,
            )
        )
    return result


def current_quickfix(cwd):
    socket = os.environ.get("NVIM")
    if not socket:
        return []
    expression = (
        "json_encode(map(getqflist(), {_, v -> extend(v, {'filename': "
        "v.bufnr > 0 && !empty(bufname(v.bufnr)) ? fnamemodify(bufname(v.bufnr), ':p') : ''})}))"
    )
    data = json.loads(run(["nvim", "--server", socket, "--remote-expr", expression], cwd))
    return quickfix_rows(data, cwd)


def diagnostics(cwd, state, checks=True):
    result = []
    explicit = state.get("diagnostics_file") or state.get("quickfix_file")
    exported = (
        Path(os.environ.get("XDG_STATE_HOME", str(Path.home() / ".local/state")))
        / "fzf-pickers/quickfix.json"
    )
    if not checks and not explicit and not os.environ.get("NVIM") and exported.is_file():
        explicit = str(exported)
    if explicit:
        text = read_text(explicit)
        try:
            data = json.loads(text)
        except json.JSONDecodeError:
            result.extend(diagnostic_rows(text, cwd))
        else:
            if isinstance(data, dict) and "stdout" in data:
                result.extend(
                    diagnostic_rows(
                        data["stdout"] + "\n" + data.get("stderr", ""),
                        data.get("cwd", cwd),
                    )
                )
                result.insert(
                    0,
                    notice(
                        f"Saved check exited {data.get('exit_code', 'unknown')}",
                        data.get("recorded_at", ""),
                    ),
                )
            else:
                result.extend(quickfix_rows(data, cwd))
    else:
        try:
            result.extend(current_quickfix(cwd))
        except ProviderError as error:
            result.append(notice("Neovim quickfix unavailable", str(error)))
        for path in sorted(saved_dir(cwd, state).glob("diagnostics-*.json")):
            data = json.loads(read_text(path))
            if data.get("framework", "").startswith("xcode-list-") and not data.get("exit_code"):
                result.extend(xcode_list_rows(data, "diagnostics"))
                continue
            result.append(
                notice(
                    f"{data.get('framework', 'check')}: exit {data.get('exit_code', 'unknown')}",
                    data.get("recorded_at", ""),
                )
            )
            result.extend(
                diagnostic_rows(
                    data.get("stdout", "") + "\n" + data.get("stderr", ""),
                    data.get("cwd", cwd),
                )
            )
    if checks:
        result.extend(nested_projects(cwd, "diagnostics"))
        swift = nearest(cwd, ["Package.swift"])
        rust = nearest(cwd, ["Cargo.toml"])
        if swift:
            result.append(
                capture_row(
                    "Run Swift build and save diagnostics",
                    ["swift", "build", "--package-path", str(swift.parent)],
                    swift.parent,
                    state,
                    "swift",
                    "diagnostics",
                )
            )
        if rust:
            result.append(
                capture_row(
                    "Run Cargo check and save diagnostics",
                    [
                        "cargo",
                        "check",
                        "--manifest-path",
                        str(rust),
                        "--message-format=json",
                    ],
                    rust.parent,
                    state,
                    "rust",
                    "diagnostics",
                )
            )
        for item in project_tasks(cwd, "diagnostics"):
            if item.get("text"):
                result.append(
                    capture_row(
                        item["label"] + " (save diagnostics)",
                        ["bash", "-c", item["text"]],
                        cwd,
                        state,
                        item["id"],
                        "diagnostics",
                    )
                )
            else:
                result.append(item)
        if nearest(cwd, ["justfile", "Justfile", ".justfile"]):
            for item in just_recipes(cwd):
                if re.search(r"lint|check|build", item["label"], re.IGNORECASE):
                    result.append(
                        capture_row(
                            item["label"] + " (save diagnostics)",
                            item["argv"],
                            cwd,
                            state,
                            item["id"],
                            "diagnostics",
                        )
                        | {"parameters": item.get("parameters", [])}
                    )
        result.extend(xcode_projects(cwd, state, "diagnostics"))
    return result or [
        notice(
            "No saved results",
            "Use the repository diagnostics picker to prepare a check, or populate the current Neovim quickfix list.",
        )
    ]


def excerpt_rows(paths, root):
    rows = []
    for path in paths:
        if not path.is_file() or path.is_symlink():
            continue
        raw = path.read_bytes()
        if b"\0" in raw:
            continue
        lines = raw.decode(errors="replace").splitlines()
        for start in range(0, len(lines), 60):
            excerpt = "\n".join(lines[start : start + 60])
            name = str(path.relative_to(root)) if path.is_relative_to(root) else str(path)
            text = f"{name}:{start + 1}\n{excerpt}"
            rows.append(
                row(
                    ("context", str(path), start),
                    f"{name}:{start + 1}-{min(start + 60, len(lines))}",
                    text,
                    text,
                    action="copy",
                    path=str(path),
                    line=start + 1,
                )
            )
    return rows


def context_rows(cwd):
    root = project_root(cwd)
    extensions = {
        ".md",
        ".rs",
        ".py",
        ".sh",
        ".swift",
        ".js",
        ".ts",
        ".tsx",
        ".jsx",
        ".lua",
        ".c",
        ".h",
        ".cpp",
        ".go",
        ".toml",
        ".yaml",
        ".yml",
        ".tmpl",
    }
    paths = [path for path in files(root) if path.suffix.lower() in extensions]
    return [
        following("files", "Choose any file for context excerpts", return_kind="dev-excerpts")
    ] + excerpt_rows(paths, root)


def diff_rows(cwd):
    root = project_root(cwd)
    rows = []
    for staged in (False, True):
        args = [
            "git",
            "-C",
            str(root),
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--unified=5",
        ]
        if staged:
            args.append("--cached")
        output = run(args)
        file_header = ""
        for part in re.split(r"(?=^diff --git )", output, flags=re.MULTILINE):
            if not part:
                continue
            chunks = re.split(r"(?=^@@ )", part, flags=re.MULTILINE)
            file_header = chunks[0]
            for index, chunk in enumerate(chunks[1:] or [""]):
                text = ("Staged\n" if staged else "Unstaged\n") + file_header + chunk
                label = file_header.splitlines()[0] + " " + (chunk.splitlines()[0] if chunk else "")
                rows.append(
                    row(
                        ("diff", staged, file_header, index),
                        label,
                        text,
                        text,
                        action="copy",
                    )
                )
    return rows


def worktree_rows(cwd):
    root = project_root(cwd)
    current = {
        path
        for path in run(["git", "-C", str(root), "diff", "--name-only", "-z", "HEAD"]).split("\0")
        if path
    }
    current.update(
        path
        for path in run(
            ["git", "-C", str(root), "ls-files", "--others", "--exclude-standard", "-z"]
        ).split("\0")
        if path
    )
    rows = []
    output = run(["git", "-C", str(root), "worktree", "list", "--porcelain", "-z"])
    for record in output.split("\0\0"):
        values = dict(field.split(" ", 1) for field in record.split("\0") if " " in field)
        path = values.get("worktree")
        if not path or Path(path).resolve() == root:
            continue
        try:
            changed = {
                name
                for name in run(["git", "-C", path, "diff", "--name-only", "-z", "HEAD"]).split("\0")
                if name
            }
            changed.update(
                name
                for name in run(
                    [
                        "git",
                        "-C",
                        path,
                        "ls-files",
                        "--others",
                        "--exclude-standard",
                        "-z",
                    ]
                ).split("\0")
                if name
            )
            overlaps = sorted(current & changed)
            detail = (
                f"Worktree: {path}\nBranch: {values.get('branch', 'detached')}\n\nOverlapping paths ({len(overlaps)}):\n"
                + "\n".join(overlaps)
            )
            detail += "\n\nPath overlap indicates shared work, not a proven merge conflict.\n\n"
            for name in overlaps:
                detail += run(
                    [
                        "git",
                        "-C",
                        path,
                        "diff",
                        "--no-ext-diff",
                        "--no-textconv",
                        "HEAD",
                        "--",
                        name,
                    ]
                )
            rows.append(
                row(
                    ("worktree", path),
                    f"{len(overlaps):>3} overlaps  {path}",
                    detail,
                    detail,
                    action="copy",
                    path=path,
                )
            )
        except ProviderError as error:
            rows.append(notice(path, str(error)))
    return rows


def instruction_rows(cwd):
    paths = set()
    for directory in ancestors(cwd):
        paths.update(
            path
            for name in ("AGENTS.md", "CLAUDE.md", "GEMINI.md")
            if (path := directory / name).is_file()
        )
    for path in files(cwd):
        if path.name in ("AGENTS.md", "CLAUDE.md", "SKILL.md") or "handoff" in path.name.lower():
            paths.add(path)
    paths.update((Path.home() / ".agents/skills").glob("*/SKILL.md"))
    return [
        row(
            ("instructions", str(path)),
            str(path),
            read_text(path),
            f"{path}\n{read_text(path)}",
            action="copy",
            path=str(path),
            line=1,
        )
        for path in sorted(paths)
        if path.is_file()
    ]


def container_rows(items, cwd):
    rows = []
    for item in items:
        identifier = item.get("ID", item.get("Id", ""))
        if not identifier:
            continue
        name = item.get("Names", item.get("Name", identifier))
        fields = {
            key: item[key]
            for key in (
                "ID",
                "Name",
                "Names",
                "Image",
                "State",
                "Status",
                "Ports",
                "Service",
                "Project",
            )
            if key in item
        }
        detail = "\n".join(f"{key}: {value}" for key, value in fields.items())
        rows.append(
            command(
                ("docker", identifier),
                f"Docker: {name}",
                [
                    "docker",
                    "inspect",
                    "--format",
                    "{{.Name}}  {{.State.Status}}  {{.Config.Image}}",
                    identifier,
                ],
                detail,
                value=identifier,
                service_kind="docker",
            )
        )
    return rows


def services(cwd):
    rows = []
    sources = [
        ("brew", ["brew", "services", "list", "--json"]),
        ("launchd", ["launchctl", "list"]),
        ("docker", ["docker", "ps", "-a", "--format", "{{json .}}"]),
        ("compose", ["docker", "compose", "ps", "--all", "--format", "json"]),
    ]
    for source, argv in sources:
        try:
            output = run(argv, cwd, timeout=8)
            if source == "brew":
                for item in json.loads(output):
                    name = item.get("name", "")
                    detail = "\n".join(
                        f"{key}: {item[key]}"
                        for key in (
                            "name",
                            "status",
                            "user",
                            "file",
                            "pid",
                            "exit_code",
                        )
                        if key in item
                    )
                    logs = []
                    if item.get("file") and Path(item["file"]).is_file():
                        with open(item["file"], "rb") as handle:
                            plist = plistlib.load(handle)
                        logs = [
                            plist[key]
                            for key in ("StandardOutPath", "StandardErrorPath")
                            if isinstance(plist.get(key), str)
                        ]
                    rows.append(
                        command(
                            ("brew", name),
                            f"Homebrew: {name}",
                            ["brew", "services", "info", name],
                            detail,
                            value=name,
                            service_kind="brew",
                            logs=list(dict.fromkeys(logs)),
                        )
                    )
            elif source == "launchd":
                for line in output.splitlines()[1:]:
                    parts = line.split(None, 2)
                    if len(parts) == 3:
                        pid, status, label = parts
                        rows.append(
                            command(
                                ("launchd", label),
                                f"launchd: {label}",
                                ["launchctl", "list", label],
                                f"Label: {label}\nPID: {pid}\nExit status: {status}",
                                value=label,
                                service_kind="launchd",
                                pid=pid,
                            )
                        )
            else:
                if output.lstrip().startswith("["):
                    items = json.loads(output)
                else:
                    items = [json.loads(line) for line in output.splitlines() if line.strip()]
                existing = {item["value"] for item in rows if item.get("service_kind") == "docker"}
                rows.extend(item for item in container_rows(items, cwd) if item["value"] not in existing)
        except (ProviderError, json.JSONDecodeError) as error:
            rows.append(notice(f"{source} unavailable", str(error)))
    return rows


def collect(kind, state):
    cwd = Path(state.get("cwd", os.getcwd())).resolve()
    try:
        if kind == "just":
            return just_recipes(cwd)
        if kind == "processes":
            processes = run(["ps", "-axo", "pid=,ppid=,user=,comm="])
            sockets = run(["lsof", "-nP", "-i", "-FpcnT"], check=False)
            return process_rows(processes, sockets)
        if kind == "chezmoi":
            return chezmoi_rows()
        if kind == "provenance":
            if not state.get("provenance_file"):
                return [notice("Parent shell command inventory unavailable")]
            data = json.loads(read_text(state["provenance_file"]))
            return [
                row(
                    ("provenance", item["name"]),
                    f"{item['name']}  {item.get('kind', '')}",
                    item.get("detail", ""),
                    item.get("value", item["name"]),
                    action="insert",
                )
                for item in data
            ]
        if kind == "tests":
            return test_rows(cwd, state)
        if kind == "dev-xcode-scheme":
            return xcode_scheme(cwd, state)
        if kind == "diagnostics":
            return diagnostics(cwd, state)
        if kind == "quickfix":
            return diagnostics(cwd, state, checks=False)
        if kind in ("sqlite", "json"):
            path = data_file(
                state,
                kind,
                [".sqlite", ".sqlite3", ".db"] if kind == "sqlite" else [".json"],
            )
            if isinstance(path, list):
                return path
            return sqlite_rows(path) if kind == "sqlite" else json_rows(path)
        if kind == "services":
            return services(cwd)
        if kind == "agents":
            return [
                following("dev-context", "Copy source excerpts with paths and lines"),
                following("dev-diffs", "Copy staged or unstaged diff hunks"),
                following("dev-failures", "Copy saved failures and diagnostics"),
                following("dev-overlaps", "Inspect overlapping worktree changes"),
                following("dev-instructions", "Find instructions, skills and handoffs"),
                following("quickfix", "Navigate current quickfix results"),
            ]
        if kind == "dev-context":
            return context_rows(cwd)
        if kind == "dev-excerpts":
            return excerpt_rows([Path(state["file"]).resolve()], project_root(cwd))
        if kind == "dev-diffs":
            return diff_rows(cwd)
        if kind == "dev-overlaps":
            return worktree_rows(cwd)
        if kind == "dev-instructions":
            return instruction_rows(cwd)
        if kind == "dev-failures":
            return [
                dict(item, action="copy", value=item["detail"])
                for item in diagnostics(cwd, state, checks=False)
            ]
        return [notice(f"Unknown developer picker: {kind}")]
    except (OSError, ValueError, sqlite3.Error, ProviderError) as error:
        return [notice(f"{kind} unavailable", str(error))]


def prompt(label):
    try:
        with open("/dev/tty", "r+") as terminal:
            terminal.write(label + ": ")
            terminal.flush()
            value = terminal.readline()
            return value.rstrip("\n") if value else None
    except (OSError, KeyboardInterrupt):
        return None


def bindings(kind, state):
    if kind == "dev-xcode-scheme":
        return {"alt-d": "Choose Xcode destination"}
    if kind == "json":
        return {"alt-j": "Copy jq expression"}
    if kind == "services":
        return {"alt-d": "Prepare service logs", "alt-e": "Prepare container shell"}
    if kind == "processes":
        return {"alt-d": "Prepare process connection details"}
    if kind == "chezmoi":
        return {"alt-t": "Copy target path"}
    return {}


def accept(kind, rows, key, state):
    if kind == "dev-xcode-scheme" and key == "alt-d":
        destination = prompt("Destination, for example platform=macOS (blank for default)")
        return {
            "type": "reload",
            "state": {} if destination is None else {"xcode_destination": destination},
        }
    if not rows:
        return None
    first = rows[0]
    if kind == "json" and key == "alt-j":
        return {
            "type": "copy",
            "text": "\n".join(item["jq"] for item in rows if "jq" in item),
        }
    if kind == "services":
        service = first.get("service_kind")
        value = first["value"]
        if key == "alt-e" and service == "docker":
            return {"type": "command", "argv": ["docker", "exec", "-it", value, "sh"]}
        if key == "alt-d":
            if service == "docker":
                return {
                    "type": "command",
                    "argv": ["docker", "logs", "--tail", "200", value],
                }
            if service == "launchd" and first.get("pid", "").isdigit():
                predicate = "processIdentifier == " + first["pid"]
                return {
                    "type": "command",
                    "argv": [
                        "/usr/bin/log",
                        "show",
                        "--last",
                        "1h",
                        "--predicate",
                        predicate,
                    ],
                }
            if service == "brew" and first.get("logs"):
                return {
                    "type": "command",
                    "argv": ["tail", "-n", "200", "--", *first["logs"]],
                }
            return {
                "type": "reload",
                "state": {"status": "No current process or log file for this service."},
            }
    if kind == "processes" and key == "alt-d":
        return {
            "type": "command",
            "argv": ["lsof", "-nP", "-a", "-p", first["value"], "-i"],
        }
    if kind == "chezmoi" and key == "alt-t":
        return {"type": "copy", "text": "\n".join(item["label"] for item in rows)}
    if key in ("enter", "") and first.get("parameters"):
        argv = list(first["argv"])
        for parameter in first["parameters"]:
            value = prompt(parameter["name"])
            if value is None:
                return {"type": "reload", "state": {}}
            argv.append(value)
        return {"type": "command", "argv": argv}
    if key in ("enter", "") and first.get("text") and first.get("action") == "command":
        return {"type": "command", "text": first["text"]}
    return None


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    recorder = commands.add_parser("capture")
    recorder.add_argument("--category", choices=["tests", "diagnostics"], required=True)
    recorder.add_argument("--output", required=True)
    recorder.add_argument("--cwd", required=True)
    recorder.add_argument("--framework", required=True)
    recorder.add_argument("argv", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    argv = arguments.argv[1:] if arguments.argv[:1] == ["--"] else arguments.argv
    if not argv:
        parser.error("capture needs a command after --")
    try:
        sys.exit(
            capture(
                arguments.category,
                arguments.output,
                arguments.cwd,
                arguments.framework,
                argv,
            )
        )
    except OSError as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
