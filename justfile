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

# Check-only drift gate, and the whole of what the pre-push hook runs: builds
# the flake's treefmt check derivation, which runs treefmt on a sandboxed copy
# of the tree, reports drift, never mutates the working tree or index (treefmt
# itself has no dry-run mode, so the sandbox copy is what makes this
# check-only). NOT CI's command: CI's first gate is a bare
# `nix flake check --all-systems`, which the `check` recipe below carries
# verbatim. Both build the same drift derivation for this host (measured: the
# same .drv path from either recipe on one tree), but without the flag nix
# evaluates only this host's flake outputs and says so: "the check omitted
# these incompatible systems: x86_64-linux".
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

# GitHub Actions hygiene: actionlint (syntax/semantics) plus zizmor (static
# security analysis). Split into two recipes because CI runs only the zizmor
# half as a gate of its own, and `ship` reuses that recipe rather than repeating
# its command line.
lint-actions: lint-actions-syntax lint-actions-security

# actionlint through treefmt, so it uses the same config `just l` uses.
lint-actions-syntax:
  nix develop .#run --command treefmt --formatters actionlint

# CI's third gate, verbatim. --offline skips the audits that need the GitHub
# API, so the result is deterministic.
lint-actions-security:
  nix develop .#run --command zizmor --offline .github/workflows

diff:
  nix develop .#run --command chezmoi diff --exclude=templates

apply-no-auth:
  nix develop .#run --command chezmoi apply --exclude=templates --force

# CI's first gate, verbatim. --all-systems EVALUATES every system the flake
# declares (x86_64-linux and aarch64-darwin), so an output that only breaks on
# the other platform is caught here; it still BUILDS only the checks this host
# can build, so the treefmt drift derivation actually runs for aarch64-darwin
# alone. CI is macOS too, so its coverage is the same. Host nix, no devshell
# wrapper, because CI runs it that way: a broken flake has to fail HERE rather
# than while building the shell that would have run the check.
#
# What it reads is the GIT TREE, not the directory. Nix copies tracked files at
# their working-tree content and skips untracked ones, so a new file is
# invisible to this gate until it is at least `git add`ed. Measured 2026-07-30
# on one badly formatted file, byte-identical on disk both times: under
# `git add -N` this gate failed on it (SC2050), untracked it reported "all
# checks passed". The other two gates walk the filesystem and do see it.
check:
  nix flake check --all-systems

# Tests live in suites by DESIGN: test/unit (single component, stub-driven, no
# flows, no sleeps; FAST is the admission rule), test/integration
# (multi-component with stubbed boundaries), test/e2e (whole-script flows and
# timing-bound tests). The pre-commit hook runs `just test-unit` only; the
# pre-push hook runs no suite at all (lint drift only); CI and `just ship` run
# `just test` (all suites). A test file sitting directly under test/ fails the
# guard in both runners so strays cannot hide.

# Unit suite only: the commit gate. --shuffle randomizes order to flush hidden
# ordering deps (seed printed for replay); --warn-slow-ms flags slow tests in a
# warn-only summary. The other suites run the same runner plain.
test-unit: validate-tests
  ./test/run-test-suite.sh --shuffle --warn-slow-ms 200 test/unit

# One suite at a time, for focused iteration. test/run-test-suite.sh runs the
# suite's executable *.sh tests, then its *.bats. The runner reads its own test
# list on fd 3 and closes fd 3 for each test it launches: a test that inherited
# the open fd could accidentally read the rest of the list and silently skip
# tests. Discovery is checked, so a failed file search fails the run instead of
# green-lighting a partial list.
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

# All suites: what CI runs. Each suite recipe runs its own *.sh and *.bats via
# the runner, and the checker's placement rules reject any bats outside a suite,
# so no separate bats backstop is needed here.
test: test-unit test-integration test-e2e test-system

# CI's second gate, verbatim: every suite inside the flake's `run` shell, so the
# tools are the pinned ones CI has rather than whatever the host happens to
# carry. bats is one of those tools and is not installed on this host, so a bare
# `just test` makes test/run-test-suite.sh re-enter this same shell once per
# suite that has a .bats file (integration, e2e and test-system today). Entering
# it once up front is what CI does; it also saves two shell entries, but those
# measured about 0.4s each warm, so do this for the fidelity, not the speed.
test-devshell:
  nix develop .#run --command just test

# The pre-PR sweep: the three gates .github/workflows/lint.yml runs, in CI's
# order, each through the recipe that holds that gate's command line. Nothing
# about that arrangement stops the two files being edited apart;
# test/unit/ship-ci-gate-parity.sh is what stops it landing, by re-reading both
# and failing when they disagree.
#
# Two things a green run does NOT promise:
#   - It reads the working tree; CI reads the pushed commit. An edit you never
#     staged can make ship green and CI red; that is the PR #116 failure
#     CLAUDE.md records under Git Hooks.
#   - A file you have not `git add`ed is invisible to the first gate (see the
#     note on `check`). Commit it and CI checks it, having never been checked
#     here. The other two gates read the filesystem and do see it.
# Pre-push deliberately runs none of this. The measurements behind that call,
# and the rest of the history, are in CLAUDE.md under Git Hooks.
ship: check test-devshell lint-actions-security

# Run the weekly Homebrew upgrade by hand (formulae + casks + Mac App Store +
# cleanup). Same job the Monday-noon com.webdavis.homebrew-weekly-upgrade
# LaunchAgent runs; use it for the first upgrade or any ad-hoc one. Runs the
# DEPLOYED helper (what launchd runs), not the repo source copy, and uses the
# host brew outside the Nix shell.
brew-upgrade:
  ~/.local/bin/homebrew-weekly-upgrade.sh

# Run only the brew shellenv cache drift test (a subset of `just test`).
test-brew-cache:
  ./test/e2e/brew-shellenv-cache-drift.sh

# Regenerate the brew shellenv cache (~/.cache/brew-shellenv.sh) from the current
# `brew shellenv`, now, instead of waiting for the next interactive shell to
# self-heal it. Use it after a Homebrew update if `just test` reports cache
# drift, and to seed the cache on a host nobody logs into interactively (the
# ~/.bashrc self-heal only runs in interactive shells). Runs the DEPLOYED writer,
# the same artifact ~/.bashrc's self-heal runs, so the atomic write (mktemp in
# the cache dir, brew success-gated, then rename) has exactly one implementation
# and this recipe cannot drift from it. The writer reaches ~/.local/bin only via
# `chezmoi apply`, so say that plainly instead of letting the shell report a bare
# "No such file or directory" from a path the reader has no reason to recognize.
brew-cache-refresh:
  #!/usr/bin/env bash
  set -euo pipefail
  deployed_writer="$HOME/.local/bin/brew-shellenv-cache-refresh.sh"
  if [[ ! -x $deployed_writer ]]; then
    printf 'brew-cache-refresh: %s is not deployed.\n' "$deployed_writer" >&2
    printf '  Run `chezmoi apply` (it is a plain file, not a template), then retry.\n' >&2
    exit 1
  fi
  "$deployed_writer"

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
