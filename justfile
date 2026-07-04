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
alias T := test
alias d := diff
alias a := apply-no-auth
alias c := check
alias D := defaults-drift

# Format everything in place. treefmt (configured in treefmt.nix) is the
# single lint/format orchestrator; the per-tool recipes below just filter it.
lint:
  nix develop .#run --command treefmt

# Check-only drift gate: builds the flake's treefmt check derivation, which
# runs treefmt on a sandboxed copy of the tree — reports drift, never mutates
# the working tree or index (treefmt itself has no dry-run mode, so the
# sandbox copy is what makes this check-only). Same gate CI runs.
lint-check:
  nix flake check

lint-shell:
  nix develop .#run --command treefmt --formatters shellcheck

format-shell:
  nix develop .#run --command treefmt --formatters shfmt

format-markdown:
  nix develop .#run --command treefmt --formatters mdformat

format-nix:
  nix develop .#run --command treefmt --formatters nixfmt

lint-toml:
  nix develop .#run --command treefmt --formatters taplo

lint-json:
  nix develop .#run --command treefmt --formatters jq-validate

lint-yaml:
  nix develop .#run --command treefmt --formatters yq-validate

diff:
  nix develop .#run --command chezmoi diff --exclude=templates

apply-no-auth:
  nix develop .#run --command chezmoi apply --exclude=templates --force

check:
  nix develop .#run --command nix flake check --all-systems

# Run all repo tests: hand-rolled executable test/*.sh (host tools, outside Nix)
# plus the bats suites (test/**/*.bats) inside the Nix devshell — the flake
# provides bats, so no host install is needed and the suite runs the same on a
# fresh machine. Find-driven and empty-safe: green when test/ is missing or has
# no test scripts. The pre-commit hook runs this too, so every commit requires
# all tests to pass.
test:
  #!/usr/bin/env bash
  set -euo pipefail
  status=0
  while IFS= read -r -d '' t; do
    printf "== %s ==\n" "$t"
    "$t" || status=1
  done < <(find test -maxdepth 1 -type f -name '*.sh' -perm -u+x -print0 2>/dev/null | sort -z)
  if find test -type f -name '*.bats' -print -quit 2>/dev/null | grep -q .; then
    printf "== %s ==\n" "bats"
    nix develop .#run --command bats --recursive test/ || status=1
  fi
  exit "$status"

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
