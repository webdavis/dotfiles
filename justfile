set shell := ["bash", "-cu"]

default:
  @just --choose

alias l := lint
alias L := lint-check
alias s := lint-shell
alias S := format-shell
alias m := format-markdown
alias t := lint-toml
alias j := lint-json
alias y := lint-yaml
alias T := test
alias d := diff
alias a := apply-no-auth
alias c := lint-check
alias D := defaults-drift

# Format everything in place. The standalone treefmt binary (brew formula,
# configured in treefmt.toml) is the single lint/format orchestrator; the
# per-tool recipes below just filter it. Tools come from Homebrew and the
# uv-managed mdformat, not nix (operator ruling 2026-08-05: this repo no
# longer depends on nix; nix stays on the machine for other uses).
lint:
  treefmt

# Drift gate, and the whole of what the pre-push hook runs. Standalone treefmt
# has no dry-run mode and no sandbox here, so unlike the old nix check
# derivation this WRITES the fixes it finds while failing the run: a red gate
# leaves the tree formatted, re-stage and retry. --no-cache so a stale cache
# can never green-light drift.
lint-check:
  treefmt --no-cache --fail-on-change

lint-shell:
  treefmt --formatters shellcheck,shellcheck-rendered-template

format-shell:
  treefmt --formatters shfmt

format-markdown:
  treefmt --formatters mdformat

lint-toml:
  treefmt --formatters taplo

lint-json:
  treefmt --formatters jq-validate,osquery-config-render

lint-yaml:
  treefmt --formatters yq-validate

# GitHub Actions hygiene: actionlint (syntax/semantics) plus zizmor (static
# security analysis). Split into two recipes because CI runs only the zizmor
# half as a gate of its own, and `ship` reuses that recipe rather than
# repeating its command line.
lint-actions: lint-actions-syntax lint-actions-security

# actionlint through treefmt, so it uses the same config `just l` uses.
lint-actions-syntax:
  treefmt --formatters actionlint

# CI's third gate, verbatim. --offline skips the audits that need the GitHub
# API, so the result is deterministic.
lint-actions-security:
  zizmor --offline .github/workflows

diff:
  chezmoi diff --exclude=templates

apply-no-auth:
  chezmoi apply --exclude=templates --force

# Tests live in suites by DESIGN: test/unit (single component, stub-driven, no
# flows, no sleeps; FAST is the admission rule), test/integration and test/e2e
# (bats coverage for the long-lived bash tools, speed-gated like everything
# else since the 2026-08-05 purge). A fourth camp does NOT live under test/,
# because it is not shell: `test-rust`
# runs the herdr plugins' inline Rust tests where cargo expects them, in each
# plugin's own crate. The pre-commit hook runs `just test-unit` only; the
# pre-push hook runs no suite at all (lint drift only); CI and `just ship` run
# `just test`.

# Unit suite only: the commit gate. --shuffle randomizes order to flush hidden
# ordering deps (seed printed for replay); --warn-slow-ms flags slow tests in a
# warn-only summary. The other suites run the same runner plain.
test-unit: validate-tests
  ./test/run-test-suite.sh --shuffle --warn-slow-ms 200 test/unit

# One suite at a time, for focused iteration. test/run-test-suite.sh runs the
# suite's executable *.sh tests, then its *.bats (host bats-core, a brew
# formula; the nix fallback is gone with the flake).
test-integration: validate-tests
  ./test/run-test-suite.sh test/integration

test-e2e: validate-tests
  ./test/run-test-suite.sh test/e2e

# The two herdr plugins' inline Rust unit tests, the one camp that is not a
# shell suite: a `#[cfg(test)] mod tests` in each plugin's src/main.rs, covering
# the pure decision functions (every Command call sits behind an untested
# boundary by design). They existed but no gate ran them until 2026-08-05.
#
# --locked matches the apply-time build
# (.chezmoitemplates/herdr-plugin-build.sh.tmpl): Cargo.lock is committed for
# both plugins and a gate must not rewrite it. cargo comes from PATH, and its
# absence FAILS this camp rather than skipping it, because a camp that skips
# itself when its toolchain is missing is how these tests went unrun. CI needs
# no new step: macos-latest ships cargo, and `just test` pulls this in.
#
# Cheap enough to sit in the default camp list: about 2.5s per plugin against an
# empty target/, 0.06s warm. target/ is plugin-local, gitignored and
# .chezmoiignore'd, so a developer pays the build once.
test-rust:
  cargo test --locked --manifest-path dot_local/share/herdr/plugins/herdr-smart-nav/Cargo.toml
  cargo test --locked --manifest-path dot_local/share/herdr/plugins/herdr-last-workspace/Cargo.toml

# Placement / mode / symlink guard (test/validate-tests.sh): every *.sh and
# *.bats below test/ must sit DIRECTLY in a recognized suite (test/unit,
# test/integration, test/e2e, test/test-system); suite *.sh must be executable;
# no symlinks are allowed anywhere below test/ (a physical find skips them, so
# they would evade every gate). A suite's helpers/ and test/fixtures/** are
# exempt.
validate-tests:
  ./test/validate-tests.sh

# All suites: what CI runs.
test: test-unit test-integration test-e2e test-rust

# The pre-PR sweep: the three gates .github/workflows/lint.yml runs, in CI's
# order, as LITERAL `just` command lines so they compare byte for byte against
# the workflow's run: steps. Keeping this list and the workflow describing the
# same work is now a manual review step (the parity test was a declaration
# cross-check, deleted 2026-08-05).
#
# A green run does NOT promise CI green: it reads the working tree; CI reads
# the pushed commit. An edit you never staged can make ship green and CI red;
# that is the PR #116 failure `docs/runbooks/git-hooks.md` records.
ship:
  just lint-check
  just test
  just lint-actions-security

# Run the weekly Homebrew upgrade by hand (formulae + casks + Mac App Store +
# cleanup). Same job the Monday-noon com.webdavis.homebrew-weekly-upgrade
# LaunchAgent runs; use it for the first upgrade or any ad-hoc one. Runs the
# DEPLOYED helper (what launchd runs), not the repo source copy.
brew-upgrade:
  ~/.local/libexec/unattended-upgrades/homebrew-weekly-upgrade.sh

# Regenerate the brew shellenv cache (~/.cache/brew-shellenv.sh) from the current
# `brew shellenv`, now, instead of waiting for the next interactive shell to
# self-heal it. Runs the DEPLOYED writer, the same artifact ~/.bashrc's
# self-heal runs, so the atomic write has exactly one implementation and this
# recipe cannot drift from it. The writer reaches ~/.local/bin only via
# `chezmoi apply`, so say that plainly instead of letting the shell report a
# bare "No such file or directory" from a path the reader has no reason to
# recognize.
brew-cache-refresh:
  #!/usr/bin/env bash
  set -euo pipefail
  deployed_writer="$HOME/.local/libexec/brew-shellenv-cache-refresh.sh"
  if [[ ! -x $deployed_writer ]]; then
    printf 'brew-cache-refresh: %s is not deployed.\n' "$deployed_writer" >&2
    printf '  Run `chezmoi apply` (it is a plain file, not a template), then retry.\n' >&2
    exit 1
  fi
  "$deployed_writer"

# macOS Defaults: drift, apply, capture

defaults-drift:
  ~/.local/libexec/macos-defaults/macos-defaults-drift.sh

defaults-apply:
  ~/.local/libexec/macos-defaults/macos-defaults-apply.sh

# `defaults-capture <domain> <key> [current]`, capture a live setting into YAML.
# Pass the literal `current` as the third arg to use ByHost storage
# (`defaults -currentHost`). Any non-empty third arg triggers ByHost mode;
# the v1 schema does not support arbitrary hostnames.
defaults-capture domain key current="":
  #!/usr/bin/env bash
  set -euo pipefail
  if [[ -n "{{current}}" ]]; then
    ~/.local/libexec/macos-defaults/macos-defaults-capture.sh "{{domain}}" "{{key}}" "--host=current"
  else
    ~/.local/libexec/macos-defaults/macos-defaults-capture.sh "{{domain}}" "{{key}}"
  fi

# macOS Defaults discovery, read-only wrappers around `defaults`.
defaults-list:
  defaults domains | tr ',' '\n' | sort

defaults-show domain:
  defaults read "{{domain}}"

defaults-dump:
  defaults read | less

# Refresh portable agent skills in the store (~/.agents/skills) + re-symlink each harness.
# Also runs weekly via launchd (com.webdavis.update-skills). Pass --dry-run to preview,
# or --install-only to only install absent manifest skills (fresh-machine bootstrap).
update-skills *args:
  ~/.local/libexec/unattended-upgrades/agent-skills/update-skills.sh {{args}}
