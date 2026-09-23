import json

import developer.execution as execution
import developer.rows as row_model


def just_recipes(cwd):
    data = json.loads(execution.run(["just", "--dump", "--dump-format", "json"], cwd))
    source = data.get("source")
    result = []

    def visit(module, prefix=""):
        for name, recipe in module.get("recipes", {}).items():
            if recipe.get("private"):
                continue
            namepath = prefix + name
            argv = ["just", "--justfile", source, namepath] if source else ["just", namepath]
            parameters = recipe.get("parameters", [])
            required = [
                p for p in parameters if p.get("default") is None and p.get("kind") != "star"
            ]
            rendered = execution.run(["just", "--show", namepath], cwd)
            detail = recipe.get("doc") or ""
            if parameters:
                detail += "\nParameters: " + ", ".join(
                    str(p["name"])
                    + (f"={p['default']}" if p.get("default") is not None else " (required)")
                    for p in parameters
                )
            result.append(
                row_model.command(
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
