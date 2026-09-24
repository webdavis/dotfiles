import os
import subprocess


def run(args, cwd, allowed=(0,)):
    result = subprocess.run(args, cwd=cwd, capture_output=True)
    if result.returncode not in allowed:
        raise RuntimeError(
            result.stderr.decode(errors="replace").strip()
            or f"{args[0]} exited {result.returncode}"
        )
    return result.stdout


def git(cwd, *args, allowed=(0,)):
    return run(["git", "--no-pager", "-c", "core.quotePath=false", *args], cwd, allowed)


def root(cwd):
    result = subprocess.run(["git", "rev-parse", "--show-toplevel"], cwd=cwd, capture_output=True)
    return os.fsdecode(result.stdout).rstrip("\n") if result.returncode == 0 else None


def strings(data):
    return [os.fsdecode(p) for p in data.split(b"\0") if p]
