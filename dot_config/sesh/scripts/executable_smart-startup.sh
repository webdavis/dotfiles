#!/usr/bin/env bash
# Smart startup dashboard for sesh sessions (v2 §4.2 simplified).
# Shows git status, Todoist tasks, and justfile recipes (if present);
# falls back to eza --tree.
# Invoked explicitly by the user (not via sesh's startup_command).

set -euo pipefail

dir="${1:-.}"
session_name="$(basename "$dir")"

# ANSI colors (avoid dependency on dedicated color libs).
CYAN='\033[0;36m'
DIM='\033[2m'
RESET='\033[0m'

header() {
  local label="$1"
  local pad
  pad=$(printf '─%.0s' $(seq 1 $((50 - ${#label}))))
  printf '\n%b── %s %b%s%b\n' "$CYAN" "$label" "$DIM" "$pad" "$RESET"
}

# ── Git ──────────────────────────────────────────
if [[ -d "$dir/.git" ]] || git -C "$dir" rev-parse --git-dir &>/dev/null; then
  header "Git"
  git -C "$dir" status -sb 2>/dev/null || true
fi

# ── Tasks (Todoist) ──────────────────────────────
td_output=""
case "$session_name" in
  casually-concerned)
    td_output=$(td task list --project "cc" --limit 5 2>/dev/null || true)
    ;;
  *)
    td_output=$(td task list -f "/$session_name" --limit 5 2>/dev/null || true)
    ;;
esac
if [[ -n $td_output ]]; then
  header "Tasks"
  echo "$td_output"
fi

# ── Project info (justfile only per v2) ──────────
if [[ -f "$dir/justfile" ]]; then
  header "Recipes (just)"
  just --summary --justfile "$dir/justfile" 2>/dev/null |
    tr ' ' '\n' | pr -3 -t -w80 2>/dev/null || true
else
  header "Files"
  eza --tree --level=1 --icons "$dir" 2>/dev/null || ls "$dir"
fi

echo ""
