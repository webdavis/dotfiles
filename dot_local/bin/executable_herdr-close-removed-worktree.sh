#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
usage: herdr-close-removed-worktree.sh [--dry-run] <worktree-path>
USAGE
  exit 1
}

fail() {
  local message="$1"
  echo "error: ${message}" >&2
  exit 1
}

require_commands() {
  local command_name
  for command_name in "$@"; do
    command -v "$command_name" >/dev/null || fail "${command_name} is required but not installed"
  done
}

inside_herdr() {
  [[ -n ${HERDR_SOCKET_PATH:-} ]]
}

list_panes() {
  herdr pane list | jq -c '.result.panes[]'
}

pane_field() {
  local field="$1"
  local pane="$2"
  jq -r ".${field}" <<<"$pane"
}

pane_is_under() {
  local pane="$1"
  local path="$2"
  jq -e --arg path "$path" '
    [.cwd, .foreground_cwd] | map(select(. != null))
    | any(. == $path or startswith($path + "/"))
  ' <<<"$pane" >/dev/null
}

pane_runs_this_script() {
  local pane="$1"
  [[ $(pane_field pane_id "$pane") == "${HERDR_PANE_ID:-}" ]]
}

panes_under() {
  local path="$1"
  local pane
  while IFS= read -r pane; do
    pane_is_under "$pane" "$path" || continue
    if pane_runs_this_script "$pane"; then
      echo "skipping the pane running this script: $(pane_field pane_id "$pane")" >&2
      continue
    fi
    echo "$pane"
  done < <(list_panes)
}

count_panes_in() {
  local field="$1"
  local id="$2"
  local panes="$3"
  jq -r --arg id "$id" "select(.${field} == \$id) | .pane_id" <<<"$panes" | wc -l | tr -d ' '
}

every_pane_of_matches() {
  local field="$1"
  local id="$2"
  local all_panes="$3"
  local matching_panes="$4"
  [[ $(count_panes_in "$field" "$id" "$all_panes") == "$(count_panes_in "$field" "$id" "$matching_panes")" ]]
}

target_for_pane() {
  local pane="$1"
  local all_panes="$2"
  local matching_panes="$3"
  local workspace_id tab_id
  workspace_id="$(pane_field workspace_id "$pane")"
  tab_id="$(pane_field tab_id "$pane")"

  if every_pane_of_matches workspace_id "$workspace_id" "$all_panes" "$matching_panes"; then
    echo "workspace ${workspace_id}"
  elif every_pane_of_matches tab_id "$tab_id" "$all_panes" "$matching_panes"; then
    echo "tab ${tab_id}"
  else
    echo "pane $(pane_field pane_id "$pane")"
  fi
}

targets_under() {
  local path="$1"
  local all_panes matching_panes pane
  all_panes="$(list_panes)"
  matching_panes="$(panes_under "$path")"
  [[ -n $matching_panes ]] || return 0

  while IFS= read -r pane; do
    target_for_pane "$pane" "$all_panes" "$matching_panes"
  done <<<"$matching_panes" | sort -u
}

close_target() {
  local kind="$1"
  local id="$2"
  local dry_run="$3"
  if [[ $dry_run == yes ]]; then
    echo "would close ${kind} ${id}"
    return
  fi
  herdr "$kind" close "$id" >/dev/null
  echo "closed ${kind} ${id}"
}

close_targets() {
  local dry_run="$1"
  local kind id
  while read -r kind id; do
    close_target "$kind" "$id" "$dry_run"
  done
}

main() {
  local dry_run="no"
  if [[ ${1:-} == --dry-run ]]; then
    dry_run="yes"
    shift
  fi
  [[ $# -eq 1 && -n $1 ]] || usage
  local path="${1%/}"

  inside_herdr || exit 0
  require_commands herdr jq
  targets_under "$path" | close_targets "$dry_run"
}

[[ ${BASH_SOURCE[0]} == "$0" ]] && main "$@"
