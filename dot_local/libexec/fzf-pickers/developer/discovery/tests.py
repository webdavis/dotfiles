import developer.capture as recording
import developer.discovery.bash as discovery_bash
import developer.discovery.saved as discovery_saved
import developer.discovery.xcode as discovery_xcode
import developer.projects as project_paths
import developer.rows as row_model
import developer.tasks.just as tasks_just
import developer.tasks.project as tasks_project


def test_rows(cwd, state):
    root = project_paths.project_root(cwd)
    rows = (
        discovery_saved.saved_tests(cwd, state)
        + tasks_project.project_tasks(cwd, "tests")
        + project_paths.nested_projects(cwd, "tests")
    )
    swift = project_paths.nearest(cwd, ["Package.swift"])
    rust = project_paths.nearest(cwd, ["Cargo.toml"])
    python = project_paths.nearest(cwd, ["pytest.ini", "pyproject.toml", "setup.cfg"])
    golang = project_paths.nearest(cwd, ["go.mod"])
    if swift:
        rows.append(
            recording.capture_row(
                "Discover Swift tests (XCTest and Swift Testing)",
                ["swift", "test", "--package-path", str(swift.parent), "list"],
                swift.parent,
                state,
                "swift",
            )
        )
        rows.append(
            row_model.command(
                ("swift-all", str(swift)),
                "Run all Swift tests",
                ["swift", "test", "--package-path", str(swift.parent)],
            )
        )
    if rust:
        base = ["cargo", "test", "--manifest-path", str(rust)]
        rows.append(
            recording.capture_row(
                "Discover Rust tests",
                base + ["--", "--list", "--format", "terse"],
                rust.parent,
                state,
                "rust",
            )
        )
        rows.append(row_model.command(("rust-all", str(rust)), "Run all Rust tests", base))
    if python:
        rows.append(
            recording.capture_row(
                "Discover pytest tests",
                ["python3", "-m", "pytest", "--collect-only", "-q"],
                python.parent,
                state,
                "pytest",
            )
        )
    if golang:
        rows.append(
            recording.capture_row(
                "Discover Go tests",
                ["go", "test", "./...", "-list", "."],
                golang.parent,
                state,
                "go",
            )
        )
    for path in project_paths.files(root):
        if not path.is_relative_to(cwd):
            continue
        if (
            path.suffix == ".sh"
            and not path.name.endswith(".test.sh")
            and path.parent.name in ("unit", "integration", "e2e")
        ):
            rows.append(
                row_model.command(
                    ("bash-script", str(path)),
                    str(path.relative_to(root)),
                    ["bash", str(path)],
                    path=str(path),
                )
            )
        if path.name.endswith(".test.sh"):
            for name, line in discovery_bash.bash_tests(path):
                rows.append(
                    row_model.command(
                        ("bash-test", str(path), name),
                        f"{path.relative_to(root)}: {name}",
                        discovery_bash.bash_test_command(path, line),
                        path=str(path),
                        line=line,
                    )
                )
    rows.extend(discovery_xcode.xcode_projects(cwd, state, "tests"))
    if project_paths.nearest(cwd, ["justfile", "Justfile", ".justfile"]):
        rows.extend(
            item for item in tasks_just.just_recipes(cwd) if "test" in item["label"].lower()
        )
    return rows or [
        row_model.notice(
            "No supported tests found",
            "Declared project test tasks, Swift, Xcode, Cargo, Go, pytest and Bashunit are supported.",
        )
    ]
