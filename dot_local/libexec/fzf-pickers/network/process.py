import subprocess
from pathlib import Path


def run(argv, timeout=3):
    try:
        result = subprocess.run(
            argv,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            errors="replace",
            timeout=timeout,
            check=False,
        )
        if result.returncode:
            raise ValueError(f"{Path(argv[0]).name} unavailable (exit {result.returncode})")
        return result.stdout
    except subprocess.TimeoutExpired as error:
        raise ValueError(f"{Path(argv[0]).name} timed out") from error
    except OSError as error:
        raise ValueError(f"{Path(argv[0]).name} unavailable") from error


def bounded_output(argv, timeout):
    try:
        return subprocess.run(
            argv, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=timeout, check=False
        ).stdout.decode(errors="replace")
    except subprocess.TimeoutExpired as error:
        return (error.stdout or b"").decode(errors="replace")
    except OSError:
        return ""
