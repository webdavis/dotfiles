#!/usr/bin/env bash

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

PRUNE_SUBJECT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/scripts/prune-merged-worktrees.sh"

fixture_git() {
  git -c user.email=fixture@example.invalid -c user.name=fixture \
    -c init.defaultBranch=main -c commit.gpgsign=false "$@"
}

function set_up_before_script() {
  PRUNE_ROOT="$(cd "$(mktemp -d)" && pwd -P)"
  export PRUNE_HERDR_LIST="$PRUNE_ROOT/herdr-worktrees.json"
  export PRUNE_HERDR_CALLS="$PRUNE_ROOT/herdr-calls"
  prune_install_herdr_stub "$PRUNE_ROOT/bin"

  prune_build_repository "$PRUNE_ROOT/sweep"
  prune_add_worktree "$PRUNE_ROOT/sweep" landed
  prune_add_worktree "$PRUNE_ROOT/sweep" in-flight
  fixture_git -C "$PRUNE_ROOT/sweep/in-flight" commit -q --allow-empty -m 'still working'
  prune_add_worktree "$PRUNE_ROOT/sweep" uncommitted
  printf 'edited\n' >"$PRUNE_ROOT/sweep/uncommitted/README"
  prune_add_worktree "$PRUNE_ROOT/sweep" regenerated
  printf '{"nodes":2}\n' >"$PRUNE_ROOT/sweep/regenerated/graphify-out/graph.json"
  fixture_git -C "$PRUNE_ROOT/sweep/repo" worktree add -q --detach "$PRUNE_ROOT/sweep/review" HEAD
  prune_publish_herdr_list \
    "$PRUNE_ROOT/sweep/landed" w1 \
    "$PRUNE_ROOT/sweep/in-flight" w2 \
    "$PRUNE_ROOT/sweep/uncommitted" w3 \
    "$PRUNE_ROOT/sweep/review" w4
  PRUNE_SWEEP_OUTPUT="$(prune_run "$PRUNE_ROOT/sweep")"
  PRUNE_SWEEP_CALLS="$(cat "$PRUNE_HERDR_CALLS")"

  prune_build_repository "$PRUNE_ROOT/preview"
  prune_add_worktree "$PRUNE_ROOT/preview" landed
  prune_add_worktree "$PRUNE_ROOT/preview" unmounted
  prune_move_worktree_aside "$PRUNE_ROOT/preview/unmounted"
  prune_publish_herdr_list "$PRUNE_ROOT/preview/landed" w1
  : >"$PRUNE_HERDR_CALLS"
  PRUNE_PREVIEW_OUTPUT="$(prune_run "$PRUNE_ROOT/preview" --dry-run)"
  PRUNE_PREVIEW_CALLS="$(cat "$PRUNE_HERDR_CALLS")"
}

function tear_down_after_script() {
  rm -rf "$PRUNE_ROOT"
}

prune_build_repository() {
  local case_directory=$1
  mkdir -p "$case_directory/repo/graphify-out"
  fixture_git init -q --bare "$case_directory/origin.git"
  fixture_git init -q "$case_directory/repo"
  printf 'readme\n' >"$case_directory/repo/README"
  printf '{"nodes":1}\n' >"$case_directory/repo/graphify-out/graph.json"
  fixture_git -C "$case_directory/repo" add -A
  fixture_git -C "$case_directory/repo" commit -q -m 'first'
  fixture_git -C "$case_directory/repo" remote add origin ../origin.git
  fixture_git -C "$case_directory/repo" push -q origin main
}

prune_add_worktree() {
  local case_directory=$1 branch=$2
  fixture_git -C "$case_directory/repo" worktree add -q -b "$branch" "$case_directory/$branch"
}

prune_move_worktree_aside() {
  local worktree_path=$1
  mv "$worktree_path" "$worktree_path-aside"
}

prune_install_herdr_stub() {
  local bin_directory=$1
  mkdir -p "$bin_directory"
  cat >"$bin_directory/herdr" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$PRUNE_HERDR_CALLS"
[[ ${1:-} == worktree ]] || exit 99
case "${2:-}" in
  list)
    cat "$PRUNE_HERDR_LIST"
    ;;
  remove)
    shift 2
    workspace_id=''
    while (($# > 0)); do
      case "$1" in
        --workspace)
          workspace_id="$2"
          shift 2
          ;;
        *) shift ;;
      esac
    done
    checkout_path="$(jq -r --arg id "$workspace_id" \
      '.result.worktrees[] | select(.open_workspace_id == $id) | .path' "$PRUNE_HERDR_LIST")"
    [[ -n $checkout_path ]] || exit 1
    git worktree remove --force "$checkout_path"
    ;;
  *) exit 99 ;;
esac
STUB
  chmod 755 "$bin_directory/herdr"
}

prune_publish_herdr_list() {
  jq -n --args '{result: {worktrees: [
    range(0; ($ARGS.positional | length); 2) as $pair |
    {path: $ARGS.positional[$pair], open_workspace_id: $ARGS.positional[$pair + 1],
     is_linked_worktree: true}]}}' "$@" >"$PRUNE_HERDR_LIST"
}

prune_run() {
  local case_directory=$1
  shift
  (cd "$case_directory/repo" &&
    PATH="$PRUNE_ROOT/bin:$PATH" env -u HERDR_ENV "$PRUNE_SUBJECT" "$@" 2>&1)
}

function test_a_merged_clean_worktree_is_removed_through_herdr() {
  assert_directory_not_exists "$PRUNE_ROOT/sweep/landed"
  assert_contains 'worktree remove --workspace w1 --force' "$PRUNE_SWEEP_CALLS"
}

function test_removing_a_worktree_leaves_its_branch_alone() {
  assert_same '  landed' "$(fixture_git -C "$PRUNE_ROOT/sweep/repo" branch --list landed)"
}

function test_an_unmerged_worktree_is_kept() {
  assert_directory_exists "$PRUNE_ROOT/sweep/in-flight"
  assert_contains 'in-flight is not merged into origin/main' "$PRUNE_SWEEP_OUTPUT"
}

function test_a_dirty_worktree_is_kept() {
  assert_directory_exists "$PRUNE_ROOT/sweep/uncommitted"
  assert_contains 'uncommitted has uncommitted changes' "$PRUNE_SWEEP_OUTPUT"
}

function test_a_worktree_dirty_only_in_the_graphify_artifact_is_removed() {
  assert_directory_not_exists "$PRUNE_ROOT/sweep/regenerated"
  assert_contains 'regenerated (regenerated, git)' "$PRUNE_SWEEP_OUTPUT"
}

function test_a_detached_worktree_is_kept() {
  assert_directory_exists "$PRUNE_ROOT/sweep/review"
  assert_contains 'detached at' "$PRUNE_SWEEP_OUTPUT"
}

function test_a_dry_run_removes_nothing() {
  assert_directory_exists "$PRUNE_ROOT/preview/landed"
  assert_contains 'would remove' "$PRUNE_PREVIEW_OUTPUT"
  assert_not_contains 'worktree remove' "$PRUNE_PREVIEW_CALLS"
}

function test_a_dry_run_keeps_the_metadata_of_an_absent_worktree() {
  assert_directory_exists "$PRUNE_ROOT/preview/repo/.git/worktrees/unmounted"
}

function test_an_unknown_argument_is_refused() {
  local exit_code=0
  prune_run "$PRUNE_ROOT/preview" --wat >/dev/null 2>&1 || exit_code=$?
  assert_same 2 "$exit_code"
  assert_directory_exists "$PRUNE_ROOT/preview/landed"
}
