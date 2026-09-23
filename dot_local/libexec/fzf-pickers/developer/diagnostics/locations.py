import json
import os
import re
from pathlib import Path

import developer.execution as execution
import developer.rows as row_model


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
                row_model.row(
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
                row_model.row(
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
            result.append(
                row_model.row(("output", index, line), line, line, action="copy", value=line)
            )
    return result


def quickfix_rows(data, cwd):
    if isinstance(data, dict):
        data = data.get("items", data.get("diagnostics", []))
    if not isinstance(data, list):
        raise execution.ProviderError("Quickfix data must be a list of location records.")
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
            row_model.row(
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
    data = json.loads(execution.run(["nvim", "--server", socket, "--remote-expr", expression], cwd))
    return quickfix_rows(data, cwd)
