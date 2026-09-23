import json
import plistlib
from pathlib import Path

import developer.execution as execution
import developer.rows as row_model


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
            row_model.command(
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
            output = execution.run(argv, cwd, timeout=8)
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
                        row_model.command(
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
                            row_model.command(
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
                rows.extend(
                    item for item in container_rows(items, cwd) if item["value"] not in existing
                )
        except (execution.ProviderError, json.JSONDecodeError) as error:
            rows.append(row_model.notice(f"{source} unavailable", str(error)))
    return rows


def accept(rows, key, state):
    if not rows:
        return None
    first = rows[0]
    service = first.get("service_kind")
    value = first["value"]
    if key == "alt-e" and service == "docker":
        return {"type": "command", "argv": ["docker", "exec", "-it", value, "sh"]}
    if key == "alt-d":
        if service == "docker":
            return {"type": "command", "argv": ["docker", "logs", "--tail", "200", value]}
        if service == "launchd" and first.get("pid", "").isdigit():
            predicate = "processIdentifier == " + first["pid"]
            return {
                "type": "command",
                "argv": ["/usr/bin/log", "show", "--last", "1h", "--predicate", predicate],
            }
        if service == "brew" and first.get("logs"):
            return {"type": "command", "argv": ["tail", "-n", "200", "--", *first["logs"]]}
        return {
            "type": "reload",
            "state": {"status": "No current process or log file for this service."},
        }
    return None
