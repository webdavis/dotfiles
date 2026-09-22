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

# Format all supported files through the configured treefmt binary.
lint:
  treefmt

# Drift gate used by the pre-push hook. It may write fixes before failing.
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

# GitHub Actions syntax and security checks.
lint-actions: lint-actions-syntax lint-actions-security

# Run actionlint through treefmt.
lint-actions-syntax:
  treefmt --formatters actionlint

# Run the offline zizmor audit used by CI.
lint-actions-security:
  zizmor --offline .github/workflows

# Both commands render templates and require an unlocked KeePassXC database.
# `apply` keeps a transcript; the script's own header says what it withholds.
diff:
  chezmoi diff

apply:
  ./scripts/chezmoi-apply-logged.sh

# Shell suites run through the shared runner. Rust tests run through test-rust.

# Unit suite and commit gate. Shuffle shell tests and report slow tests.
test-unit: validate-tests test-nvim
  ./test/run-test-suite.sh --shuffle --warn-slow-ms 200 test/unit

# Run the integration suite alone.
test-integration: validate-tests
  ./test/run-test-suite.sh test/integration

test-e2e: validate-tests
  ./test/run-test-suite.sh test/e2e

# Rust workspaces are listed explicitly because treefmt does not
# discover Rust manifests. Locked dependencies and documentation warnings are
# checked with the tests.
test-rust:
  cargo test --locked --workspace --manifest-path lights/Cargo.toml
  cargo fmt --all --check --manifest-path lights/Cargo.toml
  cargo clippy --locked --workspace --all-targets --manifest-path lights/Cargo.toml -- -D warnings
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --manifest-path lights/Cargo.toml
  cargo test --locked --workspace --features dev-tools --manifest-path pns/Cargo.toml
  cargo fmt --all --check --manifest-path pns/Cargo.toml
  cargo clippy --locked --workspace --all-targets --features dev-tools --manifest-path pns/Cargo.toml -- -D warnings
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --manifest-path pns/Cargo.toml
  @command -v chord >/dev/null || \
    { echo 'chord is not installed: cargo install --git https://github.com/webdavis/chord chord' >&2; exit 1; }
  chord check bash --table dot_config/chord/bindings.toml
  chord check menu --table dot_config/chord/bindings.toml
  cargo test --locked --workspace --manifest-path tailnet-pin/Cargo.toml
  cargo fmt --all --check --manifest-path tailnet-pin/Cargo.toml
  cargo clippy --locked --workspace --all-targets --manifest-path tailnet-pin/Cargo.toml -- -D warnings
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --manifest-path tailnet-pin/Cargo.toml
  cargo test --locked --workspace --manifest-path uu/Cargo.toml
  cargo fmt --all --check --manifest-path uu/Cargo.toml
  cargo clippy --locked --workspace --all-targets --manifest-path uu/Cargo.toml -- -D warnings
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --manifest-path uu/Cargo.toml
  cargo test --locked --workspace --manifest-path posture/Cargo.toml
  cargo fmt --all --check --manifest-path posture/Cargo.toml
  cargo clippy --locked --workspace --all-targets --manifest-path posture/Cargo.toml -- -D warnings
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --manifest-path posture/Cargo.toml

# Run the Neovim Lua specs against the source tree.
#
# EVERY GIT_* VARIABLE IS SCRUBBED FIRST. git exports GIT_DIR, GIT_INDEX_FILE
# and friends to every hook it runs, so a spec that builds a real repository in
# a temporary directory inherits the committing repository's index and operates
# on that instead: measured 2026-09-15, GIT_INDEX_FILE alone failed all eight
# dashboard_files cases and rewrote the outer commit message under the
# pre-commit hook. Scrubbed at the recipe rather than per spec, so a spec added
# later is clean without knowing about this.
test-nvim:
  #!/usr/bin/env bash
  set -euo pipefail
  while IFS= read -r name; do unset "$name"; done < <(env | sed -n 's/^\(GIT_[A-Za-z0-9_]*\)=.*/\1/p')
  nvim --headless --clean -l dot_config/nvim/tests/run.lua


# Run only the bashunit lane for one suite.
test-bashunit suite="test/unit": validate-tests
  ./test/run-test-suite.sh --only-bashunit {{ suite }}

# Validate test placement, modes, and file types.
validate-tests:
  ./test/validate-tests.sh

# Run all test suites.
test: test-unit test-integration test-e2e test-rust

# Run the three CI gates locally before opening a pull request.
ship:
  just lint-check
  just test
  just lint-actions-security

# Install the contributor toolchain. mdformat is pinned here and in CI because
# it rewrites Markdown.
setup:
  brew bundle --file=Brewfile.dev
  uv tool install mdformat==0.7.22 \
    --with mdformat-gfm==0.4.1 \
    --with mdformat-gfm-alerts==2.0.0 \
    --with mdformat-frontmatter==2.0.8 \
    --with mdformat-footnote==0.1.1 \
    --with mdformat-tables==1.0.0 \
    --with mdformat-config==0.2.1

# Run the deployed weekly Homebrew upgrade lane manually.
brew-upgrade:
  ~/.cargo/bin/uu run brew

# Refresh the deployed Homebrew shell environment cache.
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

# Capture a live macOS setting into YAML. Use `current` for ByHost storage.
defaults-capture domain key current="":
  #!/usr/bin/env bash
  set -euo pipefail
  if [[ -n "{{current}}" ]]; then
    ~/.local/libexec/macos-defaults/macos-defaults-capture.sh "{{domain}}" "{{key}}" "--host=current"
  else
    ~/.local/libexec/macos-defaults/macos-defaults-capture.sh "{{domain}}" "{{key}}"
  fi

# Read-only macOS Defaults helpers.
defaults-list:
  defaults domains | tr ',' '\n' | sort

defaults-show domain:
  defaults read "{{domain}}"

defaults-dump:
  defaults read | less

# Remove this repository's merged, clean worktrees. --dry-run only reports.
worktrees-prune *arguments:
  ~/.local/libexec/prune-merged-worktrees.sh {{arguments}}

# Refresh skills through the weekly uu lane.
update-skills:
  ~/.cargo/bin/uu run skills

# Regenerate both files the shell-agnostic binding table generates: the
# readline bind calls, and the records the binding picker reads.
chord-render:
  @command -v chord >/dev/null || \
    { echo 'chord is not installed: cargo install --git https://github.com/webdavis/chord chord' >&2; exit 1; }
  chord render bash --table dot_config/chord/bindings.toml
  chord render menu --table dot_config/chord/bindings.toml

# Regenerate the shipped pns config template from its committed values.
pns-config-render output="dot_config/pns/private_config.toml.tmpl":
  cargo run --locked --quiet --features dev-tools --manifest-path pns/Cargo.toml --bin pns-config-render -- \
    dot_config/pns/config-values.toml {{quote(output)}}
