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
# runs treefmt on a sandboxed copy of the tree, reports drift, never mutates
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

# Tests live in suites by DESIGN: test/unit (single component, stub-driven, no
# flows, no sleeps; FAST is the admission rule), test/integration
# (multi-component with stubbed boundaries), test/e2e (whole-script flows and
# timing-bound tests). The pre-commit hook runs `just test-unit` only; the
# pre-push hook and CI run `just test` (all suites). A test file sitting
# directly under test/ fails the guard in both runners so strays cannot hide.

# Unit suite only: the commit gate. --shuffle randomizes order to flush hidden
# ordering deps (seed printed for replay); --warn-slow-ms flags slow tests in a
# warn-only summary. The other suites run the same runner plain.
test-unit: validate-tests
  ./test/run-test-suite.sh --shuffle --warn-slow-ms 200 test/unit

# One suite at a time, for focused iteration. test/run-test-suite.sh runs the
# suite's executable *.sh tests (each with fd 3 closed so a test that reads stdin
# cannot swallow the discovery list) then the suite's own *.bats; its discovery
# is checked so a traversal/sort error fails the gate instead of green-gating a
# short list.
test-integration: validate-tests
  ./test/run-test-suite.sh test/integration

test-e2e: validate-tests
  ./test/run-test-suite.sh test/e2e

# The suite that tests the checker and the runner themselves.
test-system: validate-tests
  ./test/run-test-suite.sh test/test-system

# Placement / mode / symlink guard (test/validate-tests.sh): every *.sh and
# *.bats below test/ must sit DIRECTLY in a recognized suite (test/unit,
# test/integration, test/e2e, test/test-system); suite *.sh must be executable;
# no symlinks are allowed anywhere below test/ (a physical find skips them, so
# they would evade every gate). A suite's helpers/ and test/fixtures/** are
# exempt.
validate-tests:
  ./test/validate-tests.sh

# All suites: what pre-push and CI run. Each suite recipe runs its own *.sh
# and *.bats via the runner, and the checker's placement rules reject any bats
# outside a suite, so no separate bats backstop is needed here.
test: test-unit test-integration test-e2e test-system

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

# `defaults-capture <domain> <key> [current]`, capture a live setting into YAML.
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

# macOS Defaults discovery, read-only wrappers around `defaults`.
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
