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

# Drift gate used by the pre-push hook. It may write fixes before failing.
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

# Unit suite and commit gate. Shuffle shell tests and report slow tests.
test-unit: validate-tests test-nvim
  ./test/run-test-suite.sh --shuffle --warn-slow-ms 200 test/unit

# treefmt does not discover Rust manifests, so each workspace is listed here.
test-rust:
  #!/usr/bin/env bash
  set -euo pipefail
  if ! command -v chord >/dev/null; then
    echo 'chord is not installed: cargo install --git https://github.com/webdavis/chord chord' >&2
    exit 1
  fi
  test_rust_workspace() {
    local manifest="$1/Cargo.toml"
    shift
    cargo test --locked --workspace "$@" --manifest-path "$manifest"
    cargo fmt --all --check --manifest-path "$manifest"
    cargo clippy --locked --workspace --all-targets "$@" --manifest-path "$manifest" -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --manifest-path "$manifest"
  }
  test_rust_workspace lights
  test_rust_workspace pns --features dev-tools
  chord check bash --table dot_config/chord/bindings.toml
  chord check menu --table dot_config/chord/bindings.toml
  test_rust_workspace tailnet-pin
  test_rust_workspace uu
  test_rust_workspace posture

# Run the Neovim Lua specs against the source tree. Git exports GIT_* variables
# to its hooks, and specs that build temporary repositories must not inherit them.
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
test: test-unit test-rust

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

# Run the weekly Homebrew upgrade lane manually.
brew-upgrade:
  ~/.cargo/bin/uu run brew

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

# Upgrade the pinned moshi-hook after diffing the contract pns relies on.
moshi-hook-upgrade:
  #!/usr/bin/env bash
  set -euo pipefail
  [[ -t 0 ]] || { printf 'moshi-hook-upgrade needs a terminal\n' >&2; exit 1; }
  formula=rjyo/moshi/moshi-hook
  brew update --quiet
  info="$(brew info --json=v2 "$formula")"
  installed="$(jq -r '.formulae[0].installed[0].version // empty' <<<"$info")"
  latest="$(jq -r '.formulae[0].versions.stable' <<<"$info")"
  printf 'moshi-hook: installed %s, latest %s\n' "${installed:-none}" "$latest"
  [[ $installed != "$latest" ]] || exit 0
  tap="$(brew --repository rjyo/moshi)"
  release() { git -C "$tap" log -1 --format=%H --grep="^moshi-hook v$1\$"; }
  contract() {
    git -C "$tap" show "$1:docs/api.md" | awk '/^## /{ p = /^## 1\. Local socket/ } p'
    git -C "$tap" show "$1:docs/hooks.md" | awk '/^##/{ p = /^### (Claude Code|Codex CLI)$/ } p'
    git -C "$tap" show "$1:docs/usage.md" | grep -i -E 'claude|codex'
  }
  old="$(release "$installed")" new="$(release "$latest")"
  if [[ -n $old && -n $new ]]; then
    printf 'moshi-hook: documented contract, %s to %s (no diff means unchanged):\n' "$installed" "$latest"
    diff -u --label "docs $installed" --label "docs $latest" <(contract "$old") <(contract "$new") || [[ $? -eq 1 ]]
  else
    printf 'moshi-hook: the tap has no release commit for %s or %s, so its docs are not compared.\n' "$installed" "$latest"
  fi
  helps() { for subcommand in claude-hook codex-hook; do moshi-hook "$subcommand" --help; done; }
  before="$(helps)"
  read -r -p "Upgrade moshi-hook $installed to $latest? [y/N] " answer
  [[ $answer == [yY] ]] || exit 0
  if [[ $(jq -r '.formulae[0].pinned' <<<"$info") == true ]]; then brew unpin "$formula"; fi
  # Re-pin on every exit once unpinned, a failed or interrupted upgrade must
  # not leave moshi-hook free for the weekly unattended job to move.
  trap 'brew pin "$formula"' EXIT
  HOMEBREW_NO_AUTO_UPDATE=1 brew upgrade "$formula"
  printf 'moshi-hook: hook subcommand help, %s to %s (no diff means unchanged):\n' "$installed" "$latest"
  diff -u --label "help $installed" --label "help $latest" <(printf '%s\n' "$before") <(helps) || [[ $? -eq 1 ]]
  brew services restart moshi-hook
  printf '%s\n' "Now answer one approval from the phone, away from the desk, in each of:" \
    "  codex -s read-only -a on-request -c approvals_reviewer=user" \
    "  claude --permission-mode manual" \
    "When both land, set rjyo/moshi/moshi-hook to \"$latest\" under packages.macos.homebrew.pinned in" \
    ".chezmoidata/system_packages_autoinstall.yaml."

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
