import os
import subprocess
import sys
import tempfile
from pathlib import Path

for name in subprocess.check_output(
    ["git", "rev-parse", "--local-env-vars"], text=True
).splitlines():
    os.environ.pop(name, None)
os.environ.update(GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null")

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / "dot_local/libexec/fzf-pickers"))
import git_files as m

with tempfile.TemporaryDirectory() as directory:
    p = Path(directory).resolve()
    env = {**os.environ, "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_SYSTEM": "/dev/null"}

    def git(*args):
        return subprocess.check_output(
            ["git", "-C", directory, *args], env=env, stderr=subprocess.DEVNULL
        )

    git("init", "-q")
    git("config", "user.name", "Test")
    git("config", "user.email", "test@example.test")
    (p / "sub").mkdir()
    for name in ["with space", "line\nbreak", ".hidden", "sub/file"]:
        (p / name).write_text("needle\n")
    (p / ".gitignore").write_text("ignored\n")
    git("add", ".")
    git("commit", "-qm", "first")
    (p / "untracked").write_text("needle\n")
    (p / "ignored").write_text("needle\n")
    state = {"cwd": str(p / "sub"), "hidden": True, "query": "needle"}
    rows = list(m.collect("contents", state))
    assert {Path(r["path"]).relative_to(p).as_posix() for r in rows} == {
        "with space",
        "line\nbreak",
        ".hidden",
        "sub/file",
    }
    rows = list(m.collect("contents", {**state, "untracked": True}))
    assert [Path(r["path"]).name for r in rows] == ["untracked"]
    assert list(m.collect("contents", {**state, "query": ""})) == []
    (p / "sub/file").unlink()
    rows = list(m.collect("contents", state))
    assert len(rows) == 3, "A deleted tracked file must not discard other matches"
    git("mv", "with space", "new name")
    (p / ".hidden").unlink()
    changed = list(m.collect("changed", state))
    assert {"with space", "new name", ".hidden", "untracked"} <= {r["value"] for r in changed}
    historical = list(
        m.collect(
            "historical-files", {**state, "commit": git("rev-parse", "HEAD").decode().strip()}
        )
    )
    row = next(r for r in historical if r["value"] == "with space")
    action = m.accept("historical-files", [row], "enter", state)
    assert "-R" in action["text"] and "checkout" not in action["text"]
    assert not (p / "with space").exists()
print("Git picker behavior checks passed")
