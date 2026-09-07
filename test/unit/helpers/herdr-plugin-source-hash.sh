#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
python3 - "$repo_root" "$1" <<'PYTEST'
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

repo, mode = Path(sys.argv[1]), sys.argv[2]
with tempfile.TemporaryDirectory(prefix="herdr-source-hash-") as scratch:
    source = Path(scratch)
    templates = source / ".chezmoitemplates"
    templates.mkdir()
    shutil.copyfile(repo / ".chezmoitemplates/herdr-plugin-build.sh.tmpl",
                    templates / "herdr-plugin-build.sh.tmpl")
    home = source / "home"
    home.mkdir()
    (source / "config.toml").write_text("")
    plugin = source / "dot_local/share/herdr/plugins/herdr-workspace-jump"
    (plugin / "src").mkdir(parents=True)
    for name in ["src/main.rs", "Cargo.toml", "Cargo.lock", "herdr-plugin.toml"]:
        (plugin / name).write_text("original input\n")

    def render(plugin_id="herdr-workspace-jump"):
        template = '{{ includeTemplate "herdr-plugin-build.sh.tmpl" (dict "id" "' + plugin_id
        template += '" "sourceDir" .chezmoi.sourceDir) }}'
        result = subprocess.run(
            ["chezmoi", "--source", str(source), "--config", str(source / "config.toml"),
             "execute-template", "--no-tty"], input=template, text=True,
            capture_output=True, env=dict(os.environ, HOME=str(home), CI="1"))
        assert result.returncode == 0, result.stderr
        return result.stdout

    def changes(name, before):
        path = plugin / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("changed input\n")
        after = render()
        assert after != before, f"changed {name} did not change the build trigger"
        return after

    before = render()
    if mode == "modules":
        before = changes("src/jump.rs", before)
        (plugin / "src/jump.rs").write_text("second revision\n")
        after = render()
        assert after != before, "editing an existing module did not change the build trigger"
        before = changes("crates/domain/src/decision.rs", after)
        changes("crates/domain/src/decision/nested.rs", before)
    elif mode == "workspace":
        before = changes("crates/domain/Cargo.toml", before)
        before = changes("crates/domain/build.rs", before)
        changes("build.rs", before)
    else:
        (plugin / "target/generated").mkdir(parents=True)
        (plugin / "target/generated/output.rs").write_text("compiler output\n")
        assert render() == before, "compiled output changed the source trigger"
        for plugin_id in ["herdr-last-workspace", "herdr-smart-nav"]:
            sibling = plugin.parent / plugin_id
            shutil.copytree(plugin, sibling)
            first = render(plugin_id)
            (sibling / "src/main.rs").write_text("changed sibling source\n")
            assert render(plugin_id) != first, f"{plugin_id} lost its main-source trigger"
PYTEST
