import json
import re
import shlex

import developer.projects as project_paths
import developer.rows as row_model


def parse_jsonc(text):
    text = re.sub(
        r'("(?:\\.|[^"\\])*")|//[^\n]*|/\*[\s\S]*?\*/',
        lambda match: match[1] or "",
        text,
    )
    text = re.sub(r'("(?:\\.|[^"\\])*")|,\s*([}\]])', lambda match: match[1] or match[2], text)
    return json.loads(text)


def project_tasks(cwd, category):
    rows = []
    task_file = project_paths.nearest(
        cwd, [".overseer/tasks.json", "overseer.json", ".vscode/tasks.json"]
    )
    if task_file:
        data = parse_jsonc(project_paths.read_text(task_file))
        base = (
            task_file.parent.parent
            if task_file.parent.name in (".overseer", ".vscode")
            else task_file.parent
        )
        if not isinstance(data, dict) or not isinstance(data.get("tasks", []), list):
            return [row_model.notice("Invalid project task file", str(task_file))]
        for index, task in enumerate(data.get("tasks", [])):
            if not isinstance(task, dict):
                rows.append(
                    row_model.notice("Invalid project task", f"{task_file}: entry {index + 1}")
                )
                continue
            name = task.get("name", task.get("label", ""))
            if not isinstance(name, str) or not isinstance(task.get("tags", []), list):
                rows.append(
                    row_model.notice(
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
                category == "tests"
                and ("TEST" in tags or group == "test" or "test" in name.lower())
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
                    row_model.notice(
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
                rows.append(
                    row_model.notice("Invalid project task arguments", f"{task_file}: {name}")
                )
                continue
            if args:
                text += " " + shlex.join(args)
            options = task.get("options", {})
            if not isinstance(options, dict):
                rows.append(
                    row_model.notice("Invalid project task options", f"{task_file}: {name}")
                )
                continue
            task_cwd = task.get("cwd", options.get("cwd", str(base)))
            if not isinstance(task_cwd, str) or "${" in text or "${" in task_cwd:
                rows.append(
                    row_model.notice(
                        f"{name}: variables require editor task runner", str(task_file)
                    )
                )
                continue
            directory = (base / task_cwd).resolve()
            environment = task.get("env", options.get("env", {}))
            if not isinstance(environment, dict) or any(
                not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key) or not isinstance(value, str)
                for key, value in environment.items()
            ):
                rows.append(
                    row_model.notice("Invalid project task environment", f"{task_file}: {name}")
                )
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
                row_model.row(
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
    package = project_paths.nearest(cwd, ["package.json"])
    if package:
        scripts = json.loads(project_paths.read_text(package)).get("scripts", {})
        pattern = r"test" if category == "tests" else r"check|lint|typecheck|build"
        for name, script in scripts.items():
            if re.search(pattern, name, re.IGNORECASE) and isinstance(script, str):
                rows.append(
                    row_model.command(
                        ("npm", str(package), name),
                        f"npm: {name}",
                        ["npm", "run", name],
                        script,
                        cwd=package.parent,
                        task=True,
                    )
                )
    return rows
