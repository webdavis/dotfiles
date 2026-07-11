set shell := ["bash", "-c", "source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null; eval \"$@\"", "--"]

default:
  @just --choose

alias l := lint
alias L := lint-check
alias s := lint-shell
alias S := format-shell
alias m := format-markdown
alias n := format-nix
alias t := lint-toml
alias j := lint-json
alias y := lint-yaml
alias T := test
alias d := diff
alias a := apply-no-auth
alias c := check
alias D := defaults-drift

# Format everything in place. treefmt (configured in treefmt.nix) is the
# single lint/format orchestrator; the per-tool recipes below just filter it.
lint:
  nix develop .#run --command treefmt

# Check-only drift gate: builds the flake's treefmt check derivation, which
# runs treefmt on a sandboxed copy of the tree — reports drift, never mutates
# the working tree or index (treefmt itself has no dry-run mode, so the
# sandbox copy is what makes this check-only). Same gate CI runs.
lint-check:
  nix flake check

lint-shell:
  nix develop .#run --command treefmt --formatters shellcheck,shellcheck-rendered-template

format-shell:
  nix develop .#run --command treefmt --formatters shfmt

format-markdown:
  nix develop .#run --command treefmt --formatters mdformat

format-nix:
  nix develop .#run --command treefmt --formatters nixfmt

lint-toml:
  nix develop .#run --command treefmt --formatters taplo

lint-json:
  nix develop .#run --command treefmt --formatters jq-validate,osquery-config-render

lint-yaml:
  nix develop .#run --command treefmt --formatters yq-validate

# GitHub Actions hygiene: actionlint (syntax/semantics, also part of `just l`)
# plus zizmor (static security analysis; --offline skips the audits that need
# the GitHub API, so the result is deterministic). CI runs this too.
lint-actions:
  nix develop .#run --command treefmt --formatters actionlint
  nix develop .#run --command zizmor --offline .github/workflows

diff:
  nix develop .#run --command chezmoi diff --exclude=templates

apply-no-auth:
  nix develop .#run --command chezmoi apply --exclude=templates --force

check:
  nix develop .#run --command nix flake check --all-systems

# Test pyramid (operator, 2026-07-11; pattern: essential-feed-case-study).
# Tests live in camps by DESIGN: test/unit (single component, stub-driven, no
# flows, no sleeps; FAST is the admission rule), test/integration
# (multi-component with stubbed boundaries), test/e2e (whole-script flows and
# timing-bound tests). The pre-commit hook runs `just test-unit` only; the
# pre-push hook and CI run `just test` (all camps). A test file sitting
# directly under test/ fails the guard in both runners so strays cannot hide.

# Unit camp only: the commit gate. Seeded shuffle + per-test timing with a
# warn-only performance summary live in scripts/run-unit-tests.sh.
test-unit: test-guard
  ./scripts/run-unit-tests.sh

# One camp at a time, for focused iteration. Tests read on fd 3 (repo bash
# rule): a test body that reads stdin must not be able to swallow the file
# list and silently truncate the gate.
test-integration: test-guard
  #!/usr/bin/env bash
  set -euo pipefail
  status=0
  while IFS= read -r -u3 -d '' t; do
    printf "== %s ==\n" "$t"
    "$t" || status=1
  done 3< <(find test/integration -maxdepth 1 -type f -name '*.sh' -perm -u+x -print0 2>/dev/null | sort -z)
  exit "$status"

test-e2e: test-guard
  #!/usr/bin/env bash
  set -euo pipefail
  status=0
  while IFS= read -r -u3 -d '' t; do
    printf "== %s ==\n" "$t"
    "$t" || status=1
  done 3< <(find test/e2e -maxdepth 1 -type f -name '*.sh' -perm -u+x -print0 2>/dev/null | sort -z)
  exit "$status"

# Placement + mode guard: every test *.sh below test/ must sit DIRECTLY in a
# recognized camp (test/unit, test/integration, test/e2e) and be executable;
# a nested dir, an unknown camp, a stray under test/, or a chmod-less test
# would otherwise be silently invisible to every selector. test/fixtures/**
# is exempt (fixture data and sourced libs, never run directly).
test-guard:
  #!/usr/bin/env bash
  set -euo pipefail
  bad=""
  while IFS= read -r -u3 f; do
    case "$f" in
      test/fixtures/*) continue ;;
      test/unit/*/*|test/integration/*/*|test/e2e/*/*)
        bad+="$f (nested; camps are flat)"$'\n' ;;
      test/unit/*.sh|test/integration/*.sh|test/e2e/*.sh)
        [[ -x $f ]] || bad+="$f (not executable; invisible to the gate)"$'\n' ;;
      *)
        bad+="$f (outside the unit/integration/e2e camps)"$'\n' ;;
    esac
  done 3< <(find test -type f -name '*.sh' 2>/dev/null | sort)
  if [[ -n $bad ]]; then
    printf 'FAIL: misplaced or misconfigured test scripts:\n%s' "$bad" >&2
    printf 'Fix placement/mode (and REPO_ROOT depth is ../.. inside a camp).\n' >&2
    exit 1
  fi

# All camps: what pre-push and CI run. Bats suites (test/**/*.bats) run inside
# the Nix devshell; the flake provides bats, so no host install is needed and
# the suite runs the same on a fresh machine. Empty-safe throughout.
test: test-unit test-integration test-e2e
  #!/usr/bin/env bash
  set -euo pipefail
  if find test -type f -name '*.bats' -print -quit 2>/dev/null | grep -q .; then
    printf "== %s ==\n" "bats"
    nix develop .#run --command bats --jobs 4 --recursive test/
  fi

# macOS Defaults: drift, apply, capture

defaults-drift:
  ~/.local/bin/macos-defaults-drift.sh

defaults-apply:
  ~/.local/bin/macos-defaults-apply.sh

# `defaults-capture <domain> <key> [current]` — capture a live setting into YAML.
# Pass the literal `current` as the third arg to use ByHost storage
# (`defaults -currentHost`). Any non-empty third arg triggers ByHost mode;
# the v1 schema does not support arbitrary hostnames.
defaults-capture domain key current="":
  #!/usr/bin/env bash
  set -euo pipefail
  if [[ -n "{{current}}" ]]; then
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

# (agent-skill vendoring removed: herdr/moshi now live in ~/.agents/skills, symlinked per-harness)

# Refresh portable agent skills in the store (~/.agents/skills) + re-symlink each harness.
# Also runs weekly via launchd (com.webdavis.update-skills). Pass --dry-run to preview,
# or --install-only to only install absent manifest skills (fresh-machine bootstrap).
update-skills *args:
  ~/.local/bin/update-skills.sh {{args}}
