#!/usr/bin/env bash
# The worktree review launcher: what it lists, in what order, and what it
# leaves untouched.
#
# The launcher exists so an agent parked on main can pick one of several
# hundred sibling checkouts and start a review in it. Two properties make that
# usable and both are pinned here: the caller's working directory and HEAD are
# the same after a pick as before it, and a second resume starts the chosen
# review without walking the worktree list again.
#
# Every case runs against a throwaway repository with real worktrees, under a
# per-case sandbox state directory, with the picker and the review command
# replaced by recording stubs. Nothing reads the operator's state, nothing
# reaches herdr and nothing runs tuicr.
#
# Cases that change a worktree's contents create and remove their own
# worktree, because the suite runs shuffled and a leaked edit would decide
# another case's ordering assertion.
#
# bashunit SOURCES this file, so it carries no `set -euo pipefail` and no
# executable bit.
#
# assert_same, never assert_equals: bashunit's assert_equals normalizes ANSI
# and control characters away before comparing, and a branch name carrying one
# is a real defect that must not pass.

subject_under_test() {
  printf '%s' "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/dot_local/bin/executable_worktree-review.sh"
}

SUBJECT="$(subject_under_test)"

# pwd -P: git reports a worktree by its physical path, and macOS puts
# mktemp under the /var symlink, so a logical fixture path would never
# match what the launcher records.
fixture="$(cd "$(mktemp -d)" && pwd -P)"

tear_down_after_script() {
  rm -rf "$fixture"
}

# git exports GIT_DIR and friends to every hook, and this suite runs from the
# pre-commit hook, so a bare `git init` here would silently reach into this
# repository instead of the fixture.
fixture_git() {
  env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE -u GIT_COMMON_DIR \
    -u GIT_OBJECT_DIRECTORY \
    git -c user.email=fixture@example.invalid -c user.name=fixture \
    -c init.defaultBranch=main -c commit.gpgsign=false "$@"
}

# commit_at <dir> <epoch> <message>
commit_at() {
  GIT_AUTHOR_DATE="$2" GIT_COMMITTER_DATE="$2" \
    fixture_git -C "$1" commit --quiet --allow-empty -m "$3"
}

# add_worktree <name> <head epoch> : a branch off main with one commit of its
# own, dated so the ordering cases cannot pass on a name sort.
add_worktree() {
  fixture_git -C "$fixture/repo" worktree add --quiet -b "$1" "$fixture/wt/$1" >/dev/null
  commit_at "$fixture/wt/$1" "$2" "work on $1"
}

drop_worktree() {
  fixture_git -C "$fixture/repo" worktree remove --force "$fixture/wt/$1" >/dev/null 2>&1
  fixture_git -C "$fixture/repo" branch -D "$1" >/dev/null 2>&1
  return 0
}

build_repository() {
  local root="$fixture/repo"
  mkdir -p "$root"
  fixture_git init --quiet "$root"
  printf 'graphify-out/\n' >"$root/.gitignore"
  printf 'seed\n' >"$root/seed.txt"
  fixture_git -C "$root" add .gitignore seed.txt
  commit_at "$root" 1000000000 "seed"
  add_worktree alpha 1700000000
  add_worktree zulu 1700003600
  add_worktree mike 1700001800
}

build_repository

# --- stubs ------------------------------------------------------------------
#
# The picker stub echoes back one line of what it was offered, so a case
# controls the selection without a terminal. The review stub records its argv,
# its working directory and the origin variables it inherited.

mkdir -p "$fixture/bin"

cat >"$fixture/bin/picker" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
cat >"$PICKER_OFFERED"
: >"${PICKER_RAN:-/dev/null}"
sed -n "${PICKER_LINE:-1}p" "$PICKER_OFFERED"
STUB

cat >"$fixture/bin/review" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
{
  printf 'argv:%s\n' "$*"
  printf 'cwd:%s\n' "$PWD"
} >"${REVIEW_RECORD:-/dev/null}"
STUB

# The default review command, reachable only through PATH, so a case can prove
# the launcher runs tuicr when nothing names a command.
cp "$fixture/bin/review" "$fixture/bin/tuicr"

chmod +x "$fixture/bin/picker" "$fixture/bin/review" "$fixture/bin/tuicr"

# run_subject <args...> : runs the launcher from the repository root with this
# case's sandbox state directory. STDOUT lands in $OUT, stderr in $ERR and the
# status in $RC.
run_subject() {
  RC=0
  env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE -u GIT_COMMON_DIR \
    -u GIT_OBJECT_DIRECTORY \
    WORKTREE_REVIEW_STATE_DIR="$STATE" \
    WORKTREE_REVIEW_PICKER="$fixture/bin/picker" \
    WORKTREE_REVIEW_NOW=1700007200 \
    PICKER_OFFERED="$STATE.offered" \
    bash "$SUBJECT" "$@" >"$OUT" 2>"$ERR" || RC=$?
}

set_up() {
  local case_dir
  case_dir="$(mktemp -d)"
  STATE="$case_dir/state"
  OUT="$case_dir/out"
  ERR="$case_dir/err"
  RECORD="$case_dir/review-record"
  RAN="$case_dir/picker-ran"
  unset REVIEW_RECORD PICKER_RAN PICKER_LINE
  cd "$fixture/repo" || return 1
}

# A glob-looking pattern handed to bashunit's regex assertion makes grep error
# out, and that assertion reads a grep error as a pass, so every case below
# uses literal containment or an exact field comparison instead.
branch_order() {
  awk -v rows="${1:-99}" 'NR <= rows { printf "%s ", $2 }' "$OUT"
}

first_branch() {
  awk 'NR == 1 { print $2 }' "$OUT"
}

age_column() {
  awk '{ print $1 }' <<<"$1"
}

# The launcher keys its state by the repository's common git directory, so the
# generation file sits one slug deep inside the sandbox state directory.
cache_file() {
  find "$STATE" -name activity.tsv -print -quit
}

selection_file() {
  find "$STATE" -name selection -print -quit
}

row_for() {
  grep "	$fixture/wt/$1\$" "$OUT"
}

# --- cases ------------------------------------------------------------------

function test_the_listing_puts_the_most_recently_updated_worktree_first() {
  run_subject list
  assert_same 0 "$RC"
  assert_same "zulu mike alpha " "$(branch_order 3)"
}

function test_a_working_tree_edit_outranks_an_older_commit() {
  add_worktree quiet 1700002000
  printf 'edited\n' >"$fixture/wt/quiet/seed.txt"
  touch -t 202512310000 "$fixture/wt/quiet/seed.txt"
  run_subject list
  drop_worktree quiet
  assert_same "quiet" "$(first_branch)"
}

function test_the_listing_counts_edited_files_and_skips_ignored_ones() {
  add_worktree messy 1700002000
  printf 'dirty\n' >"$fixture/wt/messy/seed.txt"
  mkdir -p "$fixture/wt/messy/graphify-out"
  printf '{}\n' >"$fixture/wt/messy/graphify-out/other.json"
  printf 'new\n' >"$fixture/wt/messy/untracked.txt"
  run_subject list
  local row
  row="$(row_for messy)"
  drop_worktree messy
  assert_not_contains 'other.json' "$row"
  assert_contains '2 edited' "$row"
}

function test_the_post_commit_graph_artifact_is_not_an_edit() {
  add_worktree graphed 1700002000
  mkdir -p "$fixture/wt/graphed/graphify-out"
  printf '{"a":1}\n' >"$fixture/wt/graphed/graphify-out/graph.json"
  fixture_git -C "$fixture/wt/graphed" add -f graphify-out/graph.json
  commit_at "$fixture/wt/graphed" 1700002000 "track the graph"
  printf '{"a":2}\n' >"$fixture/wt/graphed/graphify-out/graph.json"
  run_subject list
  local row
  row="$(row_for graphed)"
  drop_worktree graphed
  assert_not_contains 'edited' "$row"
}

function test_a_deletion_is_dated_by_the_directory_the_unlink_stamped() {
  # The worktree's own commit is older than the clock the cases run against, so
  # a row dated "now" can only have come from the parent directory's mtime,
  # which the unlink moved to real wall-clock time.
  add_worktree pruned 1700002000
  rm "$fixture/wt/pruned/seed.txt"
  run_subject list
  local row
  row="$(row_for pruned)"
  drop_worktree pruned
  assert_contains '1 edited' "$row"
  assert_same "now" "$(age_column "$row")"
}

function test_a_renamed_file_counts_once() {
  # git reports a rename as one entry plus a second NUL record holding the
  # origin path; counting both would double every `git mv`.
  add_worktree renamed 1700002000
  fixture_git -C "$fixture/wt/renamed" mv seed.txt moved.txt
  run_subject list
  local row
  row="$(row_for renamed)"
  drop_worktree renamed
  assert_contains '1 edited' "$row"
}

function test_the_listing_counts_the_commits_the_default_branch_lacks() {
  run_subject list
  assert_contains '1 ahead' "$(row_for alpha)"
  assert_not_contains 'ahead' "$(grep '	'"$fixture"'/repo$' "$OUT")"
}

function test_picking_leaves_the_callers_directory_and_head_unchanged() {
  local before_cwd before_head
  before_cwd="$PWD"
  before_head="$(fixture_git -C "$fixture/repo" rev-parse HEAD)"
  run_subject pick
  assert_same 0 "$RC"
  assert_same "$before_cwd" "$PWD"
  assert_same "$before_head" "$(fixture_git -C "$fixture/repo" rev-parse HEAD)"
  assert_same "main" "$(fixture_git -C "$fixture/repo" rev-parse --abbrev-ref HEAD)"
  assert_same "$fixture/wt/zulu" "$(cat "$OUT")"
}

function test_a_resume_with_no_target_picks_one_and_starts_the_review_there() {
  export REVIEW_RECORD="$RECORD" PICKER_RAN="$RAN"
  run_subject resume -- "$fixture/bin/review" --flag
  assert_same 0 "$RC"
  assert_file_exists "$RAN"
  assert_same "argv:--flag" "$(sed -n 1p "$RECORD")"
  assert_same "cwd:$fixture/wt/zulu" "$(sed -n 2p "$RECORD")"
}

function test_a_second_resume_starts_the_review_without_repeating_discovery() {
  export REVIEW_RECORD="$RECORD"
  run_subject resume -- "$fixture/bin/review"
  export PICKER_RAN="$RAN"
  run_subject resume -- "$fixture/bin/review"
  assert_same 0 "$RC"
  assert_file_not_exists "$RAN"
  assert_same "cwd:$fixture/wt/zulu" "$(sed -n 2p "$RECORD")"
}

function test_a_resume_with_no_command_runs_tuicr() {
  export REVIEW_RECORD="$RECORD" PATH="$fixture/bin:$PATH"
  run_subject resume
  assert_same 0 "$RC"
  assert_same "argv:" "$(sed -n 1p "$RECORD")"
  assert_same "cwd:$fixture/wt/zulu" "$(sed -n 2p "$RECORD")"
}

function test_a_target_that_cannot_be_entered_is_refused() {
  # Fail closed: a recorded target that is still a directory but no longer
  # enterable must stop the launcher, never start the review somewhere else.
  local locked="$fixture/locked"
  mkdir -p "$locked"
  run_subject pick
  printf '%s\n' "$locked" >"$(selection_file)"
  chmod 000 "$locked"
  run_subject resume -- "$fixture/bin/review"
  chmod 755 "$locked"
  assert_not_same 0 "$RC"
  assert_contains 'cannot enter' "$(cat "$ERR")"
}

function test_a_refresh_publishes_a_whole_new_generation() {
  run_subject list
  local cache before_inode before_body
  cache="$(cache_file)"
  before_inode="$(stat -f '%i' "$cache")"
  before_body="$(cat "$cache")"
  commit_at "$fixture/wt/alpha" 1700007100 "a commit during the refresh"
  run_subject refresh
  fixture_git -C "$fixture/wt/alpha" reset --quiet --hard HEAD~1
  assert_same 0 "$RC"
  assert_not_same "$before_inode" "$(stat -f '%i' "$cache")"
  assert_not_same "$before_body" "$(cat "$cache")"
}

function test_a_selection_survives_a_refresh_that_reorders_the_list() {
  export REVIEW_RECORD="$RECORD"
  run_subject pick
  assert_same "$fixture/wt/zulu" "$(cat "$OUT")"
  commit_at "$fixture/wt/alpha" 1700007100 "a commit that takes the top row"
  run_subject refresh
  run_subject list
  assert_same "alpha" "$(first_branch)"
  run_subject resume -- "$fixture/bin/review"
  fixture_git -C "$fixture/wt/alpha" reset --quiet --hard HEAD~1
  assert_same "cwd:$fixture/wt/zulu" "$(sed -n 2p "$RECORD")"
}

function test_a_checkout_that_is_gone_leaves_the_listing() {
  add_worktree ghost 1700002000
  rm -rf "$fixture/wt/ghost"
  run_subject list
  fixture_git -C "$fixture/repo" worktree prune
  fixture_git -C "$fixture/repo" branch -D ghost >/dev/null 2>&1
  assert_same 0 "$RC"
  assert_not_contains 'ghost' "$(cat "$OUT")"
}

function test_the_bare_entry_of_a_bare_repository_is_not_a_row() {
  fixture_git clone --quiet --bare "$fixture/repo" "$fixture/bare.git" >/dev/null 2>&1
  fixture_git -C "$fixture/bare.git" worktree add --quiet "$fixture/barewt" main >/dev/null
  cd "$fixture/barewt" || return 1
  run_subject list
  assert_same 0 "$RC"
  assert_same 1 "$(grep -c . "$OUT")"
  assert_not_contains 'bare.git' "$(cat "$OUT")"
}

function test_a_checkout_marked_bare_still_reports_its_edits() {
  # Not a hypothetical: the dashboard spec's environment leak wrote core.bare =
  # true into this repository's own config on 2026-09-15, on a checkout with a
  # full working tree. Its own fixture repository, because core.bare lives in
  # the shared config and would reach every other case's worktree.
  local odd="$fixture/marked-bare"
  fixture_git init --quiet "$odd"
  printf 'seed\n' >"$odd/seed.txt"
  fixture_git -C "$odd" add seed.txt
  commit_at "$odd" 1700002000 "seed"
  fixture_git -C "$odd" config core.bare true
  printf 'dirty\n' >"$odd/seed.txt"
  cd "$odd" || return 1
  run_subject list
  assert_same 0 "$RC"
  assert_contains '1 edited' "$(cat "$OUT")"
  assert_same "" "$(cat "$ERR")"
}

function test_an_unknown_command_is_a_usage_error() {
  run_subject sprint
  assert_not_same 0 "$RC"
  assert_same "" "$(cat "$OUT")"
  assert_contains 'usage' "$(cat "$ERR")"
}
