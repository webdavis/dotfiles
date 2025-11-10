default:
  @just --choose

alias l := lint
alias s := lint-shell
alias S := format-shell
alias m := format-markdown
alias n := format-nix

lint:
  nix develop .#adhoc --command ./scripts/lint.sh

lint-shell:
  nix develop .#adhoc --command ./scripts/lint.sh -s

format-shell:
  nix develop .#adhoc --command ./scripts/lint.sh -S

format-markdown:
  nix develop .#adhoc --command ./scripts/lint.sh -m

format-nix:
  nix develop .#adhoc --command ./scripts/lint.sh -n
