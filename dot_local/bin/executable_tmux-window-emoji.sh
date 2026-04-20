#!/usr/bin/env bash
# Output an emoji for the window at $1 (e.g. "uriel:3") based on what's
# running in its active pane. Silent for shells and interactive TUIs.
# Consumed by @tmux2k-window-list-format (see dot_tmux.conf v2 §21.2).

target="${1:-}"
[[ -z $target ]] && exit 0
cmd=$(tmux display-message -p -t "$target" '#{pane_current_command}' 2>/dev/null)

case "$cmd" in
  # Shells and interactive TUIs — silent.
  bash | zsh | fish | sh | dash) ;;
  nvim | vim | vi | view | less | more | man | top | btop | htop | tmux | ssh | mosh | fzf) ;;

  # AI agents.
  claude | codex | aider | goose | cursor-agent) printf '🤖' ;;

  # Test runners.
  pytest | jest | vitest | rspec | mocha | phpunit | tox) printf '🧪' ;;

  # Build tools / task runners / package managers.
  cargo | go | make | gmake | just | webpack | vite | rollup | esbuild | tsc | swift | xcodebuild) printf '🔨' ;;
  docker | nix | nix-build | nixos-rebuild | npm | pnpm | yarn | bun) printf '🔨' ;;
  gradle | mvn | ant | meson | ninja | bazel | buck | cmake) printf '🔨' ;;

  # Everything else that isn't a shell — generic long-running.
  *) printf '⏳' ;;
esac
