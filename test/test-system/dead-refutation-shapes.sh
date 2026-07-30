#!/usr/bin/env bash
# shellcheck disable=SC2016 # fixture lines are literal bats source: their $vars expand at scan time, never here
# dead-refutation-shapes.sh. test/unit/no-dead-refutation-in-bats.sh must flag
# every position where a bare inverted command's status is DISCARDED inside a
# bats test body, not just an inversion sitting alone on a non-final line. The
# property (measured, see the guard's header): an inverted pipeline can fail
# the test only as the last command the body executes, because `set -e` and
# bats' ERR trap both ignore a `!` pipeline, and that exemption propagates
# through brace groups, if/loop bodies, and and-or lists -- but not through
# subshells, command substitutions, or function calls.
#
# The flagged tree plants one fixture per dead shape (mid-line `! cmd; other`,
# tail `other; ! cmd`, backgrounded, and-or tails, compound bodies, `time`
# prefix, continuations); the clean tree holds every live spelling the guard
# must leave alone (final-position inversions, conditions, `|| handler`
# consumption, subshells, [[ ! ... ]], heredoc text, quoted separators); the
# boundary tree pins the shapes the static scan is KNOWN to presume live or
# consumed (dead inversions inside a FINAL compound statement, case branches,
# process substitutions, function bodies), so a future widening or narrowing
# of the scan's scope shows up as a test change. This drives the guard against
# scratch fixture trees via its optional scan-root argument.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=helpers/find-repo-root.sh
source "$here/helpers/find-repo-root.sh"
# shellcheck source=helpers/capture-output.sh
source "$here/helpers/capture-output.sh"
# shellcheck source=helpers/report-test-failures.sh
source "$here/helpers/report-test-failures.sh"

REPO_ROOT="$(find_repo_root)" || exit 1
GUARD="$REPO_ROOT/test/unit/no-dead-refutation-in-bats.sh"

# Set in main; global so the EXIT trap can still see them after main returns.
flagged_root=""
clean_root=""
boundary_root=""
empty_root=""

# write_bats_file <root> <name> <line>... -- write <root>/test/integration/
# <name>.bats containing the given lines verbatim and print its SCAN-RELATIVE
# path (what the guard's report names). The fixture trees are scan targets
# only; nothing ever executes these bats files.
write_bats_file() {
  local root="$1" name="$2"
  shift 2
  local path="$root/test/integration/$name.bats"
  mkdir -p "$(dirname "$path")"
  printf '%s\n' "$@" >"$path"
  printf 'integration/%s.bats\n' "$name"
}

# write_test_body <root> <name> <body-line>... -- write a fixture whose line 1
# is the @test opener, so body line K sits on file line K+1.
write_test_body() {
  local root="$1" name="$2"
  shift 2
  write_bats_file "$root" "$name" "@test \"$name\" {" "$@" "}"
}

# run_guard <output-variable-name> <status-variable-name> <scanned-root>
run_guard() {
  local output_variable_name="$1" status_variable_name="$2"
  capture_output "$output_variable_name" "$status_variable_name" bash "$GUARD" "$3"
}

# Build the flagged tree's fixtures under $flagged_root and record each
# expected "path:line" in the caller-named associative array, keyed by fixture
# name. Every fixture holds exactly one dead inversion; the two decoys hold
# live final-position inversions that must stay unreported.
# shellcheck disable=SC2034 # nameref: every write lands in the caller's array
create_flagged_tree_fixtures() {
  local -n flagged_expected_destination="$1"

  # Shape 1 of the confirmed defect: the inversion is on the FINAL line but
  # another command follows it on that line, so its status is discarded.
  flagged_expected_destination["shape1_final_line"]="$(write_test_body "$flagged_root" shape1-final-line \
    '  touch "$BATS_TEST_TMPDIR/f"' \
    '  ! test -e "$BATS_TEST_TMPDIR/f"; true'):3"

  # Shape 2 of the confirmed defect: the inversion is last on its line but the
  # line is not final.
  flagged_expected_destination["shape2_tail_nonfinal"]="$(write_test_body "$flagged_root" shape2-tail-nonfinal \
    '  true; ! test -e /nope' \
    '  true'):2"

  # The shape the old guard already caught, kept as a regression fixture.
  flagged_expected_destination["own_line_mid"]="$(write_test_body "$flagged_root" own-line-mid \
    '  ! test -e /nope' \
    '  true'):2"

  # Backgrounding discards the status even in final position (measured: a
  # following `wait` does not recover it either).
  flagged_expected_destination["background_final"]="$(write_test_body "$flagged_root" background-final \
    '  ! test -e /nope &'):2"
  flagged_expected_destination["background_mid"]="$(write_test_body "$flagged_root" background-mid \
    '  ! test -e /nope &' \
    '  true'):2"

  # An inversion as the TAIL of an and-or list, mid-body: the list returns the
  # inverted status and the `!` exemption still applies.
  flagged_expected_destination["and_tail_mid"]="$(write_test_body "$flagged_root" and-tail-mid \
    '  true && ! test -e /nope' \
    '  true'):2"
  flagged_expected_destination["or_tail_mid"]="$(write_test_body "$flagged_root" or-tail-mid \
    '  false || ! test -e /nope' \
    '  true'):2"

  # An inversion on the LEFT of `&&` with no `||` after it: on violation the
  # list short-circuits to the discarded inverted status, so nothing can fail.
  flagged_expected_destination["and_left_mid"]="$(write_test_body "$flagged_root" and-left-mid \
    '  ! test -e /nope && echo why' \
    '  true'):2"

  # The `!` exemption propagates through brace groups and if/loop BODIES
  # (measured), so a mid-body compound cannot rescue the status.
  flagged_expected_destination["brace_group_mid"]="$(write_test_body "$flagged_root" brace-group-mid \
    '  { ! test -e /nope; }' \
    '  true'):2"
  flagged_expected_destination["if_body_mid_oneline"]="$(write_test_body "$flagged_root" if-body-mid-oneline \
    '  if true; then ! test -e /nope; fi' \
    '  true'):2"
  flagged_expected_destination["if_body_mid_multiline"]="$(write_test_body "$flagged_root" if-body-mid-multiline \
    '  if true; then' \
    '    ! test -e /nope' \
    '  fi' \
    '  true'):3"
  flagged_expected_destination["loop_body_mid"]="$(write_test_body "$flagged_root" loop-body-mid \
    '  for i in 1; do ! test -e /nope; done' \
    '  true'):2"

  # `time` is pipeline syntax, so `time ! cmd` mid-body is the same dead shape.
  flagged_expected_destination["time_prefix_mid"]="$(write_test_body "$flagged_root" time-prefix-mid \
    '  time ! test -e /nope' \
    '  true'):2"

  # The `!` inverts the WHOLE pipeline; mid-body it is still exempt.
  flagged_expected_destination["pipeline_mid"]="$(write_test_body "$flagged_root" pipeline-mid \
    '  ! grep -q x /etc/hosts | cat' \
    '  true'):2"

  # An inverted brace group is a `!` pipeline too.
  flagged_expected_destination["inverted_group_mid"]="$(write_test_body "$flagged_root" inverted-group-mid \
    '  ! { grep -q x /etc/hosts; }' \
    '  true'):2"

  # A backslash continuation is one statement, reported at its first line.
  # shellcheck disable=SC1003 # the trailing backslash is literal fixture text, not quote escaping
  flagged_expected_destination["continuation_mid"]="$(write_test_body "$flagged_root" continuation-mid \
    '  ! grep -q x \' \
    '    /etc/hosts' \
    '  true'):2"

  # `! [[ ... ]]` (bang OUTSIDE the brackets) is an inverted pipeline; only
  # `[[ ! ... ]]` fails via the [[ compound itself.
  flagged_expected_destination["negated_dbracket_mid"]="$(write_test_body "$flagged_root" negated-dbracket-mid \
    '  ! [[ -e /nope ]]' \
    '  true'):2"

  # setup() runs under the same mechanism, so a dead inversion there is the
  # same defect; the guard scans setup/teardown bodies too.
  flagged_expected_destination["setup_body_mid"]="$(write_bats_file "$flagged_root" setup-body-mid \
    'setup() {' \
    '  ! test -e /nope' \
    '  true' \
    '}' \
    '@test "t" {' \
    '  true' \
    '}'):2"

  # Live decoys inside the flagged tree: final-position inversions that must
  # stay OUT of the rejection report.
  flagged_expected_destination["decoy_final_tail"]="$(write_test_body "$flagged_root" decoy-final-tail \
    '  true' \
    '  true; ! test -e /nope')"
  flagged_expected_destination["decoy_final_alone"]="$(write_test_body "$flagged_root" decoy-final-alone \
    '  ! test -e /nope')"
}

# Build the clean tree's fixtures under $clean_root: every live or consumed
# spelling the guard must leave alone, plus the lexing hazards (heredocs,
# quotes, parameter expansions, escaped semicolons) that must not confuse the
# scan into a false positive.
create_clean_tree_fixtures() {
  local fixture_path
  fixture_path="$(write_test_body "$clean_root" final-alone \
    '  ! test -e /nope')"
  fixture_path="$(write_test_body "$clean_root" final-tail \
    '  true' \
    '  true; ! test -e /nope')"
  # shellcheck disable=SC1003 # the trailing backslash is literal fixture text, not quote escaping
  fixture_path="$(write_test_body "$clean_root" final-continuation \
    '  ! grep -q x \' \
    '    /etc/hosts')"
  fixture_path="$(write_test_body "$clean_root" final-comment \
    '  ! test -e /nope # the comment does not discard the status')"
  fixture_path="$(write_test_body "$clean_root" final-semicolon \
    '  ! test -e /nope;')"
  fixture_path="$(write_test_body "$clean_root" if-condition \
    '  if ! test -e /nope; then :; fi' \
    '  true')"
  fixture_path="$(write_test_body "$clean_root" elif-condition \
    '  if false; then :; elif ! test -e /nope; then :; fi' \
    '  true')"
  fixture_path="$(write_test_body "$clean_root" while-condition \
    '  while ! test -e /nope; do break; done' \
    '  true')"
  fixture_path="$(write_test_body "$clean_root" until-condition \
    '  until ! test -e /nope; do break; done' \
    '  true')"
  fixture_path="$(write_test_body "$clean_root" consumed-by-or-handler \
    '  ! test -e /nope || echo caught' \
    '  true')"
  fixture_path="$(write_test_body "$clean_root" consumed-by-later-or \
    '  ! test -e /nope && echo held || echo violated' \
    '  true')"
  # Subshells and command substitutions surface the status to errexit at the
  # enclosing simple command (measured live).
  fixture_path="$(write_test_body "$clean_root" subshell-mid \
    '  ( ! true )' \
    '  true')"
  fixture_path="$(write_test_body "$clean_root" command-substitution-mid \
    '  x="$(! true)"' \
    '  true')"
  fixture_path="$(write_test_body "$clean_root" eval-quoted-mid \
    "  eval '! true'" \
    '  true')"
  # [[ ! ... ]] fails via the [[ compound, which errexit does see.
  fixture_path="$(write_test_body "$clean_root" dbracket-negations-mid \
    '  [[ ! -e /nope && ! -e /also ]]' \
    '  true')"
  # bats' run consumes the status by design.
  fixture_path="$(write_test_body "$clean_root" run-wrapper-mid \
    '  run ! test -e /nope' \
    '  true')"
  # Heredoc text is data, not statements.
  fixture_path="$(write_test_body "$clean_root" heredoc-mid \
    "  cat >\"\$BATS_TEST_TMPDIR/s\" <<'EOF'" \
    '! true; true' \
    'true; ! true' \
    'EOF' \
    '  true')"
  fixture_path="$(write_test_body "$clean_root" heredoc-dash-mid \
    '  cat <<-EOF >/dev/null' \
    "$(printf '\t')! not a statement" \
    "$(printf '\t')EOF" \
    '  true')"
  # Separators inside quotes are not separators.
  fixture_path="$(write_test_body "$clean_root" quoted-separators \
    "  echo 'x; ! y' >/dev/null" \
    "  ! grep -qF 'a;b' /etc/hosts")"
  fixture_path="$(write_test_body "$clean_root" nested-quotes-in-substitution \
    '  v="$(printf "%s" "a;b")"' \
    '  ! test -e "$v"')"
  # find's escaped \; is an argument, not a separator.
  fixture_path="$(write_test_body "$clean_root" escaped-semicolon \
    '  find /var/empty -maxdepth 0 -name x -exec true {} \;' \
    '  ! test -e /nope')"
  # A single-command refute helper defined in the body: the CALL is live.
  fixture_path="$(write_test_body "$clean_root" refute-helper-in-body \
    '  refute_x() { ! grep -q x /etc/hosts; }' \
    '  refute_x')"
  # Final compound statements are presumed live (each branch here IS live).
  fixture_path="$(write_test_body "$clean_root" else-branch-final \
    '  if false; then true; else ! true; fi')"
  fixture_path="$(write_test_body "$clean_root" case-branch-final \
    '  true' \
    '  case x in x) ! true ;; esac')"
  # Parameter expansion braces and separators must not corrupt the scan.
  fixture_path="$(write_test_body "$clean_root" parameter-expansion-in-final-if \
    '  if true; then' \
    '    x="${y:-a;b}"' \
    '    ! test -e "$x"' \
    '  fi')"
  # A test NAME containing a bang must not open an inversion site.
  fixture_path="$(write_bats_file "$clean_root" bang-in-test-name \
    '@test "a name with ! inside" {' \
    '  ! test -e /nope' \
    '}')"
  # A clean setup body: final-position inversion is live there too.
  fixture_path="$(write_bats_file "$clean_root" setup-final-clean \
    'setup() {' \
    '  ! test -e /nope' \
    '}' \
    '@test "t" {' \
    '  true' \
    '}')"
  : "$fixture_path"
}

# Build the boundary tree under $boundary_root: shapes that are DEAD in
# reality but that the static scan deliberately presumes live or consumed,
# pinned so a scope change shows up here.
create_boundary_tree_fixtures() {
  local fixture_path
  # Inside the body's FINAL compound statement the scan presumes the inversion
  # live, because which inner command runs last is data-dependent; these two
  # are the dead variants of that presumption.
  fixture_path="$(write_test_body "$boundary_root" final-group-inner-dead \
    '  true' \
    '  { ! true; true; }')"
  fixture_path="$(write_test_body "$boundary_root" final-if-inner-dead \
    '  true' \
    '  if true; then' \
    '    ! true' \
    '    true' \
    '  fi')"
  # case bodies are scanned opaquely (their `)` patterns defeat the lexer's
  # paren tracking), so a dead branch inversion passes.
  fixture_path="$(write_test_body "$boundary_root" case-branch-mid-dead \
    '  case x in x) ! true ;; esac' \
    '  true')"
  # Parenthesized contexts are presumed consumed; a process substitution
  # discards the status yet passes.
  fixture_path="$(write_test_body "$boundary_root" process-substitution-mid-dead \
    '  cat <(! true) >/dev/null' \
    '  true')"
  # Function BODIES are exempt (the call is what matters, and the common
  # single-command helper is live); a multi-command body hiding a dead
  # inversion passes.
  fixture_path="$(write_test_body "$boundary_root" function-body-inner-dead \
    '  f() { ! true; true; }' \
    '  f')"
  # File-scope helper functions are outside the scanned bodies entirely.
  fixture_path="$(write_bats_file "$boundary_root" file-scope-helper-inner-dead \
    'helper() {' \
    '  ! true' \
    '  true' \
    '}' \
    '@test "t" {' \
    '  helper' \
    '}')"
  : "$fixture_path"
}

# The flagged tree is rejected, each dead shape is named at its exact
# path:line, and the live decoys stay unreported.
assert_flagged_tree_rejected() {
  local -n flagged_expected_reference="$1"
  local guard_output guard_status key expectation
  run_guard guard_output guard_status "$flagged_root/test"
  if [[ $guard_status -eq 0 ]]; then
    record_failure "flagged tree (every dead-inversion shape) was NOT rejected (guard exit 0)"
    return 0
  fi
  for key in "${!flagged_expected_reference[@]}"; do
    expectation="${flagged_expected_reference[$key]}"
    case "$key" in
      decoy_*)
        if grep -qF "$expectation" <<<"$guard_output"; then
          record_failure "live decoy $key was wrongly reported: $guard_output"
        fi
        ;;
      *)
        grep -qF "$expectation" <<<"$guard_output" ||
          record_failure "dead shape $key not reported at $expectation: $guard_output"
        ;;
    esac
  done
}

# A tree of only live or consumed spellings exits 0.
assert_clean_tree_passes() {
  local guard_output guard_status
  run_guard guard_output guard_status "$clean_root/test"
  [[ $guard_status -eq 0 ]] ||
    record_failure "clean tree (live/consumed spellings) was wrongly rejected: $guard_output"
}

# The documented limits stay limits: the boundary tree passes.
assert_boundary_tree_passes() {
  local guard_output guard_status
  run_guard guard_output guard_status "$boundary_root/test"
  [[ $guard_status -eq 0 ]] ||
    record_failure "documented boundary shapes must pass (scope change?): $guard_output"
}

# A root with no bats files at all is a green no-op.
assert_empty_tree_passes() {
  local guard_output guard_status
  mkdir -p "$empty_root/test/integration"
  run_guard guard_output guard_status "$empty_root/test"
  [[ $guard_status -eq 0 ]] ||
    record_failure "empty tree was wrongly rejected: $guard_output"
}

# Fail-closed: a python3 tool error must FAIL the guard, never yield an empty
# report and a green pass. The exported function shadows python3 inside the
# guard child only.
assert_python_failure_fails_guard() {
  local guard_output guard_status
  set +e
  guard_output="$(
    # shellcheck disable=SC2329,SC2317 # invoked indirectly: exported into the guard child
    python3() { return 7; }
    export -f python3
    bash "$GUARD" "$clean_root/test" 2>&1
  )"
  guard_status=$?
  set -e
  [[ $guard_status -ne 0 ]] ||
    record_failure "python3 failure (exit 7) did not fail the guard (fails open)"
}

main() {
  [[ -x $GUARD ]] || {
    printf 'dead-refutation-shapes: guard missing or not executable: %s\n' "$GUARD" >&2
    exit 1
  }

  flagged_root="$(mktemp -d)"
  clean_root="$(mktemp -d)"
  boundary_root="$(mktemp -d)"
  empty_root="$(mktemp -d)"
  trap 'rm -rf "$flagged_root" "$clean_root" "$boundary_root" "$empty_root"' EXIT

  # shellcheck disable=SC2034 # filled and read through namerefs by name
  local -A flagged_expected=()
  create_flagged_tree_fixtures flagged_expected
  create_clean_tree_fixtures
  create_boundary_tree_fixtures

  assert_flagged_tree_rejected flagged_expected
  assert_clean_tree_passes
  assert_boundary_tree_passes
  assert_empty_tree_passes
  assert_python_failure_fails_guard

  report_failures dead-refutation-shapes
}

main "$@"
