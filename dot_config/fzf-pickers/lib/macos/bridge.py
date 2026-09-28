import json
import subprocess
from pathlib import Path


def run(argv, timeout=30, binary=False):
    try:
        result = subprocess.run(argv, capture_output=True, timeout=timeout, check=False)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise RuntimeError(
            f"{Path(argv[0]).name} unavailable or timed out. Check app access in Privacy & Security and dismiss pending app dialogs."
        ) from exc
    if result.returncode:
        error = result.stderr.decode("utf-8", "replace").strip()
        raise RuntimeError(f"{Path(argv[0]).name}: {error or 'request failed'}")
    return result.stdout if binary else result.stdout.decode("utf-8", "replace")


def native(action, **kwargs):
    script = Path(__file__).with_name("automation.js")
    return json.loads(
        run(
            ["osascript", "-l", "JavaScript", str(script), json.dumps({"action": action, **kwargs})]
        )
    )
