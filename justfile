set shell := ["bash", "-c", "source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh 2>/dev/null; eval \"$@\"", "--"]

default:
  @just --choose

alias h := install-hooks
alias l := lint
alias s := lint-shell
alias S := format-shell
alias m := format-markdown
alias n := format-nix

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

install-hooks:
  @echo "Installing Git pre-commit hooks..."
  git config core.hooksPath .githooks
  chmod +x .githooks/pre-commit
  @echo "✓ Git hooks installed. Pre-commit will run lint.sh"
