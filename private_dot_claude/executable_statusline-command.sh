#!/usr/bin/env bash
# Claude Code status line - inspired by Starship config at ~/.config/starship.toml

input=$(cat)

cwd=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // empty')
model=$(echo "$input" | jq -r '.model.display_name // empty')
used_pct=$(echo "$input" | jq -r '.context_window.used_percentage // empty')

# Host
host=$(hostname -s)

# Directory: show only last component (matching truncation_length = 1)
dir=$(basename "$cwd")

# Git branch (skip lock to avoid interference)
git_branch=""
if git -C "$cwd" rev-parse --is-inside-work-tree &>/dev/null 2>&1; then
  git_branch=$(git -C "$cwd" --no-optional-locks symbolic-ref --short HEAD 2>/dev/null || git -C "$cwd" --no-optional-locks rev-parse --short HEAD 2>/dev/null)
fi

# Context usage bar
context_info=""
if [ -n "$used_pct" ]; then
  used_int=${used_pct%.*}
  context_info=" ctx:${used_int}%"
fi

# Build status line with ANSI colors matching Tokyo Night palette
# Colors are dimmed by Claude Code, so we use the original palette values
printf '\033[38;2;163;174;210m%s\033[0m' "$host" # hostname: #a3aed2
printf ' \033[38;2;72;127;235m%s\033[0m' "$dir"  # directory: #487feb

if [ -n "$git_branch" ]; then
  printf ' \033[38;2;118;159;240m%s\033[0m' " $git_branch" # git branch: #769ff0
fi

if [ -n "$model" ]; then
  printf ' \033[38;2;97;104;126m%s\033[0m' "[$model]" # model: #61687e
fi

if [ -n "$context_info" ]; then
  printf ' \033[38;2;160;169;203m%s\033[0m' "$context_info" # context: #a0a9cb
fi

printf '\n'
