set shell := ["bash", "-c", "source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null; eval \"$@\"", "--"]

default:
  @just --choose

alias h := install-hooks
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

install-hooks:
  @echo "Installing Git pre-commit hooks..."
  git config core.hooksPath .githooks
  chmod +x .githooks/pre-commit
  @echo "✓ Git hooks installed. Pre-commit will run lint.sh"

# macOS Defaults: drift, apply, capture
alias D := defaults-drift

defaults-drift:
  ~/.local/bin/macos-defaults-drift.sh

defaults-apply:
  ~/.local/bin/macos-defaults-apply.sh

defaults-capture domain key host="":
  #!/usr/bin/env bash
  set -euo pipefail
  if [[ -n "{{host}}" ]]; then
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
