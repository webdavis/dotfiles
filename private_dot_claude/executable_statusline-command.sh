#!/usr/bin/env bash
# Claude Code status line - inspired by Starship config at ~/.config/starship.toml
set -euo pipefail

input=$(cat)

cwd=$(echo "$input" | jq -r '.workspace.current_dir // .cwd // empty')
project_dir=$(echo "$input" | jq -r '.workspace.project_dir // empty')
model=$(echo "$input" | jq -r '.model.display_name // empty')
used_pct=$(echo "$input" | jq -r '.context_window.used_percentage // empty')
git_worktree=$(echo "$input" | jq -r '.workspace.git_worktree // empty')
fast_mode=$(echo "$input" | jq -r '.fast_mode // false')
thinking_on=$(echo "$input" | jq -r '.thinking.enabled // false')
over_200k=$(echo "$input" | jq -r '.exceeds_200k_tokens // false')
window_size=$(echo "$input" | jq -r '.context_window.context_window_size // empty')
cache_present=$(echo "$input" | jq -r 'if .prompt_cache == null then "no" else "yes" end')
cache_warm=$(echo "$input" | jq -r '.prompt_cache.warm // false')
cache_expires=$(echo "$input" | jq -r '.prompt_cache.expires_at // empty')
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
# The parentheses show where inside the project you are, and nothing when you
# sit at its root; a directory outside the project shows its full path.
if [[ -n $project_dir && $cwd == "$project_dir" ]]; then
  dir="$project"
elif [[ -n $project_dir && $cwd == "$project_dir"/* ]]; then
  dir="$project (${cwd#"$project_dir"/})"
else
  dir="$project (${cwd/#"$HOME"/\~})"
fi

# Git branch (skip lock to avoid interference)
git_branch=""
if git -C "$cwd" rev-parse --is-inside-work-tree &>/dev/null 2>&1; then
  git_branch=$(git -C "$cwd" --no-optional-locks symbolic-ref --short HEAD 2>/dev/null || git -C "$cwd" --no-optional-locks rev-parse --short HEAD 2>/dev/null || true)
fi

# Context usage bar
context_info=""
context_color='160;169;203' # #a0a9cb, calm
# Compaction estimate for Claude Code 2.1.263: reserve up to 20k output tokens,
# then 13k for the summary; a percentage override can only lower that ceiling.
# The statusline payload omits session-only /autocompact values and model-specific
# output defaults. Assume a 20k output reserve unless explicitly lowered, use
# the full model window when no supported window setting is visible, and mark
# the gauge with ~. This remains a percentage of the estimated compaction ceiling.
if [[ -n $used_pct ]]; then
  compact_settings='{}'
  settings_path="${CLAUDE_CONFIG_DIR:-$HOME/.claude}/settings.json"
  if [[ -r $settings_path ]]; then
    compact_settings=$(jq -c '{
      pct: .env.CLAUDE_AUTOCOMPACT_PCT_OVERRIDE,
      window: (.env.CLAUDE_CODE_AUTO_COMPACT_WINDOW // .autoCompactWindow),
      output: .env.CLAUDE_CODE_MAX_OUTPUT_TOKENS
    }' "$settings_path" 2>/dev/null || printf '{}')
  fi
  # An explicitly set environment value, including an empty one, wins over
  # the settings fallback. Parse decimal strings in jq, never as shell octal.
  context_values=$(printf '%s' "$compact_settings" | jq -r \
    --arg pct "${CLAUDE_AUTOCOMPACT_PCT_OVERRIDE-}" \
    --arg pct_set "${CLAUDE_AUTOCOMPACT_PCT_OVERRIDE+x}" \
    --arg window "${CLAUDE_CODE_AUTO_COMPACT_WINDOW-}" \
    --arg window_set "${CLAUDE_CODE_AUTO_COMPACT_WINDOW+x}" \
    --arg output "${CLAUDE_CODE_MAX_OUTPUT_TOKENS-}" \
    --arg output_set "${CLAUDE_CODE_MAX_OUTPUT_TOKENS+x}" \
    --argjson model_window "${window_size:-200000}" --argjson used "$used_pct" '
      def decimal:
        tostring | if test("^[0-9]+([.][0-9]+)?$") then tonumber else null end;
      (if $pct_set == "x" then $pct else .pct end | decimal) as $percentage |
      (if $window_set == "x" then $window else .window end | decimal) as $configured_window |
      (if $output_set == "x" then $output else .output end | decimal) as $configured_output |
      (if $configured_window > 0 then
        [([$configured_window | floor, 100000] | max), 1000000, $model_window] | min
       else $model_window end) as $capacity |
      (if $configured_output > 0 then [$configured_output | floor, 20000] | min
       else 20000 end) as $reserve |
      ($capacity - $reserve) as $effective |
      ($effective - 13000) as $default_ceiling |
      (if $percentage >= 1 and $percentage <= 100 then
        [($effective * $percentage / 100 | floor), $default_ceiling] | min
       else $default_ceiling end) as $ceiling |
      [($used * $model_window / $ceiling | floor), $ceiling] | @tsv')
  read -r gauge_pct ceiling <<<"$context_values"
  context_info=" ctx:~${gauge_pct}%"
  if ((ceiling >= 1000000)); then
    context_info+="/$((ceiling / 1000000))M"
  else
    context_info+="/$((ceiling / 1000))k"
  fi
  # Yellow at four fifths of the way to compaction, red at nine tenths.
  if ((gauge_pct >= 90)); then
    context_color='247;118;142' # #f7768e, red
  elif ((gauge_pct >= 80)); then
    context_color='224;175;104' # #e0af68, yellow
  fi
fi
# The 200k flag is a fixed line Claude Code draws regardless of window size; it
# only says something on a window larger than 200k (on a 200k window the red
# gauge already says it), so it is shown only then.
if [[ $over_200k == true && -n $window_size && $window_size -gt 200000 ]]; then
  context_info+=" 💰200k"
fi

# Prompt cache: a warm cache makes the next turn cheap; show how long it stays
# warm, or an empty circle once it has gone cold. Omitted when the input has
# no prompt_cache object at all (before the first API response).
cache_info=""
if [[ $cache_present == yes ]]; then
  if [[ $cache_warm == true && -n $cache_expires ]]; then
    cache_left=$((${cache_expires%.*} - $(date +%s)))
    if ((cache_left >= 3600)); then
      cache_info="cache ● $((cache_left / 3600))h$(((cache_left % 3600) / 60))m"
    elif ((cache_left > 0)); then
      cache_info="cache ● $((cache_left / 60))m"
    else
      cache_info="cache ○"
    fi
  else
    cache_info="cache ○"
  fi
fi

# Session flags: fast mode and extended thinking both change what a turn costs.
flags=""
[[ $fast_mode == true ]] && flags+="⚡"
[[ $thinking_on == true ]] && flags+="💭"

# Usage windows (5-hour session and weekly), rendered the way the Claude.ai
# usage page does: a ten-cell bar of the share USED, the percent, and when the
# window resets. The session window shows a countdown; the weekly window shows
# the weekday and clock time. The stdin JSON only carries rate_limits for
# Claude.ai subscribers, or behind a gateway, and only after the first API
# response, so both segments are omitted when the fields are absent. resets_at
# is Unix epoch seconds (documented).
usage_bar() {
  local pct="$1" filled bar=""
  filled=$(((pct + 5) / 10))
  ((filled > 10)) && filled=10
  local i
  for ((i = 0; i < 10; i++)); do
    if ((i < filled)); then bar+="▓"; else bar+="░"; fi
  done
  printf '%s' "$bar"
}
usage_window() {
  local used="$1" reset="$2" label="$3" style="$4"
  local pct when=""
  pct=$(jq -n --argjson used "$used" '$used | floor')
  if [[ -n $reset ]]; then
    local epoch="${reset%.*}"
    if [[ $style == countdown ]]; then
      local now secs
      now=$(date +%s)
      secs=$((epoch - now))
      if ((secs <= 0)); then
        when="resets now"
      elif ((secs < 3600)); then
        when="resets in $((secs / 60))m"
      else
        when="resets in $((secs / 3600))h$(((secs % 3600) / 60))m"
      fi
    else
      when="resets $(date -r "$epoch" +'%a %H:%M' 2>/dev/null || true)"
    fi
  fi
  local bar
  bar=$(usage_bar "$pct")
  # Color by share used: calm under 50, yellow from 50, red from 80.
  local color='97;104;126'
  if ((pct >= 80)); then color='247;118;142'; elif ((pct >= 50)); then color='224;175;104'; fi
  printf '\033[38;2;%sm' "$color"
  if [[ -n $when ]]; then
    printf '%s %s %s%% used, %s' "$label" "$bar" "$pct" "$when"
  else
    printf '%s %s %s%% used' "$label" "$bar" "$pct"
  fi
  printf '\033[0m'
}

rate_segments=()
if [[ -n $five_hour_used ]]; then
  rate_segments+=("$(usage_window "$five_hour_used" "$five_hour_reset" "5h" countdown)")
fi
if [[ -n $seven_day_used ]]; then
  rate_segments+=("$(usage_window "$seven_day_used" "$seven_day_reset" "week" clock)")
fi

rate_info=""
if [[ ${#rate_segments[@]} -eq 2 ]]; then
  rate_info=" ${rate_segments[0]}  ${rate_segments[1]}"
elif [[ ${#rate_segments[@]} -eq 1 ]]; then
  rate_info=" ${rate_segments[0]}"
fi

# Build status line with ANSI colors matching Tokyo Night palette
# Colors are dimmed by Claude Code, so we use the original palette values
printf '\033[38;2;163;174;210m%s\033[0m' "$host" # hostname: #a3aed2
printf ' \033[38;2;72;127;235m%s\033[0m' "$dir"  # directory: #487feb

if [[ -n $git_branch ]]; then
  branch_tag=" $git_branch"
  # A linked worktree gets a fork glyph so the pane says which checkout it is;
  # the worktree name is shown only when it differs from the branch.
  if [[ -n $git_worktree ]]; then
    if [[ $git_worktree == "$git_branch" ]]; then
      branch_tag="⑂ $git_branch"
    else
      branch_tag="⑂ $git_worktree ($git_branch)"
    fi
  fi
  printf ' \033[38;2;118;159;240m%s\033[0m' "$branch_tag" # git branch: #769ff0
fi

if [[ -n $model ]]; then
  model_tag="$model"
  if [[ -n $effort_level ]]; then
    model_tag="$model $effort_level"
  fi
  if [[ -n $flags ]]; then
    model_tag="$model_tag $flags"
  fi
  printf ' \033[38;2;97;104;126m%s\033[0m' "[$model_tag]" # model: #61687e
fi

if [[ -n $context_info ]]; then
  printf ' \033[38;2;%sm%s\033[0m' "$context_color" "$context_info" # context: calm, yellow or red
fi

if [[ -n $cache_info ]]; then
  printf '  \033[38;2;97;104;126m%s\033[0m' "$cache_info" # cache: #61687e
fi

if [[ -n $rate_info ]]; then
  printf ' %s' "$rate_info" # each window carries its own color
fi

printf '\n'
