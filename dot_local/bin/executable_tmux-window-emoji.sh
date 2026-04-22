#!/usr/bin/env bash
# Output an emoji for the window at $1 (e.g. "uriel:3") based on what's
# running in its active pane. Silent for shells and interactive TUIs.
# Consumed by @tmux2k-window-list-format and by tmux-last-proc.sh
# (see dot_tmux.conf v2 §21.2).

target="${1:-}"
[[ -z $target ]] && exit 0
cmd=$(tmux display-message -p -t "$target" '#{pane_current_command}' 2>/dev/null)

case "$cmd" in
  # Shells and interactive TUIs — silent.
  bash | zsh | fish | sh | dash) ;;
  nvim | vim | vi | view | less | more | man | top | btop | htop | tmux | ssh | mosh | fzf) ;;
  mtr | wireshark | tshark | ngrep | glow | csvlens | tealdeer | tldr) ;;

  # AI agents.
  claude | codex | aider | goose | cursor-agent) printf '🤖' ;;
  gemini | gemini-cli | whisper | openai-whisper | mlx-whisper | mlx_whisper) printf '🤖' ;;

  # Test runners.
  pytest | jest | vitest | rspec | mocha | phpunit | tox) printf '🧪' ;;
  hurl | ansible-lint) printf '🧪' ;;

  # Build tools / task runners / package managers.
  cargo | go | zig | make | gmake | just | webpack | vite | rollup | esbuild | tsc | swift | xcodebuild) printf '🔨' ;;
  docker | nix | nix-build | nixos-rebuild | npm | pnpm | yarn | bun | deno | node | uv) printf '🔨' ;;
  gradle | mvn | ant | meson | ninja | bazel | buck | cmake) printf '🔨' ;;
  ansible | ansible-playbook | ansible-pull) printf '🔨' ;;
  xcodegen | swiftformat | swiftlint | vapor) printf '🔨' ;;
  ffmpeg | magick | convert | imagemagick) printf '🔨' ;;
  wget | curl | act | restic | parallel | nmap | rsync | git-filter-repo) printf '🔨' ;;
  7z | 7zz | p7zip | gzip | gunzip | xz | unxz | tar) printf '🔨' ;;
  postgres | pg_dump | pg_restore) printf '🔨' ;;

  # Everything else that isn't a shell — generic long-running.
  *) printf '⏳' ;;
esac
