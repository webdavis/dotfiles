import os
import subprocess
import tomllib
from pathlib import Path

from rows import home, row

SETTINGS = Path(__file__).resolve().parents[2] / "broccoli.toml"


class NotConfigured(Exception):
    """Things the owner sets up once; carries one row for each that says how."""

    def __init__(self, *rows):
        super().__init__("; ".join(made["label"] for made in rows))
        self.rows = rows


def all_set(*checks):
    """Each check's value, or every missing piece in one report."""
    values, missing = [], []
    for check in checks:
        try:
            values.append(check())
        except NotConfigured as unset:
            missing.extend(unset.rows)
    if missing:
        raise NotConfigured(*missing)
    return values


def fixtures():
    """A folder of synthetic responses that stands in for Temporal and Pylon, for tests."""
    folder = os.environ.get("BROCCOLI_FIXTURES")
    return Path(folder) if folder else None


def settings(section):
    try:
        with SETTINGS.open("rb") as stream:
            return tomllib.load(stream).get(section, {})
    except (OSError, ValueError):
        return {}


def required(section, *names):
    found = settings(section)
    missing = [name for name in names if not found.get(name)]
    if missing:
        text = f"Set {', '.join(missing)} under [{section}] in broccoli.toml"
        path = source_of(SETTINGS)
        raise NotConfigured(
            row(text, {"edit": {"path": path}}, f"{text}\n\nEnter opens {home(path)}.")
        )
    return found


def source_of(path):
    """The dotfiles' copy of a deployed file, so an edit survives the next chezmoi apply."""
    done = subprocess.run(
        ["chezmoi", "source-path", str(path)], capture_output=True, text=True, check=False
    )
    return done.stdout.strip() if done.returncode == 0 and done.stdout.strip() else str(path)


def secret(service, record):
    """The Keychain's copy of a KeePassXC entry, held only in memory."""
    done = subprocess.run(
        ["security", "find-generic-password", "-w", "-s", service],
        capture_output=True,
        text=True,
        check=False,
    )
    if done.returncode == 0 and done.stdout.strip():
        return done.stdout.strip()
    command = f"security add-generic-password -a \"$USER\" -s '{service}' -w"
    preview = (
        f"The Keychain has no entry named {service}.\n\n"
        f"Copy the key from KeePassXC ({record}), then run:\n\n  {command}\n\n"
        "It asks for the key, so the key never lands in your shell history.\n"
        "Enter types the command at your prompt without running it."
    )
    raise NotConfigured(row(f"Add {service} to the Keychain", {"insert": command}, preview))
