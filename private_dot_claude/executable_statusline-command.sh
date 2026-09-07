#!/usr/bin/env bash
# Claude Code status line - inspired by Starship config at ~/.config/starship.toml
set -euo pipefail

input=$(cat)

cwd=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // empty')
project_dir=$(echo "$input" | jq -r '.workspace.project_dir // empty')
model=$(echo "$input" | jq -r '.model.display_name // empty')
used_pct=$(echo "$input" | jq -r '.context_window.used_percentage // empty')
# Effort level: the live session value from stdin when the model supports
# reasoning effort; otherwise fall back to the configured default in
# settings.json (chezmoi-enforced, so always present there).
effort_level=$(echo "$input" | jq -r '.effort.level // empty')
if [[ -z $effort_level && -r "$HOME/.claude/settings.json" ]]; then
  effort_level=$(jq -r '.effortLevel // empty' "$HOME/.claude/settings.json" 2>/dev/null || true)
fi
five_hour_used=$(echo "$input" | jq -r '.rate_limits.five_hour.used_percentage // empty')
five_hour_reset=$(echo "$input" | jq -r '.rate_limits.five_hour.resets_at // empty')
seven_day_used=$(echo "$input" | jq -r '.rate_limits.seven_day.used_percentage // empty')
seven_day_reset=$(echo "$input" | jq -r '.rate_limits.seven_day.resets_at // empty')

# Host
host=$(hostname -s)

# Directory: project name (basename of the project root, falling back to the
# working directory's own basename when project_dir is absent), with the
# actual working directory in parens, shortened with ~ for $HOME.
if [[ -n $project_dir ]]; then
  project=$(basename "$project_dir")
else
  project=$(basename "$cwd")
fi
cwd_display="${cwd/#"$HOME"/\~}"
dir="$project ($cwd_display)"

# Git branch (skip lock to avoid interference)
git_branch=""
if git -C "$cwd" rev-parse --is-inside-work-tree &>/dev/null 2>&1; then
  git_branch=$(git -C "$cwd" --no-optional-locks symbolic-ref --short HEAD 2>/dev/null || git -C "$cwd" --no-optional-locks rev-parse --short HEAD 2>/dev/null || true)
fi

# Context usage bar
context_info=""
if [[ -n $used_pct ]]; then
  used_int=${used_pct%.*}
  context_info=" ctx:${used_int}%"
fi

# Rate-limit remaining (5-hour and weekly windows). Claude Code's stdin JSON
# only carries rate_limits for Claude.ai subscribers, or behind a gateway
# spend limit; when a session's input has neither window, this segment is
# skipped rather than faked.
rate_limit_remaining() {
  local used="$1" reset="$2" label="$3"
  local remaining
  remaining=$(jq -n --argjson used "$used" '(100 - $used) | floor')
  if [[ -n $reset ]]; then
    printf '%s %s%%@%s' "$label" "$remaining" "$(date -r "${reset%.*}" +%H:%M 2>/dev/null)"
  else
    printf '%s %s%%' "$label" "$remaining"
  fi
}

rate_segments=()
if [[ -n $five_hour_used ]]; then
  rate_segments+=("$(rate_limit_remaining "$five_hour_used" "$five_hour_reset" "5h")")
fi
if [[ -n $seven_day_used ]]; then
  rate_segments+=("$(rate_limit_remaining "$seven_day_used" "$seven_day_reset" "wk")")
fi

rate_info=""
if [[ ${#rate_segments[@]} -eq 2 ]]; then
  rate_info=" ${rate_segments[0]} · ${rate_segments[1]}"
elif [[ ${#rate_segments[@]} -eq 1 ]]; then
  rate_info=" ${rate_segments[0]}"
fi

# Build status line with ANSI colors matching Tokyo Night palette
# Colors are dimmed by Claude Code, so we use the original palette values
printf '\033[38;2;163;174;210m%s\033[0m' "$host" # hostname: #a3aed2
printf ' \033[38;2;72;127;235m%s\033[0m' "$dir"  # directory: #487feb

if [[ -n $git_branch ]]; then
  printf ' \033[38;2;118;159;240m%s\033[0m' " $git_branch" # git branch: #769ff0
fi

if [[ -n $model ]]; then
  model_tag="$model"
  if [[ -n $effort_level ]]; then
    model_tag="$model $effort_level"
  fi
  printf ' \033[38;2;97;104;126m%s\033[0m' "[$model_tag]" # model: #61687e
fi

if [[ -n $context_info ]]; then
  printf ' \033[38;2;160;169;203m%s\033[0m' "$context_info" # context: #a0a9cb
fi

if [[ -n $rate_info ]]; then
  printf ' \033[38;2;97;104;126m%s\033[0m' "$rate_info" # rate limits: #61687e
fi

printf '\n'
