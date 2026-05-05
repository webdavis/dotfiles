# macOS Defaults Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add declarative tracking of macOS user defaults and sudo-required system settings to this chezmoi repo, applied automatically at `chezmoi apply` time, with helpers for drift detection, forced reapply, capture (current-state-to-YAML), and discovery.

**Architecture:** Two `.chezmoidata/` YAML files (`macos_defaults.yaml`, `macos_system_setup.yaml`) drive two Tier-1/Tier-2 chezmoiscript runners (rendered to OS-empty bodies on Linux). Three helper scripts under `dot_local/bin/` (drift, apply, capture) cover the daily workflow. Six justfile recipes glue the surface (`D`, `defaults-apply`, `defaults-capture`, `defaults-list`, `defaults-show`, `defaults-dump`).

**Tech Stack:** chezmoi (Go templates + YAML data), bash 5+, macOS `defaults` / `killall` / `osascript`, yq (parse), shellcheck + shfmt (lint), just (recipes), KeePassXC unrelated to this work.

**Source spec:** `docs/superpowers/specs/2026-05-05-macos-defaults-management-design.md`. Read it before starting.

**Working-environment notes:**

- All commands assume `cwd = /Users/stephen/.local/share/chezmoi`.
- Lint via `just l` (runs all checkers); shell-only `just s`, mdformat-only `just m`, yq-only `just y`.
- Pre-commit hook runs `just l`. Use `SKIP_AI_COMMIT=1 git commit ...` to skip the AI commit-message-prepopulation hook (we'll write messages explicitly).
- Shell style: `set -euo pipefail`, `shfmt -i 2 -ci -s`, double-quote expansions, `cd X || exit`.
- Commit style: conventional commits, no `Co-Authored-By` trailer.
- Never run bare `chezmoi apply` from automation (KeePassXC templates will block); use `chezmoi apply --exclude=templates` for scripted apply, or named-file apply (`chezmoi apply ~/.specific-file`). The two new chezmoiscripts in this plan are NOT KeePassXC templates, so they're safe.

---

## File Structure

**Files to create:**

| Path | Responsibility |
|------|---------------|
| `.chezmoidata/macos_defaults.yaml` | Declarative source of `defaults write` records + `killall` list. |
| `.chezmoidata/macos_system_setup.yaml` | Declarative source of sudo-prefixed system commands. |
| `.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl` | Tier 1 runner: hash-gated darwin-only script that materializes one `defaults write` per record + `killall` post-loop. |
| `.chezmoiscripts/run_onchange_after_40-macos-system-setup.sh.tmpl` | Tier 2 runner: hash-gated darwin-only script that runs each system command (with sudo as configured). |
| `dot_local/bin/executable_macos-defaults-drift.sh` | Read-only drift checker; exit 1 on drift. |
| `dot_local/bin/executable_macos-defaults-apply.sh` | Forced reapplier; same write-loop as Tier 1 runner. |
| `dot_local/bin/executable_macos-defaults-capture.sh` | Capture helper: read live value+type, append to YAML. |
| `docs/runbooks/macos-fresh-machine-quickstart.md` | Manual checklist for fresh-Mac steps that aren't `defaults`-tractable. |

**Files to modify:**

| Path | Change |
|------|--------|
| `justfile` | Add 6 recipes: `D`, `defaults-apply`, `defaults-capture`, `defaults-list`, `defaults-show`, `defaults-dump`. |
| `CLAUDE.md` | Add "macOS Defaults" section. |
| `.chezmoiignore` | Linux-gate the three helper scripts. |

**Test approach (this repo has no shell unit-test framework):** Each script step uses `shellcheck` + `shfmt --diff` for lint correctness, plus a representative-input run on the live Mac with explicit expected exit codes and on-disk-state assertions. For chezmoiscripts: render via `chezmoi execute-template` and shellcheck the rendered bash. Strict-TDD red→green is replaced with **shellcheck-passes → script-runs-as-expected → state-matches-expectation**.

---

## Task 1: Create empty YAML data files

**Files:**

- Create: `.chezmoidata/macos_defaults.yaml`
- Create: `.chezmoidata/macos_system_setup.yaml`

- [ ] **Step 1: Create `macos_defaults.yaml` with empty arrays**

```yaml
# macOS user defaults — declarative source of truth.
#
# Each record under `defaults:` becomes one `defaults write` invocation at
# `chezmoi apply` time (Tier 1 runner: run_onchange_after_30-macos-defaults).
#
# Schema:
#   - domain: <string>   # com.apple.dock, NSGlobalDomain, etc.
#     key:    <string>
#     type:   bool|int|float|string
#     value:  <scalar>   # must match `type`
#     host:   current    # OPTIONAL — if set, runner uses `defaults -currentHost`
#
# Each entry in `killall:` runs after the defaults loop. cfprefsd is required
# for changes to take effect immediately (long-standing macOS plist-cache
# footgun).

macos:
  defaults: []
  killall:
    - Dock
    - Finder
    - SystemUIServer
    - cfprefsd
```

- [ ] **Step 2: Create `macos_system_setup.yaml` with empty array**

```yaml
# macOS sudo-required system settings — declarative source of truth.
#
# Each record under `system_setup:` is a command run by the Tier 2 runner
# (run_onchange_after_40-macos-system-setup) at `chezmoi apply` time. The
# runner does `sudo -v` once upfront, then prefixes commands with `sudo` only
# when `sudo: true`.
#
# All commands MUST be idempotent — the runner re-runs the entire list every
# time the YAML changes (chezmoi hash gate). For non-idempotent commands,
# put them in the runbook (docs/runbooks/macos-fresh-machine-quickstart.md)
# instead of here.
#
# Schema:
#   - description: <string>   # echoed before execution; trace output
#     command:     <string>   # bash, NO `sudo` prefix
#     sudo:        <bool>

macos:
  system_setup: []
```

- [ ] **Step 3: Verify both parse cleanly with yq**

Run:

```bash
nix develop .#run --command yq eval '.' .chezmoidata/macos_defaults.yaml
nix develop .#run --command yq eval '.' .chezmoidata/macos_system_setup.yaml
```

Expected: both echo back the parsed YAML structure, exit 0. If exit non-zero, fix the YAML.

- [ ] **Step 4: Run `just y` to confirm lint passes**

Run: `just y`
Expected: SUMMARY shows `yq ✅`. (Other tools may or may not be checked depending on what changed; only yq is the gate here.)

- [ ] **Step 5: Commit**

```bash
SKIP_AI_COMMIT=1 git add .chezmoidata/macos_defaults.yaml .chezmoidata/macos_system_setup.yaml
SKIP_AI_COMMIT=1 git commit -m "feat(chezmoidata): add macos_defaults and macos_system_setup data files

Empty arrays seed the new declarative-defaults workflow. The Tier 1
runner (next commit) will iterate macos.defaults and run defaults write
for each record; killall fires for each entry in macos.killall (cfprefsd
included so plist-cache changes take effect immediately). The Tier 2
runner will iterate macos.system_setup for sudo system commands."
```

---

## Task 2: Tier 1 runner (chezmoiscript)

**Files:**

- Create: `.chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl`

- [ ] **Step 1: Write the runner template**

```sh
{{ if eq .chezmoi.os "darwin" -}}
#!/usr/bin/env bash
# Tier 1 — macOS user defaults runner.
# chezmoi hash-gates on the rendered template body; this script re-runs only
# when .chezmoidata/macos_defaults.yaml changes.

set -euo pipefail

# Pre-flight: close System Settings if open. macOS caches plist values inside
# Settings and writes them back on close, silently overwriting our writes.
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true

# Main loop: one `defaults write` per record.
{{ range .macos.defaults -}}
defaults {{ if .host }}-currentHost {{ end }}write {{ .domain | quote }} {{ .key | quote }} -{{ .type }} {{ .value | quote }}
{{ end -}}

# Post-loop: restart user-facing processes so changes take effect immediately.
# cfprefsd kill is non-negotiable (caches plist values in memory).
{{ range .macos.killall -}}
killall {{ . | quote }} 2>/dev/null || true
{{ end }}
{{- end }}
```

- [ ] **Step 2: Render the template and inspect the output**

Run:

```bash
chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl
```

Expected on darwin: a bash script with `set -euo pipefail`, the osascript pre-flight, no `defaults write` lines (because `macos_defaults.yaml`'s array is still empty), and four `killall ... 2>/dev/null || true` lines.
Expected on linux: empty output (the outer `{{ if eq .chezmoi.os "darwin" }}...{{ end }}` evaluates to false).

- [ ] **Step 3: Shellcheck the rendered output**

Run:

```bash
chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl | shellcheck -
```

Expected: exit 0, no findings.

- [ ] **Step 4: Apply the template specifically (will run on darwin)**

Run:

```bash
chezmoi apply .chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl
```

Expected: chezmoi materializes the rendered script in its scripts cache and runs it once. Visible side effect: System Settings closes if it was open; the four `killall` invocations restart Dock/Finder/SystemUIServer/cfprefsd (you'll see Dock and Finder briefly redraw). Exit 0.

If you want to verify the run happened, check `chezmoi state get-bucket scriptState | grep macos-defaults` — there should be an entry.

- [ ] **Step 5: Commit**

```bash
SKIP_AI_COMMIT=1 git add .chezmoiscripts/run_onchange_after_30-macos-defaults.sh.tmpl
SKIP_AI_COMMIT=1 git commit -m "feat(chezmoiscripts): add tier 1 macos defaults runner

Hash-gated darwin-only script that iterates macos.defaults and runs
defaults [-currentHost] write for each record, then killalls each
entry in macos.killall. osascript pre-flight closes System Settings
to avoid the plist-cache-overwrite footgun. Renders to empty body on
linux."
```

---

## Task 3: Drift helper script (`just D`)

**Files:**

- Create: `dot_local/bin/executable_macos-defaults-drift.sh`
- Modify: `.chezmoiignore` (Linux gate)

- [ ] **Step 1: Write the drift script**

```bash
#!/usr/bin/env bash
# macos-defaults-drift.sh — read-only drift checker for tracked macOS defaults.
#
# Compares each record in .chezmoidata/macos_defaults.yaml against the live
# value via `defaults [-currentHost] read`. Prints a tab-aligned table of
# drifted rows only. Never writes.
#
# Exit codes:
#   0 — no drift
#   1 — drift detected
#   2 — data file missing or unreadable

set -euo pipefail

DATA_FILE="${HOME}/.local/share/chezmoi/.chezmoidata/macos_defaults.yaml"

if [[ ! -r "$DATA_FILE" ]]; then
  printf 'error: cannot read %s\n' "$DATA_FILE" >&2
  exit 2
fi

# Normalize a value for comparison. macOS stores bools as 0/1; YAML ships them
# as true/false. Strings/ints/floats compare directly.
normalize() {
  local type="$1" value="$2"
  case "$type" in
    bool)
      case "$value" in
        true | yes | 1) printf '1' ;;
        false | no | 0) printf '0' ;;
        *) printf '%s' "$value" ;;
      esac
      ;;
    *) printf '%s' "$value" ;;
  esac
}

drift_count=0
header_printed=0
print_header() {
  if ((header_printed == 0)); then
    printf 'DOMAIN\tKEY\tEXPECTED\tACTUAL\n'
    header_printed=1
  fi
}

# yq -r outputs each record as a single TSV line: domain<TAB>key<TAB>type<TAB>value<TAB>host
yq eval -r '.macos.defaults[] | [.domain, .key, .type, .value, (.host // "")] | @tsv' "$DATA_FILE" \
  | while IFS=$'\t' read -r domain key type value host; do
    expected="$(normalize "$type" "$value")"
    if [[ -n "$host" ]]; then
      actual="$(defaults -currentHost read "$domain" "$key" 2>/dev/null || printf '<unset>')"
    else
      actual="$(defaults read "$domain" "$key" 2>/dev/null || printf '<unset>')"
    fi
    if [[ "$expected" != "$actual" ]]; then
      print_header
      printf '%s\t%s\t%s\t%s\n' "$domain" "$key" "$expected" "$actual"
      drift_count=$((drift_count + 1))
    fi
  done

if ((drift_count > 0)); then
  printf '\n%d drift row(s) detected.\n' "$drift_count" >&2
  exit 1
fi
exit 0
```

- [ ] **Step 2: Lint with shellcheck and shfmt**

Run:

```bash
nix develop .#run --command shellcheck dot_local/bin/executable_macos-defaults-drift.sh
nix develop .#run --command shfmt -i 2 -ci -s --diff dot_local/bin/executable_macos-defaults-drift.sh
```

Expected: shellcheck exit 0, shfmt no diff (exit 0).

- [ ] **Step 3: Run against live Mac with empty YAML**

Run:

```bash
chezmoi apply ~/.local/bin/macos-defaults-drift.sh
~/.local/bin/macos-defaults-drift.sh
echo "exit=$?"
```

Expected: no output (empty YAML → no records to check), `exit=0`.

- [ ] **Step 4: Linux-gate the helper in `.chezmoiignore`**

Edit `.chezmoiignore`. Find the existing block at the bottom:

```
{{- if eq .chezmoi.os "linux" -}}
.config/yabai
Library
{{- end -}}
```

Replace it with:

```
{{- if eq .chezmoi.os "linux" -}}
.config/yabai
Library
.local/bin/macos-defaults-drift.sh
{{- end -}}
```

(Subsequent tasks add more entries to this block.)

- [ ] **Step 5: Commit**

```bash
SKIP_AI_COMMIT=1 git add dot_local/bin/executable_macos-defaults-drift.sh .chezmoiignore
SKIP_AI_COMMIT=1 git commit -m "feat(macos-defaults): add drift helper (just D)

Read-only checker that prints a tab-aligned table of tracked-defaults
rows whose live value diverges from the YAML. Exits 0 clean, 1 on
drift, 2 on missing data file. Bool normalization treats true/yes/1
as identical and false/no/0 as identical so YAML and macOS storage
compare cleanly. Linux-gated via .chezmoiignore."
```

---

## Task 4: Apply helper script (`just defaults-apply`)

**Files:**

- Create: `dot_local/bin/executable_macos-defaults-apply.sh`
- Modify: `.chezmoiignore`

- [ ] **Step 1: Write the apply script**

```bash
#!/usr/bin/env bash
# macos-defaults-apply.sh — forced reapply of tracked macOS defaults.
#
# Same defaults-write loop as the Tier 1 chezmoiscript runner, but invocable
# on demand without bumping the chezmoi hash gate. Use after fiddling in
# System Settings to revert disk state to the YAML.

set -euo pipefail

DATA_FILE="${HOME}/.local/share/chezmoi/.chezmoidata/macos_defaults.yaml"

if [[ ! -r "$DATA_FILE" ]]; then
  printf 'error: cannot read %s\n' "$DATA_FILE" >&2
  exit 2
fi

# Pre-flight: close System Settings if open (same reason as runner).
osascript -e 'tell application "System Settings" to quit' 2>/dev/null || true

# Main loop: one `defaults write` per record.
yq eval -r '.macos.defaults[] | [.domain, .key, .type, .value, (.host // "")] | @tsv' "$DATA_FILE" \
  | while IFS=$'\t' read -r domain key type value host; do
    if [[ -n "$host" ]]; then
      defaults -currentHost write "$domain" "$key" "-$type" "$value"
    else
      defaults write "$domain" "$key" "-$type" "$value"
    fi
  done

# Post-loop: restart processes per killall list.
yq eval -r '.macos.killall[]' "$DATA_FILE" \
  | while read -r proc; do
    killall "$proc" 2>/dev/null || true
  done

exit 0
```

- [ ] **Step 2: Lint with shellcheck and shfmt**

Run:

```bash
nix develop .#run --command shellcheck dot_local/bin/executable_macos-defaults-apply.sh
nix develop .#run --command shfmt -i 2 -ci -s --diff dot_local/bin/executable_macos-defaults-apply.sh
```

Expected: both exit 0.

- [ ] **Step 3: Run against live Mac with empty YAML**

Run:

```bash
chezmoi apply ~/.local/bin/macos-defaults-apply.sh
~/.local/bin/macos-defaults-apply.sh
echo "exit=$?"
```

Expected: System Settings closes if open; Dock/Finder/SystemUIServer/cfprefsd restart (visible Dock+Finder redraw); `exit=0`.

- [ ] **Step 4: Add to `.chezmoiignore` Linux gate**

Edit `.chezmoiignore`. Add the new line inside the linux block:

```
{{- if eq .chezmoi.os "linux" -}}
.config/yabai
Library
.local/bin/macos-defaults-drift.sh
.local/bin/macos-defaults-apply.sh
{{- end -}}
```

- [ ] **Step 5: Commit**

```bash
SKIP_AI_COMMIT=1 git add dot_local/bin/executable_macos-defaults-apply.sh .chezmoiignore
SKIP_AI_COMMIT=1 git commit -m "feat(macos-defaults): add apply helper (just defaults-apply)

Forced reapplier with the same defaults-write+killall loop as the Tier 1
chezmoiscript, invocable on demand without bumping the chezmoi hash
gate. Useful when System Settings has been touched and you want to
revert disk state to the YAML. Linux-gated."
```

---

## Task 5: Capture helper script (`just defaults-capture`)

**Files:**

- Create: `dot_local/bin/executable_macos-defaults-capture.sh`
- Modify: `.chezmoiignore`

- [ ] **Step 1: Write the capture script**

```bash
#!/usr/bin/env bash
# macos-defaults-capture.sh — append a live setting to macos_defaults.yaml.
#
# Reads the current value+type via `defaults read-type` + `defaults read`,
# normalizes, appends to the YAML if not already tracked. If the entry is
# already tracked AND the live value matches: no-op (exit 0). If the entry
# is already tracked but the live value DIVERGES: exit 2 (drift) — resolve
# via `just defaults-apply` (revert) or hand-edit YAML (capture intent).
#
# Usage: macos-defaults-capture.sh <domain> <key> [--host current]
#
# Exit codes:
#   0 — appended, or already in sync
#   1 — key not currently set on this Mac
#   2 — YAML has a different value than disk (drift; resolve before re-running)
#   3 — malformed args

set -euo pipefail

DATA_FILE="${HOME}/.local/share/chezmoi/.chezmoidata/macos_defaults.yaml"

usage() {
  printf 'usage: macos-defaults-capture <domain> <key> [--host current]\n' >&2
  exit 3
}

[[ $# -lt 2 || $# -gt 4 ]] && usage

domain="$1"
key="$2"
shift 2

# Optional host argument. Three accepted forms:
#   --host=current  (single token, what the justfile recipe emits)
#   --host current  (two tokens, what a direct CLI invocation might use)
#   (omitted)       (global storage, no -currentHost flag)
host=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --host=current)
      host="current"
      shift
      ;;
    --host)
      [[ $# -lt 2 || "$2" != "current" ]] && usage
      host="current"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

if [[ ! -r "$DATA_FILE" ]]; then
  printf 'error: cannot read %s\n' "$DATA_FILE" >&2
  exit 2
fi

# Read live type. `defaults read-type` outputs e.g. "Type is boolean".
host_flag=()
[[ -n "$host" ]] && host_flag=(-currentHost)

if ! raw_type="$(defaults "${host_flag[@]}" read-type "$domain" "$key" 2>/dev/null)"; then
  printf 'error: %s %s is not currently set on this Mac\n' "$domain" "$key" >&2
  exit 1
fi

case "$raw_type" in
  *boolean*) schema_type="bool" ;;
  *integer*) schema_type="int" ;;
  *float*) schema_type="float" ;;
  *string*) schema_type="string" ;;
  *)
    printf 'error: unsupported defaults type %q for %s %s (only bool/int/float/string in v1 schema)\n' \
      "$raw_type" "$domain" "$key" >&2
    exit 1
    ;;
esac

raw_value="$(defaults "${host_flag[@]}" read "$domain" "$key")"

# Normalize for YAML emission.
case "$schema_type" in
  bool)
    case "$raw_value" in
      1) yaml_value="true" ;;
      0) yaml_value="false" ;;
      *) yaml_value="$raw_value" ;;
    esac
    ;;
  string)
    # Quote the string for safe YAML emission.
    yaml_value="\"${raw_value//\"/\\\"}\""
    ;;
  *)
    yaml_value="$raw_value"
    ;;
esac

# Check whether (domain, key, host) is already in the YAML.
existing_value="$(yq eval -r \
  ".macos.defaults[] | select(.domain == \"$domain\" and .key == \"$key\" and ((.host // \"\") == \"$host\")) | .value" \
  "$DATA_FILE")"

if [[ -n "$existing_value" ]]; then
  # Already tracked. Compare.
  case "$schema_type" in
    bool)
      existing_norm="$existing_value"
      live_norm="$yaml_value"
      ;;
    string)
      existing_norm="\"$existing_value\""
      live_norm="$yaml_value"
      ;;
    *)
      existing_norm="$existing_value"
      live_norm="$yaml_value"
      ;;
  esac
  if [[ "$existing_norm" == "$live_norm" ]]; then
    printf 'already tracked: %s %s = %s\n' "$domain" "$key" "$existing_value"
    exit 0
  else
    printf 'drift: %s %s — yaml=%s disk=%s\n' "$domain" "$key" "$existing_value" "$raw_value" >&2
    printf '  resolve via `just defaults-apply` (revert) or hand-edit YAML.\n' >&2
    exit 2
  fi
fi

# Append a new record.
host_field=""
[[ -n "$host" ]] && host_field=", host: $host"

record="  - { domain: \"$domain\", key: \"$key\", type: $schema_type, value: $yaml_value$host_field }"

# Insert before the killall section. We do this by appending under macos.defaults
# in YAML using yq's `.macos.defaults += [...]` operator, which preserves the
# rest of the file structure.
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

yq eval \
  ".macos.defaults += [{\"domain\": \"$domain\", \"key\": \"$key\", \"type\": \"$schema_type\", \"value\": $yaml_value$([[ -n "$host" ]] && printf ', "host": "%s"' "$host")}]" \
  "$DATA_FILE" >"$tmp"

mv "$tmp" "$DATA_FILE"
trap - EXIT

printf 'captured: %s %s = %s (type=%s%s)\n' "$domain" "$key" "$raw_value" "$schema_type" \
  "$([[ -n "$host" ]] && printf ' host=%s' "$host")"
```

- [ ] **Step 2: Lint with shellcheck and shfmt**

Run:

```bash
nix develop .#run --command shellcheck dot_local/bin/executable_macos-defaults-capture.sh
nix develop .#run --command shfmt -i 2 -ci -s --diff dot_local/bin/executable_macos-defaults-capture.sh
```

Expected: both exit 0. If shellcheck flags something, fix and re-run before proceeding.

- [ ] **Step 3: Apply the script onto disk**

```bash
chezmoi apply ~/.local/bin/macos-defaults-capture.sh
```

- [ ] **Step 4: Verify behavior with a live setting**

Run:

```bash
~/.local/bin/macos-defaults-capture.sh com.apple.dock tilesize
echo "exit=$?"
nix develop .#run --command yq eval '.macos.defaults' .chezmoidata/macos_defaults.yaml
```

Expected: prints `captured: com.apple.dock tilesize = <number> (type=int)`, `exit=0`. The yq output should now show one record under `macos.defaults`.

- [ ] **Step 5: Verify idempotency (no-op on re-run)**

Run:

```bash
~/.local/bin/macos-defaults-capture.sh com.apple.dock tilesize
echo "exit=$?"
```

Expected: prints `already tracked: com.apple.dock tilesize = <same number>`, `exit=0`. The YAML is not modified.

- [ ] **Step 6: Verify "key not set" error path**

Run:

```bash
~/.local/bin/macos-defaults-capture.sh com.apple.dock zzzz-bogus-key
echo "exit=$?"
```

Expected: prints `error: com.apple.dock zzzz-bogus-key is not currently set on this Mac`, `exit=1`.

- [ ] **Step 7: Roll back the test capture**

The `tilesize` record was a smoke test, not part of the baseline. Revert the YAML to its committed
state:

```bash
git checkout .chezmoidata/macos_defaults.yaml
nix develop .#run --command yq eval '.macos.defaults' .chezmoidata/macos_defaults.yaml
```

Expected: the `yq` output is `[]` (empty array), matching the post-Task-1 state.

- [ ] **Step 8: Add to `.chezmoiignore` Linux gate**

```
{{- if eq .chezmoi.os "linux" -}}
.config/yabai
Library
.local/bin/macos-defaults-drift.sh
.local/bin/macos-defaults-apply.sh
.local/bin/macos-defaults-capture.sh
{{- end -}}
```

- [ ] **Step 9: Commit**

```bash
SKIP_AI_COMMIT=1 git add dot_local/bin/executable_macos-defaults-capture.sh .chezmoiignore
SKIP_AI_COMMIT=1 git commit -m "feat(macos-defaults): add capture helper (just defaults-capture)

Reads live value+type via defaults read-type/read, normalizes to the
v1 schema's bool/int/float/string tag, appends to macos_defaults.yaml.
Idempotent on already-tracked-and-matching entries; refuses to silently
overwrite divergent entries (exit 2, forces explicit resolve). Linux-
gated."
```

---

## Task 6: Tier 2 runner (chezmoiscript)

**Files:**

- Create: `.chezmoiscripts/run_onchange_after_40-macos-system-setup.sh.tmpl`

- [ ] **Step 1: Write the runner template**

```sh
{{ if eq .chezmoi.os "darwin" -}}
#!/usr/bin/env bash
# Tier 2 — macOS sudo system-setup runner.
# chezmoi hash-gates on the rendered template body; this script re-runs only
# when .chezmoidata/macos_system_setup.yaml changes.

set -euo pipefail

# Early-return if no commands are configured (avoids spurious sudo prompt).
{{ if eq (len .macos.system_setup) 0 -}}
exit 0
{{ end -}}

# Pre-flight: refresh sudo timestamp upfront. One password prompt at start;
# none during the loop.
sudo -v

{{ range .macos.system_setup -}}
echo "→ {{ .description }}"
{{ if .sudo }}sudo {{ end }}{{ .command }}
{{ end -}}
{{- end }}
```

- [ ] **Step 2: Render and shellcheck**

Run:

```bash
chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_40-macos-system-setup.sh.tmpl
```

Expected on darwin: a bash script with `set -euo pipefail` and an `exit 0` near the top (because the YAML array is empty).

```bash
chezmoi execute-template --no-tty < .chezmoiscripts/run_onchange_after_40-macos-system-setup.sh.tmpl | shellcheck -
```

Expected: exit 0.

- [ ] **Step 3: Apply the chezmoiscript**

Run: `chezmoi apply .chezmoiscripts/run_onchange_after_40-macos-system-setup.sh.tmpl`
Expected: chezmoi runs the script; the early-return fires (no sudo prompt). Exit 0.

- [ ] **Step 4: Commit**

```bash
SKIP_AI_COMMIT=1 git add .chezmoiscripts/run_onchange_after_40-macos-system-setup.sh.tmpl
SKIP_AI_COMMIT=1 git commit -m "feat(chezmoiscripts): add tier 2 macos system-setup runner

Hash-gated darwin-only script that iterates macos.system_setup and
runs each command (with sudo prefix when sudo: true). Single sudo -v
upfront so the user gets one password prompt regardless of how many
commands follow. Early-returns when the array is empty to avoid a
spurious sudo prompt during initial setup."
```

---

## Task 7: Justfile recipes

**Files:**

- Modify: `justfile`

- [ ] **Step 1: Read existing justfile to see current style**

Run: `head -40 justfile` and observe the recipe naming pattern (single-letter aliases, kebab-case full names, `nix develop` wrappers, etc.). Match the style in your additions.

- [ ] **Step 2: Append the six recipes**

Find a sensible insertion point near the existing `chezmoi`-flavored recipes (`a` for apply, `d` for diff). Append:

```just
# macOS Defaults: drift, apply, capture
alias D := defaults-drift

defaults-drift:
  ~/.local/bin/macos-defaults-drift.sh

defaults-apply:
  ~/.local/bin/macos-defaults-apply.sh

defaults-capture domain key host="":
  #!/usr/bin/env bash
  set -euo pipefail
  if [[ -n "{{host}}" ]]; then
    ~/.local/bin/macos-defaults-capture.sh "{{domain}}" "{{key}}" "--host=current"
  else
    ~/.local/bin/macos-defaults-capture.sh "{{domain}}" "{{key}}"
  fi

# macOS Defaults discovery — read-only wrappers around `defaults`.
defaults-list:
  defaults domains | tr ',' '\n' | sort

defaults-show domain:
  defaults read "{{domain}}"

defaults-dump:
  defaults read | less
```

- [ ] **Step 3: Smoke-test each recipe**

```bash
just defaults-list | head -5      # expect 5 domain names
just defaults-show com.apple.dock | head -5   # expect first 5 lines of dock plist
just D                            # expect exit 0 (no drift, empty YAML)
just defaults-apply               # expect Dock/Finder redraw, exit 0
echo "exit=$?"
```

Expected: each recipe runs without complaint. `just defaults-dump` is interactive (less), so don't run it in CI; just confirm it opens and quits cleanly with `q`.

- [ ] **Step 4: Lint pass**

Run: `just l`
Expected: SUMMARY all green.

- [ ] **Step 5: Commit**

```bash
SKIP_AI_COMMIT=1 git add justfile
SKIP_AI_COMMIT=1 git commit -m "feat(justfile): add macos defaults recipes

Six recipes for the new defaults workflow: D (alias defaults-drift),
defaults-apply, defaults-capture <domain> <key> [host], plus three
discovery wrappers around macOS's defaults command — defaults-list
(domains), defaults-show <domain> (one domain's keys), defaults-dump
(full corpus, paged through less)."
```

---

## Task 8: CLAUDE.md documentation

**Files:**

- Modify: `CLAUDE.md`

- [ ] **Step 1: Open `CLAUDE.md` and locate the existing "Homebrew install workflow" subsection**

Search for "Homebrew install workflow" in CLAUDE.md. Add a new "macOS Defaults" subsection after it.

- [ ] **Step 2: Insert the new subsection**

Add (immediately after the existing Homebrew block):

````markdown
### macOS Defaults Management

Two `.chezmoidata/` files declaratively track macOS settings; two `.chezmoiscripts/` runners apply them
at `chezmoi apply` time on darwin (no-op on linux):

- `.chezmoidata/macos_defaults.yaml` + `run_onchange_after_30-macos-defaults.sh.tmpl` — per-user
  `defaults write` records, plus a `killall` list (Dock/Finder/SystemUIServer/cfprefsd; cfprefsd kill
  is required for plist changes to take effect immediately).
- `.chezmoidata/macos_system_setup.yaml` + `run_onchange_after_40-macos-system-setup.sh.tmpl` — sudo
  system commands (one `sudo -v` upfront, then loop). Early-returns when the array is empty.

**Daily workflow:**

| Operation | Command |
|---|---|
| Discover available domains | `just defaults-list` |
| Browse one domain's keys | `just defaults-show <domain>` |
| Capture a setting into YAML | `just defaults-capture <domain> <key> [current]` |
| Check for drift | `just D` |
| Force reapply (revert disk to YAML) | `just defaults-apply` |

The capture helper is the canonical way to add a tracked setting: toggle it in System Settings, run
`just defaults-capture`, then `chezmoi apply` to commit. The helper refuses to silently overwrite a
tracked entry whose live value diverges from YAML (exits 2) — resolve via `just defaults-apply` to
revert, or hand-edit YAML to capture the new intent.

**Aerospace required defaults:** `com.apple.dock mru-spaces=false` is the single most common Aerospace
breakage. Several `com.apple.WindowManager` keys (Stage Manager, Sequoia tiling) are recommended off.
See the design spec for the full list.
````

- [ ] **Step 3: Lint pass**

Run: `just m`
Expected: mdformat ✅, no diff. (CLAUDE.md is in the mdformat target list.)

- [ ] **Step 4: Commit**

```bash
SKIP_AI_COMMIT=1 git add CLAUDE.md
SKIP_AI_COMMIT=1 git commit -m "docs(claude): add macos defaults workflow section

Documents the two-file/two-runner data model, the six just recipes,
and the daily workflow (discover -> capture -> apply). Calls out the
Aerospace-required defaults (mru-spaces, WindowManager) that future
contributors must not delete."
```

---

## Task 9: Fresh-machine runbook

**Files:**

- Create: `docs/runbooks/macos-fresh-machine-quickstart.md`

- [ ] **Step 1: Confirm the directory exists; create if not**

Run: `ls docs/runbooks 2>/dev/null || mkdir -p docs/runbooks`

- [ ] **Step 2: Write the runbook**

```markdown
# macOS Fresh-Machine Quickstart

A checklist for everything that `chezmoi apply` can't (or shouldn't) automate. Read top-to-bottom on a
brand-new Mac before running `chezmoi apply` for the first time.

## Before first `chezmoi apply`

1. **Install Xcode Command Line Tools** — `xcode-select --install`. Required for git and brew.
2. **Sign into Apple ID** — System Settings → Apple ID. Required for iCloud Drive (KeePassXC db sync)
   and `mas` App Store installs.
3. **Retrieve the KeePassXC database** — from offline backup or iCloud Drive. Place at the path
   referenced in `.chezmoi.toml.tmpl`.
4. **Install chezmoi** — `brew install chezmoi` (or pre-install via homebrew bootstrap).
5. **Initialize chezmoi** — `chezmoi init <repo-url>`. This will require the KeePassXC db to be
   reachable for any KeePassXC-templated files.

## During `chezmoi apply`

The Tier 2 runner (`run_onchange_after_40-macos-system-setup.sh.tmpl`) will prompt once for sudo if
the system_setup YAML is non-empty. Enter your password.

## After first `chezmoi apply`

These steps require GUI interaction or interactive auth — there's no `defaults` equivalent.

### Aerospace compatibility

- **System Settings → Desktop & Dock → Mission Control → Displays have separate Spaces** — set per
  machine: ON for tri-monitor, OFF for single-monitor.
- **System Settings → Desktop & Dock → Click wallpaper to reveal desktop** — set to "Only in Stage
  Manager" (the `defaults` key changes name across Sequoia point releases, so manual is more durable).

### TCC privacy grants

System Settings → Privacy & Security → grant the following:

- **Full Disk Access** — Ghostty, Karabiner-Elements, Hammerspoon.
- **Screen Recording** — any tool you use that needs it (Loom, Zoom, OBS).
- **Accessibility** — Karabiner-Elements, Rectangle, any keyboard-remap tools.
- **Input Monitoring** — Karabiner-Elements.

Each grant requires opening the Privacy sheet and dragging the app into the listed sheet — there's no
CLI surface.

### Hardware pairing

- **Bluetooth** — pair AirPods, mice, keyboards via System Settings → Bluetooth.
- **Wi-Fi profiles / 802.1X** — connect to your network; the password / cert flow is interactive.
- **Touch ID** — enroll fingerprints via System Settings → Touch ID & Password.

### App authentication

- **Browser sign-ins** — 1Password browser extension, GitHub, work accounts.
- **App Store apps requiring purchase confirmation** — after `mas install <id>`, confirm purchase in
  the modal that appears.

### Login Items

System Settings → General → Login Items → add anything not covered by an installed-app's preferences
(launchd is generally the better path; this is a fallback).

### Out-of-scope items (by design)

The following are intentionally NOT tracked in the YAML:

- **Karabiner-Elements rules** — managed by Karabiner's own JSON in `dot_config/private_karabiner/`.
- **SIP-protected toggles** (`nvram`, `csrutil`) — recovery-mode only.
- **Hot Corners / Mission Control assignments** — `defaults` keys vary by macOS major version;
  punt to v2.
- **Per-app keyboard shortcuts** (`NSGlobalDomain NSUserKeyEquivalents`) — arrays-of-dicts not
  supported by v1 schema; punt to v2.

## Sanity checks after setup is complete

```bash
# Aerospace required default
defaults read com.apple.dock mru-spaces  # expect 0

# All tracked defaults match YAML
just D  # expect exit 0, no output

# Aerospace itself running
pgrep -x AeroSpace  # expect a PID
```
```

- [ ] **Step 3: Lint pass**

Run: `just m`
Expected: mdformat ✅. (`docs/runbooks/` is in the mdformat target — same `.mdformat.toml` rules apply.)

- [ ] **Step 4: Commit**

```bash
SKIP_AI_COMMIT=1 git add docs/runbooks/macos-fresh-machine-quickstart.md
SKIP_AI_COMMIT=1 git commit -m "docs(runbooks): add macos fresh-machine quickstart

Checklist for everything chezmoi apply can't automate: pre-apply Apple
ID + KeePassXC db retrieval, post-apply TCC grants and Bluetooth/Wi-Fi
pairing, plus the two Aerospace-related System Settings steps that
have no reliable defaults equivalent across major macOS versions.
Includes a sanity-check section with concrete commands."
```

---

## Task 10: Aerospace baseline + first real apply

**Files:**

- Modify: `.chezmoidata/macos_defaults.yaml`

This task uses the capture helper to populate the spec's required + recommended Aerospace entries.
Before running each capture, the live value on disk must match what the spec says it should be —
otherwise the helper captures the wrong value. So we set each value first, then capture.

- [ ] **Step 1: Set the seven Aerospace-relevant defaults to their target values**

```bash
defaults write com.apple.dock mru-spaces -bool false
defaults write com.apple.dock expose-group-apps -bool false
defaults write com.apple.WindowManager GloballyEnabled -bool false
defaults write com.apple.WindowManager EnableStandardClickToShowDesktop -bool false
defaults write com.apple.WindowManager EnableTilingByEdgeDrag -bool false
defaults write com.apple.WindowManager EnableTilingOptionAccelerator -bool false
defaults write com.apple.WindowManager EnableTopTilingByEdgeDrag -bool false
killall cfprefsd
```

Verify each:

```bash
defaults read com.apple.dock mru-spaces                            # expect 0
defaults read com.apple.dock expose-group-apps                     # expect 0
defaults read com.apple.WindowManager GloballyEnabled              # expect 0
defaults read com.apple.WindowManager EnableStandardClickToShowDesktop   # expect 0
defaults read com.apple.WindowManager EnableTilingByEdgeDrag       # expect 0
defaults read com.apple.WindowManager EnableTilingOptionAccelerator   # expect 0
defaults read com.apple.WindowManager EnableTopTilingByEdgeDrag    # expect 0
```

- [ ] **Step 2: Capture each into YAML**

```bash
just defaults-capture com.apple.dock mru-spaces
just defaults-capture com.apple.dock expose-group-apps
just defaults-capture com.apple.WindowManager GloballyEnabled
just defaults-capture com.apple.WindowManager EnableStandardClickToShowDesktop
just defaults-capture com.apple.WindowManager EnableTilingByEdgeDrag
just defaults-capture com.apple.WindowManager EnableTilingOptionAccelerator
just defaults-capture com.apple.WindowManager EnableTopTilingByEdgeDrag
```

Expected: each invocation prints `captured: <domain> <key> = false (type=bool)` and exits 0.

- [ ] **Step 3: Verify the YAML now has 7 records**

Run:

```bash
nix develop .#run --command yq eval '.macos.defaults | length' .chezmoidata/macos_defaults.yaml
```

Expected: `7`.

- [ ] **Step 4: Run `just D` — should be clean (YAML matches disk)**

Run: `just D`
Expected: exit 0, no output.

- [ ] **Step 5: Apply via chezmoi (should be a no-op since the live values already match)**

Run: `chezmoi apply --exclude=templates`
Expected: chezmoi notices the rendered Tier 1 script's hash changed (because YAML changed), re-runs
it. The runner emits 7 `defaults write` calls (all idempotent — values already match) and 4 killall
calls. Dock + Finder briefly redraw. Exit 0.

- [ ] **Step 6: Verify the post-apply state**

Run:

```bash
just D                                          # expect exit 0, no drift
defaults read com.apple.dock mru-spaces         # expect 0 (Aerospace required)
```

- [ ] **Step 7: Commit**

```bash
SKIP_AI_COMMIT=1 git add .chezmoidata/macos_defaults.yaml
SKIP_AI_COMMIT=1 git commit -m "feat(macos-defaults): seed aerospace baseline

Seven required + recommended Aerospace-compatibility defaults: dock
mru-spaces (the must-have), dock expose-group-apps, and five
WindowManager keys covering Stage Manager and Sequoia's built-in
window tiling. All set to false. With these in YAML, just D is the
tripwire that catches any future regression (e.g., a macOS update
flipping mru-spaces back to true)."
```

---

## Task 11: Ongoing seeding — incremental capture

This task is open-ended and user-driven. The implementation work is done after Task 10; what
follows is the ongoing flow that the user owns.

For each setting the user wants tracked beyond the Aerospace baseline:

1. **Discover** — `just defaults-list` to see all preference domains, then
   `just defaults-show <domain>` to browse one domain's keys.
2. **Capture** — `just defaults-capture <domain> <key>` (or with `--host=current` for ByHost
   settings). Helper writes the record into `.chezmoidata/macos_defaults.yaml`.
3. **Apply** — `chezmoi apply`. Tier 1 runner re-fires (hash gate), the new defaults take effect,
   killalls fire as needed.
4. **Commit** — one commit per logical group of captures (`feat(macos-defaults): track dock+finder
   visibility settings`, etc.).

For sudo system commands (`pmset`, `systemsetup`, etc.):

1. **Hand-edit** `.chezmoidata/macos_system_setup.yaml`. The capture helper does not cover Tier 2
   (sudo commands aren't `defaults`-shaped).
2. **Apply** — `chezmoi apply`. Tier 2 runner fires; one sudo prompt, then the loop.
3. **Commit**.

Stop when you've captured everything you care about. The system is fully usable at any incremental
checkpoint — partial coverage is fine.

---

## Verification gates (run after every task)

- `just l` — lint passes.
- `git status` — clean working tree before next task.
- `just D` — drift checker exits 0 (after Task 3 onward, with empty YAML).

## Final acceptance check (run after Task 10)

```bash
just l                                         # all green
just D                                         # exit 0, no drift
chezmoi apply --exclude=templates              # idempotent, no errors
defaults read com.apple.dock mru-spaces        # 0
nix develop .#run --command yq eval '.macos.defaults | length' .chezmoidata/macos_defaults.yaml   # 7
git log --oneline -10                          # 10+ new commits
```

If all of the above succeed, the implementation is complete. The "ongoing seeding" task (Task 11) is a
permanent workflow, not a finite step.
