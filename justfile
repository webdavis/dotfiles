set shell := ["bash", "-cu"]

default:
  @just --choose

alias a := apply
alias d := diff
alias g := lint-gate
alias l := lint
alias s := ship
alias t := test

# Format all supported files through the configured treefmt binary.
lint:
  treefmt

# Drift gate. It may write fixes before failing.
lint-gate:
  treefmt --no-cache --fail-on-change

# Run the offline zizmor audit used by CI.
lint-actions-security:
  zizmor --offline .github/workflows

# Renders templates, so KeePassXC must be unlocked.
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

# Run the Neovim Lua specs with Git's hook variables cleared, so test repositories start clean.
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

# Install the contributor toolchain. mdformat matches the CI version because it rewrites Markdown.
setup:
  brew bundle --file=brewfile.dev
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

# List KeePass entries the templates read that the database doesn't have. Asks for the database password.
check-keepass:
  ./scripts/check-keepass-entries.sh

# Zip each Claude Desktop skill into one folder, ready to drag into Claude Desktop.
claude-desktop-skills folder=(home_directory() / "Desktop" / "claude-desktop-skills"):
  #!/usr/bin/env bash
  set -euo pipefail
  data="{{justfile_directory()}}/.chezmoidata/agent-skills.yaml"
  mkdir -p "{{folder}}"
  stage="$(mktemp -d)"
  trap 'rm -rf "$stage"' EXIT
  while read -r repo; do
    clone="$stage/$(tr / - <<<"$repo")"
    gh repo clone "$repo" "$clone" -- --depth 1 --quiet
    for skill in $(yq -r ".skills.harnesses.claude_desktop[] | select(.repo == \"$repo\") | .skills[]" "$data"); do
      folder="$(find "$clone" -name SKILL.md -not -path '*/node_modules/*' -exec grep -lx "name: $skill" {} + | head -n 1 | xargs -I{} dirname {})"
      [[ -n $folder ]] || { echo "$repo has no SKILL.md named $skill." >&2; exit 1; }
      rm -rf "$stage/$skill" && cp -R "$folder" "$stage/$skill"
      (cd "$stage" && zip -FSrq "{{folder}}/$skill.zip" "$skill" -x '*.DS_Store' "$skill/.claude-plugin/*" "$skill/.github/*" "$skill/.git/*")
    done
  done < <(yq -r '.skills.harnesses.claude_desktop[].repo' "$data")
  echo "Drag the zips in {{folder}} into Claude Desktop."

# Open Raycast's import for ~/.config/raycast/raycast.rayconfig; Raycast needs the passphrase typed by hand.
raycast-import:
  @[[ -f ~/.config/raycast/raycast.rayconfig ]] || \
    { echo 'No ~/.config/raycast/raycast.rayconfig yet; run just apply first.' >&2; exit 1; }
  @echo 'Pick ~/.config/raycast/raycast.rayconfig and enter the export passphrase.'
  open "raycast://extensions/raycast/raycast/import-settings-data"

# Refresh skills through the weekly uu lanes.
update-skills:
  ~/.cargo/bin/uu run skills

# Regenerate the readline bindings and the binding-picker records from the binding table.
chord-render:
  @command -v chord >/dev/null || \
    { echo 'chord is not installed: cargo install --git https://github.com/webdavis/chord chord' >&2; exit 1; }
  chord render bash --table dot_config/chord/bindings.toml
  chord render menu --table dot_config/chord/bindings.toml
