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
alias a := apply
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

# Both reach templates, so both need KeePassXC unlocked and an interactive
# terminal. That is deliberate: excluding templates left the deployed copy of a
# templated target behind its source, which the osquery known-good manifest
# reads as tampering. The operator runs these until the vault is replaced by
# something an agent can unlock.
diff:
  chezmoi diff

apply:
  chezmoi apply -v

# Test suites: test/unit (single component, stub-driven, fast), then
# test/integration and test/e2e. Rust tests live in each plugin's own crate,
# not under test/, and run via `test-rust`. The pre-commit hook runs
# `just test-unit`; the pre-push hook runs no suite (lint drift only); CI and
# `just ship` run `just test`.

# Unit suite only: the commit gate. The two Lua camps run first (the nvim
# config's specs, then neotest-bashunit's), then the one runner, which runs the
# suite's own three lanes in order: its bashunit `*.test.sh` files, its
# executable *.sh tests, its *.bats suites. --shuffle randomizes the *.sh order
# to flush hidden ordering deps (seed printed for replay); --warn-slow-ms flags
# slow tests in a warn-only summary. The other suites run the same runner plain.
test-unit: validate-tests test-nvim test-neotest-bashunit
  ./test/run-test-suite.sh --shuffle --warn-slow-ms 200 test/unit

# One suite at a time, for focused iteration. test/run-test-suite.sh runs the
# suite's executable *.sh tests, then its *.bats (host bats-core, a brew
# formula; the nix fallback is gone with the flake).
test-integration: validate-tests
  ./test/run-test-suite.sh test/integration

test-e2e: validate-tests
  ./test/run-test-suite.sh test/e2e

# The four Rust crates' tests, the one camp that is not a shell suite. The two
# herdr plugins cover the pure decision functions in their src/main.rs (every
# Command call sits behind an untested boundary by design) with inline
# `#[cfg(test)] mod tests`. pns and uu are NOT pure-decision libraries: each
# has four (pns) or six (uu) integration binaries under tests/ that spawn the
# real compiled engine as a subprocess against a private sandboxed HOME,
# alongside their own inline unit tests.
#
# THE RUST CAMP RUNS UNDER A ONE-SECOND TEST BUDGET, enforced by a Drop guard
# on each integration binary's sandbox harness (both crates keep it at
# tests/support/mod.rs). A sandbox alive past one second at its own drop WARNS
# on stderr, greppable as "test budget"; past five seconds it FAILS the
# build, unless the test called `allow_slow("reason")` on it because the cost
# is structural (an epoch-second lease, a whole-second deadline config key)
# rather than a regression. This is a LOWER BOUND on the test's own sandbox
# lifetime, not an upper bound on the code, and it says nothing about a unit
# test, none of which owns a sandbox or runs over a second alone today.
#
# THE AGGREGATE IS STILL MULTI-SECOND, and that is not a bug in the budget:
# warm, parallel per-binary totals here run daemon ~5s (one structural
# `allow_slow` test dominates it), dispatch ~4s, hooks ~1.5s, native ~0.3s.
# The one-second line bounds each TEST's own sandbox, not the sum across a
# binary's whole suite.
#
# TWO STABLE ALTERNATIVES TO THE NIGHTLY-ONLY `--report-time --ensure-time`
# CALIBRATION FLAGS WERE REJECTED, not adopted: `RUSTC_BOOTSTRAP=1
# RUST_TEST_TIME_UNIT=500,1000 cargo +stable test -- -Z unstable-options
# --report-time --ensure-time` works on stable 1.88 (verified with a scratch
# crate) but is an escape hatch that changes cargo's rustc fingerprint, and it
# measures the same contended wall clock the guard already accounts for;
# cargo-nextest is not installed and would be a new Brewfile.dev and CI
# dependency for one calibration run. The Drop guard needs neither: it is
# ordinary safe Rust, so it runs identically on nightly here and on CI's
# stable macOS.
#
# THE CRATES ARE ENUMERATED BY HAND. treefmt has no Rust coverage and nothing
# discovers a manifest, so a new crate is invisible to this gate until its
# three lines are written here.
#
# --locked matches the apply-time build
# (.chezmoitemplates/herdr-plugin-build.sh.tmpl): Cargo.lock is committed for
# every crate and a gate must not rewrite it. cargo comes from PATH, and its
# absence FAILS this camp rather than skipping it, because a camp that skips
# itself when its toolchain is missing is how these tests went unrun. CI needs
# no new step: macos-latest ships cargo, clippy and rustfmt, and `just test`
# pulls this in.
#
# fmt and clippy run for pns and uu ONLY. Nothing linted Rust anywhere before
# pns, and adopting the two for the herdr plugins is a separate decision with
# its own diff; a gate that fails on code this slice did not touch would just be
# turned off. --all-targets so the test modules are linted too, since that is
# where most of those crates' code lives.
#
# pns is a WORKSPACE and the other three crates are not, which is why only its
# three lines carry --workspace (--all is what cargo fmt calls the same thing).
# Its root manifest is still a package as well as the workspace root, so
# without those words cargo tests, formats and lints that one package and
# skips every member crate without saying so.
#
# The two herdr plugins' own build cost is cheap enough to sit in the default
# camp list: about 2.5s per crate against an empty target/, well under a
# second warm. target/ is crate-local, gitignored and .chezmoiignore'd, so a
# developer pays the build once.
test-rust:
  cargo test --locked --manifest-path dot_local/share/herdr/plugins/herdr-smart-nav/Cargo.toml
  cargo test --locked --manifest-path dot_local/share/herdr/plugins/herdr-workspace-jump/Cargo.toml
  cargo test --locked --workspace --manifest-path dot_local/share/pns/Cargo.toml
  cargo fmt --all --check --manifest-path dot_local/share/pns/Cargo.toml
  cargo clippy --locked --workspace --all-targets --manifest-path dot_local/share/pns/Cargo.toml -- -D warnings
  cargo test --locked --manifest-path dot_local/share/uu/Cargo.toml
  cargo fmt --check --manifest-path dot_local/share/uu/Cargo.toml
  cargo clippy --locked --all-targets --manifest-path dot_local/share/uu/Cargo.toml -- -D warnings

# The nvim config's headless Lua specs (spec 6.3), run against the SOURCE tree.
# `--clean` keeps the plugin tree out, so a whole run costs about 30 ms. The
# runner globs tests/*_spec.lua itself, so a new spec file needs no registration
# anywhere. test-unit depends on this recipe: that is what puts the specs in the
# commit gate and, through test-unit, in `just test`.
test-nvim:
  nvim --headless --clean -l dot_config/nvim/tests/run.lua

# neotest-bashunit's own specs (dot_local/share/neotest-bashunit/tests), the
# same runner shape one directory over. `--clean` is load-bearing rather than
# merely fast here: the rules under test are the pure ones in parse.lua, so they
# must hold with neotest itself not installed. test-unit depends on this recipe.
test-neotest-bashunit:
  nvim --headless --clean -l dot_local/share/neotest-bashunit/tests/run.lua

# ONE suite's bashunit `<name>.test.sh` files, for focused iteration. Every
# suite recipe above already runs its own bashunit lane through the same
# runner, so this adds no coverage: it narrows a run to that lane, and defaults
# to the unit suite because that is where the migration starts.
#
# bashunit is never handed a DIRECTORY, here or in the runner. Its own path
# argument scans recursively for `*[tT]est.sh` plus a `.bash` twin, which
# reaches fixtures, the executable *.sh tests the other lane runs, and every
# other suite's files; the runner passes an exact, suite-local list instead.
# validate-tests is a dependency for the same reason it is on every other suite
# recipe: the mode and placement rules are what keep the two lanes from
# claiming one file.
test-bashunit suite="test/unit": validate-tests
  ./test/run-test-suite.sh --only-bashunit {{ suite }}

# Placement / mode / symlink guard (test/validate-tests.sh): every *.sh and
# *.bats below test/ must sit DIRECTLY in a recognized suite (test/unit,
# test/integration, test/e2e, test/test-system); suite *.sh must be executable,
# except a bashunit `<name>.test.sh`, which must NOT be and which never belongs
# in helpers/ or fixtures/; no symlinks are allowed anywhere below test/ (a
# physical find skips them, so they would evade every gate). A suite's helpers/
# and test/fixtures/** are otherwise exempt.
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

# Install the contributor toolchain into a fresh checkout: every gate below
# (lint-check, test, lint-actions-security) assumes these are on PATH.
#
# Two lanes, because the tools split by how they drift. Binary tools come from
# Brewfile.dev, where a floating version is fine: a newer shfmt or shellcheck
# changes findings, not formatting bytes. mdformat is the exception and the
# reason this recipe exists rather than a bare `brew bundle`. It REWRITES
# markdown, so a version bump silently rewraps every file and the drift gate
# fails on work nobody did; it and all six plugins are therefore pinned to the
# exact versions CI installs.
#
# THE PIN SET LIVES HERE AND IN CI, hand-synced. The toolchain step in
# .github/workflows/lint.yml carries the same `==` versions, nothing enforces
# that the two agree, and they must move together or local and CI disagree
# about what formatted markdown looks like.
setup:
  brew bundle --file=Brewfile.dev
  uv tool install mdformat==0.7.22 \
    --with mdformat-gfm==0.4.1 \
    --with mdformat-gfm-alerts==2.0.0 \
    --with mdformat-frontmatter==2.0.8 \
    --with mdformat-footnote==0.1.1 \
    --with mdformat-tables==1.0.0 \
    --with mdformat-config==0.2.1

# Run the weekly Homebrew upgrade by hand (formulae + casks + Mac App Store +
# cleanup). The same lane the Sunday-noon com.webdavis.uu LaunchAgent runs; use
# it for the first upgrade or any ad-hoc one. Runs the DEPLOYED binary (what
# launchd runs), not the repo source copy. It takes uu's own run lock, so a run
# that overlaps the scheduled one says so and exits rather than racing it.
brew-upgrade:
  ~/.local/libexec/uu/uu run brew

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

# Regenerate the shipped pns config template from the committed values file.
# `just test-rust` pins the result byte for byte, so a hand edit to the
# template (or an honest edit to the values file) fails there; this recipe is
# what makes it green again. Dev-only: `pns-config-render` is never installed.
pns-config-render:
  cargo run --locked --quiet --manifest-path dot_local/share/pns/Cargo.toml --bin pns-config-render -- \
    dot_config/pns/config-values.toml dot_config/pns/private_config.toml.tmpl
