#!/usr/bin/env bash
#
# prune-merged-worktrees.sh -- remove every linked worktree of the current
# repository whose work has already landed, and leave every other one alone.
#
# A herdr worktree is a workspace as much as a checkout: the sidebar row and the
# directory are created together by `herdr worktree create`, so they have to go
# away together. Removing the directory with git alone leaves the row behind
# pointing at nothing, which is why a registered worktree is removed through
# `herdr worktree remove` and only an unregistered one falls back to git.
#
# Two conditions decide, and both are conservative:
#
#   - the worktree's HEAD is an ancestor of origin/main after a fetch, which is
#     true of a branch merged with `--merge` (the convention here) and false of
#     anything still in flight, and
#   - the tree is clean, ignoring graphify-out/graph.json, which the post-commit
#     graphify hook rewrites in place and which is therefore modified in most
#     worktrees that are otherwise finished.
#
# Everything else is kept and says why: a detached HEAD has no branch to check,
# an unmerged branch is still work, and a dirty tree holds the only copy of
# something. NO BRANCH IS EVER DELETED; neither removal path touches a ref, so a
# worktree removed here can be recreated from its branch.
set -euo pipefail

readonly UPSTREAM_REF='origin/main'
# Tracked, rewritten by the post-commit graphify hook, and never worth keeping a
# worktree alive for.
readonly IGNORED_DIRTY_PATH='graphify-out/graph.json'
readonly EXIT_USAGE=2

script_name="${0##*/}"
dry_run=0
removal_failed=0
# The worktree this run is standing in, which is never removed out from under
# the shell running the removal.
current_worktree_path=''

# Populated from `herdr worktree list` when this shell is inside herdr. A path
# that is absent from it has no sidebar row, so git can remove it directly.
declare -A workspace_id_by_path=()

usage() {
  printf 'Usage: %s [--dry-run]\n' "$script_name"
  printf '\n'
  printf 'Remove every linked worktree of this repository whose HEAD is an ancestor\n'
  printf 'of %s and whose tree is clean. Branches are never deleted.\n' "$UPSTREAM_REF"
  printf '\n'
  printf '  -n, --dry-run  Print what would be removed and remove nothing.\n'
  printf '  -h, --help     Print this message.\n'
}

fail() {
  printf '%s: %s\n' "$script_name" "$*" >&2
  exit 1
}

report() {
  printf '%-12s %s (%s)\n' "$1" "$2" "$3"
}

parse_arguments() {
  local argument
  for argument in "$@"; do
    case "$argument" in
      -n | --dry-run)
        dry_run=1
        ;;
      -h | --help)
        usage
        exit 0
        ;;
      *)
        printf '%s: unknown argument: %s\n' "$script_name" "$argument" >&2
        usage >&2
        exit "$EXIT_USAGE"
        ;;
    esac
  done
}

# The workspace id of every worktree herdr currently has open, keyed by checkout
# path. herdr lists the worktrees of every repository it knows, not just this
# one, which is why git owns the candidate set and this is only a lookup.
load_herdr_workspace_ids() {
  [[ -n ${HERDR_ENV:-} ]] || return 0
  command -v herdr >/dev/null 2>&1 || return 0
  command -v jq >/dev/null 2>&1 || fail 'jq is required to read the herdr worktree list'
  local worktree_path workspace_id
  while IFS=$'\t' read -r worktree_path workspace_id; do
    [[ -n $worktree_path && -n $workspace_id ]] || continue
    workspace_id_by_path["$worktree_path"]="$workspace_id"
  done < <(herdr worktree list 2>/dev/null |
    jq -r '.result.worktrees[]? | select(.open_workspace_id) | [.path, .open_workspace_id] | @tsv')
}

# The main checkout keeps a .git DIRECTORY; a linked worktree keeps a .git FILE
# holding a gitdir: pointer. Only the latter is ever a candidate.
worktree_is_linked() {
  [[ -f $1/.git ]]
}

branch_has_landed() {
  git merge-base --is-ancestor "$1" "$UPSTREAM_REF" 2>/dev/null
}

tree_is_clean() {
  local worktree_path="$1" status_line
  while IFS= read -r status_line; do
    # Porcelain v1: two status columns, a space, then the path.
    if [[ ${status_line:3} == "$IGNORED_DIRTY_PATH" ]]; then
      continue
    fi
    return 1
  done < <(git -C "$worktree_path" status --porcelain)
  return 0
}

remove_worktree() {
  local worktree_path="$1" branch_name="$2"
  local workspace_id="${workspace_id_by_path[$worktree_path]:-}"
  local removal_label
  local -a removal_command
  if [[ -n $workspace_id ]]; then
    removal_command=(herdr worktree remove --workspace "$workspace_id" --force)
    removal_label="herdr $workspace_id"
  else
    removal_command=(git worktree remove --force "$worktree_path")
    removal_label='git'
  fi
  if ((dry_run)); then
    report 'would remove' "$worktree_path" "$branch_name, $removal_label"
    return 0
  fi
  if "${removal_command[@]}" >/dev/null 2>&1; then
    report remove "$worktree_path" "$branch_name, $removal_label"
  else
    report FAILED "$worktree_path" "$branch_name, $removal_label"
    removal_failed=1
  fi
}

# One worktree record from `git worktree list --porcelain`, already parsed.
decide_worktree() {
  local worktree_path="$1" head="$2" branch_reference="$3"
  local branch_name="${branch_reference#refs/heads/}"
  worktree_is_linked "$worktree_path" || return 0
  if [[ $worktree_path -ef $current_worktree_path ]]; then
    report keep "$worktree_path" 'the current worktree'
    return 0
  fi
  if [[ -z $branch_reference ]]; then
    report keep "$worktree_path" "detached at ${head:0:12}"
    return 0
  fi
  if ! branch_has_landed "$head"; then
    report keep "$worktree_path" "$branch_name is not merged into $UPSTREAM_REF"
    return 0
  fi
  if ! tree_is_clean "$worktree_path"; then
    report keep "$worktree_path" "$branch_name has uncommitted changes"
    return 0
  fi
  remove_worktree "$worktree_path" "$branch_name"
}

# The record stream is read on fd 3, not stdin: `git worktree remove` runs from
# inside this loop, and a git command that inherited the stream could consume the
# records still waiting in it.
prune_worktrees() {
  local line worktree_path='' head='' branch_reference=''
  while IFS= read -r -u3 line; do
    case "$line" in
      'worktree '*)
        worktree_path="${line#worktree }"
        head=''
        branch_reference=''
        ;;
      'HEAD '*)
        head="${line#HEAD }"
        ;;
      'branch '*)
        branch_reference="${line#branch }"
        ;;
      '')
        # End of a record. A path that no longer exists is left to the final
        # `git worktree prune`, which is what the metadata needs.
        if [[ -n $worktree_path && -d $worktree_path ]]; then
          decide_worktree "$worktree_path" "$head" "$branch_reference"
        fi
        worktree_path=''
        ;;
    esac
  done 3< <(git worktree list --porcelain)
  if [[ -n $worktree_path && -d $worktree_path ]]; then
    decide_worktree "$worktree_path" "$head" "$branch_reference"
  fi
}

main() {
  parse_arguments "$@"
  current_worktree_path="$(git rev-parse --show-toplevel 2>/dev/null)" ||
    fail 'not inside a git worktree'
  git fetch --prune --quiet origin ||
    fail 'could not fetch origin; nothing was removed'
  git rev-parse --verify --quiet "$UPSTREAM_REF^{commit}" >/dev/null ||
    fail "$UPSTREAM_REF does not exist; nothing was removed"
  load_herdr_workspace_ids
  prune_worktrees
  # A bare `git worktree prune` deregisters immediately, so it is a write and
  # stays out of a dry run. The three-month grace is `gc.worktreePruneExpire`,
  # which `git gc` applies and this does not, so an unguarded call here would
  # deregister a worktree whose directory is only momentarily absent.
  ((dry_run)) || git worktree prune
  ((removal_failed == 0)) || fail 'at least one worktree could not be removed'
}

main "$@"
