# osquery Approval UX (PR #2) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended)
> or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax
> for tracking. TDD throughout: failing test → run red → minimal implementation → run green → full suite
> green → commit per task. A previously-green test turning red is a regression and is **not** allowed.
>
> **This plan changes code AND touches human-approval-gated areas.** It creates a new Discord bot
> ("Butters") secret in KeePassXC and writes a skill under `~/.hermes/` (`dot_hermes/`). Per the repo
> rules, anything touching `dot_hermes/` and any `keepassxc` template is applied **interactively** (TTY +
> KeePassXC unlocked), never from automation. Do not run bare `chezmoi apply`.

**Goal:** Add the PR #2 allowlist UX on top of merged PR #1 — tap **Approve/Deny** buttons in Discord
(primary, posted by Stephen's spare bot "Butters") plus a `/osquery allow|deny|list` Hermes skill (typed
fallback, rides Bob) — both writing the **same** allowlist file through the one `osquery-allowlist.sh`
tool.

**Architecture:** PR #1 already ships the manual allowlist file + the shared `osquery-allowlist.sh` tool
with its **add** verb (`-a`) — the one security boundary. PR #2 adds: (1) the **deny** (`-d`, removes a
label) and **list** (`-l`) verbs to that same getopts tool (for the skill's `deny`/undo and `list`); (2)
a **pending-scoped** discord.py bot (uv project) that launchd keeps alive only while an approval is
pending (`KeepAlive.PathState` on a sentinel file), kickstarted by the alerter when a new non-allowlisted
user-LaunchAgent label appears; (3) restart-safe persistent buttons via `discord.ui.DynamicItem`; (4) the
`/osquery` skill. The bot calls `osquery-allowlist.sh -a` via subprocess — the agent and the bot are
never the security boundary, the fail-closed script is. Buttons are LLM-free; the skill is agent-mediated
(tokens), so buttons stay primary.

**Tech Stack:** Python 3.12 + `discord.py>=2.4` (DynamicItem persistent views), **uv** (project + runtime
venv), **pytest + pytest-asyncio**, bash (writer/wrapper/integration), chezmoi (templates, LaunchAgent,
run_onchange), launchd (`KeepAlive.PathState`, `launchctl kickstart`), KeePassXC (the Butters token).

**Preconditions (must be true before starting):**

- **PR #1 is merged.** This plan depends on these PR #1 deliverables existing on `main`:
  `~/.local/bin/osquery-allowlist.sh` (the shared tool, with the `-a` add verb + `test_allowlist.bats`),
  `dot_config/osquery/page-launchd-allowlist.txt`, the 3-outcome gate in
  `executable_osquery-results-alerter.sh` (with a `persistence_launchd` user-agent digest arm), the bats
  harness `test/osquery-alerter/lib.bash`, and a `just test` recipe that runs bats.
- **Branch:** create `feat/osquery-approval-ux` off `main` (never commit to `main`; ship as one reviewed
  PR per \[[github-pr-merge-convention]\]).
- **Decision source of truth:**
  `docs/superpowers/decisions/2026-06-10-osquery-alerting-v2-decision-addendum.md` §D-V2-15 and spec
  `…master-spec-v2.md` §9. Test coverage: `…test-matrix-v2.md` "Allowlist tests".

**Out of scope (do NOT build here):** the manual allowlist file + the `osquery-allowlist.sh -a` add verb
(PR #1); the Mouse advisory (PR #3); any change to the page/digest tiering; multi-host/homelab.

______________________________________________________________________

## File Structure

| Path                                                                                  | Responsibility                                                                       | Change |
| ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ------ |
| `flake.nix`                                                                           | add `pkgs.uv` to the dev shell                                                       | modify |
| `justfile`                                                                            | extend `test` to also run the bot's `uv run pytest`                                  | modify |
| `.chezmoiignore`                                                                      | exclude the bot's `.venv`, `tests`, `__pycache__`, `.pytest_cache` from target state | modify |
| `dot_local/bin/executable_osquery-allowlist.sh`                                       | add the `-d` (deny) and `-l` (list) verbs to the one getopts tool                    | modify |
| `test/osquery-alerter/test_allowlist_remove_list.bats`                                | bats for the `-d` / `-l` verbs                                                       | create |
| `dot_local/share/osquery-approval-bot/pyproject.toml`                                 | uv project (discord.py; dev: pytest, pytest-asyncio)                                 | create |
| `dot_local/share/osquery-approval-bot/uv.lock`                                        | pinned lockfile (drives `uv sync`)                                                   | create |
| `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/__init__.py`        | package marker                                                                       | create |
| `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/core.py`            | pure logic: request id, pending store, auth gate, decision→script                    | create |
| `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/bot.py`             | discord.py wiring: DynamicItem buttons, on_ready post, callbacks                     | create |
| `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/__main__.py`        | entrypoint (`python -m osquery_approval_bot`)                                        | create |
| `dot_local/share/osquery-approval-bot/tests/conftest.py`                              | pytest fixtures (per-test state directory, fake allowlist script)                    | create |
| `dot_local/share/osquery-approval-bot/tests/test_core.py`                             | pytest for `core.py`                                                                 | create |
| `dot_local/share/osquery-approval-bot/tests/test_bot.py`                              | pytest-asyncio for `bot.py` callbacks (mocked Interaction)                           | create |
| `dot_local/bin/executable_osquery-approval-bot.sh`                                    | wrapper: load token+config, exec `.venv/bin/python -m osquery_approval_bot`          | create |
| `Library/LaunchAgents/com.webdavis.osquery-approval-bot.plist.tmpl`                   | LaunchAgent: `RunAtLoad=false`, `KeepAlive.PathState` on the sentinel                | create |
| `.chezmoiscripts/run_onchange_after_60-load-osquery-approval-bot-launchagent.sh.tmpl` | bootstrap the LaunchAgent                                                            | create |
| `.chezmoiscripts/run_onchange_after_55-uv-sync-approval-bot.sh.tmpl`                  | `uv sync --no-dev` to build the runtime venv                                         | create |
| `dot_config/osquery/private_approval-bot-token.tmpl`                                  | the Butters token (600) via KeePassXC                                                | create |
| `dot_config/osquery/approval-bot.config.tmpl`                                         | non-secret IDs (owner/channel/guild) from chezmoi data                               | create |
| `.chezmoi.toml.tmpl`                                                                  | add `[data.osquery]` approval IDs                                                    | modify |
| `CLAUDE.md`                                                                           | add the token template to the KeePassXC-gated list                                   | modify |
| `dot_local/bin/executable_osquery-results-alerter.sh`                                 | drop a pending request + kickstart on a new non-allowlisted user-agent label         | modify |
| `dot_local/bin/executable_osquery-uptime-watchdog.sh`                                 | guard: sentinel exists ⇒ bot running                                                 | modify |
| `test/osquery-alerter/test_approval_integration.bats`                                 | bats for alerter-drop + watchdog-guard                                               | create |
| `dot_hermes/skills/osquery/SKILL.md`                                                  | the `/osquery allow / deny / list` Hermes skill (calls the one tool)                 | create |

**Runtime state (created at runtime, not chezmoi-managed):**
`~/.local/state/osquery-approval-bot/pending/<request_id>.json` (one per pending label),
`~/.local/state/osquery-approval-bot/declined/<request_id>` (deny markers, suppress re-ask),
`~/.local/state/osquery-approval-bot/active` (the **sentinel** — `KeepAlive.PathState` watches this), all
under a `0700` directory.

______________________________________________________________________

## Task 0: Tooling — uv in the flake, test wiring, chezmoi exclusions

**Files:**

- Modify: `flake.nix` (the `baseShell.buildInputs` list, lines ~30–52)

- Modify: `justfile` (the `test` recipe added by PR #1)

- Modify: `.chezmoiignore`

- [ ] **Step 1: Add `uv` to the dev shell.** In `flake.nix`, add `pkgs.uv` to `baseShell.buildInputs`
  (keep alphabetical-ish grouping; it sits with the other `pkgs.*`):

```nix
  pkgs.taplo # TOML formatter/linter (v2 §19.1)
  pkgs.uv # Python project/venv manager for the osquery approval bot (PR #2)
  pkgs.yq-go # YAML validator (v2 §19.1)
```

- [ ] **Step 2: Verify uv is available.** Run:

```bash
nix develop .#run --command uv --version
```

Expected: prints a `uv 0.x.y` version line, exit 0.

- [ ] **Step 3: Extend `just test` to run the bot's pytest.** Open `justfile`, find the `test` recipe
  added by PR #1 (it runs bats). Append the bot's pytest so one command covers everything. The recipe
  becomes (keep PR #1's bats line; add the second line):

```just
# Run all tests (bats for the shell pipeline, pytest for the approval bot).
test:
  nix develop .#run --command ./scripts/run-bats.sh
  nix develop .#run --command bash -c 'cd dot_local/share/osquery-approval-bot && uv run --frozen pytest -q'
```

(If PR #1 named its bats entrypoint differently, keep that first line exactly as PR #1 wrote it and only
add the second `uv run` line.)

- [ ] **Step 4: Exclude the bot's generated/source-only paths from target state.** Append to
  `.chezmoiignore` (these are **target** paths):

```
# osquery approval bot (PR #2): generated venv + source-only tests never reach $HOME
.local/share/osquery-approval-bot/.venv
.local/share/osquery-approval-bot/.venv/**
.local/share/osquery-approval-bot/tests
.local/share/osquery-approval-bot/tests/**
# repo-root bats test tree is dev/CI-only — never apply to $HOME. (PR #1's v2 plan does not
# .chezmoiignore test/, so PR #2 must add it itself or the new *.bats leak into ~/test/.
# chezmoi de-dups glob patterns, so a duplicate here is harmless if PR #1 also adds it.)
test
test/**
**/__pycache__
**/__pycache__/**
**/.pytest_cache
**/.pytest_cache/**
```

- [ ] **Step 5: Keep pytest cruft out of git.** The `.chezmoiignore` entries above use chezmoi *target*
  paths and do nothing for git. The first `just test` makes pytest write `__pycache__` under the
  source-prefixed `dot_local/share/osquery-approval-bot/` tree, which would leave `git status` dirty
  before the worktrunk `[[pre-merge]]` gate. (uv self-ignores `.venv/` and pytest self-ignores
  `.pytest_cache/` via auto-written `.gitignore` files, so `__pycache__` is the only real residue.)
  Append to the repo-root `.gitignore`:

```
# osquery approval bot (PR #2): pytest/compiled-Python residue under the source tree
**/__pycache__/
*.py[cod]
dot_local/share/osquery-approval-bot/.venv/
```

- [ ] **Step 6: Verify chezmoi still evaluates, the venv + the test tree are filtered, and git ignores
  the cruft.** Run:

```bash
nix develop .#run --command chezmoi --source . execute-template '{{ .chezmoi.os }}'
chezmoi --source . managed --exclude=dirs | grep -c 'osquery-approval-bot/.venv' || true
chezmoi --source . managed | grep -c '^test/' || true
git check-ignore dot_local/share/osquery-approval-bot/source/osquery_approval_bot/__pycache__/x.pyc
```

Expected: prints `darwin`; both greps print `0` (venv and the repo `test/` tree are not managed); the
`git check-ignore` echoes the matching pattern (exit 0).

- [ ] **Step 7: Commit.**

```bash
git add flake.nix justfile .chezmoiignore .gitignore
git commit -m "build(osquery): add uv to dev shell + wire approval-bot pytest into just test"
```

______________________________________________________________________

## Task 1: Extend the shared `osquery-allowlist.sh` to one getopts tool (add / deny / list)

PR #1 ships **one** shared writer `osquery-allowlist.sh` with an **add** verb (`-a <label>`) — the single
security boundary every path uses. PR #2 needs **deny** (remove a label — the skill's `deny`, and undoing
a fat-fingered `allow`) and a **list** (the skill's `list`, printed nicely). Rather than add sibling
scripts, this task extends the one tool with getopts flags: `-a <label>` allow/add, `-d <label>`
deny/remove, `-l` list. Same `@`-aware contract, same file, fail-closed.

**Files:**

- Modify: `dot_local/bin/executable_osquery-allowlist.sh` (PR #1's add-only version → full add/deny/list)

- Test: `test/osquery-alerter/test_allowlist_remove_list.bats` (PR #1's `test_allowlist.bats` covers add)

- [ ] **Step 1: Write the failing tests.** Create `test/osquery-alerter/test_allowlist_remove_list.bats`:

```bash
#!/usr/bin/env bats

setup() {
  temp_directory="$(mktemp -d)"
  export OSQUERY_LAUNCHD_ALLOWLIST="$temp_directory/page-launchd-allowlist.txt"
  allowlist_script="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-allowlist.sh"
  printf '# header\ncom.foo\ncom.bar\nhomebrew.mxcl.postgresql@17\n' >"$OSQUERY_LAUNCHD_ALLOWLIST"
}

teardown() { rm -rf "$temp_directory"; }

@test "-d removes an existing label, leaving the rest" {
  run bash "$allowlist_script" -d com.foo
  [ "$status" -eq 0 ]
  run grep -qxF "com.foo" "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -ne 0 ]
  run grep -qxF "com.bar" "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ]
}

@test "-d on an absent label is a no-op success" {
  run bash "$allowlist_script" -d com.nope
  [ "$status" -eq 0 ]
  run grep -qxF "com.bar" "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ]
}

@test "-d matches the exact full line only (no prefix/substring removal)" {
  printf '# header\ncom.foobar\n' >"$OSQUERY_LAUNCHD_ALLOWLIST"
  run bash "$allowlist_script" -d com.foo
  [ "$status" -eq 0 ]
  run grep -qxF "com.foobar" "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -eq 0 ]
}

@test "-d rejects a malformed label without touching the file" {
  before="$(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
  run bash "$allowlist_script" -d '../etc/passwd'
  [ "$status" -ne 0 ]
  [ "$(cat "$OSQUERY_LAUNCHD_ALLOWLIST")" = "$before" ]
}

@test "-d preserves the '@' label class" {
  run bash "$allowlist_script" -d 'homebrew.mxcl.postgresql@17'
  [ "$status" -eq 0 ]
  run grep -qxF 'homebrew.mxcl.postgresql@17' "$OSQUERY_LAUNCHD_ALLOWLIST"
  [ "$status" -ne 0 ]
}

@test "-a refuses an Apple/system label (com.apple.*)" {
  before="$(cat "$OSQUERY_LAUNCHD_ALLOWLIST")"
  run bash "$allowlist_script" -a 'com.apple.something'
  [ "$status" -ne 0 ]
  [ "$(cat "$OSQUERY_LAUNCHD_ALLOWLIST")" = "$before" ]
}

@test "-a is idempotent (adding an existing label does not duplicate it)" {
  run bash "$allowlist_script" -a com.bar
  [ "$status" -eq 0 ]
  run bash -c "grep -cxF com.bar '$OSQUERY_LAUNCHD_ALLOWLIST'"
  [ "$output" = "1" ]
}

@test "-l prints the labels with a count header, sorted and bulleted" {
  run bash "$allowlist_script" -l
  [ "$status" -eq 0 ]
  [[ "$output" == *"3 label(s)"* ]]
  [[ "$output" == *"• com.bar"* ]]
  [[ "$output" == *"• com.foo"* ]]
}

@test "-l on an empty allowlist says so" {
  printf '# only a comment\n' >"$OSQUERY_LAUNCHD_ALLOWLIST"
  run bash "$allowlist_script" -l
  [ "$status" -eq 0 ]
  [[ "$output" == *"empty"* ]]
}

@test "no flag prints usage and exits non-zero" {
  run bash "$allowlist_script"
  [ "$status" -ne 0 ]
  [[ "$output" == *"usage:"* ]]
}
```

- [ ] **Step 2: Run it red.**

```bash
nix develop .#run --command bats test/osquery-alerter/test_allowlist_remove_list.bats
```

Expected: FAIL — PR #1's script only understands `-a`; `-d`/`-l`/usage are unimplemented.

- [ ] **Step 3: Replace the script with the full getopts tool.** Overwrite
  `dot_local/bin/executable_osquery-allowlist.sh` (this is PR #1's add-only script extended to all three
  verbs — keep the exact add contract PR #1 tested):

```bash
#!/opt/homebrew/bin/bash
# osquery-allowlist.sh — the one tool that manages the page-launchd allowlist (the security boundary).
#
#   -a <label>   add a label  (allow; validated, deduped, refuses Apple/system labels)
#   -d <label>   deny a label (remove its exact full-line from the allowlist)
#   -l           list the allowlist (formatted)
#
# Fail-closed: a malformed label exits non-zero and writes nothing. The reader (the alerter gate) matches
# with `grep -qxF`, so labels are stored ONE BARE label per line (write history is git on the source file).
set -euo pipefail

allowlist_file="${OSQUERY_LAUNCHD_ALLOWLIST:-$HOME/.config/osquery/page-launchd-allowlist.txt}"
label_pattern='^[A-Za-z0-9][A-Za-z0-9._@-]+$'

usage() {
  echo "usage: osquery-allowlist.sh -a <label> | -d <label> | -l" >&2
  exit 64
}

validate_syntax() {
  local label="$1"
  if [[ ! $label =~ $label_pattern ]]; then
    echo "refusing malformed label: ${label@Q}" >&2
    exit 2
  fi
}

add_label() {
  local label="$1"
  validate_syntax "$label"
  # Apple/system reverse-DNS labels are never user-allowlistable (they page by path in the gate).
  if [[ $label == com.apple.* ]]; then
    echo "refusing Apple/system label: ${label@Q}" >&2
    exit 3
  fi
  mkdir -p "$(dirname "$allowlist_file")"
  [[ -f $allowlist_file ]] || : >"$allowlist_file"
  grep -qxF -- "$label" "$allowlist_file" && return 0 # already present → dedup no-op
  printf '%s\n' "$label" >>"$allowlist_file"
}

deny_label() {
  local label="$1" temp_file line
  validate_syntax "$label"
  [[ -f $allowlist_file ]] || exit 0
  temp_file="$(mktemp "${allowlist_file}.XXXXXX")"
  trap 'rm -f "$temp_file"' EXIT
  while IFS= read -r line || [[ -n $line ]]; do
    [[ $line == "$label" ]] && continue # drop the exact full-line match; keep comments/blanks
    printf '%s\n' "$line"
  done <"$allowlist_file" >"$temp_file"
  mv -f "$temp_file" "$allowlist_file"
  trap - EXIT
}

list_labels() {
  local labels label_count
  [[ -f $allowlist_file ]] || {
    echo "(the page-launchd allowlist is empty)"
    return 0
  }
  labels="$(grep -v '^[[:space:]]*#' "$allowlist_file" | grep -v '^[[:space:]]*$' | sort)"
  if [[ -z $labels ]]; then
    echo "(the page-launchd allowlist is empty)"
    return 0
  fi
  label_count="$(printf '%s\n' "$labels" | wc -l | tr -d ' ')"
  echo "page-launchd allowlist — ${label_count} label(s):"
  printf '%s\n' "$labels" | sed 's/^/  • /'
}

[[ $# -gt 0 ]] || usage
action=""
label=""
while getopts ":a:d:l" option; do
  case "$option" in
    a) action="add" label="$OPTARG" ;;
    d) action="deny" label="$OPTARG" ;;
    l) action="list" ;;
    :)
      echo "option -${OPTARG} requires a <label>" >&2
      usage
      ;;
    *) usage ;;
  esac
done
[[ -n $action ]] || usage

case "$action" in
  add) add_label "$label" ;;
  deny) deny_label "$label" ;;
  list) list_labels ;;
esac
```

- [ ] **Step 4: Run it green.**

```bash
nix develop .#run --command bats test/osquery-alerter/test_allowlist_remove_list.bats
nix develop .#run --command bats test/osquery-alerter/test_allowlist.bats   # PR #1's add tests still pass
```

Expected: all new tests PASS, and PR #1's add tests still PASS (the `-a` contract is unchanged).

- [ ] **Step 5: Lint the script.**

```bash
nix develop .#run --command bash -c 'shellcheck dot_local/bin/executable_osquery-allowlist.sh && shfmt -i 2 -ci -s -d dot_local/bin/executable_osquery-allowlist.sh'
```

Expected: no shellcheck findings; shfmt prints no diff.

- [ ] **Step 6: Commit.**

```bash
git add dot_local/bin/executable_osquery-allowlist.sh test/osquery-alerter/test_allowlist_remove_list.bats
git commit -m "feat(osquery): one getopts allowlist tool — add/deny/list verbs (fail-closed, @-aware)"
```

______________________________________________________________________

## Task 2: The uv bot project skeleton + a passing pytest harness

**Files:**

- Create: `dot_local/share/osquery-approval-bot/pyproject.toml`

- Create: `dot_local/share/osquery-approval-bot/uv.lock` (generated)

- Create: `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/__init__.py`

- Create: `dot_local/share/osquery-approval-bot/tests/test_smoke.py`

- [ ] **Step 1: Write the project metadata.** Create
  `dot_local/share/osquery-approval-bot/pyproject.toml`:

```toml
[project]
name = "osquery-approval-bot"
version = "0.1.0"
description = "Pending-scoped Discord bot posting Approve/Deny buttons for osquery launchd allowlisting"
# Upper bound is load-bearing: the apply-time `uv sync` (Task 8) runs OUTSIDE the nix shell, so a bare
# `>=3.12` lets uv pick a host 3.13 and the LaunchAgent's `.venv/bin/python` drifts from the dev/test
# interpreter. Pinning `<3.13` makes uv resolve 3.12.x even when 3.13 is installed. (Do NOT use a
# `[tool.uv] python` key — not a real uv setting — or a dev-shell-only `UV_PYTHON`; neither reaches Task 8.)
requires-python = ">=3.12,<3.13"
dependencies = [
  "discord.py>=2.4,<3",
]

[dependency-groups]
dev = [
  "pytest>=8",
  "pytest-asyncio>=0.24",
]

[tool.pytest.ini_options]
asyncio_mode = "auto"
testpaths = ["tests"]
pythonpath = ["source"]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["source/osquery_approval_bot"]
```

- [ ] **Step 2: Create the package marker.** Create
  `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/__init__.py`:

```python
"""Pending-scoped Discord approval bot for osquery launchd allowlisting."""

__all__ = []
```

- [ ] **Step 3: Write a smoke test.** Create `dot_local/share/osquery-approval-bot/tests/test_smoke.py`:

```python
def test_package_imports():
    import osquery_approval_bot  # noqa: F401
```

- [ ] **Step 4: Generate the lockfile.** Run:

```bash
nix develop .#run --command bash -c 'cd dot_local/share/osquery-approval-bot && uv lock'
```

Expected: writes `uv.lock`, exit 0.

- [ ] **Step 5: Run the smoke test green.**

```bash
nix develop .#run --command bash -c 'cd dot_local/share/osquery-approval-bot && uv run --frozen pytest -q'
```

Expected: 1 passed.

- [ ] **Step 6: Commit.**

```bash
git add dot_local/share/osquery-approval-bot/pyproject.toml dot_local/share/osquery-approval-bot/uv.lock dot_local/share/osquery-approval-bot/source/osquery_approval_bot/__init__.py dot_local/share/osquery-approval-bot/tests/test_smoke.py
git commit -m "feat(osquery): scaffold approval-bot uv project (discord.py) + pytest harness"
```

______________________________________________________________________

## Task 3: Core logic — request id, pending store, auth gate, decision (pure, TDD)

All deterministic, no Discord. This is the unit-testable heart. The bot (Task 4) is thin glue over it.

**Files:**

- Create: `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/core.py`

- Create: `dot_local/share/osquery-approval-bot/tests/conftest.py`

- Create: `dot_local/share/osquery-approval-bot/tests/test_core.py`

- [ ] **Step 1: Write fixtures.** Create `dot_local/share/osquery-approval-bot/tests/conftest.py`:

```python
import os
import stat
from pathlib import Path

import pytest


@pytest.fixture
def state_directory(tmp_path: Path) -> Path:
    directory = tmp_path / "osquery-approval-bot"
    directory.mkdir()
    return directory


@pytest.fixture
def fake_allowlist_script(tmp_path: Path) -> Path:
    """A stand-in for osquery-allowlist.sh: on `-a <label>` appends the label to a file (exit 0),
    or exits 2 when the label contains '*' (to exercise the fail-closed path)."""
    allowlist = tmp_path / "allowlist.txt"
    allowlist.write_text("")
    script = tmp_path / "fake-allowlist.sh"
    script.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        f'allowlist="{allowlist}"\n'
        'while getopts ":a:d:l" option; do case "$option" in\n'
        '  a) case "$OPTARG" in *"*"*) echo bad >&2; exit 2;; esac\n'
        '     printf "%s\\n" "$OPTARG" >>"$allowlist" ;;\n'
        '  d) : ;;\n'
        '  l) cat "$allowlist" ;;\n'
        'esac; done\n'
    )
    script.chmod(script.stat().st_mode | stat.S_IEXEC)
    os.environ["TEST_ALLOWLIST_FILE"] = str(allowlist)
    return script
```

- [ ] **Step 2: Write the failing tests.** Create
  `dot_local/share/osquery-approval-bot/tests/test_core.py`:

```python
from pathlib import Path

from osquery_approval_bot import core


def test_request_id_is_stable_and_short():
    first = core.request_id_for_label("homebrew.mxcl.postgresql@17")
    second = core.request_id_for_label("homebrew.mxcl.postgresql@17")
    assert first == second
    assert len(first) == 16 and all(char in "0123456789abcdef" for char in first)
    assert first != core.request_id_for_label("com.other")


def test_write_and_read_pending(state_directory: Path):
    store = core.Store(state_directory)
    request_id = store.add_pending("com.foo", detector="persistence_launchd", summary="new agent com.foo")
    assert (state_directory / "pending" / f"{request_id}.json").exists()
    assert (state_directory / "active").exists()  # sentinel created
    request = store.read_pending(request_id)
    assert request["label"] == "com.foo"
    assert request["detector"] == "persistence_launchd"


def test_add_pending_is_idempotent_per_label(state_directory: Path):
    store = core.Store(state_directory)
    first_id = store.add_pending("com.foo", detector="d", summary="s")
    second_id = store.add_pending("com.foo", detector="d", summary="s")
    assert first_id == second_id
    assert len(list((state_directory / "pending").glob("*.json"))) == 1


def test_resolve_removes_pending_and_clears_sentinel_when_empty(state_directory: Path):
    store = core.Store(state_directory)
    request_id = store.add_pending("com.foo", detector="d", summary="s")
    store.resolve(request_id)
    assert not (state_directory / "pending" / f"{request_id}.json").exists()
    assert not (state_directory / "active").exists()  # last pending gone → sentinel removed


def test_resolve_keeps_sentinel_while_other_pending_remain(state_directory: Path):
    store = core.Store(state_directory)
    first_id = store.add_pending("com.a", detector="d", summary="s")
    store.add_pending("com.b", detector="d", summary="s")
    store.resolve(first_id)
    assert (state_directory / "active").exists()


def test_decline_writes_marker_and_resolves(state_directory: Path):
    store = core.Store(state_directory)
    request_id = store.add_pending("com.foo", detector="d", summary="s")
    store.decline(request_id)
    assert (state_directory / "declined" / request_id).exists()
    assert not (state_directory / "pending" / f"{request_id}.json").exists()


def test_is_authorized_fail_closed():
    assert core.is_authorized(user_id=1, channel_id=2, owner_id=1, channel_allow=2) is True
    assert core.is_authorized(user_id=9, channel_id=2, owner_id=1, channel_allow=2) is False
    assert core.is_authorized(user_id=1, channel_id=9, owner_id=1, channel_allow=2) is False
    assert core.is_authorized(user_id=None, channel_id=2, owner_id=1, channel_allow=2) is False


def test_apply_approve_calls_the_allowlist_script(state_directory: Path, fake_allowlist_script: Path):
    store = core.Store(state_directory)
    request_id = store.add_pending("com.foo", detector="d", summary="s")
    approved = core.apply_approve(store, request_id, allowlist_script=str(fake_allowlist_script))
    assert approved is True
    allowlist = Path(__import__("os").environ["TEST_ALLOWLIST_FILE"]).read_text()
    assert "com.foo" in allowlist
    assert not (state_directory / "pending" / f"{request_id}.json").exists()


def test_apply_approve_propagates_script_rejection(state_directory: Path, fake_allowlist_script: Path):
    store = core.Store(state_directory)
    request_id = store.add_pending("bad*label", detector="d", summary="s")
    approved = core.apply_approve(store, request_id, allowlist_script=str(fake_allowlist_script))
    assert approved is False  # script exited non-zero → not approved
    assert (state_directory / "pending" / f"{request_id}.json").exists()  # left pending for retry


def test_reconcile_sentinel_clears_orphan(state_directory: Path):
    store = core.Store(state_directory)
    store.sentinel.touch()  # orphan: sentinel present with an empty pending/ directory
    store.reconcile_sentinel()
    assert not store.sentinel.exists()
```

- [ ] **Step 3: Run red.**

```bash
nix develop .#run --command bash -c 'cd dot_local/share/osquery-approval-bot && uv run --frozen pytest -q tests/test_core.py'
```

Expected: FAIL — `core` has no such attributes.

- [ ] **Step 4: Implement `core.py`.** Create
  `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/core.py`:

```python
"""Deterministic core for the approval bot: ids, the pending store, auth, decisions.

No Discord imports here so this stays fully unit-testable. The bot module is thin glue.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path
from typing import Optional


def request_id_for_label(label: str) -> str:
    """Stable 16-hex id for a label, so the same label dedups to one pending request.
    Must match the alerter's `shasum -a 256 | cut -c1-16` byte-for-byte."""
    return hashlib.sha256(label.encode("utf-8")).hexdigest()[:16]


def is_authorized(*, user_id, channel_id, owner_id: int, channel_allow: int) -> bool:
    """Fail-closed: only the owner, only the security channel. None/mismatch → False."""
    if user_id is None or channel_id is None:
        return False
    return int(user_id) == int(owner_id) and int(channel_id) == int(channel_allow)


class Store:
    """Pending-request store under a 0700 directory, with the launchd KeepAlive sentinel."""

    def __init__(self, root: Path):
        self.root = Path(root)
        self.pending = self.root / "pending"
        self.declined = self.root / "declined"
        self.sentinel = self.root / "active"
        for d in (self.root, self.pending, self.declined):
            d.mkdir(parents=True, exist_ok=True)
            d.chmod(0o700)

    def _refresh_sentinel(self) -> None:
        if any(self.pending.glob("*.json")):
            self.sentinel.touch()
        elif self.sentinel.exists():
            self.sentinel.unlink()

    def reconcile_sentinel(self) -> None:
        """Public: make the sentinel match reality — clears an ORPHANED sentinel (present with
        no pending, e.g. after a SIGKILL between resolve()'s two unlinks, or a stray manual
        `touch active`). The bot calls this at boot so launchd's PathState cannot relaunch it
        into a connect/close loop."""
        self._refresh_sentinel()

    def add_pending(self, label: str, *, detector: str, summary: str) -> str:
        request_id = request_id_for_label(label)
        path = self.pending / f"{request_id}.json"
        if not path.exists():
            temp_path = path.with_suffix(".json.tmp")
            temp_path.write_text(json.dumps({"request_id": request_id, "label": label,
                                             "detector": detector, "summary": summary}))
            temp_path.replace(path)
        self._refresh_sentinel()
        return request_id

    def read_pending(self, request_id: str) -> Optional[dict]:
        path = self.pending / f"{request_id}.json"
        if not path.exists():
            return None
        return json.loads(path.read_text())

    def list_pending(self) -> list[dict]:
        return [json.loads(p.read_text()) for p in sorted(self.pending.glob("*.json"))]

    def resolve(self, request_id: str) -> None:
        (self.pending / f"{request_id}.json").unlink(missing_ok=True)
        self._refresh_sentinel()

    def decline(self, request_id: str) -> None:
        (self.declined / request_id).touch()
        self.resolve(request_id)


def apply_approve(store: Store, request_id: str, *, allowlist_script: str) -> bool:
    """Approve: call the shared allowlist script's add verb for the label. On success, resolve.
    The SCRIPT is the security boundary — we only pass it the label and trust its exit code."""
    request = store.read_pending(request_id)
    if request is None:
        return False
    result = subprocess.run([allowlist_script, "-a", request["label"]], capture_output=True, text=True)
    if result.returncode != 0:
        return False  # script refused (malformed / Apple-system label) → leave pending
    store.resolve(request_id)
    return True
```

- [ ] **Step 5: Run green.**

```bash
nix develop .#run --command bash -c 'cd dot_local/share/osquery-approval-bot && uv run --frozen pytest -q tests/test_core.py'
```

Expected: all tests PASS.

- [ ] **Step 6: Commit.**

```bash
git add dot_local/share/osquery-approval-bot/source/osquery_approval_bot/core.py dot_local/share/osquery-approval-bot/tests/conftest.py dot_local/share/osquery-approval-bot/tests/test_core.py
git commit -m "feat(osquery): approval-bot core (request id, pending store, fail-closed auth, decision→script)"
```

______________________________________________________________________

## Task 4: discord.py wiring — DynamicItem persistent buttons + callbacks (pytest-asyncio)

The bot's only Discord-touching module. Buttons survive restarts via `DynamicItem` (a custom_id
**template**, registered once at boot — no per-message re-add needed). The 3-second ack is `defer()`
first. The owner/channel gate is `core.is_authorized`. We TDD the callback against a **mocked**
`Interaction`; the live gateway connection is a manual Dresden smoke (Task 12/13).

**Files:**

- Create: `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/bot.py`

- Create: `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/__main__.py`

- Create: `dot_local/share/osquery-approval-bot/tests/test_bot.py`

- [ ] **Step 1: Write the failing async tests.** Create
  `dot_local/share/osquery-approval-bot/tests/test_bot.py`:

```python
import types
from pathlib import Path
from unittest.mock import AsyncMock

import pytest

from osquery_approval_bot import bot as botmod
from osquery_approval_bot import core


class FakeConfig:
    owner_id = 1
    channel_id = 2
    allowlist_script = None  # set per-test


def make_interaction(*, user_id, channel_id):
    interaction = types.SimpleNamespace()
    interaction.user = types.SimpleNamespace(id=user_id)
    interaction.channel_id = channel_id
    interaction.response = types.SimpleNamespace(defer=AsyncMock(), send_message=AsyncMock())
    interaction.followup = types.SimpleNamespace(send=AsyncMock())
    interaction.message = types.SimpleNamespace(edit=AsyncMock())
    return interaction


@pytest.fixture
def config(fake_allowlist_script: Path):
    config = FakeConfig()
    config.allowlist_script = str(fake_allowlist_script)
    return config


async def test_unauthorized_user_is_rejected_no_write(state_directory, config):
    store = core.Store(state_directory)
    request_id = store.add_pending("com.foo", detector="d", summary="s")
    interaction = make_interaction(user_id=999, channel_id=2)  # wrong user
    await botmod.handle_decision(interaction, action="approve", request_id=request_id,
                                 store=store, config=config)
    interaction.response.defer.assert_awaited()  # acked within 3s
    assert (state_directory / "pending" / f"{request_id}.json").exists()  # not approved
    interaction.message.edit.assert_not_awaited()  # buttons not disabled for an unauthorized tap


async def test_authorized_approve_writes_and_disables(state_directory, config):
    store = core.Store(state_directory)
    request_id = store.add_pending("com.foo", detector="d", summary="s")
    interaction = make_interaction(user_id=1, channel_id=2)
    await botmod.handle_decision(interaction, action="approve", request_id=request_id,
                                 store=store, config=config)
    interaction.response.defer.assert_awaited()
    import os
    assert "com.foo" in Path(os.environ["TEST_ALLOWLIST_FILE"]).read_text()
    assert not (state_directory / "pending" / f"{request_id}.json").exists()
    interaction.message.edit.assert_awaited()  # buttons disabled / outcome shown


async def test_authorized_deny_marks_declined(state_directory, config):
    store = core.Store(state_directory)
    request_id = store.add_pending("com.foo", detector="d", summary="s")
    interaction = make_interaction(user_id=1, channel_id=2)
    await botmod.handle_decision(interaction, action="deny", request_id=request_id,
                                 store=store, config=config)
    assert (state_directory / "declined" / request_id).exists()
    assert not (state_directory / "pending" / f"{request_id}.json").exists()
    interaction.message.edit.assert_awaited()


def test_render_prompt_mentions_label_and_actions(state_directory):
    text = botmod.render_prompt({"label": "com.foo", "detector": "persistence_launchd",
                                 "summary": "new user agent com.foo"})
    assert "com.foo" in text
    assert "Approve" in text and "Deny" in text


async def test_post_new_pending_dedups_on_refire_and_posts_concurrent(state_directory, config):
    # rank 3: a reconnect re-fire of on_ready must NOT re-post a still-pending prompt.
    # rank 4: a SECOND request arriving while the bot is already running MUST get posted.
    client = botmod.ApprovalClient(botmod.Config(
        token="x", owner_id=1, channel_id=2, state_root=state_directory,
        allowlist_script=config.allowlist_script))
    client._channel = AsyncMock()  # bypass fetch_channel (no live gateway)
    store = client.store
    store.add_pending("com.a", detector="d", summary="s")
    assert await client._post_new_pending() == 1   # first label posted
    assert await client._post_new_pending() == 0   # second call (re-fire) → no duplicate
    store.add_pending("com.b", detector="d", summary="s")
    assert await client._post_new_pending() == 1   # concurrent request → posted once
    assert client._channel.send.await_count == 2
```

- [ ] **Step 2: Run red.**

```bash
nix develop .#run --command bash -c 'cd dot_local/share/osquery-approval-bot && uv run --frozen pytest -q tests/test_bot.py'
```

Expected: FAIL — `bot` has no `handle_decision` / `render_prompt`.

- [ ] **Step 3: Implement `bot.py`.** Create
  `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/bot.py`:

```python
"""Discord wiring for the approval bot. Pending-scoped: posts Approve/Deny for each pending
request, exits when none remain. Persistent buttons via DynamicItem so a restart still owns
old buttons. Zero gateway intents (component interactions are delivered regardless)."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

import discord
from discord.ext import tasks

from . import core


@dataclass
class Config:
    token: str
    owner_id: int
    channel_id: int
    state_root: Path
    allowlist_script: str

    @classmethod
    def from_env(cls) -> "Config":
        return cls(
            token=os.environ["APPROVAL_BOT_DISCORD_TOKEN"],
            owner_id=int(os.environ["APPROVAL_BOT_OWNER_ID"]),
            channel_id=int(os.environ["APPROVAL_BOT_CHANNEL_ID"]),
            state_root=Path(os.environ.get(
                "APPROVAL_BOT_STATE",
                str(Path.home() / ".local/state/osquery-approval-bot"))),
            allowlist_script=os.environ.get(
                "APPROVAL_BOT_ALLOWLIST_SCRIPT",
                str(Path.home() / ".local/bin/osquery-allowlist.sh")),
        )


def render_prompt(request: dict) -> str:
    return (
        f"🆕 **New launchd label** `{request['label']}`\n"
        f"_{request.get('summary', '')}_\n\n"
        "**Approve** → add to the page allowlist (stops notifying). **Deny** → leave it (still digested)."
    )


def disabled_view(outcome_label: str) -> discord.ui.View:
    view = discord.ui.View(timeout=None)
    button = discord.ui.Button(label=outcome_label, style=discord.ButtonStyle.secondary, disabled=True)
    view.add_item(button)
    return view


async def handle_decision(interaction, *, action: str, request_id: str, store: core.Store, config) -> None:
    """Shared callback body. Ack within 3s, gate fail-closed, apply, then disable buttons."""
    await interaction.response.defer()
    if not core.is_authorized(user_id=interaction.user.id, channel_id=interaction.channel_id,
                              owner_id=config.owner_id, channel_allow=config.channel_id):
        await interaction.followup.send("Not authorized.", ephemeral=True)
        return
    request = store.read_pending(request_id)
    if request is None:
        await interaction.followup.send("Already resolved.", ephemeral=True)
        return
    if action == "approve":
        approved = core.apply_approve(store, request_id, allowlist_script=config.allowlist_script)
        outcome = (f"✅ Allowlisted `{request['label']}`" if approved
                   else f"⚠️ Allowlist script refused `{request['label']}`")
        if not approved:
            await interaction.followup.send(outcome, ephemeral=True)
            return
    else:
        store.decline(request_id)
        outcome = f"🚫 Denied `{request['label']}` (left in digest)"
    await interaction.message.edit(content=outcome, view=disabled_view(outcome))


class ApprovalButton(discord.ui.DynamicItem[discord.ui.Button],
                     template=r"osqa:(?P<action>approve|deny):(?P<request_id>[0-9a-f]{16})"):
    def __init__(self, action: str, request_id: str):
        self.action = action
        self.request_id = request_id
        style = discord.ButtonStyle.success if action == "approve" else discord.ButtonStyle.danger
        label = "Approve" if action == "approve" else "Deny"
        super().__init__(discord.ui.Button(label=label, style=style,
                                           custom_id=f"osqa:{action}:{request_id}"))

    @classmethod
    async def from_custom_id(cls, interaction, item, match):
        return cls(match["action"], match["request_id"])

    async def callback(self, interaction):
        client: "ApprovalClient" = interaction.client  # type: ignore[assignment]
        await handle_decision(interaction, action=self.action, request_id=self.request_id,
                              store=client.store, config=client.config)
        # When the last request resolves, exit cleanly (deterministic shutdown; launchd's
        # KeepAlive.PathState — sentinel gone — is the backstop, not the sole mechanism).
        if not client.store.list_pending():
            await client.close()


def prompt_view(request_id: str) -> discord.ui.View:
    view = discord.ui.View(timeout=None)
    view.add_item(ApprovalButton("approve", request_id))
    view.add_item(ApprovalButton("deny", request_id))
    return view


class ApprovalClient(discord.Client):
    def __init__(self, config: Config):
        super().__init__(intents=discord.Intents.none())
        self.config = config
        self.store = core.Store(config.state_root)
        self._posted: set[str] = set()  # request ids already announced — survives on_ready re-fires
        self._channel = None

    async def setup_hook(self) -> None:
        self.add_dynamic_items(ApprovalButton)  # restart-safe button routing

    async def _post_new_pending(self) -> int:
        """Post a prompt for each pending request id not yet announced. Idempotent across on_ready
        re-fires (discord.py re-fires on_ready on every RESUME) and reused by the poller."""
        pending = self.store.list_pending()
        if not pending:
            return 0
        if self._channel is None:
            self._channel = await self.fetch_channel(self.config.channel_id)
        posted = 0
        for request in pending:
            request_id = request["request_id"]
            if request_id in self._posted:
                continue
            self._posted.add(request_id)
            await self._channel.send(content=render_prompt(request), view=prompt_view(request_id))
            posted += 1
        return posted

    @tasks.loop(seconds=5)
    async def _poll_pending(self) -> None:
        # A second label arriving while the bot is already up writes a new pending file, but the
        # alerter's `launchctl kickstart` (no -k) is a no-op on a running process — so poll for it.
        if self.is_closed():
            return
        await self._post_new_pending()

    async def on_ready(self) -> None:
        # Self-heal an orphaned sentinel (present with no pending, e.g. after an abnormal exit) so
        # launchd's PathState cannot relaunch us into a connect/close loop; then exit if idle.
        self.store.reconcile_sentinel()
        await self._post_new_pending()
        if not self.store.list_pending():
            await self.close()
            return
        if not self._poll_pending.is_running():
            self._poll_pending.start()


def main() -> None:
    config = Config.from_env()
    ApprovalClient(config).run(config.token, log_handler=None)
```

- [ ] **Step 4: Implement the entrypoint.** Create
  `dot_local/share/osquery-approval-bot/source/osquery_approval_bot/__main__.py`:

```python
from .bot import main

if __name__ == "__main__":
    main()
```

- [ ] **Step 5: Run green.**

```bash
nix develop .#run --command bash -c 'cd dot_local/share/osquery-approval-bot && uv run --frozen pytest -q'
```

Expected: every test (smoke + core + bot) PASSES.

- [ ] **Step 6: Commit.**

```bash
git add dot_local/share/osquery-approval-bot/source/osquery_approval_bot/bot.py dot_local/share/osquery-approval-bot/source/osquery_approval_bot/__main__.py dot_local/share/osquery-approval-bot/tests/test_bot.py
git commit -m "feat(osquery): approval-bot discord wiring (DynamicItem persistent buttons, 3s defer, fail-closed gate)"
```

______________________________________________________________________

## Task 5: The wrapper script (load token + config, exec the venv python)

The LaunchAgent runs bash → this wrapper → the runtime venv python. The wrapper is where the secret token
and non-secret IDs become process env, mirroring the repo's "bash + script" LaunchAgent pattern.

**Files:**

- Create: `dot_local/bin/executable_osquery-approval-bot.sh`

- [ ] **Step 1: Implement the wrapper.** Create `dot_local/bin/executable_osquery-approval-bot.sh`:

```bash
#!/opt/homebrew/bin/bash
# Launches the osquery approval bot under its uv-built runtime venv.
# Reads the Butters token (600 file) and non-secret IDs (config file) into env, then execs.
set -euo pipefail

project_dir="$HOME/.local/share/osquery-approval-bot"
venv_python="$project_dir/.venv/bin/python"
token_file="${APPROVAL_BOT_TOKEN_FILE:-$HOME/.config/osquery/approval-bot-token}"
config_file="${APPROVAL_BOT_CONFIG_FILE:-$HOME/.config/osquery/approval-bot.config}"

[[ -x $venv_python ]] || {
  echo "approval-bot venv missing: $venv_python (run uv sync)" >&2
  exit 1
}
[[ -r $token_file ]] || {
  echo "approval-bot token missing: $token_file" >&2
  exit 1
}
[[ -r $config_file ]] || {
  echo "approval-bot config missing: $config_file" >&2
  exit 1
}

# config_file provides APPROVAL_BOT_OWNER_ID and APPROVAL_BOT_CHANNEL_ID (non-secret).
# shellcheck source=/dev/null
source "$config_file"

APPROVAL_BOT_DISCORD_TOKEN="$(tr -d '\r\n' <"$token_file")"
export APPROVAL_BOT_DISCORD_TOKEN APPROVAL_BOT_OWNER_ID APPROVAL_BOT_CHANNEL_ID

exec "$venv_python" -m osquery_approval_bot
```

- [ ] **Step 2: Lint it.**

```bash
nix develop .#run --command bash -c 'shellcheck dot_local/bin/executable_osquery-approval-bot.sh && shfmt -i 2 -ci -s -d dot_local/bin/executable_osquery-approval-bot.sh'
```

Expected: no findings; no diff.

- [ ] **Step 3: Smoke the guard paths** (no token/config/venv → clean non-zero, no traceback):

```bash
APPROVAL_BOT_TOKEN_FILE=/nonexistent APPROVAL_BOT_CONFIG_FILE=/nonexistent \
  bash dot_local/bin/executable_osquery-approval-bot.sh; echo "exit=$?"
```

Expected: prints `approval-bot venv missing: ...` (or token/config missing) and `exit=1`.

- [ ] **Step 4: Commit.**

```bash
git add dot_local/bin/executable_osquery-approval-bot.sh
git commit -m "feat(osquery): approval-bot launch wrapper (load token+config, exec venv python)"
```

______________________________________________________________________

## Task 6: LaunchAgent plist (KeepAlive.PathState) + loader script

Pending-scoped via `KeepAlive.PathState` on the **sentinel**: launchd keeps the bot alive exactly while a
request is pending (and restarts it if it crashes mid-window). The alerter (Task 9) kickstarts it
explicitly for immediacy; PathState handles persistence.

> **Important launchd nuance (verified — `man 5 launchd.plist`, KeepAlive):** *"The use of this key
> implicitly implies `RunAtLoad`, causing launchd to speculatively launch the job."* So
> **`RunAtLoad=false` does NOT prevent a load-time/login launch** — `KeepAlive` (even the `PathState`
> dict form) makes launchd speculatively start the bot once at bootstrap and at each login, sentinel or
> no sentinel. That is **harmless here only because** `on_ready` calls `self.close()` when
> `list_pending()` is empty (Task 4): the speculative instance connects, finds nothing, and exits 0
> within seconds. We keep `RunAtLoad=false` to document intent, not because it suppresses the launch. Do
> **not** assert an instantaneous "not running" state in verification — see Task 12 Step 2.

**Files:**

- Create: `Library/LaunchAgents/com.webdavis.osquery-approval-bot.plist.tmpl`

- Create: `.chezmoiscripts/run_onchange_after_60-load-osquery-approval-bot-launchagent.sh.tmpl`

- [ ] **Step 1: Write the plist template.** Create
  `Library/LaunchAgents/com.webdavis.osquery-approval-bot.plist.tmpl`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.webdavis.osquery-approval-bot</string>
  <key>ProgramArguments</key>
  <array>
    <string>/opt/homebrew/bin/bash</string>
    <string>{{ .chezmoi.homeDir }}/.local/bin/osquery-approval-bot.sh</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
  </dict>
  <key>RunAtLoad</key>
  <false/>
  <key>KeepAlive</key>
  <dict>
    <key>PathState</key>
    <dict>
      <key>{{ .chezmoi.homeDir }}/.local/state/osquery-approval-bot/active</key>
      <true/>
    </dict>
  </dict>
  <key>ProcessType</key>
  <string>Background</string>
  <key>StandardOutPath</key>
  <string>{{ .chezmoi.homeDir }}/.local/log/osquery/approval-bot.log</string>
  <key>StandardErrorPath</key>
  <string>{{ .chezmoi.homeDir }}/.local/log/osquery/approval-bot.log</string>
</dict>
</plist>
```

- [ ] **Step 2: Write the loader script** (mirrors the existing osquery loaders exactly — bootout,
  bootstrap with retry, plist-hash change-detection). Create
  `.chezmoiscripts/run_onchange_after_60-load-osquery-approval-bot-launchagent.sh.tmpl`:

```bash
{{- if eq .chezmoi.os "darwin" }}
#!/bin/bash
# run_onchange_after_60-load-osquery-approval-bot-launchagent.sh
# plist hash: {{ include "Library/LaunchAgents/com.webdavis.osquery-approval-bot.plist.tmpl" | sha256sum }}

set -euo pipefail

mkdir -p "$HOME/.local/log/osquery"
mkdir -p "$HOME/.local/state/osquery-approval-bot"
chmod 0700 "$HOME/.local/state/osquery-approval-bot"

PLIST="$HOME/Library/LaunchAgents/com.webdavis.osquery-approval-bot.plist"
TARGET="gui/$(id -u)/com.webdavis.osquery-approval-bot"

launchctl bootout "$TARGET" 2>/dev/null || true

for _ in 1 2 3; do
  if launchctl bootstrap "gui/$(id -u)" "$PLIST" 2>/dev/null; then
    exit 0
  fi
  sleep 1
done

launchctl bootstrap "gui/$(id -u)" "$PLIST"
{{- end }}
```

- [ ] **Step 3: Validate the rendered plist is well-formed XML.** Run:

```bash
nix develop .#run --command chezmoi --source . execute-template < "Library/LaunchAgents/com.webdavis.osquery-approval-bot.plist.tmpl" | plutil -lint -
```

Expected: `- : OK` (the rendered plist parses). (`plutil` is system-provided on macOS.)

- [ ] **Step 4: Shellcheck the loader (render first, per the repo's template-shellcheck workaround).**

```bash
nix develop .#run --command bash -c 'CI=1 chezmoi --source . execute-template --no-tty < ".chezmoiscripts/run_onchange_after_60-load-osquery-approval-bot-launchagent.sh.tmpl" | shellcheck -'
```

Expected: no findings.

- [ ] **Step 5: Commit.**

```bash
git add "Library/LaunchAgents/com.webdavis.osquery-approval-bot.plist.tmpl" ".chezmoiscripts/run_onchange_after_60-load-osquery-approval-bot-launchagent.sh.tmpl"
git commit -m "feat(osquery): approval-bot LaunchAgent (RunAtLoad=false, KeepAlive.PathState sentinel) + loader"
```

______________________________________________________________________

## Task 7: Secret + config templating, chezmoi data, CLAUDE.md

The Butters token is a new KeePassXC secret (600 file, mirrors the `webhook-secret` pattern).
Owner/channel IDs are **non-secret** chezmoi data templated into a separate config file. The token
template is KeePassXC-gated → add it to CLAUDE.md's interactive-apply list.

**Manual prerequisite (Stephen, interactive — not an agent step):** create the Discord application
"Butters"

- bot, invite it to the guild with the `bot` scope and **View Channel + Send Messages on the security
  channel only**, and store its token in KeePassXC as entry **`Hermes :: Discord :: Butters Token`**
  (Password field). Note the owner user-id, security channel-id, guild-id (Discord → Developer Mode →
  Copy ID).

**Files:**

- Create: `dot_config/osquery/private_approval-bot-token.tmpl`

- Create: `dot_config/osquery/approval-bot.config.tmpl`

- Modify: `.chezmoi.toml.tmpl` (add `[data.osquery]` IDs)

- Modify: `CLAUDE.md` (KeePassXC-gated list)

- [ ] **Step 1: The token template (600 via `private_`).** Create
  `dot_config/osquery/private_approval-bot-token.tmpl`:

```
{{ (keepassxc "Hermes :: Discord :: Butters Token").Password }}
```

- [ ] **Step 2: Add the non-secret IDs to chezmoi data.** In `.chezmoi.toml.tmpl`, add a `[data.osquery]`
  block (create it if PR #1 hasn't already; if PR #1 added `digestHour`/`digestMinute` here, append these
  keys to the same block). Replace the placeholder integers with the real IDs you copied:

```toml
[data.osquery]
  approvalOwnerId = 000000000000000000
  approvalChannelId = 000000000000000000
  approvalGuildId = 000000000000000000
```

- [ ] **Step 3: The non-secret config template.** Create `dot_config/osquery/approval-bot.config.tmpl`:

```
# Non-secret IDs for the osquery approval bot. Sourced by osquery-approval-bot.sh.
APPROVAL_BOT_OWNER_ID={{ .osquery.approvalOwnerId }}
APPROVAL_BOT_CHANNEL_ID={{ .osquery.approvalChannelId }}
```

- [ ] **Step 4: Add the token to the KeePassXC-gated list in `CLAUDE.md`.** In the "Never run bare
  `chezmoi apply`" paragraph, append `~/.config/osquery/approval-bot-token` to the list of templates that
  call `keepassxc`:

```
... `~/Library/Application Support/gogcli/credentials.json`, `~/.config/osquery/approval-bot-token`. Apply
those from an interactive terminal with KeePassXC unlocked.
```

- [ ] **Step 5: Verify templates render (non-secret one fully; secret one parses).** Run:

```bash
nix develop .#run --command chezmoi --source . execute-template < dot_config/osquery/approval-bot.config.tmpl
```

Expected: prints `APPROVAL_BOT_OWNER_ID=<id>` / `APPROVAL_BOT_CHANNEL_ID=<id>` with the real ids (proves
the `[data.osquery]` wiring). (Do **not** render the token template here — it needs KeePassXC/TTY.)

- [ ] **Step 6: Commit.**

```bash
git add dot_config/osquery/private_approval-bot-token.tmpl dot_config/osquery/approval-bot.config.tmpl .chezmoi.toml.tmpl CLAUDE.md
git commit -m "feat(osquery): approval-bot secret (KeePassXC) + non-secret IDs (chezmoi data) + CLAUDE.md gate"
```

______________________________________________________________________

## Task 8: Build the runtime venv at apply time (`uv sync`)

The LaunchAgent runs `~/.local/share/osquery-approval-bot/.venv/bin/python`. That venv is built at
`chezmoi apply` from the committed `pyproject.toml` + `uv.lock`, keyed so it only re-syncs when the lock
changes (network at apply, never at boot). `--no-dev` keeps the runtime venv lean (no pytest).

**Files:**

- Create: `.chezmoiscripts/run_onchange_after_55-uv-sync-approval-bot.sh.tmpl`

- [ ] **Step 1: Write the sync script.** Create
  `.chezmoiscripts/run_onchange_after_55-uv-sync-approval-bot.sh.tmpl`:

```bash
{{- if eq .chezmoi.os "darwin" }}
#!/bin/bash
# run_onchange_after_55-uv-sync-approval-bot.sh
# lock hash: {{ include "dot_local/share/osquery-approval-bot/uv.lock" | sha256sum }}

set -euo pipefail

project_dir="$HOME/.local/share/osquery-approval-bot"
[[ -f "$project_dir/uv.lock" ]] || exit 0

if ! command -v uv >/dev/null 2>&1; then
  echo "uv not found on PATH; skipping approval-bot venv build (install uv, then re-apply)" >&2
  exit 0
fi

cd "$project_dir"
uv sync --frozen --no-dev
{{- end }}
```

- [ ] **Step 2: Shellcheck (render first).**

```bash
nix develop .#run --command bash -c 'CI=1 chezmoi --source . execute-template --no-tty < ".chezmoiscripts/run_onchange_after_55-uv-sync-approval-bot.sh.tmpl" | shellcheck -'
```

Expected: no findings.

- [ ] **Step 3: Dry-run the sync locally** (proves `pyproject`+`uv.lock` resolve and the runtime venv
  builds; this is the same command the apply-time script runs):

```bash
nix develop .#run --command bash -c 'cd dot_local/share/osquery-approval-bot && uv sync --frozen --no-dev && ls .venv/bin/python'
```

Expected: prints a path ending `.venv/bin/python`, exit 0. (This `.venv` is chezmoi-ignored — Task 0.)

- [ ] **Step 4: Commit.**

```bash
git add ".chezmoiscripts/run_onchange_after_55-uv-sync-approval-bot.sh.tmpl"
git commit -m "feat(osquery): build approval-bot runtime venv via uv sync at apply time"
```

______________________________________________________________________

## Task 9: Alerter integration — drop a pending request + kickstart

When PR #1's gate routes a **new, non-allowlisted user LaunchAgent label** to the digest, PR #2 ALSO
drops a pending-approval request and kickstarts the bot — so you get a tap-to-allowlist button. Dedup by
request id (same label → same file, no re-prompt); skip labels already declined.

> **Locate the integration point against PR #1's code:** in
> `dot_local/bin/executable_osquery-results-alerter.sh`, find the `persistence_launchd` arm of the
> 3-outcome gate where a user LaunchAgent label that is **not** in the allowlist is appended to the
> digest (`_digest_append … ; continue`). Insert the helper call **immediately before** that
> `_digest_append`.

**Files:**

- Modify: `dot_local/bin/executable_osquery-results-alerter.sh`

- Test: `test/osquery-alerter/test_approval_integration.bats`

- [ ] **Step 1: Write the failing bats.** Create `test/osquery-alerter/test_approval_integration.bats`:

```bash
#!/usr/bin/env bats
# Exercises the pure helper that the alerter sources. We test the helper directly
# (no launchd needed) by stubbing `launchctl`.

setup() {
  TMP="$(mktemp -d)"
  export HOME="$TMP"
  mkdir -p "$TMP/.config/osquery" "$TMP/bin" "$TMP/.local/bin"
  export OSQUERY_LAUNCHD_ALLOWLIST="$TMP/.config/osquery/page-launchd-allowlist.txt"
  printf '# header\ncom.known\n' >"$OSQUERY_LAUNCHD_ALLOWLIST"
  # Stub launchctl so kickstart is observable and never touches the real system.
  cat >"$TMP/bin/launchctl" <<'EOF'
#!/usr/bin/env bash
echo "$@" >>"$HOME/.launchctl.calls"
EOF
  chmod +x "$TMP/bin/launchctl"
  export PATH="$TMP/bin:$PATH"
  # Stub the dispatcher the alerter sources at top-level (HOME=$TMP, so the real one is absent).
  # Without this, `source`-ing the alerter under `set -e` aborts before _approval_offer is defined.
  # send_alert() is load-bearing in the real run (alerter lines 314/327), so the source line stays.
  printf '#!/usr/bin/env bash\nsend_alert() { :; }\n' >"$TMP/.local/bin/osquery-alert-dispatch.sh"
  HELPER="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-results-alerter.sh"
}

teardown() { rm -rf "$TMP"; }

# Source only the helper function out of the alerter (it is guarded to define-only when sourced).
load_helper() { OSQUERY_ALERTER_SOURCE_ONLY=1 source "$HELPER"; }

@test "a new non-allowlisted label drops a pending request and kickstarts the bot" {
  load_helper
  _approval_offer "com.newtool" "persistence_launchd" "new user agent com.newtool"
  run bash -c 'ls "$HOME/.local/state/osquery-approval-bot/pending/"*.json'
  [ "$status" -eq 0 ]
  run grep -q 'com.webdavis.osquery-approval-bot' "$HOME/.launchctl.calls"
  [ "$status" -eq 0 ]
  run test -e "$HOME/.local/state/osquery-approval-bot/active"
  [ "$status" -eq 0 ]
}

@test "an already-allowlisted label offers nothing" {
  load_helper
  _approval_offer "com.known" "persistence_launchd" "x"
  run bash -c 'ls "$HOME/.local/state/osquery-approval-bot/pending/" 2>/dev/null'
  [ -z "$output" ]
}

@test "a previously-declined label is not re-offered" {
  load_helper
  mkdir -p "$HOME/.local/state/osquery-approval-bot/declined"
  request_id="$(echo -n com.declined | shasum -a 256 | cut -c1-16)"
  touch "$HOME/.local/state/osquery-approval-bot/declined/$request_id"
  _approval_offer "com.declined" "persistence_launchd" "x"
  run bash -c 'ls "$HOME/.local/state/osquery-approval-bot/pending/" 2>/dev/null'
  [ -z "$output" ]
}
```

- [ ] **Step 2: Run red.**

```bash
nix develop .#run --command bats test/osquery-alerter/test_approval_integration.bats
```

Expected: FAIL — `_approval_offer` undefined / no source-only guard.

- [ ] **Step 3: Add the helper + the source-only guard + the call site** in
  `dot_local/bin/executable_osquery-results-alerter.sh`.

  3a. **Placement is load-bearing.** The real alerter does
  `source "$HOME/.local/bin/osquery-alert-dispatch.sh"` right after its var defaults (around line 19).
  Insert the helper **and** the `... && return 0` early-return **ABOVE that `source` line** — i.e.
  immediately after `set -euo pipefail` and the `LOG=`/`STATE=` defaults, before the dispatcher `source`
  and before any `mkdir`. If the guard lands *below* the `source`, a test `source` (HOME=$TMP, no
  dispatcher) aborts under `set -e` before `_approval_offer` is defined. (The Task 9 bats `setup()` also
  drops a stub dispatcher as belt-and-suspenders.) If PR #1 already defines `ALLOWLIST`, reuse it — add
  only `APPROVAL_STATE`, the helper, and the early-return:

```bash
APPROVAL_STATE="${OSQUERY_APPROVAL_STATE:-$HOME/.local/state/osquery-approval-bot}"
ALLOWLIST="${OSQUERY_LAUNCHD_ALLOWLIST:-$HOME/.config/osquery/page-launchd-allowlist.txt}"

# Offer a tap-to-allowlist approval for a new, non-allowlisted, non-declined launchd label.
# Idempotent per label (request_id = sha256(label)[:16]); kickstarts the pending-scoped bot.
_approval_offer() {
  local label="$1" detector="$2" summary="$3" request_id pending_file
  [[ -n $label ]] || return 0
  grep -qxF -- "$label" "$ALLOWLIST" 2>/dev/null && return 0
  request_id="$(printf '%s' "$label" | shasum -a 256 | cut -c1-16)"
  [[ -e "$APPROVAL_STATE/declined/$request_id" ]] && return 0
  mkdir -p "$APPROVAL_STATE/pending"
  chmod 0700 "$APPROVAL_STATE"
  pending_file="$APPROVAL_STATE/pending/$request_id.json"
  if [[ ! -e $pending_file ]]; then
    jq -cn --arg request_id "$request_id" --arg label "$label" \
      --arg detector "$detector" --arg summary "$summary" \
      '{request_id:$request_id,label:$label,detector:$detector,summary:$summary}' \
      >"$pending_file.tmp" && mv -f "$pending_file.tmp" "$pending_file"
  fi
  touch "$APPROVAL_STATE/active"
  launchctl kickstart "gui/$(id -u)/com.webdavis.osquery-approval-bot" 2>/dev/null || true
}

# When sourced by tests, define functions and stop before doing real work.
[[ -n ${OSQUERY_ALERTER_SOURCE_ONLY:-} ]] && return 0
```

3b. In the `persistence_launchd` digest arm (the user-LaunchAgent, not-allowlisted branch PR #1 created),
call the helper immediately before the existing `_digest_append`:

```bash
        # PR #2: offer a tap-to-allowlist approval alongside the digest entry.
        _approval_offer "$label" "persistence_launchd" "new user LaunchAgent ${label}"
        _digest_append "$finding"; continue ;;
```

- [ ] **Step 4: Run green.**

```bash
nix develop .#run --command bats test/osquery-alerter/test_approval_integration.bats
```

Expected: 3 PASS. Then run the full alerter bats suite to prove no regression:

```bash
nix develop .#run --command bats test/osquery-alerter/
```

Expected: all PR #1 alerter tests still PASS (the source-only guard returns before the main body, so they
are unaffected).

- [ ] **Step 5: Shellcheck the alerter (render not needed — it's a plain `.sh`, not a `.tmpl`).**

```bash
nix develop .#run --command bash -c 'shellcheck dot_local/bin/executable_osquery-results-alerter.sh && shfmt -i 2 -ci -s -d dot_local/bin/executable_osquery-results-alerter.sh'
```

Expected: no findings; no diff.

- [ ] **Step 6: Commit.**

```bash
git add dot_local/bin/executable_osquery-results-alerter.sh test/osquery-alerter/test_approval_integration.bats
git commit -m "feat(osquery): alerter offers tap-to-allowlist (pending request + kickstart) for new user-agent labels"
```

______________________________________________________________________

## Task 10: Watchdog guard — sentinel exists ⇒ bot running

The bot is *supposed* to be down when idle, so it must **not** join the `AGENTS` array (that would alarm
on its normal state). Instead a separate check: if the sentinel exists but the bot is not running,
kickstart it (covers a missed PathState start or a crash between watchdog ticks).

**Files:**

- Modify: `dot_local/bin/executable_osquery-uptime-watchdog.sh`

- Test: append to `test/osquery-alerter/test_approval_integration.bats`

- [ ] **Step 1: Add the failing test** (append to the existing integration bats from Task 9):

```bash
@test "watchdog kickstarts the bot when a pending sentinel exists but the bot is down" {
  watchdog_script="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-uptime-watchdog.sh"
  mkdir -p "$HOME/.local/state/osquery-approval-bot"
  touch "$HOME/.local/state/osquery-approval-bot/active"
  # `launchctl list <label>` must report "not loaded" → our stub returns non-zero for `list`.
  cat >"$HOME/bin/launchctl" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  list) exit 1 ;;                                  # not loaded
  *) echo "$@" >>"$HOME/.launchctl.calls" ;;       # record kickstart/bootstrap
esac
EOF
  chmod +x "$HOME/bin/launchctl"
  OSQUERY_WATCHDOG_SOURCE_ONLY=1 source "$watchdog_script"
  _approval_bot_guard
  run grep -q 'kickstart .*com.webdavis.osquery-approval-bot' "$HOME/.launchctl.calls"
  [ "$status" -eq 0 ]
}

@test "watchdog leaves the bot alone when no sentinel (idle is correct)" {
  watchdog_script="${BATS_TEST_DIRNAME}/../../dot_local/bin/executable_osquery-uptime-watchdog.sh"
  rm -f "$HOME/.local/state/osquery-approval-bot/active"
  OSQUERY_WATCHDOG_SOURCE_ONLY=1 source "$watchdog_script"
  _approval_bot_guard
  run bash -c 'grep -q kickstart "$HOME/.launchctl.calls" 2>/dev/null'
  [ "$status" -ne 0 ]
}
```

- [ ] **Step 2: Run red.**

```bash
nix develop .#run --command bats test/osquery-alerter/test_approval_integration.bats
```

Expected: the two new tests FAIL (`_approval_bot_guard` undefined / no source-only guard).

- [ ] **Step 3: Implement the guard + source-only hook** in
  `dot_local/bin/executable_osquery-uptime-watchdog.sh`. The real watchdog does
  `source "$HOME/.local/bin/osquery-alert-dispatch.sh"` around line 21, just below the `AGENTS` array
  (lines 15–18). Add the function **and** the `... && return 0` early-return **after the `AGENTS` array
  but ABOVE that `source` line and above any real work** (the agent-check loop, the spool drain), so a
  test `source` returns before either. (The shared Task 9 `setup()` also drops a stub dispatcher, so the
  source cannot fail the load even if PR #1 reorders things.)

```bash
# PR #2: the approval bot is pending-scoped (down when idle). Do NOT add it to AGENTS.
# If a pending sentinel exists but the bot is not loaded, (re)start it.
_approval_bot_guard() {
  local sentinel="$HOME/.local/state/osquery-approval-bot/active"
  [[ -e $sentinel ]] || return 0
  launchctl list "com.webdavis.osquery-approval-bot" >/dev/null 2>&1 && return 0
  launchctl kickstart "gui/$(id -u)/com.webdavis.osquery-approval-bot" 2>/dev/null || true
}

[[ -n ${OSQUERY_WATCHDOG_SOURCE_ONLY:-} ]] && return 0
```

Then call it inside the watchdog's main flow, right after the existing `for agent in "${AGENTS[@]}"` loop
(after line ~38):

```bash
_approval_bot_guard
```

- [ ] **Step 4: Run green.**

```bash
nix develop .#run --command bats test/osquery-alerter/test_approval_integration.bats
```

Expected: all integration tests (Task 9 + Task 10) PASS.

- [ ] **Step 5: Shellcheck.**

```bash
nix develop .#run --command bash -c 'shellcheck dot_local/bin/executable_osquery-uptime-watchdog.sh && shfmt -i 2 -ci -s -d dot_local/bin/executable_osquery-uptime-watchdog.sh'
```

Expected: no findings; no diff.

- [ ] **Step 6: Commit.**

```bash
git add dot_local/bin/executable_osquery-uptime-watchdog.sh test/osquery-alerter/test_approval_integration.bats
git commit -m "feat(osquery): watchdog kickstarts the approval bot while a pending request exists"
```

______________________________________________________________________

## Task 11: The `/osquery` Hermes skill (typed fallback)

The skill is **agent-mediated** (the agent reads it and runs the writers), so it is not deterministically
unit-testable — its safety rests on the fail-closed writers (Tasks 1 + PR #1) and the owner+channel
scope. Verification here is a `SKILL.md` lint + a manual Dresden smoke (Task 12).

> **Human-approval gate:** this writes under `dot_hermes/` (the Hermes tree). Per the repo rules, review
> and apply it interactively; never from automation.

**Files:**

- Create: `dot_hermes/skills/osquery/SKILL.md`

- [ ] **Step 1: Write the skill.** Create `dot_hermes/skills/osquery/SKILL.md`:

```markdown
---
name: osquery
description: Manage the osquery page-launchd allowlist from Discord. Use ONLY when the owner types `/osquery allow <label>`, `/osquery deny <label>`, or `/osquery list` in the security channel. Adds/removes an exact launchd label to/from the page allowlist, or lists it. Never infer a label the user did not type.
---

# osquery allowlist control

You manage the osquery **page-launchd allowlist** — the file that suppresses paging for known-good launchd
labels — through one tool, `~/.local/bin/osquery-allowlist.sh`. Three exact sub-commands; the trailing text
after `/osquery` is the instruction.

**Hard rules (do not deviate):**

- You are the agent. YOU run the one shell command for the typed sub-command (via your terminal tool),
  substituting only the **exact label the owner typed** — never guess, expand, or infer a label.
- The script is the security boundary; it validates and refuses bad input. Run it verbatim and report its
  exit status. Do not edit the allowlist file directly.
- One action per invocation. After running it, reply with one short confirmation line. Do nothing else.

**allow `<label>`** — add the label to the allowlist (stops paging). Run this exact command, substituting
the typed label:

`~/.local/bin/osquery-allowlist.sh -a '<label>'`

- exit 0 → reply: `✅ Allowlisted <label>`
- non-zero → reply: `⚠️ Refused <label> (invalid or an Apple/system label)`

**deny `<label>`** — remove the label from the allowlist (undo). Run:

`~/.local/bin/osquery-allowlist.sh -d '<label>'`

- exit 0 → reply: `↩️ Removed <label> from the allowlist`
- non-zero → reply: `⚠️ Refused <label> (invalid label)`

**list** — show the current allowlist. Run:

`~/.local/bin/osquery-allowlist.sh -l`

- reply with the script's output verbatim in a fenced block (it already formats the count and labels).
```

- [ ] **Step 2: Lint the markdown.**

```bash
nix develop .#run --command mdformat --check dot_hermes/skills/osquery/SKILL.md || nix develop .#run --command mdformat dot_hermes/skills/osquery/SKILL.md
```

Expected: clean (or formatted in place, then clean).

- [ ] **Step 3: Verify the frontmatter parses + the body calls only the one real script.**

```bash
nix develop .#run --command bash -c "yq -f extract '.name' dot_hermes/skills/osquery/SKILL.md 2>/dev/null || sed -n '1,6p' dot_hermes/skills/osquery/SKILL.md"
grep -oE 'osquery-allowlist\.sh -[adl]' dot_hermes/skills/osquery/SKILL.md | sort -u
```

Expected: shows `name: osquery`; lists exactly `osquery-allowlist.sh -a`, `osquery-allowlist.sh -d`, and
`osquery-allowlist.sh -l` (the one tool that exists — PR #1 + Task 1). No other script names.

- [ ] **Step 4: Commit.**

```bash
git add dot_hermes/skills/osquery/SKILL.md
git commit -m "feat(osquery): /osquery allow|deny|list Hermes skill (typed fallback, calls the one allowlist tool)"
```

______________________________________________________________________

## Task 12: Full-suite green, interactive apply, end-to-end Dresden smoke, open the PR

This is the verification-before-completion gate. No new code — prove the whole thing works on the host,
then open the reviewed PR.

- [ ] **Step 1: Full test + lint suite green.**

```bash
just test
just l
```

Expected: all bats + all pytest PASS; lint reports no diffs.

- [ ] **Step 2: Interactive apply (TTY + KeePassXC unlocked — Stephen, not an agent).** This is the only
  way the Butters token + the Hermes skill reach `$HOME`. Use a **bare full `chezmoi apply`** — NOT a
  scoped path list and NOT `--exclude=templates`:

```bash
chezmoi diff                      # review the token, config, plist, skill, scripts
chezmoi apply                     # bare: renders the KeePassXC token AND fires the run_onchange scripts
```

> **Why bare/full, not scoped (verified, chezmoi v2.70.5):** a path-scoped apply that does not *name* the
> `.chezmoiscripts/run_onchange_*` scripts does **not** fire them, so the venv (Task 8 `…after_55`) is
> never built and the LaunchAgent (Task 6 `…after_60`) is never bootstrapped — Step 2's own checks below
> then fail. `chezmoi apply <script>` is also wrong (a `run_` script has no `$HOME` target →
> `not managed`, exit 1, aborting the whole command), and `--exclude=templates` skips both templated
> run_onchange scripts. The agent-only "never bare apply / `--exclude=templates`" rule (CLAUDE.md) does
> **not** bind a human at an unlocked TTY — that rule exists precisely so agents don't trigger KeePassXC
> prompts; here you *want* the token rendered. A full apply renders the token, fires both run_onchange
> scripts (their hashes are new on a fresh host), and lands the bot tree in one pass.

Confirm the scripts did their work:

```bash
ls ~/.local/share/osquery-approval-bot/.venv/bin/python      # built by run_onchange_after_55 (uv sync)
launchctl print "gui/$(id -u)/com.webdavis.osquery-approval-bot" >/dev/null 2>&1 && echo bootstrapped
test -e ~/.local/state/osquery-approval-bot/active && echo SENTINEL-PRESENT || echo no-sentinel
tail -n 20 ~/.local/log/osquery/approval-bot.log 2>/dev/null   # speculative instance: connect→exit 0
```

Expected: the venv python exists (built by `run_onchange_after_55`); the LaunchAgent is `bootstrapped`
(by `run_onchange_after_60`); `no-sentinel`. **Do not expect an instantaneous "not running" state** —
because `KeepAlive` implies a speculative load-time launch (Task 6 note), launchd starts the bot once at
bootstrap; with no sentinel it connects, `on_ready` finds nothing, and it self-closes (exit 0) within
seconds. The log should show a clean connect→exit and **no traceback**, and there should be no leftover
`active` sentinel.

- [ ] **Step 3: Button smoke.** Simulate a pending request and confirm the bot posts + a tap works:

```bash
mkdir -p ~/.local/state/osquery-approval-bot/pending
request_id="$(echo -n 'com.smoketest.example' | shasum -a 256 | cut -c1-16)"
printf '{"request_id":"%s","label":"com.smoketest.example","detector":"persistence_launchd","summary":"smoke"}' \
  "$request_id" > ~/.local/state/osquery-approval-bot/pending/"$request_id".json
touch ~/.local/state/osquery-approval-bot/active
launchctl kickstart "gui/$(id -u)/com.webdavis.osquery-approval-bot"
```

Expected: an Approve/Deny message appears in the security channel from **Butters**. Tap **Approve** →
`com.smoketest.example` appears in `~/.config/osquery/page-launchd-allowlist.txt`; the message edits to
the outcome; the pending file and `active` sentinel are gone; the bot process exits. Tap **Deny** instead
(on a fresh request) → a `declined/<request_id>` marker is written, allowlist unchanged. Clean up the
smoke label:

```bash
~/.local/bin/osquery-allowlist.sh -d com.smoketest.example
rm -f ~/.local/state/osquery-approval-bot/declined/*
```

- [ ] **Step 4: Skill smoke.** In the security channel, type `/osquery list` (expect the current
  allowlist), `/osquery allow com.smoketest.example` (expect `✅`, label added),
  `/osquery deny com.smoketest.example` (expect `↩️`, label removed). Confirm no agent/LLM action fired
  for an ordinary message containing the word "list" (type `list my open PRs` → normal Bob behavior, no
  allowlist change).

- [ ] **Step 5: Wrong-user gate smoke (if a second account is available, optional).** A non-owner tap →
  ephemeral "Not authorized", allowlist unchanged. (Otherwise this is covered by `test_bot.py`.)

- [ ] **Step 6: Update the decision record + open the PR.** Tick D-V2-15's build items as done in
  `docs/superpowers/decisions/2026-06-10-osquery-alerting-v2-decision-addendum.md` if it tracks status,
  then:

```bash
git push -u origin feat/osquery-approval-ux
gh pr create --title "feat(osquery): PR #2 — tap-to-approve buttons (Butters) + /osquery skill" \
  --body "Implements D-V2-15: pending-scoped Discord approval bot + /osquery allow|deny|list skill, both writing the shared fail-closed allowlist writer. Buttons primary (LLM-free), skill fallback (agent-mediated). Depends on PR #1."
```

Expected: PR opens against `main` for review (merge per \[[github-pr-merge-convention]\] after approval).

______________________________________________________________________

## Self-Review

**1. Spec coverage (D-V2-15 + spec §9):**

- Tap Approve/Deny under Butters' own token → Tasks 4, 6, 7. ✓
- Pending-scoped daemon (alerter kickstart → `KeepAlive.PathState` → exit on resolution → zero steady
  state) → Tasks 6 (PathState), 9 (kickstart), 4 (exit when empty), 10 (watchdog). ✓
- Persistent views (timeout=None, stable custom_id, registered every boot, 3s defer) → Task 4
  (`DynamicItem` template + `add_dynamic_items` in `setup_hook` + `defer()` first). ✓
- Owner+channel auth, fail-closed → `core.is_authorized` (Task 3) + `handle_decision` (Task 4). ✓
- New secret = Butters token, KeePassXC → chezmoi, `bot` scope + View/Send only → Task 7 (+ manual
  prereq). ✓
- Runtime = uv; LaunchAgent execs `.venv/bin/python`; `uv sync` at apply (network at apply — not at boot;
  a cold-cache CI `--frozen` also needs network), interpreter pinned `<3.13` so the runtime venv matches
  → Tasks 0, 2, 8. ✓
- Tests = pytest + pytest-asyncio; the one allowlist tool keeps bats → Tasks 1, 3, 4 (+ `just test`
  wiring Task 0). ✓
- `/osquery allow|deny|list` skill, rides Bob, retires the plugin/`osq`-prefix → Task 11. ✓
- Skill is agent-mediated; the script is the boundary; buttons stay LLM-free primary → Tasks 3/4 (script
  subprocess), 11 (skill calls the script). ✓
- Same file under the same contract for every caller → `-a` add (PR #1), `-d`/`-l` deny/list (Task 1),
  the bot (Task 4, `-a`) and the skill (Task 11, all three). ✓

**2. Placeholder scan:** No "TBD"/"handle errors"/"similar to". The one intentionally deferred value is
the real Discord ID integers in Task 7 (the manual prereq supplies them; the template wiring is
concrete). The Task 9 insertion point is described against PR #1's named arm (not a line number) because
PR #1 is a separate, not-yet-built change — flagged explicitly. ✓

**3. Type/name consistency:** `core.Store`, `add_pending/read_pending/list_pending/resolve/decline`,
`reqid_for_label`, `is_authorized`, `apply_approve`; `bot.handle_decision/render_prompt/ApprovalClient`;
custom_id template `osqa:(approve|deny):<16hex>` is `ApprovalButton.template` (discord.py
compiles/matches it itself — there is no separate module-level regex); env vars
`APPROVAL_BOT_DISCORD_TOKEN/OWNER_ID/CHANNEL_ID/STATE/ADD_WRITER` consistent across `bot.py`, the wrapper
(Task 5), and the config template (Task 7); sentinel path `~/.local/state/osquery-approval-bot/active`
identical in `core.py`, the plist PathState (Task 6), the alerter helper (Task 9), and the watchdog guard
(Task 10); the one `osquery-allowlist.sh` tool (`-a`/`-d`/`-l`) is consistent across core, skill, tests.
✓

**Known seams flagged for the executor:** (a) `Intents.none()` + `fetch_channel` is the minimal-privilege
choice — if the live connection misbehaves on Dresden, that is the first thing to verify (Task 12 smoke).
(b) custom_id ≤ 100 chars: a label longer than ~88 chars would overflow `osqa:approve:<16hex>`+label-free
design — but we key on the request id (16 hex), so label length is irrelevant to the button; only the
pending JSON holds the label. ✓ (c) PathState auto-start latency is backstopped by the explicit kickstart
(Task 9) and the watchdog (Task 10).

______________________________________________________________________

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-13-osquery-approval-ux-pr2-plan.md`. Two
execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task with two-stage review between tasks;
   fast iteration, each task's tests gate the next.
1. **Inline Execution** — execute tasks in this session with checkpoints for review.

Which approach? (Note the hard prerequisites before either: **PR #1 merged**, and the **manual Butters
setup** — create the Discord app/bot/token and copy the owner/channel/guild IDs — since Tasks 7 and 12
can't complete without them.)
