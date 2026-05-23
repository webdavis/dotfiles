#!/usr/bin/env bash
# Default sesh preview command. Wired in dot_tmux.conf:
#   --preview '~/.local/bin/sesh-preview.sh {2..}'
#
# Output sections (in order):
#   1. Todoist tasks for the section named after the path's basename.
#      Per ~/.claude/CLAUDE.md, sections in Todoist == git repo names, so
#      `td task list --filter "/<basename>"` is the canonical lookup.
#   2. `git status -sb` (short + branch) if <path> is a git working tree.
#   3. eza tree depth 2 — .gitignore-aware in git repos, plain otherwise.
#
# Argument resolution: sesh's `{}` substitutes a PATH for zoxide/wildcard
# entries but a SESSION NAME for already-running tmux sessions. The script
# tries the input as a path first; if not a directory, falls back to
# `sesh list -tczj | jq` lookup.
#
# Always exits 0 so sesh's preview pane never shows "Exit status 1."
# `--color=always` is required because fzf's preview window is not a TTY,
# so eza/git would otherwise drop their ANSI escapes.

set -uo pipefail

arg="${1:-}"

if [[ -z $arg ]]; then
  echo "(no argument passed to sesh-preview.sh)"
  exit 0
fi

path="$arg"
[[ $path == "~"* ]] && path="${HOME}${path:1}"

if [[ ! -d $path ]]; then
  resolved=$(sesh list -tczj 2>/dev/null |
    jq -r --arg n "$arg" 'map(select(.Name == $n))[0].Path // empty' 2>/dev/null)
  if [[ -n $resolved && -d $resolved ]]; then
    path="$resolved"
  else
    echo "(could not resolve: $arg)"
    exit 0
  fi
fi

section=$(basename "$path")

# Section header helper: bg-color band with bold-white title + icon.
# Args: <bg-256-color> <icon> <title>
hdr() {
  local bg="$1" icon="$2" title="$3"
  printf '\033[48;5;%sm\033[1;97m %s  %s \033[0m\n' "$bg" "$icon" "$title"
}

# ── 1. Todoist tasks for the section ──
# Post-2026-05 schema: each repo is its OWN Todoist (sub)project and the session
# basename equals the project name, so query `#<basename>` directly — dynamic, no
# hardcoded mapping. The optional override map only covers the rare case where a
# session's basename differs from its Todoist project name.
# `td ... --filter "#<project>"` returns the project's tasks, or a clean
# "No tasks found." (exit 0) when the project is absent or empty.
if command -v td &>/dev/null; then
  td_map="$HOME/.config/sesh/todoist-project-map.toml"
  project="$section"
  if [[ -f $td_map ]] && command -v python3 &>/dev/null; then
    override=$(python3 -c '
import tomllib, sys
with open(sys.argv[2], "rb") as f:
    print(tomllib.load(f).get(sys.argv[1], ""))
' "$section" "$td_map" 2>/dev/null)
    [[ -n $override ]] && project="$override"
  fi

  hdr 24 "📋" "Todoist  (#$project)"
  # FORCE_COLOR=1 makes td emit ANSI escapes even when piped (Node convention).
  FORCE_COLOR=1 td task list --filter "#$project" --limit 5 2>/dev/null || echo "(td unavailable)"
  echo
fi

# ── 2. git status (if git working tree) ──
is_git=0
if git -C "$path" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  is_git=1
  hdr 22 "🌿" "git status"
  git -C "$path" -c color.ui=always -c color.status=always status --short --branch 2>/dev/null
  echo
fi

# ── 3. eza tree ──
hdr 94 "🌲" "tree"
if ((is_git)); then
  eza --tree --level=2 --icons --git-ignore --color=always "$path" 2>/dev/null || true
else
  eza --tree --level=2 --icons --color=always "$path" 2>/dev/null || true
fi

exit 0
