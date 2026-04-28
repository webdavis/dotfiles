set shell := ["bash", "-c", "source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null; eval \"$@\"", "--"]

default:
  @just --choose

alias h := install-hooks
alias l := lint
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
