#!/usr/bin/env bash
# The worktree sweep's decisions, taken against a scratch repository with a fake
# origin and a stubbed herdr.
#
# One repository holds one worktree in each state the sweep has to tell apart,
# and the subject runs over all of them ONCE, which is the situation it meets on
# this machine. Every behavior below then asks one question of that finished
# run: is this checkout still on disk, and did the line explaining it say the
# right thing. The removals are real, so a decision that fires when it should not
# takes a directory with it and the assertion sees it.
#
# Running once is also what keeps this file fast: a checkout, a fetch and a
# worktree add cost more than every assertion here put together, so paying for
# them per behavior would buy nothing but seconds.
#
# `git` runs through fixture_git, and the inherited GIT_* variables are unset at
# script scope: this suite runs from the pre-commit hook, and git exports GIT_DIR
# to every hook, which would point `git init` and the subject's own calls at THIS
# repository instead of at the fixture.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit.

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

PRUNE_SUBJECT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/dot_local/libexec/executable_prune-merged-worktrees.sh"

fixture_git() {
  git -c user.email=fixture@example.invalid -c user.name=fixture \
    -c init.defaultBranch=main -c commit.gpgsign=false "$@"
}

function set_up_before_script() {
  # A physical path: git reports one, and the herdr fixture has to agree with it
  # for a join on the checkout path to find anything.
  PRUNE_ROOT="$(cd "$(mktemp -d)" && pwd -P)"
  export PRUNE_HERDR_LIST="$PRUNE_ROOT/herdr-worktrees.json"
  export PRUNE_HERDR_CALLS="$PRUNE_ROOT/herdr-calls"
  prune_install_herdr_stub "$PRUNE_ROOT/bin"
  prune_build_repository "$PRUNE_ROOT/sweep"
  # One worktree per state the sweep has to tell apart. Only `regenerated` is
  # left out of the herdr list, which is what sends it down the plain git path.
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

  # The dry run needs a repository the sweep has not already emptied.
  prune_build_repository "$PRUNE_ROOT/preview"
  prune_add_worktree "$PRUNE_ROOT/preview" landed
  prune_publish_herdr_list "$PRUNE_ROOT/preview/landed" w1
  : >"$PRUNE_HERDR_CALLS"
  PRUNE_PREVIEW_OUTPUT="$(prune_run "$PRUNE_ROOT/preview" --dry-run)"
  PRUNE_PREVIEW_CALLS="$(cat "$PRUNE_HERDR_CALLS")"
}

function tear_down_after_script() {
  rm -rf "$PRUNE_ROOT"
}

prune_build_repository() { # <case directory>
  mkdir -p "$1/repo/graphify-out"
  fixture_git init -q --bare "$1/origin.git"
  fixture_git init -q "$1/repo"
  printf 'readme\n' >"$1/repo/README"
  printf '{"nodes":1}\n' >"$1/repo/graphify-out/graph.json"
  fixture_git -C "$1/repo" add -A
  fixture_git -C "$1/repo" commit -q -m 'first'
  # Relative, so the remote is the origin beside the checkout wherever it sits.
  fixture_git -C "$1/repo" remote add origin ../origin.git
  fixture_git -C "$1/repo" push -q origin main
}

prune_add_worktree() { # <case directory> <branch>
  fixture_git -C "$1/repo" worktree add -q -b "$2" "$1/$2"
}

# A herdr that answers `worktree list` from the published fixture and makes
# `worktree remove` really remove the checkout, the way the real one does.
prune_install_herdr_stub() { # <bin directory>
  mkdir -p "$1"
  cat >"$1/herdr" <<'STUB'
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
  chmod 755 "$1/herdr"
}

# Pairs of <checkout path> <workspace id>. A worktree left out of the list has no
# sidebar row, which is what sends the subject down the plain git path.
prune_publish_herdr_list() {
  jq -n --args '{result: {worktrees: [
    range(0; ($ARGS.positional | length); 2) as $pair |
    {path: $ARGS.positional[$pair], open_workspace_id: $ARGS.positional[$pair + 1],
     is_linked_worktree: true}]}}' "$@" >"$PRUNE_HERDR_LIST"
}

prune_run() { # <case directory> [argument ...]
  local case_directory="$1"
  shift
  (cd "$case_directory/repo" && PATH="$PRUNE_ROOT/bin:$PATH" HERDR_ENV=1 \
    "$PRUNE_SUBJECT" "$@" 2>&1)
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

function test_an_unknown_argument_is_refused() {
  local exit_code=0
  prune_run "$PRUNE_ROOT/preview" --wat >/dev/null 2>&1 || exit_code=$?
  assert_same 2 "$exit_code"
  assert_directory_exists "$PRUNE_ROOT/preview/landed"
}
