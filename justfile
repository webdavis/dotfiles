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

# One camp at a time, for focused iteration. scripts/run-camp.sh runs the
# camp's executable *.sh tests (each with fd 3 closed so a test that reads stdin
# cannot swallow the discovery list) then the camp's own *.bats suites; its
# discovery is checked so a traversal/sort error fails the gate instead of
# green-gating a short list.
test-integration: test-guard
  ./scripts/run-camp.sh test/integration

test-e2e: test-guard
  ./scripts/run-camp.sh test/e2e

# Placement / mode / symlink guard (scripts/test-guard.sh): every *.sh and
# *.bats below test/ must sit DIRECTLY in a recognized camp (test/unit,
# test/integration, test/e2e); camp *.sh must be executable; no symlinks are
# allowed anywhere below test/ (a physical find skips them, so they would evade
# every gate). test/fixtures/** is exempt.
test-guard:
  ./scripts/test-guard.sh

# All camps: what pre-push and CI run. The per-camp recipes above already run
# each camp's bats; this aggregate ALSO runs every bats suite (test/**/*.bats)
# as the backstop. Bats runs inside the Nix devshell when the host lacks it (the
# flake provides bats + GNU parallel). Discovery is checked and empty-safe.
test: test-unit test-integration test-e2e
  #!/usr/bin/env bash
  set -euo pipefail
  bats_list="$(mktemp)"
  trap 'rm -f "$bats_list"' EXIT
  if ! find test -type f -name '*.bats' -print0 | sort -z >"$bats_list"; then
    printf 'FAIL: bats discovery failed; refusing to skip a partial list\n' >&2
    exit 1
  fi
  bats_files=()
  while IFS= read -r -d '' b; do
    bats_files+=("$b")
  done <"$bats_list"
  if ((${#bats_files[@]} > 0)); then
    printf "== bats (all camps) ==\n"
    if command -v bats >/dev/null 2>&1; then
      bats --jobs 4 "${bats_files[@]}"
    else
      nix develop .#run --command bats --jobs 4 "${bats_files[@]}"
    fi
  fi

# Run the weekly Homebrew upgrade by hand (formulae + casks + Mac App Store +
# cleanup). Same job the Monday-noon com.webdavis.homebrew-weekly-upgrade
# LaunchAgent runs; use it for the first upgrade or any ad-hoc one. Runs the
# DEPLOYED helper (what launchd runs), not the repo source copy, and uses the
# host brew outside the Nix shell.
brew-upgrade:
  ~/.local/bin/homebrew-weekly-upgrade.sh

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
