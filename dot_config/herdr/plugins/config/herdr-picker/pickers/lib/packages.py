import json
import os
import re
import tomllib
from pathlib import Path

from data import annotated
from rows import edit, emit

HOME = Path.home()
BREW = Path(os.environ.get("HOMEBREW_PREFIX", "/opt/homebrew"))
NODE = HOME / ".local/share/fnm/aliases/default/lib/node_modules"


def linked_commands():
    """Each Homebrew command with the Cellar or Caskroom path its link points into."""
    links = {}
    for folder in (BREW / "bin", BREW / "sbin"):
        try:
            for entry in os.scandir(folder):
                if entry.is_symlink():
                    links[entry.name] = os.readlink(entry.path)
        except OSError:
            continue
    return links


def from_links(links, marker):
    return sorted(name for name, target in links.items() if marker in target)


def crates():
    try:
        installs = json.loads((HOME / ".cargo/.crates2.json").read_text())["installs"]
    except (OSError, ValueError, KeyError):
        return {}
    return {key.split()[0]: value.get("bins", []) for key, value in installs.items()}


def uv_tool(name):
    base = re.split(r"[\[=<>@ ]", name)[0]
    try:
        with (HOME / ".local/share/uv/tools" / base / "uv-receipt.toml").open("rb") as stream:
            return True, [point["name"] for point in tomllib.load(stream)["tool"]["entrypoints"]]
    except (OSError, KeyError, ValueError):
        return False, []


def npm_package(name):
    base = name if name.rfind("@") <= 0 else name[: name.rfind("@")]
    try:
        manifest = json.loads((NODE / base / "package.json").read_text())
    except (OSError, ValueError):
        return False, []
    bins = manifest.get("bin") or {}
    return True, sorted(bins) if isinstance(bins, dict) else [base.split("/")[-1]]


def entries(tree):
    """Each listed package as (manager, name, line, installed, commands); installed is None
    where nothing on disk says."""
    links, built = linked_commands(), crates()
    root = tree["packages"]
    brew = root["macos"]["homebrew"]
    for item in brew["formulae"]:
        short = item["v"].split("/")[-1]
        commands = from_links(links, f"/Cellar/{short}/")
        yield "brew", item["v"], item["l"], (BREW / "opt" / short).exists(), commands
    for item in brew["casks"]:
        short = item["v"].split("/")[-1]
        commands = from_links(links, f"/Caskroom/{short}/")
        yield "cask", item["v"], item["l"], (BREW / "Caskroom" / short).exists(), commands
    for item in brew["mas"]:
        name = item["name"]
        yield "mas", name["v"], name["l"], Path(f"/Applications/{name['v']}.app").exists(), []
    for item in root["macos"]["cargo"]:
        name = item["name"]
        yield "cargo", name["v"], name["l"], name["v"] in built, built.get(name["v"], [])
    for item in root["macos"]["uv"]:
        yield "uv", item["v"], item["l"], *uv_tool(item["v"])
    for node in root["macos"]["fnm"]:
        for item in node["packages"]:
            yield "npm", item["v"], item["l"], *npm_package(item["v"])
    for item in root["cargo_git_tools"]:
        name, package = item["name"], item["package"]["v"]
        yield "cargo-git", name["v"], name["l"], package in built, built.get(package, [])
    installed = HOME / ".config/herdr/plugins/github-installations"
    for item in root["herdr_plugins"]:
        found = any(installed.glob(item["id"]["v"] + "-*"))
        yield "herdr", item["id"]["v"], item["id"]["l"], found, []
    for item in root["herdr_linked_plugins"]:
        yield "herdr", item["id"]["v"], item["id"]["l"], None, []
    for name, item in root["source_builds"].items():
        found = (HOME / item["install_dir"]["v"]).exists()
        yield "source", name, item["source_dir"]["l"] - 1, found, []


def packages():
    path, tree = annotated("system_packages_autoinstall.yaml")
    for manager, name, line, installed, commands in entries(tree):
        state = {True: "yes", False: "NO, listed but not installed", None: "not checked"}
        preview = f"{name}\nManager: {manager}\nInstalled: {state[installed]}\n\n"
        preview += "Commands:\n" + "\n".join(commands) if commands else "Provides no commands."
        flag = "  [not installed]" if installed is False else ""
        emit(name + flag, edit(path, line), preview, manager)
