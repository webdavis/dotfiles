# Homebrew Weekly Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Homebrew's daily unattended auto-upgrader with a weekly Monday-noon upgrade that runs only when the operator is present.

**Architecture:** A chezmoi-managed user LaunchAgent fires a resilient helper script every Monday 12:00 (`brew update`/`outdated`/`upgrade` + `mas outdated`/`upgrade` + `cleanup`). The old `domt4/autoupdate` tap and its schedule are torn down. No Gatekeeper/quarantine bypass.

**Tech Stack:** chezmoi (Go templates), bash, launchd (`StartCalendarInterval`), Homebrew + `mas`, the repo's `just`/`scripts/lint.sh` tooling.

**Reference spec:** `docs/superpowers/specs/2026-06-21-homebrew-weekly-upgrade-design.md`

## Global Constraints

- Operator is REMOTE until ~Monday noon: **never run the real `brew upgrade`/`mas upgrade` as a test** — the first real run is the scheduled Monday-noon one. All verification is plumbing-only (render+shellcheck, `plutil -lint`, mock-based test, `launchctl print`, `brew autoupdate status`).
- `RunAtLoad=false` on the LaunchAgent — loading it must never trigger an upgrade.
- `Weekday 1 = Monday` in launchd (`man launchd.plist`: "0 and 7 are Sunday").
- Shell style: `#!/usr/bin/env bash` for helpers/tests, `#!/bin/bash` for chezmoiscripts; `set -euo pipefail` (helper uses `set -uo pipefail` — see Task 1); 2-space indent, `shfmt -i 2 -ci -s`.
- Every commit passes the pre-commit hook (`just lint-check` + `just test`).
- Branch: `feat/cli-agent-tracking-workflow`.

---

### Task 1: Weekly-upgrade helper + resilience test + just recipe

**Files:**
- Create: `dot_local/bin/executable_homebrew-weekly-upgrade.sh`
- Create: `test/homebrew-weekly-upgrade.sh`
- Modify: `justfile` (add `brew-upgrade` recipe near `test-brew-cache`)

**Interfaces:**
- Produces: helper at `~/.local/bin/homebrew-weekly-upgrade.sh`; prints sectioned report to stdout with markers `== brew update ==`, `== brew outdated ==`, `== mas outdated ==`, `== brew upgrade ==`, `== mas upgrade ==`, `== brew cleanup ==`, `=== done …`. Honors `HOMEBREW_WEEKLY_BREW` / `HOMEBREW_WEEKLY_MAS` overrides (default `/opt/homebrew/bin/brew`, `/opt/homebrew/bin/mas`) for test injection. Consumed by the plist (Task 2) and `just brew-upgrade`.

- [ ] **Step 1: Write the failing test** — `test/homebrew-weekly-upgrade.sh`

```bash
#!/usr/bin/env bash
#
# Verifies homebrew-weekly-upgrade.sh is resilient: a failing step is logged but
# does NOT abort the run, and every later step (including cleanup) still runs.
# Uses mock brew/mas (no real upgrade), so it is safe to run anywhere.
set -uo pipefail

helper="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/dot_local/bin/executable_homebrew-weekly-upgrade.sh"

if [[ ! -x $helper ]]; then
  echo "homebrew-weekly-upgrade: FAIL -- helper not found/executable: $helper" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Mock brew: succeed on everything EXCEPT `upgrade`, which fails (exit 1) -- to
# prove the run continues past a failed step.
cat >"$tmp/brew" <<'MOCK'
#!/usr/bin/env bash
echo "mock brew $*"
[[ ${1:-} == upgrade ]] && exit 1
exit 0
MOCK
# Mock mas: succeed on everything.
cat >"$tmp/mas" <<'MOCK'
#!/usr/bin/env bash
echo "mock mas $*"
exit 0
MOCK
chmod +x "$tmp/brew" "$tmp/mas"

out="$(HOMEBREW_WEEKLY_BREW="$tmp/brew" HOMEBREW_WEEKLY_MAS="$tmp/mas" bash "$helper" 2>&1)"

fail=0
for marker in "== brew update ==" "== brew outdated ==" "== mas outdated ==" \
  "== brew upgrade ==" "== mas upgrade ==" "== brew cleanup ==" "=== done"; do
  if ! grep -qF "$marker" <<<"$out"; then
    echo "homebrew-weekly-upgrade: FAIL -- missing section: $marker" >&2
    fail=1
  fi
done
grep -qF "FAILED" <<<"$out" || {
  echo "homebrew-weekly-upgrade: FAIL -- failed step not reported" >&2
  fail=1
}

if [[ $fail -ne 0 ]]; then
  echo "--- helper output ---" >&2
  echo "$out" >&2
  exit 1
fi
echo "homebrew-weekly-upgrade: OK -- resilient (continued past failure; all sections + cleanup ran)"
```

- [ ] **Step 2: Run the test, verify it fails**

Run: `bash test/homebrew-weekly-upgrade.sh`
Expected: FAIL — "helper not found/executable" (the helper does not exist yet).

- [ ] **Step 3: Write the helper** — `dot_local/bin/executable_homebrew-weekly-upgrade.sh`

```bash
#!/usr/bin/env bash
#
# homebrew-weekly-upgrade.sh -- run by the com.webdavis.homebrew-weekly-upgrade
# LaunchAgent every Monday at 12:00 (when the operator is present). Upgrades
# Homebrew formulae + casks + Mac App Store apps, then cleans up. Prints a
# sectioned, timestamped report to stdout; the LaunchAgent routes that to
# ~/.local/log/homebrew/weekly-upgrade.log. Resilient: a failing step is logged
# but never aborts the rest, and cleanup always runs. No Gatekeeper/quarantine
# stripping -- present-time "Open?" prompts are acceptable (operator is here).
#
# brew/mas are overridable (HOMEBREW_WEEKLY_BREW / HOMEBREW_WEEKLY_MAS) so the
# test harness can inject mocks; default to absolute Homebrew paths.
set -uo pipefail

BREW="${HOMEBREW_WEEKLY_BREW:-/opt/homebrew/bin/brew}"
MAS="${HOMEBREW_WEEKLY_MAS:-/opt/homebrew/bin/mas}"

run() {
  # run "<label>" cmd args... -- print a section header, run, log the outcome,
  # and continue regardless of exit status.
  local label="$1"
  shift
  printf '== %s ==\n' "$label"
  if "$@"; then
    printf '   ok: %s\n' "$label"
  else
    printf '   FAILED (exit %d): %s\n' "$?" "$label" >&2
  fi
}

printf '=== homebrew-weekly-upgrade %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

run "brew update" "$BREW" update
run "brew outdated" "$BREW" outdated
run "mas outdated" "$MAS" outdated
run "brew upgrade" "$BREW" upgrade
run "mas upgrade" "$MAS" upgrade
run "brew cleanup" "$BREW" cleanup

printf '=== done %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
```

- [ ] **Step 4: Run the test, verify it passes**

Run: `bash test/homebrew-weekly-upgrade.sh`
Expected: PASS — "OK -- resilient …". (Confirms `brew upgrade` failing did not stop `mas upgrade`/`brew cleanup`/done.)

- [ ] **Step 5: Add the `just brew-upgrade` recipe** — `justfile`, immediately after the `test-brew-cache` recipe

```text
# Run the weekly Homebrew upgrade by hand (formulae + casks + Mac App Store +
# cleanup). Same job the Monday-noon LaunchAgent runs; use for the first upgrade
# or any ad-hoc one. Uses the host brew, outside the Nix shell.
brew-upgrade:
  ./dot_local/bin/executable_homebrew-weekly-upgrade.sh
```

- [ ] **Step 6: Validate shellcheck + recipe parse**

Run: `shellcheck dot_local/bin/executable_homebrew-weekly-upgrade.sh test/homebrew-weekly-upgrade.sh`
Expected: clean.
Run: `just --show brew-upgrade`
Expected: prints the recipe (parses OK).

- [ ] **Step 7: Commit**

```bash
git add dot_local/bin/executable_homebrew-weekly-upgrade.sh test/homebrew-weekly-upgrade.sh justfile
git commit -m "feat(brew): weekly-upgrade helper + resilience test + just recipe"
```

---

### Task 2: Weekly-upgrade LaunchAgent plist

**Files:**
- Create: `Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist.tmpl`

**Interfaces:**
- Consumes: the helper from Task 1 (`~/.local/bin/homebrew-weekly-upgrade.sh`).
- Produces: label `com.webdavis.homebrew-weekly-upgrade`; loaded by the loader in Task 3.

- [ ] **Step 1: Write the plist**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.webdavis.homebrew-weekly-upgrade</string>
  <key>ProgramArguments</key>
  <array>
    <string>/opt/homebrew/bin/bash</string>
    <string>{{ .chezmoi.homeDir }}/.local/bin/homebrew-weekly-upgrade.sh</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    <key>HOME</key>
    <string>{{ .chezmoi.homeDir }}</string>
  </dict>
  <key>RunAtLoad</key>
  <false/>
  <key>StartCalendarInterval</key>
  <dict>
    <key>Weekday</key>
    <integer>1</integer>
    <key>Hour</key>
    <integer>12</integer>
    <key>Minute</key>
    <integer>0</integer>
  </dict>
  <key>StandardOutPath</key>
  <string>{{ .chezmoi.homeDir }}/.local/log/homebrew/weekly-upgrade.log</string>
  <key>StandardErrorPath</key>
  <string>{{ .chezmoi.homeDir }}/.local/log/homebrew/weekly-upgrade.log</string>
</dict>
</plist>
```

- [ ] **Step 2: Render + validate the plist**

Run: `CI=1 chezmoi execute-template --no-tty < Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist.tmpl | plutil -lint -`
Expected: `- : OK` (well-formed plist; `RunAtLoad => false`, `Weekday => 1`, `Hour => 12`).

- [ ] **Step 3: Commit**

```bash
git add Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist.tmpl
git commit -m "feat(brew): Monday-noon weekly-upgrade LaunchAgent (RunAtLoad=false)"
```

---

### Task 3: Loader chezmoiscript + lint wiring

**Files:**
- Create: `.chezmoiscripts/run_onchange_after_65-load-homebrew-weekly-upgrade-launchagent.sh.tmpl`
- Modify: `scripts/lint.sh` (add the loader to `find_shell_templates`)

**Interfaces:**
- Consumes: the plist from Task 2.
- Produces: a loaded LaunchAgent (`launchctl bootstrap`) on apply.

- [ ] **Step 1: Write the loader** (exact copy of `run_onchange_after_40-load-atuin-daemon-launchagent.sh.tmpl`, retargeted)

```text
#!/bin/bash
# run_onchange_after_65-load-homebrew-weekly-upgrade-launchagent.sh
# plist hash: {{ include "Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist.tmpl" | sha256sum }}

{{- if eq .chezmoi.os "darwin" }}
set -euo pipefail

mkdir -p "$HOME/.local/log/homebrew"

PLIST="$HOME/Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist"
TARGET="gui/$(id -u)/com.webdavis.homebrew-weekly-upgrade"

# Re-bootstrap so a changed plist takes effect. launchd may return EIO when
# bootstrap fires right after bootout; retry silently up to 3 times before
# surfacing the real error on the final attempt.
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

- [ ] **Step 2: Add the loader to `find_shell_templates`** — `scripts/lint.sh`, in the `-o -name` list (after the brew-shellenv line)

```bash
    -o -name "run_after_44-cache-brew-shellenv.sh.tmpl" \
    -o -name "run_onchange_after_65-load-homebrew-weekly-upgrade-launchagent.sh.tmpl" \
```

- [ ] **Step 3: Render + shellcheck the loader**

Run: `CI=1 chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_65-load-homebrew-weekly-upgrade-launchagent.sh.tmpl | shellcheck -`
Expected: clean.

- [ ] **Step 4: Confirm lint picks it up**

Run: `just s` (shellcheck via the renderer)
Expected: ✅ shellcheck (the new loader is now in the rendered-template set).

- [ ] **Step 5: Commit**

```bash
git add .chezmoiscripts/run_onchange_after_65-load-homebrew-weekly-upgrade-launchagent.sh.tmpl scripts/lint.sh
git commit -m "feat(brew): loader for the weekly-upgrade LaunchAgent + lint wiring"
```

---

### Task 4: Remove the daily auto-upgrader

**Files:**
- Modify: `.chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl` (insert teardown after `set -euo pipefail` on line 4; delete the old autoupdate block)
- Modify: `.chezmoidata/system_packages_autoinstall.yaml` (remove `domt4/autoupdate` from `taps:` line 8 and `trusted_taps:` line 32)

**Interfaces:** none consumed/produced by other tasks.

- [ ] **Step 1: Insert the teardown at the top of the darwin block** — after `set -euo pipefail` (line 4), before the `# Pre-trust taps` comment (line 6)

```bash
# Tear down the old domt4/autoupdate daily auto-upgrader (replaced by the
# com.webdavis.homebrew-weekly-upgrade Monday-noon LaunchAgent). MUST run BEFORE
# `brew bundle --cleanup` below untaps domt4/autoupdate -- otherwise the
# `brew autoupdate` subcommand is gone when we call it. Guarded on the tap being
# present so it is a clean no-op once removed / on fresh machines.
if /opt/homebrew/bin/brew tap | grep -q '^domt4/autoupdate$'; then
  /opt/homebrew/bin/brew autoupdate stop 2>/dev/null || true
  /opt/homebrew/bin/brew autoupdate delete 2>/dev/null || true
fi
```

- [ ] **Step 2: Delete the old autoupdate block** — remove these lines (the `# Configure Homebrew autoupdate …` block):

```bash
# Configure Homebrew autoupdate (idempotent — only restart if not already running).
HOMEBREW_BIN="${HOMEBREW_PREFIX:-/opt/homebrew}/bin/brew"

if ! "$HOMEBREW_BIN" autoupdate status 2>/dev/null | grep -q "running"; then
  echo "Starting domt4/autoupdate [stop → delete → start]..."
  "$HOMEBREW_BIN" autoupdate stop 2>/dev/null || true
  "$HOMEBREW_BIN" autoupdate delete 2>/dev/null || true
  "$HOMEBREW_BIN" autoupdate start 86400 --upgrade --cleanup --immediate --sudo
else
  echo "Homebrew autoupdate already running — skipping restart."
fi
```

- [ ] **Step 3: Remove `domt4/autoupdate` from the package data** — `.chezmoidata/system_packages_autoinstall.yaml`

Delete line 8 (`        - domt4/autoupdate # Automatic Upgrades.`) under `taps:` and line 32 (`        - domt4/autoupdate`) under `trusted_taps:`.

- [ ] **Step 4: Render + shellcheck the script**

Run: `CI=1 chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl | shellcheck -`
Expected: clean.
Run: `just y` (yq validates the YAML)
Expected: ✅ yq.

- [ ] **Step 5: Commit**

```bash
git add .chezmoiscripts/run_onchange_before_10-system-packages.sh.tmpl .chezmoidata/system_packages_autoinstall.yaml
git commit -m "feat(brew): remove domt4/autoupdate daily auto-upgrader (teardown before untap)"
```

---

### Task 5: Document in CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` (a subsection under "System Package Management")

- [ ] **Step 1: Add the subsection** — after the existing System Package Management content

```markdown
**Weekly upgrades (not daily).** The `domt4/autoupdate` daily auto-upgrader has been removed in favor of
a chezmoi-managed user LaunchAgent, `com.webdavis.homebrew-weekly-upgrade`, that runs
`~/.local/bin/homebrew-weekly-upgrade.sh` every **Monday 12:00** (launchd `Weekday 1 = Monday`;
`man launchd.plist`: "0 and 7 are Sunday"), when the operator is present — so app restarts/prompts never
happen unattended. The helper does `brew update` → log `brew outdated`/`mas outdated` → `brew upgrade` →
`mas upgrade` → `brew cleanup`, is resilient (a failing step is logged but does not abort the run), and
does **no** Gatekeeper/quarantine stripping. `RunAtLoad=false` so loading the agent never triggers an
upgrade. Run it on demand with `just brew-upgrade`; logs at `~/.local/log/homebrew/weekly-upgrade.log`.
The `run_onchange_before_10-system-packages` script tears down any old autoupdate **before**
`brew bundle --cleanup` untaps it (ordering is load-bearing — do not reorder).
```

- [ ] **Step 2: Format + verify**

Run: `just m` (mdformat)
Expected: ✅ mdformat (105-col wrap applied).

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document the weekly Homebrew upgrade LaunchAgent"
```

---

### Task 6: Activate now + verify (rollout)

**Files:** none (live actions on this machine; connection-safe — no upgrade runs).

- [ ] **Step 1: Tear down the old autoupdater + untap** (mirrors Task 4's committed logic, applied to this machine now)

```bash
/opt/homebrew/bin/brew autoupdate stop 2>/dev/null || true
/opt/homebrew/bin/brew autoupdate delete 2>/dev/null || true
/opt/homebrew/bin/brew untap domt4/autoupdate 2>/dev/null || true
```

- [ ] **Step 2: Place + load the LaunchAgent** (specific non-KeePassXC targets — no full `chezmoi apply`)

```bash
chezmoi apply --force "$HOME/Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist"
mkdir -p "$HOME/.local/log/homebrew"
launchctl bootout "gui/$(id -u)/com.webdavis.homebrew-weekly-upgrade" 2>/dev/null || true
launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/com.webdavis.homebrew-weekly-upgrade.plist"
```

- [ ] **Step 3: Verify the schedule + that nothing upgraded**

```bash
launchctl print "gui/$(id -u)/com.webdavis.homebrew-weekly-upgrade" | grep -iE 'state|runatload|weekday|hour'
/opt/homebrew/bin/brew autoupdate status 2>&1 | head -1   # expect not-installed / command gone
/opt/homebrew/bin/brew tap | grep -c '^domt4/autoupdate$' # expect 0
test ! -s "$HOME/.local/log/homebrew/weekly-upgrade.log" && echo "log empty -> no upgrade ran (correct)"
```
Expected: agent loaded, `RunAtLoad=false`, Weekday=1/Hour=12; autoupdate/tap gone; log empty (no upgrade ran).

- [ ] **Step 4: Verify the `mas` caveat** (read-only)

```bash
/opt/homebrew/bin/mas version && /opt/homebrew/bin/mas outdated
```
If `mas outdated` errors on macOS 26.2: drop the `mas upgrade` line from the helper (Task 1), re-run its test, recommit, and note in CLAUDE.md that MAS falls back to App Store auto-update. If it works: no change.

- [ ] **Step 5: Final suite + status**

Run: `just l && just test`
Expected: all ✅. Then `git status` clean (everything committed in Tasks 1–5).

---

## Self-Review

**Spec coverage:** remove daily autoupdate → Task 4 + Task 6 step 1; weekly LaunchAgent → Task 2; helper (update/outdated/upgrade/mas/cleanup, resilient, log, no-strip) → Task 1; loader → Task 3; lint + docs → Task 3/Task 5; MAS included → Task 1 (`mas upgrade`) + Task 6 step 4 caveat; outdated-only/scoped → inherent in `brew upgrade` (documented); rollout activate-now → Task 6; verification plumbing-only → all tasks. No gaps.

**Placeholder scan:** none — every file's full content and every command is inline.

**Type/name consistency:** label `com.webdavis.homebrew-weekly-upgrade` and helper path `~/.local/bin/homebrew-weekly-upgrade.sh` and log path `~/.local/log/homebrew/weekly-upgrade.log` are identical across the plist (Task 2), loader (Task 3), helper (Task 1), and activation (Task 6). Section markers asserted in the test (Task 1 step 1) match those printed by the helper (Task 1 step 3). The override env vars `HOMEBREW_WEEKLY_BREW`/`HOMEBREW_WEEKLY_MAS` match between test and helper.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-21-homebrew-weekly-upgrade.md`.
