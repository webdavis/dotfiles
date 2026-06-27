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
alias d := diff
alias a := apply-no-auth
alias c := check
alias D := defaults-drift

lint:
  nix develop .#run --command ./scripts/lint.sh

lint-check:
  LINT_CHECK=1 nix develop .#run --command ./scripts/lint.sh

lint-shell:
  nix develop .#run --command ./scripts/lint.sh -s

format-shell:
  nix develop .#run --command ./scripts/lint.sh -S

format-markdown:
  nix develop .#run --command ./scripts/lint.sh -m

format-nix:
  nix develop .#run --command ./scripts/lint.sh -n

lint-toml:
  nix develop .#run --command ./scripts/lint.sh -t

lint-json:
  nix develop .#run --command ./scripts/lint.sh -j

lint-yaml:
  nix develop .#run --command ./scripts/lint.sh -y

diff:
  nix develop .#run --command chezmoi diff --exclude=templates

apply-no-auth:
  nix develop .#run --command chezmoi apply --exclude=templates --force

check:
  nix develop .#run --command nix flake check --all-systems

# Run all repo tests (test/*.sh). Build-tool style: the pre-commit hook runs this
# too, so every commit requires all tests to pass. Tests use host tools (e.g.
# brew), so this runs outside the Nix shell.
test:
  @for t in test/*.sh; do echo "== $t =="; bash "$t" || exit 1; done

# Run only the brew shellenv cache drift test (a subset of `just test`).
test-brew-cache:
  ./test/brew-shellenv-cache-drift.sh

# Regenerate the brew shellenv cache (~/.cache/brew-shellenv.sh) from the current
# `brew shellenv`, without a full `chezmoi apply`. Use after a Homebrew update if
# `just test` reports cache drift.
brew-cache-refresh:
  mkdir -p "${XDG_CACHE_HOME:-$HOME/.cache}" && /opt/homebrew/bin/brew shellenv > "${XDG_CACHE_HOME:-$HOME/.cache}/brew-shellenv.sh" && echo "Regenerated brew shellenv cache; run 'just test' to confirm."

# Run the weekly Homebrew upgrade by hand (formulae + casks + Mac App Store +
# cleanup). Same job the Monday-noon LaunchAgent runs; use for the first upgrade
# or any ad-hoc one. Uses the host brew, outside the Nix shell.
brew-upgrade:
  ./dot_local/bin/executable_homebrew-weekly-upgrade.sh

# macOS Defaults: drift, apply, capture

defaults-drift:
  ~/.local/bin/macos-defaults-drift.sh

defaults-apply:
  ~/.local/bin/macos-defaults-apply.sh

# `defaults-capture <domain> <key> [current]` — capture a live setting into YAML.
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

# macOS Defaults discovery — read-only wrappers around `defaults`.
defaults-list:
  defaults domains | tr ',' '\n' | sort

defaults-show domain:
  defaults read "{{domain}}"

defaults-dump:
  defaults read | less

# (agent-skill vendoring removed: herdr/moshi now live in ~/.agents/skills, symlinked per-harness)

# Refresh portable agent skills in the store (~/.agents/skills) + re-symlink each harness.
# Also runs weekly via launchd (com.webdavis.update-skills). Pass --dry-run to preview.
update-skills *args:
  ~/.local/bin/update-skills.sh {{args}}
