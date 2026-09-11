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
diff:
  chezmoi diff

apply:
  chezmoi apply -v

# Shell suites run through the shared runner. Rust tests run through test-rust.

# Unit suite and commit gate. Shuffle shell tests and report slow tests.
test-unit: validate-tests test-nvim
  ./test/run-test-suite.sh --shuffle --warn-slow-ms 200 test/unit

# Run the integration suite alone.
test-integration: validate-tests
  ./test/run-test-suite.sh test/integration

test-e2e: validate-tests
  ./test/run-test-suite.sh test/e2e

# Rust workspaces and plugins are listed explicitly because treefmt does not
# discover Rust manifests. Locked dependencies and documentation warnings are
# checked with the tests.
test-rust:
  cargo test --locked --workspace --manifest-path lights/Cargo.toml
  cargo fmt --all --check --manifest-path lights/Cargo.toml
  cargo clippy --locked --workspace --all-targets --manifest-path lights/Cargo.toml -- -D warnings
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --manifest-path lights/Cargo.toml
  cargo test --workspace --locked --manifest-path dot_local/share/herdr/plugins/herdr-smart-nav/Cargo.toml
  cargo test --workspace --locked --manifest-path dot_local/share/herdr/plugins/herdr-workspace-jump/Cargo.toml
  cargo test --locked --workspace --features dev-tools --manifest-path pns/Cargo.toml
  cargo fmt --all --check --manifest-path pns/Cargo.toml
  cargo clippy --locked --workspace --all-targets --features dev-tools --manifest-path pns/Cargo.toml -- -D warnings
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --manifest-path pns/Cargo.toml
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
test-nvim:
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

# Refresh skills through the weekly uu lane.
update-skills:
  ~/.cargo/bin/uu run skills

# Regenerate the shipped pns config template from its committed values.
pns-config-render output="dot_config/pns/private_config.toml.tmpl":
  cargo run --locked --quiet --features dev-tools --manifest-path pns/Cargo.toml --bin pns-config-render -- \
    dot_config/pns/config-values.toml {{quote(output)}}
