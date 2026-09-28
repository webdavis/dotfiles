set shell := ["bash", "-cu"]

default:
  @just --choose

alias a := apply
alias d := diff
alias g := lint-gate
alias l := lint
alias s := ship
alias t := test

alias D := macos-defaults-drift
alias J := lint-json
alias S := lint-shell
alias T := lint-toml
alias Y := lint-yaml

alias fs := format-shell
alias fm := format-markdown

# Format all supported files through the configured treefmt binary.
lint:
  treefmt

# Drift gate. It may write fixes before failing.
lint-gate:
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
  ./scripts/chezmoi-apply-logged.sh

test-rust:
  #!/usr/bin/env bash
  set -euo pipefail
  if ! command -v chord >/dev/null; then
    echo 'chord is not installed: cargo install --git https://github.com/webdavis/chord chord' >&2
    exit 1
  fi
  chord check bash --table dot_config/chord/bindings.toml
  chord check menu --table dot_config/chord/bindings.toml

# Run the Neovim Lua specs against the source tree. Git exports GIT_* variables
# to its hooks, and specs that build temporary repositories must not inherit them.
test-nvim:
  #!/usr/bin/env bash
  set -euo pipefail
  while IFS= read -r name; do unset "$name"; done < <(env | sed -n 's/^\(GIT_[A-Za-z0-9_]*\)=.*/\1/p')
  nvim --headless --clean -l dot_config/nvim/tests/run.lua

# Run all test suites.
test: test-nvim test-rust

# Run the three CI gates locally before opening a pull request.
ship:
  just lint-gate
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

# Run the weekly Homebrew and App Store upgrade lanes manually.
brew-upgrade:
  ~/.cargo/bin/uu run brew-and-repairs
  ~/.cargo/bin/uu run mas

# Refresh the Homebrew shell environment cache.
brew-cache-refresh:
  #!/usr/bin/env bash
  set -euo pipefail
  deployed_writer="$HOME/.local/libexec/brew-shellenv-cache-refresh.sh"
  if [[ ! -x $deployed_writer ]]; then
    printf 'brew-cache-refresh: %s is not deployed; run `just apply`, then retry.\n' "$deployed_writer" >&2
    exit 1
  fi
  "$deployed_writer"

# macOS Defaults: drift, apply, capture.
macos-defaults-drift:
  ./scripts/macos-defaults/macos-defaults-drift.sh

macos-defaults-apply:
  ./scripts/macos-defaults/macos-defaults-apply.sh

# Capture a live macOS setting into YAML. Use `current` for ByHost storage.
macos-defaults-capture domain key current="":
  #!/usr/bin/env bash
  set -euo pipefail
  if [[ -n "{{current}}" ]]; then
    ./scripts/macos-defaults/macos-defaults-capture.sh "{{domain}}" "{{key}}" "--host=current"
  else
    ./scripts/macos-defaults/macos-defaults-capture.sh "{{domain}}" "{{key}}"
  fi

# Read-only macOS Defaults helpers.
macos-defaults-list:
  defaults domains | tr ',' '\n' | sort

macos-defaults-show domain:
  defaults read "{{domain}}"

macos-defaults-dump:
  defaults read | less

# Remove this repository's merged, clean worktrees. --dry-run only reports.
worktrees-prune *arguments:
  ./scripts/prune-merged-worktrees.sh {{arguments}}

# Refresh skills through the weekly uu lanes.
update-skills:
  ~/.cargo/bin/uu run skills

# Regenerate both files the shell-agnostic binding table generates: the
# readline bind calls, and the records the binding picker reads.
chord-render:
  @command -v chord >/dev/null || \
    { echo 'chord is not installed: cargo install --git https://github.com/webdavis/chord chord' >&2; exit 1; }
  chord render bash --table dot_config/chord/bindings.toml
  chord render menu --table dot_config/chord/bindings.toml
