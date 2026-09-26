#!/usr/bin/env bash

set -euo pipefail

dotfiles_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# shellcheck source=.chezmoitemplates/cli-print-style-lib.sh.tmpl
source "$dotfiles_directory/.chezmoitemplates/cli-print-style-lib.sh.tmpl"

readonly script_name="${0##*/}"
readonly upstream_branch='origin/main'
readonly graphify_rewritten_path='graphify-out/graph.json'
readonly status_prefix_length=3
readonly short_commit_length=12
readonly end_of_worktree_record=''
readonly exit_success=0
readonly exit_failure=1
readonly exit_bad_arguments=2

dry_run=0
help_requested=0
unknown_argument=''
removed_count=0
kept_count=0
removal_failed=0
current_worktree_path=''
declare -A herdr_workspace_id_by_path=()

print_usage() {
  printf 'Usage: %s [--dry-run]\n' "$script_name"
  printf '\n'
  printf 'Remove every linked worktree of this repository whose HEAD is an ancestor\n'
  printf 'of %s and whose tree is clean. Branches are never deleted.\n' "$upstream_branch"
  printf '\n'
  printf '  -n, --dry-run  Print what would be removed and remove nothing.\n'
  printf '  -h, --help     Print this message.\n'
}

report_error() {
  local kind=$1 message=$2
  report_line "error[$kind]: $message" >&2
}

report_decision() {
  local decision=$1 worktree_path=$2 details=$3
  report_line "$(printf '%-12s %s (%s)' "$decision" "$worktree_path" "$details")"
}

parse_arguments() {
  local argument
  for argument in "$@"; do
    case "$argument" in
      -n | --dry-run) dry_run=1 ;;
      -h | --help)
        help_requested=1
        return 0
        ;;
      *)
        unknown_argument=$argument
        return 1
        ;;
    esac
  done
}

help_was_requested() {
  ((help_requested == 1))
}

is_dry_run() {
  ((dry_run == 1))
}

a_removal_failed() {
  ((removal_failed == 1))
}

herdr_is_installed() {
  command -v herdr >/dev/null 2>&1
}

jq_is_installed() {
  command -v jq >/dev/null 2>&1
}

current_worktree_root() {
  git rev-parse --show-toplevel 2>/dev/null
}

inside_a_git_worktree() {
  current_worktree_root >/dev/null
}

fetch_origin() {
  git fetch --prune --quiet origin
}

upstream_branch_exists() {
  git rev-parse --verify --quiet "$upstream_branch^{commit}" >/dev/null
}

list_open_herdr_worktrees() {
  herdr worktree list 2>/dev/null |
    jq -r '.result.worktrees[]? | select(.open_workspace_id) | [.path, .open_workspace_id] | @tsv'
}

herdr_record_is_complete() {
  local worktree_path=$1 workspace_id=$2
  [[ -n $worktree_path && -n $workspace_id ]]
}

load_herdr_workspace_ids() {
  local worktree_path workspace_id
  while IFS=$'\t' read -r worktree_path workspace_id; do
    if herdr_record_is_complete "$worktree_path" "$workspace_id"; then
      herdr_workspace_id_by_path["$worktree_path"]="$workspace_id"
    fi
  done < <(list_open_herdr_worktrees)
}

herdr_workspace_id_for() {
  local worktree_path=$1
  printf '%s' "${herdr_workspace_id_by_path[$worktree_path]:-}"
}

worktree_is_open_in_herdr() {
  local worktree_path=$1
  [[ -n $(herdr_workspace_id_for "$worktree_path") ]]
}

list_worktree_records() {
  git worktree list --porcelain
}

print_one_line_per_worktree() {
  local line worktree_path='' head_commit='' branch_reference=''
  while IFS= read -r line; do
    case "$line" in
      'worktree '*)
        worktree_path="${line#worktree }"
        head_commit=''
        branch_reference=''
        ;;
      'HEAD '*) head_commit="${line#HEAD }" ;;
      'branch '*) branch_reference="${line#branch }" ;;
      "$end_of_worktree_record")
        printf '%s\t%s\t%s\n' "$worktree_path" "$head_commit" "$branch_reference"
        ;;
    esac
  done
}

worktree_is_linked() {
  local worktree_path=$1
  [[ -f $worktree_path/.git ]]
}

worktree_is_current() {
  local worktree_path=$1
  [[ $worktree_path -ef $current_worktree_path ]]
}

worktree_is_detached() {
  local branch_reference=$1
  [[ -z $branch_reference ]]
}

commit_is_merged_upstream() {
  local commit=$1
  git merge-base --is-ancestor "$commit" "$upstream_branch" 2>/dev/null
}

list_uncommitted_changes() {
  local worktree_path=$1
  git -C "$worktree_path" status --porcelain
}

path_from_status_line() {
  local status_line=$1
  printf '%s' "${status_line:status_prefix_length}"
}

status_line_is_uncommitted_work() {
  local status_line=$1
  [[ $(path_from_status_line "$status_line") != "$graphify_rewritten_path" ]]
}

worktree_is_clean() {
  local worktree_path=$1
  local status_line
  while IFS= read -r status_line; do
    if status_line_is_uncommitted_work "$status_line"; then
      return 1
    fi
  done < <(list_uncommitted_changes "$worktree_path")
  return 0
}

branch_name_from_reference() {
  local branch_reference=$1
  printf '%s' "${branch_reference#refs/heads/}"
}

reason_to_keep_worktree() {
  local worktree_path=$1 head_commit=$2 branch_reference=$3
  local branch_name
  branch_name="$(branch_name_from_reference "$branch_reference")"
  if worktree_is_current "$worktree_path"; then
    printf 'the current worktree'
  elif worktree_is_detached "$branch_reference"; then
    printf 'detached at %s' "${head_commit:0:short_commit_length}"
  elif ! commit_is_merged_upstream "$head_commit"; then
    printf '%s is not merged into %s' "$branch_name" "$upstream_branch"
  elif ! worktree_is_clean "$worktree_path"; then
    printf '%s has uncommitted changes' "$branch_name"
  fi
}

reason_was_found() {
  local reason=$1
  [[ -n $reason ]]
}

removal_method_for() {
  local worktree_path=$1
  if worktree_is_open_in_herdr "$worktree_path"; then
    printf 'herdr %s' "$(herdr_workspace_id_for "$worktree_path")"
  else
    printf 'git'
  fi
}

report_removal() {
  local decision=$1 worktree_path=$2 branch_reference=$3
  local branch_name removal_method
  branch_name="$(branch_name_from_reference "$branch_reference")"
  removal_method="$(removal_method_for "$worktree_path")"
  report_decision "$decision" "$worktree_path" "$branch_name, $removal_method"
}

record_kept_worktree() {
  local worktree_path=$1 reason=$2
  report_decision keep "$worktree_path" "$reason"
  kept_count=$((kept_count + 1))
}

record_dry_run_removal() {
  local worktree_path=$1 branch_reference=$2
  report_removal 'would remove' "$worktree_path" "$branch_reference"
  removed_count=$((removed_count + 1))
}

record_removed_worktree() {
  local worktree_path=$1 branch_reference=$2
  report_removal remove "$worktree_path" "$branch_reference"
  removed_count=$((removed_count + 1))
}

record_failed_removal() {
  local worktree_path=$1 branch_reference=$2
  report_removal FAILED "$worktree_path" "$branch_reference"
  removal_failed=1
}

remove_worktree_with_herdr() {
  local worktree_path=$1
  local workspace_id
  workspace_id="$(herdr_workspace_id_for "$worktree_path")"
  herdr worktree remove --workspace "$workspace_id" --force >/dev/null 2>&1
}

remove_worktree_with_git() {
  local worktree_path=$1
  git worktree remove --force "$worktree_path" >/dev/null 2>&1
}

remove_worktree() {
  local worktree_path=$1
  if worktree_is_open_in_herdr "$worktree_path"; then
    remove_worktree_with_herdr "$worktree_path"
  else
    remove_worktree_with_git "$worktree_path"
  fi
}

attempt_removal() {
  local worktree_path=$1 branch_reference=$2
  if remove_worktree "$worktree_path"; then
    record_removed_worktree "$worktree_path" "$branch_reference"
  else
    record_failed_removal "$worktree_path" "$branch_reference"
  fi
}

decide_worktree() {
  local worktree_path=$1 head_commit=$2 branch_reference=$3
  local reason_to_keep
  reason_to_keep="$(reason_to_keep_worktree "$worktree_path" "$head_commit" "$branch_reference")"
  if reason_was_found "$reason_to_keep"; then
    record_kept_worktree "$worktree_path" "$reason_to_keep"
  elif is_dry_run; then
    record_dry_run_removal "$worktree_path" "$branch_reference"
  else
    attempt_removal "$worktree_path" "$branch_reference"
  fi
}

decide_every_linked_worktree() {
  local worktree_lines worktree_line worktree_path head_commit branch_reference
  mapfile -t worktree_lines < <(list_worktree_records | print_one_line_per_worktree)
  for worktree_line in "${worktree_lines[@]}"; do
    IFS=$'\t' read -r worktree_path head_commit branch_reference <<<"$worktree_line"
    if worktree_is_linked "$worktree_path"; then
      decide_worktree "$worktree_path" "$head_commit" "$branch_reference"
    fi
  done
}

deregister_worktrees_whose_directories_are_gone() {
  git worktree prune
}

report_summary() {
  if is_dry_run; then
    report_line "dry run: $removed_count would be removed, $kept_count kept."
  else
    report_line "removed $removed_count, kept $kept_count."
  fi
}

main() {
  if ! parse_arguments "$@"; then
    report_error unknown-argument "$unknown_argument"
    print_usage >&2
    exit "$exit_bad_arguments"
  fi
  if help_was_requested; then
    print_usage
    exit "$exit_success"
  fi
  report_section 'worktrees' 'prune merged'

  if ! inside_a_git_worktree; then
    report_error not-a-worktree 'not inside a git worktree.'
    exit "$exit_failure"
  fi
  current_worktree_path="$(current_worktree_root)"
  report_line "fetching origin..."
  if ! fetch_origin; then
    report_error fetch-failed 'could not fetch origin; nothing was removed.'
    exit "$exit_failure"
  fi
  if ! upstream_branch_exists; then
    report_error missing-upstream "$upstream_branch does not exist; nothing was removed."
    exit "$exit_failure"
  fi
  if herdr_is_installed; then
    if ! jq_is_installed; then
      report_error missing-tool 'jq is required to read the herdr worktree list.'
      exit "$exit_failure"
    fi
    load_herdr_workspace_ids
  fi

  report_line "checking worktrees against $upstream_branch..."
  decide_every_linked_worktree
  if ! is_dry_run; then
    deregister_worktrees_whose_directories_are_gone
  fi
  report_summary

  if a_removal_failed; then
    report_error removal-failed 'at least one worktree could not be removed.'
    exit "$exit_failure"
  fi
}

main "$@"
