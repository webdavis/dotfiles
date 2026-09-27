import json
import re

import developer.capture as recording
import developer.projects as project_paths
import developer.rows as row_model


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
        return list(
            dict.fromkeys(line[:-6] for line in output.splitlines() if line.endswith(": test"))
        )
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
                line
                for line in output.splitlines()
                if re.fullmatch(r"Test\w+|Example\w*|Fuzz\w+", line)
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
                        item
                        for item in identifiers
                        if isinstance(item, str) and item.count("/") >= 2
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
    for path in sorted(recording.saved_dir(cwd, state).glob("tests-*.json")):
        data = json.loads(project_paths.read_text(path))
        framework = data["framework"]
        base = data["argv"]
        content = data.get("stdout", "") + "\n" + data.get("stderr", "")
        if data.get("exit_code"):
            rows.append(
                row_model.notice(
                    f"Discovery failed: {framework}, exit {data['exit_code']}", content
                )
            )
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
                row_model.command(
                    ("test", framework, str(cwd), name),
                    name,
                    argv,
                    f"Discovered: {data.get('recorded_at', '')}\nFramework: {framework}",
                    cwd=data["cwd"],
                )
            )
    return rows


def xcode_list_rows(record, category):
    data = json.loads(record.get("stdout", "{}"))
    listing = data.get("project", data.get("workspace", {}))
    base = record["argv"][: record["argv"].index("-list")]
    return [
        row_model.following(
            "dev-xcode-scheme",
            f"Xcode scheme: {scheme}",
            cwd=record["cwd"],
            xcode_base=base + ["-scheme", scheme],
            xcode_category=category,
        )
        for scheme in listing.get("schemes", [])
        if isinstance(scheme, str)
    ]
