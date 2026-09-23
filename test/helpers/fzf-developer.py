import os
import sys
import tempfile
import time
from pathlib import Path
from unittest.mock import patch

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / "dot_local/libexec/fzf-pickers"))
from fzf_developer import data, diagnostics, discovery, project_tasks, system

for module in (data, diagnostics, discovery, project_tasks, system):
    for name, test in vars(module).items():
        if not name.startswith("test_"):
            continue
        with tempfile.TemporaryDirectory(prefix="fzf-developer-") as scratch:
            cwd = Path(scratch).resolve()
            state = {"cwd": str(cwd), "state_home": str(cwd / "state")}
            env = dict(os.environ, HOME=scratch, XDG_STATE_HOME=state["state_home"])
            env.pop("NVIM", None)
            with patch.dict(os.environ, env, clear=True):
                started = time.monotonic()
                test(cwd, state)
                duration = time.monotonic() - started
                assert duration < 1, f"{name}: {duration:.3f}s exceeds 1s"
        print(f"PASS: {name} ({duration:.3f}s)")
