import subprocess


class ProviderError(RuntimeError):
    pass


def run(argv, cwd=None, check=True, timeout=15):
    try:
        result = subprocess.run(
            argv,
            cwd=cwd,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProviderError(f"{argv[0]}: {error}") from error
    if check and result.returncode:
        raise ProviderError(f"{argv[0]} exited {result.returncode}: {result.stderr.strip()}")
    return result.stdout
