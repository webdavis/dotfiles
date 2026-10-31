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

# Check the binding table renders cleanly.
test-bindings:
  ./scripts/chord.sh check

# Run the Neovim Lua specs against the source tree.
test-nvim:
  ./scripts/test-nvim.sh

test-espanso-url-menu:
  python3 scripts/test-espanso-url-menu.py

# Run all test suites.
test: test-nvim test-bindings test-espanso-url-menu

# Run the three CI gates locally before opening a pull request.
ship:
  just lint-gate
  just test
  just lint-actions-security

# Install the contributor toolchain.
setup:
  ./scripts/setup.sh

# Run the weekly Homebrew and App Store upgrade lanes manually.
brew-upgrade:
  ~/.cargo/bin/uu run brew-and-repairs
  ~/.cargo/bin/uu run mas

# Refresh the Homebrew shell environment cache.
brew-cache-refresh:
  ./scripts/brew-cache-refresh.sh

# Report macOS settings that differ from the tracked YAML.
macos-defaults-drift:
  ./scripts/macos-defaults/macos-defaults-drift.sh

# Apply the tracked macOS settings.
macos-defaults-apply:
  ./scripts/macos-defaults/macos-defaults-apply.sh

# Capture a live macOS setting into YAML. Pass `--host current` for ByHost storage.
macos-defaults-capture domain key *options:
  ./scripts/macos-defaults/macos-defaults-capture.sh "{{domain}}" "{{key}}" {{options}}

# List KeePass entries the templates read that the database doesn't have. Asks for the database password.
check-keepass:
  ./scripts/check-keepass-entries.sh

# Zip each Claude Desktop skill into one folder, ready to drag into Claude Desktop.
claude-desktop-skills folder=(home_directory() / "Desktop" / "claude-desktop-skills"):
  ./scripts/claude-desktop-skills.sh "{{folder}}"

# Open Raycast's Import Settings & Data for ~/.config/raycast/raycast.rayconfig.
raycast-import:
  ./scripts/raycast-import.sh

# Refresh skills through the weekly uu lanes.
update-skills:
  ~/.cargo/bin/uu run skills

# Regenerate the readline bindings and the binding-picker records from the binding table.
chord-render:
  ./scripts/chord.sh render
