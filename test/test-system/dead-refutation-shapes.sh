#!/usr/bin/env bash
# shellcheck disable=SC2016 # fixture lines are literal bats source: their $vars expand at scan time, never here
# dead-refutation-shapes.sh. test/unit/no-dead-refutation-in-bats.sh must flag
# every position where a bare inverted command's status is DISCARDED inside a
# bats test body, not just an inversion sitting alone on a non-final line. The
# property (measured, see the guard's header): the status of an inverted
# command must REACH something that can act on it, because `set -e` and bats'
# ERR trap both ignore a `!` pipeline, and that exemption propagates through
# brace groups, if/loop bodies, and and-or lists -- but not through subshells,
# command substitutions, or function calls.
#
# The guard has two obligations and this suite exercises both. COVERAGE: every
# body bats executes must actually be analyzed, so the trees below cover each
# spelling of a body definition bats accepts, both file suffixes a body can
# live in, symlinked and unreadable directories, and the structures that used
# to desync the scan. JUDGEMENT: inside an analyzed body, only a status that
# genuinely reaches a consumer is left alone.
#
# The flagged tree plants one fixture per dead shape, each pinned at its exact
# path, line AND reason; the clean tree holds every live spelling the guard
# must leave alone plus the lexing hazards (heredocs, quotes, parameter
# expansions, escaped semicolons, a file with no trailing newline) that must
# not confuse the scan into a false positive; the boundary tree pins the shapes
# the static scan is KNOWN to presume live; and the refusal table pins the
# input it refuses to read at all. The last two are keyed by the bracketed
# limit identifiers in the guard's header, and
# assert_documented_limits_are_all_pinned diffs the union against that header
# so no limit can be documented without a fixture or pinned without being
# documented. This drives the guard against scratch fixture trees via its
# optional scan-root argument.
#
# Two things here are MEASURED rather than asserted from wording, because a
# pin that restates the code cannot fail with it: the liveness of every
# spelling the guard's failure message recommends (run under a bats-faithful
# harness with the refutation violated), and the fixture shapes themselves,
# each of which was checked against the defect it exists to catch. A fixture
# that passes identically with and without the fix pins nothing, which is
# exactly what a balanced case...esac fixture did here before.
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

# The guard's three rejection reasons, mirrored so every flagged-shape
# expectation pins WHICH one the guard printed. Location alone is not enough: a
# guard that reports the right line under the wrong reason has stopped telling
# the reader why the status vanished, and an expectation of `path:line` is
# satisfied by any reason at all.
REASON_BACKGROUNDED='backgrounded'
REASON_DISCARDED='status discarded'
REASON_DISCARDED_IN_CONDITION='discarded in condition list'

# Helper names that are legal in bash but are NOT POSIX identifiers, one per
# name class the guard's own comment enumerates (verified with
# `bash -c '<name>() { :; }'`). Both trees build a fixture per class from this
# one list, so a class cannot be covered in the direction that is easy to get
# right (the clean tree, where a missed definition is a false positive) while
# the direction that fails open (the flagged tree, where a missed definition
# closes the body early and hides everything after it) is left to one name.
# KEEP THE KEYS QUOTED: shfmt reads an unquoted associative-array subscript as
# arithmetic and would rewrite `[leading-digit]` into `[leading - digit]`.
declare -A BASH_LEGAL_HELPER_NAMES=(
  ["hyphenated"]='refute-x'
  ["dotted"]='refute.x'
  ["colon"]='refute:x'
  ["leading-digit"]='2fa'
)

# The functions bats executes around a test, from bats-core's documented
# lifecycle: setup/teardown per test, setup_file/teardown_file per file,
# setup_suite/teardown_suite per suite. A dead refutation in any of them is the
# same defect as one in a @test body.
#
# This is the WANTED set and it is deliberately INDEPENDENT of the guard's list,
# which it is diffed against. Deriving the fixtures from the guard's own list
# instead was measured to be worthless: dropping two names from the guard
# dropped their fixtures with them and the suite stayed green, which is the very
# fail-open the check was added to catch.
BATS_EXECUTED_BODY_NAMES=(
  setup teardown setup_file teardown_file setup_suite teardown_suite
)

# The spellings the guard's failure message tells people to use, and the
# near-miss it must warn about. Liveness is MEASURED here rather than taken
# from the message's wording: each runnable line is the spelling with its
# refutation VIOLATED, so running it answers the only question that matters,
# can this spelling fail. A guard that recommends a shape the shell discards
# trains people to switch it off, which is exactly how `|| echo ...` came to be
# recommended by a guard whose whole purpose is catching refutations that
# cannot fail. The three recommendation arrays are index-aligned and their
# lengths are asserted, as are the two discouraged ones.
RECOMMENDED_ADVICE_MARKERS=(
  'if cmd; then echo "why this is wrong"; false; fi'
  'call a single-command refute helper'
  '|| { echo "why this is wrong"; false; }'
)
RECOMMENDED_ADVICE_FIXTURE_LINES=(
  '  if grep -q zzz /etc/hosts; then echo why; false; fi'
  '  refute_absent() { ! grep -q zzz /etc/hosts; }; refute_absent'
  '  ! grep -q zzz /etc/hosts || { echo why; false; }'
)
RECOMMENDED_ADVICE_RUNNABLE_LINES=(
  'if true; then echo why; false; fi'
  'refute_absent() { ! true; }; refute_absent'
  '! true || { echo why; false; }'
)

# The spelling the message must name as unable to fail, in marker and runnable
# form. `|| echo ` is the substring that separates it from the recommended
# `|| { echo ...; false; }`, which is what lets the recommendation region be
# checked for its ABSENCE.
DISCOURAGED_ADVICE_MARKERS=('|| echo ')
DISCOURAGED_ADVICE_RUNNABLE_LINES=('! true || echo why')

# Where the guard's advice splits into regions. The recommendation and the
# warning are separate lines so that trading their contents cannot be hidden: a
# message recommending `|| echo` and warning about the handler that fails still
# contains every substring of the honest one, so a whole-output substring
# search cannot tell the two apart.
ADVICE_RECOMMENDATION_LEAD_IN='Give the status somewhere to go, and make that somewhere FAIL:'
ADVICE_WARNING_LEAD_IN='cannot fail the test'

# Set in main; global so the EXIT trap can still see them after main returns.
# The refusal fixtures do not appear here: each is built into a private root
# that its own loop iteration creates and removes, because several of them are
# deliberately unreadable and must not outlive the check that uses them.
flagged_root=""
clean_root=""
boundary_root=""
empty_root=""
symlink_root=""

# write_scan_fixture <root> <scan-relative-path> <line>... -- write
# <root>/test/<path> containing the given lines verbatim and print the
# SCAN-RELATIVE path (what the guard's report names). Used directly when the
# FILENAME is load-bearing (a .bash helper, a non-scanned suffix); most
# fixtures go through write_bats_file. The fixture trees are scan targets only;
# nothing ever executes these files.
write_scan_fixture() {
  local root="$1" relative_path="$2"
  shift 2
  local path="$root/test/$relative_path"
  mkdir -p "$(dirname "$path")"
  printf '%s\n' "$@" >"$path"
  printf '%s\n' "$relative_path"
}

# write_bats_file <root> <name> <line>... -- the common case: an integration
# suite .bats file.
write_bats_file() {
  local root="$1" name="$2"
  shift 2
  write_scan_fixture "$root" "integration/$name.bats" "$@"
}

# write_test_body <root> <name> <body-line>... -- write a fixture whose line 1
# is the @test opener, so body line K sits on file line K+1.
write_test_body() {
  local root="$1" name="$2"
  shift 2
  write_bats_file "$root" "$name" "@test \"$name\" {" "$@" "}"
}

# write_fixture_without_trailing_newline <root> <name> <line>... -- the same,
# minus the final newline. A file that stops mid-token used to abort the whole
# scan with a Python traceback, because peeking past the end returns the empty
# string and the empty string is a substring of every character class.
write_fixture_without_trailing_newline() {
  local root="$1" name="$2"
  shift 2
  local path="$root/test/integration/$name.bats" line first=1
  mkdir -p "$(dirname "$path")"
  : >"$path"
  for line in "$@"; do
    if [[ $first -eq 1 ]]; then
      first=0
      printf '%s' "$line" >>"$path"
    else
      printf '\n%s' "$line" >>"$path"
    fi
  done
  printf 'integration/%s.bats\n' "$name"
}

# run_guard <output-variable-name> <status-variable-name> <scanned-root>
run_guard() {
  local output_variable_name="$1" status_variable_name="$2"
  capture_output "$output_variable_name" "$status_variable_name" bash "$GUARD" "$3"
}

# expect_finding <scan-relative-path> <line> <reason> -- the exact prefix the
# guard prints for one finding, location AND reason. Pinning both is what makes
# the expectation fail when two reasons are collapsed into one string.
expect_finding() {
  printf '%s:%s  [%s]' "$1" "$2" "$3"
}

# measure_refutation_liveness <body-line>... -- run the lines the way bats runs
# a test body (a function under errexit whose return status is the verdict) and
# print live when that status can fail the test, dead when it cannot. This is
# the ground truth the advice assertions rest on, so the message's honesty is
# measured instead of pattern-matched. The model was validated against the
# repo's bats 1.11.1 over a 98-shape corpus with zero disagreements, and
# assert_liveness_measurement_discriminates re-checks it against a known-live
# and a known-dead control on every run.
measure_refutation_liveness() {
  local script
  script="$(printf '%s\n' 'set -eET -o pipefail' 'bats_test_body() {' "$@" '}' 'bats_test_body')"
  if bash -c "$script" >/dev/null 2>&1; then
    printf 'dead\n'
  else
    printf 'live\n'
  fi
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
  flagged_expected_destination["shape1_final_line"]="$(expect_finding "$(write_test_body "$flagged_root" shape1-final-line \
    '  touch "$BATS_TEST_TMPDIR/f"' \
    '  ! test -e "$BATS_TEST_TMPDIR/f"; true')" 3 "$REASON_DISCARDED")"

  # Shape 2 of the confirmed defect: the inversion is last on its line but the
  # line is not final.
  flagged_expected_destination["shape2_tail_nonfinal"]="$(expect_finding "$(write_test_body "$flagged_root" shape2-tail-nonfinal \
    '  true; ! test -e /nope' \
    '  true')" 2 "$REASON_DISCARDED")"

  # The shape the old guard already caught, kept as a regression fixture.
  flagged_expected_destination["own_line_mid"]="$(expect_finding "$(write_test_body "$flagged_root" own-line-mid \
    '  ! test -e /nope' \
    '  true')" 2 "$REASON_DISCARDED")"

  # Backgrounding discards the status even in final position (measured: a
  # following `wait` does not recover it either).
  flagged_expected_destination["background_final"]="$(expect_finding "$(write_test_body "$flagged_root" background-final \
    '  ! test -e /nope &')" 2 "$REASON_BACKGROUNDED")"
  flagged_expected_destination["background_mid"]="$(expect_finding "$(write_test_body "$flagged_root" background-mid \
    '  ! test -e /nope &' \
    '  true')" 2 "$REASON_BACKGROUNDED")"

  # An inversion as the TAIL of an and-or list, mid-body: the list returns the
  # inverted status and the `!` exemption still applies.
  flagged_expected_destination["and_tail_mid"]="$(expect_finding "$(write_test_body "$flagged_root" and-tail-mid \
    '  true && ! test -e /nope' \
    '  true')" 2 "$REASON_DISCARDED")"
  flagged_expected_destination["or_tail_mid"]="$(expect_finding "$(write_test_body "$flagged_root" or-tail-mid \
    '  false || ! test -e /nope' \
    '  true')" 2 "$REASON_DISCARDED")"

  # An inversion on the LEFT of `&&` with no `||` after it: on violation the
  # list short-circuits to the discarded inverted status, so nothing can fail.
  flagged_expected_destination["and_left_mid"]="$(expect_finding "$(write_test_body "$flagged_root" and-left-mid \
    '  ! test -e /nope && echo why' \
    '  true')" 2 "$REASON_DISCARDED")"

  # The `!` exemption propagates through brace groups and if/loop BODIES
  # (measured), so a mid-body compound cannot rescue the status.
  flagged_expected_destination["brace_group_mid"]="$(expect_finding "$(write_test_body "$flagged_root" brace-group-mid \
    '  { ! test -e /nope; }' \
    '  true')" 2 "$REASON_DISCARDED")"
  flagged_expected_destination["if_body_mid_oneline"]="$(expect_finding "$(write_test_body "$flagged_root" if-body-mid-oneline \
    '  if true; then ! test -e /nope; fi' \
    '  true')" 2 "$REASON_DISCARDED")"
  flagged_expected_destination["if_body_mid_multiline"]="$(expect_finding "$(write_test_body "$flagged_root" if-body-mid-multiline \
    '  if true; then' \
    '    ! test -e /nope' \
    '  fi' \
    '  true')" 3 "$REASON_DISCARDED")"
  flagged_expected_destination["loop_body_mid"]="$(expect_finding "$(write_test_body "$flagged_root" loop-body-mid \
    '  for i in 1; do ! test -e /nope; done' \
    '  true')" 2 "$REASON_DISCARDED")"

  # Only the LAST command of an if/while/until condition list decides the
  # compound, so an inversion in any earlier command of that list is discarded
  # (measured: both of these pass under bats with the refutation violated).
  flagged_expected_destination["condition_nonfinal_if"]="$(expect_finding "$(write_test_body "$flagged_root" condition-nonfinal-if \
    '  if ! test -e /nope; true; then :; fi' \
    '  true')" 2 "$REASON_DISCARDED_IN_CONDITION")"
  flagged_expected_destination["condition_nonfinal_while"]="$(expect_finding "$(write_test_body "$flagged_root" condition-nonfinal-while \
    '  while ! test -e /nope; false; do break; done' \
    '  true')" 2 "$REASON_DISCARDED_IN_CONDITION")"
  flagged_expected_destination["condition_nonfinal_multiline"]="$(expect_finding "$(write_test_body "$flagged_root" condition-nonfinal-multiline \
    '  if ! test -e /nope' \
    '     true' \
    '  then' \
    '    :' \
    '  fi' \
    '  true')" 2 "$REASON_DISCARDED_IN_CONDITION")"

  # `time` is pipeline syntax, so `time ! cmd` mid-body is the same dead shape.
  flagged_expected_destination["time_prefix_mid"]="$(expect_finding "$(write_test_body "$flagged_root" time-prefix-mid \
    '  time ! test -e /nope' \
    '  true')" 2 "$REASON_DISCARDED")"

  # The `!` inverts the WHOLE pipeline; mid-body it is still exempt.
  flagged_expected_destination["pipeline_mid"]="$(expect_finding "$(write_test_body "$flagged_root" pipeline-mid \
    '  ! grep -q x /etc/hosts | cat' \
    '  true')" 2 "$REASON_DISCARDED")"

  # An inverted brace group is a `!` pipeline too.
  flagged_expected_destination["inverted_group_mid"]="$(expect_finding "$(write_test_body "$flagged_root" inverted-group-mid \
    '  ! { grep -q x /etc/hosts; }' \
    '  true')" 2 "$REASON_DISCARDED")"

  # A backslash continuation is one statement, reported at its first line.
  # shellcheck disable=SC1003 # the trailing backslash is literal fixture text, not quote escaping
  flagged_expected_destination["continuation_mid"]="$(expect_finding "$(write_test_body "$flagged_root" continuation-mid \
    '  ! grep -q x \' \
    '    /etc/hosts' \
    '  true')" 2 "$REASON_DISCARDED")"

  # `! [[ ... ]]` (bang OUTSIDE the brackets) is an inverted pipeline; only
  # `[[ ! ... ]]` fails via the [[ compound itself.
  flagged_expected_destination["negated_dbracket_mid"]="$(expect_finding "$(write_test_body "$flagged_root" negated-dbracket-mid \
    '  ! [[ -e /nope ]]' \
    '  true')" 2 "$REASON_DISCARDED")"

  # Every function bats executes around a test runs under the same mechanism, so
  # a dead inversion in any of them is the same defect. One fixture per name in
  # BATS_EXECUTED_BODY_NAMES, which is the wanted set and NOT the guard's own
  # list: a name dropped from the guard leaves this fixture in place and
  # unreported. The suite variants live in setup_suite.bash, which is where bats
  # requires them and which is also what proves the .bash suffix is scanned.
  local body_name body_fixture
  for body_name in "${BATS_EXECUTED_BODY_NAMES[@]}"; do
    if [[ $body_name == *_suite ]]; then
      body_fixture="$(write_scan_fixture "$flagged_root" "integration/$body_name/setup_suite.bash" \
        "$body_name() {" \
        '  ! test -e /nope' \
        '  true' \
        '}')"
    else
      body_fixture="$(write_bats_file "$flagged_root" "body-$body_name" \
        "$body_name() {" \
        '  ! test -e /nope' \
        '  true' \
        '}' \
        '@test "t" {' \
        '  true' \
        '}')"
    fi
    flagged_expected_destination["body_name_$body_name"]="$(expect_finding "$body_fixture" 2 "$REASON_DISCARDED")"
  done

  # Every spelling of a body definition bash and bats accept reaches the same
  # analysis. A brace on the next line, the `function` keyword, and the spaced
  # parens are all ordinary bash; skipping any of them skips a whole body.
  flagged_expected_destination["setup_brace_next_line"]="$(expect_finding "$(write_bats_file "$flagged_root" setup-brace-next-line \
    'setup()' \
    '{' \
    '  ! test -e /nope' \
    '  true' \
    '}' \
    '@test "t" {' \
    '  true' \
    '}')" 3 "$REASON_DISCARDED")"
  flagged_expected_destination["setup_function_keyword"]="$(expect_finding "$(write_bats_file "$flagged_root" setup-function-keyword \
    'function teardown {' \
    '  ! test -e /nope' \
    '  true' \
    '}' \
    '@test "t" {' \
    '  true' \
    '}')" 2 "$REASON_DISCARDED")"
  flagged_expected_destination["setup_spaced_parens"]="$(expect_finding "$(write_bats_file "$flagged_root" setup-spaced-parens \
    'setup_file ()' \
    '{' \
    '  ! test -e /nope' \
    '  true' \
    '}' \
    '@test "t" {' \
    '  true' \
    '}')" 3 "$REASON_DISCARDED")"

  # bats' SECOND documented test syntax (bats-preprocess BATS_TEST_PATTERN_
  # COMMENT): the declaration is a comment, which the lexer strips, so it has
  # to be recognized from the raw line.
  flagged_expected_destination["comment_syntax_test"]="$(expect_finding "$(write_bats_file "$flagged_root" comment-syntax-test \
    'refutes_the_thing() { # @test' \
    '  ! test -e /nope' \
    '  true' \
    '}')" 2 "$REASON_DISCARDED")"

  # A `case` used as an ARGUMENT must not move the case tracker. It once did,
  # and the desync froze brace counting, so every remaining body in the file
  # went unanalyzed with no diagnostic. The dead inversion here sits AFTER the
  # case, and is only reachable if the region closed correctly.
  #
  # KEEP THE KEYWORD-SHAPED WORDS UNBALANCED. This fixture used to carry one
  # stray `case` and one stray `esac`, which a word counter that cannot tell a
  # keyword from an argument gets RIGHT by cancellation: the fixture passed
  # identically against the tracker and against the defect it exists to catch,
  # so it pinned nothing. With a lone stray `case`, the naive count never
  # returns to zero, the region never closes, and this expectation fails.
  flagged_expected_destination["case_argument_resync"]="$(expect_finding "$(write_test_body "$flagged_root" case-argument-resync \
    '  case x in' \
    '    x) grep -q case /etc/hosts || true ;;' \
    '  esac' \
    '  ! test -e /nope' \
    '  true')" 5 "$REASON_DISCARDED")"

  # A `}` in ARGUMENT position is a literal brace, not a group closer
  # (measured: `bash -c "{ echo }; }"` prints a brace). Counting it as a closer
  # ended the body early and reread every later line as unscanned file scope,
  # so the dead inversion below was reported clean -- the guard's own
  # fail-open. The inversion is reachable only while the brace stays an
  # argument.
  flagged_expected_destination["argument_close_brace"]="$(expect_finding "$(write_test_body "$flagged_root" argument-close-brace \
    '  printf "%s" } >/dev/null' \
    '  ! test -e /nope' \
    '  true')" 3 "$REASON_DISCARDED")"

  # An empty array assignment is NOT a function definition, so the brace group
  # after it is an ordinary group and the dead inversion inside it must still
  # be reported. This is the shape rule's other side: a rule loose enough to
  # read `x=()` as a definition would exempt every group that follows one.
  flagged_expected_destination["empty_array_assignment"]="$(expect_finding "$(write_test_body "$flagged_root" empty-array-assignment \
    '  x=()' \
    '  { ! grep -q x /etc/hosts; }' \
    '  true')" 3 "$REASON_DISCARDED")"

  # A helper whose name is legal in bash but not a POSIX identifier must still
  # be recognized as a DEFINITION, or its opening `{` stops being an opener
  # while its closing `}` still closes something -- and what it closes is the
  # BODY, so everything after it goes unscanned with no diagnostic. The dead
  # inversion below sits after the call, where only a correctly recognized
  # definition leaves it visible. This is the coverage half of the same defect
  # whose judgement half (a false positive on the helper's own inversion) the
  # clean tree pins, and it runs over every name class rather than one, because
  # one name class covered is not the same as the rule covered.
  local helper_name_class
  for helper_name_class in "${!BASH_LEGAL_HELPER_NAMES[@]}"; do
    flagged_expected_destination["helper_name_$helper_name_class"]="$(expect_finding "$(write_test_body "$flagged_root" "helper-name-$helper_name_class" \
      "  ${BASH_LEGAL_HELPER_NAMES[$helper_name_class]}() {" \
      '    ! grep -q x /etc/hosts' \
      '  }' \
      "  ${BASH_LEGAL_HELPER_NAMES[$helper_name_class]}" \
      '  ! test -e /nope' \
      '  true')" 6 "$REASON_DISCARDED")"
  done

  # A brace-shaped case PATTERN is a pattern, not a brace (`case "}" in }) ...`
  # matches, measured). Counting it would close the body inside the region and
  # leave the rest of it unscanned, so the dead inversion after `esac` is
  # reachable only while the region stays opaque to the brace count.
  flagged_expected_destination["case_brace_pattern"]="$(expect_finding "$(write_test_body "$flagged_root" case-brace-pattern \
    '  case x in' \
    '    }) : ;;' \
    '    *) : ;;' \
    '  esac' \
    '  ! test -e /nope' \
    '  true')" 6 "$REASON_DISCARDED")"

  # A file that stops mid-token used to abort the entire scan.
  flagged_expected_destination["no_trailing_newline"]="$(expect_finding "$(write_fixture_without_trailing_newline "$flagged_root" no-trailing-newline \
    '@test "t" {' \
    '  ! test -e /nope' \
    '  true' \
    '}' \
    'true;')" 2 "$REASON_DISCARDED")"

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
  # The inversion IS the final command of the condition list, spread over two
  # lines and joined by &&: the whole and-or list decides the compound, so no
  # element of it is discarded.
  fixture_path="$(write_test_body "$clean_root" condition-final-andor \
    '  if true && ! test -e /nope; then :; fi' \
    '  true')"
  fixture_path="$(write_test_body "$clean_root" condition-final-multiline \
    '  if true' \
    '     ! test -e /nope' \
    '  then' \
    '    :' \
    '  fi' \
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
  # A single-command refute helper defined in the body: the CALL is live. All
  # four definition spellings mean the same thing to bash, so all four must be
  # recognized; a spelling the scan does not know reads as a bare brace group
  # and its inversion is reported as dead, which is a FALSE POSITIVE against
  # working code.
  fixture_path="$(write_test_body "$clean_root" refute-helper-in-body \
    '  refute_x() { ! grep -q x /etc/hosts; }' \
    '  refute_x')"
  fixture_path="$(write_test_body "$clean_root" refute-helper-spaced-parens \
    '  refute_x () {' \
    '    ! grep -q x /etc/hosts' \
    '  }' \
    '  refute_x')"
  fixture_path="$(write_test_body "$clean_root" refute-helper-function-keyword \
    '  function refute_x {' \
    '    ! grep -q x /etc/hosts' \
    '  }' \
    '  refute_x')"
  fixture_path="$(write_test_body "$clean_root" refute-helper-brace-next-line \
    '  refute_x()' \
    '  {' \
    '    ! grep -q x /etc/hosts' \
    '  }' \
    '  refute_x')"
  # bash's function names are much wider than a POSIX identifier: each name in
  # BASH_LEGAL_HELPER_NAMES defines a function and each call is live (measured
  # with `bash -c "<name>() { :; }"`). A scan that recognizes only
  # [A-Za-z_][A-Za-z0-9_]* reads these bodies as bare brace groups and reports
  # their inversions dead, a FALSE POSITIVE against working code that also
  # contradicts the header's [function-body] limit.
  #
  # KEEP THE HELPER BODY AT TWO COMMANDS. With ONE, this fixture pinned nothing:
  # an unrecognized definition does not become a false positive, it becomes a
  # silent early close, because the helper's `}` closes the BODY instead and the
  # single inversion is then the body's own final statement, which is presumed
  # live. Measured: all four name classes passed identically with and without
  # the fix. The second command is what makes the inversion non-final in that
  # misreading, so an unrecognized name is reported and this fixture bites.
  local name_class
  for name_class in "${!BASH_LEGAL_HELPER_NAMES[@]}"; do
    fixture_path="$(write_test_body "$clean_root" "refute-helper-$name_class-name" \
      "  ${BASH_LEGAL_HELPER_NAMES[$name_class]}() {" \
      '    ! grep -q x /etc/hosts' \
      '    true' \
      '  }' \
      "  ${BASH_LEGAL_HELPER_NAMES[$name_class]}")"
  done
  # A `{` in ARGUMENT position is a literal brace, not a group opener, so it
  # must neither open a frame nor make the body look unclosed. Refusing here
  # was the fail-open's mirror image: same misreading, louder outcome.
  fixture_path="$(write_test_body "$clean_root" argument-open-brace \
    '  printf "%s" { >/dev/null' \
    '  ! test -e /nope')"
  # Final compound statements are presumed live (each branch here IS live).
  fixture_path="$(write_test_body "$clean_root" else-branch-final \
    '  if false; then true; else ! true; fi')"
  fixture_path="$(write_test_body "$clean_root" case-branch-final \
    '  true' \
    '  case x in x) ! true ;; esac')"
  # A case PATTERN spelled `case` is not a keyword; the region must still
  # close, leaving the final `true` as the body's last statement. The dead
  # inversion sits in the branch guarded by that keyword-shaped PATTERN, so
  # closing the region there would expose it as an ordinary non-final statement
  # and report it: the fixture is clean only while the pattern is read as a
  # pattern.
  #
  # KEEP THE KEYWORD-SHAPED WORDS UNBALANCED, for the reason recorded on the
  # flagged tree's case-argument-resync fixture: a stray `case` paired with a
  # stray `esac` cancels out, and a word counter that cannot tell a keyword
  # from a pattern then reaches the same verdict as the tracker.
  fixture_path="$(write_test_body "$clean_root" case-pattern-shaped-like-keywords \
    '  case x in' \
    '    case) ! test -e /nope ;;' \
    '    *) : ;;' \
    '  esac' \
    '  true')"
  # An `esac` in ARGUMENT position must not close the region either. The
  # inversion after it is still inside the branch, so it stays unreported
  # (limit [case-body]); a tracker that closed at the argument would read it as
  # an ordinary non-final statement and report it.
  fixture_path="$(write_test_body "$clean_root" case-esac-argument-in-branch \
    '  case x in' \
    '    x) echo esac >/dev/null; ! test -e /nope ;;' \
    '  esac' \
    '  true')"
  # A nested case inside a branch body must nest, not close the outer region.
  # The inversion sits AFTER the nested esac and still inside the outer branch,
  # so a tracker that closed the whole region at the inner `esac` would read it
  # as an ordinary non-final statement and report it. Without that inversion
  # the fixture passed whether the tracker nested or not.
  fixture_path="$(write_test_body "$clean_root" case-nested \
    '  case x in' \
    '    x) case y in y) true ;; esac; ! test -e /nope ;;' \
    '  esac' \
    '  true')"
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
  # A clean setup body: final-position inversion is live there too, in every
  # definition spelling.
  fixture_path="$(write_bats_file "$clean_root" setup-final-clean \
    'setup() {' \
    '  ! test -e /nope' \
    '}' \
    '@test "t" {' \
    '  true' \
    '}')"
  fixture_path="$(write_bats_file "$clean_root" setup-final-clean-brace-next-line \
    'setup()' \
    '{' \
    '  ! test -e /nope' \
    '}' \
    '@test "t" {' \
    '  true' \
    '}')"
  # bats' comment test syntax, final position: live, so unreported.
  fixture_path="$(write_bats_file "$clean_root" comment-syntax-final \
    'refutes_the_thing() { # @test' \
    '  ! test -e /nope' \
    '}')"
  # A file with no trailing newline must still be analyzed, not crash the scan.
  fixture_path="$(write_fixture_without_trailing_newline "$clean_root" clean-no-trailing-newline \
    '@test "t" {' \
    '  ! test -e /nope' \
    '}' \
    'true;')"
  # bats anchors @test to the start of a line, so a @test WORD in argument
  # position is not a declaration. Treating it as one opens a body at the wrong
  # brace, which either swallows the real @test below it or, for the trailing
  # decoy, opens a body that never closes at all. The TRAILING decoy is what
  # makes this fixture discriminate: a decoy that only precedes a real body can
  # be absorbed into it and still come out clean.
  fixture_path="$(write_bats_file "$clean_root" at-test-word-in-argument-position \
    'printf "%s\n" @test "x" { >/dev/null' \
    '@test "t" {' \
    '  ! test -e /nope' \
    '}' \
    'printf "%s\n" @test "y" { >/dev/null')"

  # The five fixtures below pin PRECISION rules: each names a way the guard
  # decides something is NOT a bats body. Loosening any of them costs nothing in
  # coverage and turns the guard into a false accuser against ordinary code,
  # which is why each was measured to pass here and to be reported once the rule
  # is loosened. They are all clean because the shape is legitimate; the dead
  # inversion inside each one is what the loosened rule would report.
  #
  # bats' comment test syntax requires the comment to END at `@test`, so a
  # mention of @test in a longer comment is just a comment on a file-scope
  # helper (limit [file-scope-helper]).
  fixture_path="$(write_bats_file "$clean_root" at-test-mentioned-in-a-comment \
    'helper() { # @test is mentioned in this comment' \
    '  ! true' \
    '  true' \
    '}' \
    '@test "t" {' \
    '  true' \
    '}')"
  # A body function name must match EXACTLY. A helper whose name merely starts
  # with one is an ordinary file-scope helper.
  fixture_path="$(write_bats_file "$clean_root" body-name-prefix-is-not-a-body \
    'setup_the_harness() {' \
    '  ! true' \
    '  true' \
    '}' \
    '@test "t" {' \
    '  true' \
    '}')"
  # `setup {` is the command `setup` with the argument `{`, not a definition:
  # bash needs parens or the `function` keyword. (Not runnable bash, and that is
  # the point -- the closing brace matches nothing. The fixture trees are scan
  # targets only.)
  fixture_path="$(write_bats_file "$clean_root" bare-name-brace-is-not-a-definition \
    'setup {' \
    '  ! true' \
    '  true' \
    '}' \
    '@test "t" {' \
    '  true' \
    '}')"
  # bats anchors the opening brace of a @test to the DECLARATION LINE
  # (BATS_TEST_PATTERN in bats-preprocess), so a brace on the next line means
  # bats never made a test out of this and neither may the guard.
  fixture_path="$(write_bats_file "$clean_root" at-test-brace-on-the-next-line \
    '@test "not a declaration without the brace"' \
    '{' \
    '  ! true' \
    '  true' \
    '}')"
  # A line ending in a continuation operator continues the SAME statement, so
  # the inversion below is still the body's final statement and still live.
  # Reading the newline as a statement end reports it. `||` cannot be pinned
  # this way and is not claimed to be: an inversion followed by `||` is presumed
  # live by limit [or-handler] whichever way the newline is read.
  local continuation_operator continuation_index=0
  for continuation_operator in '|' '&&' '|&'; do
    continuation_index=$((continuation_index + 1))
    fixture_path="$(write_test_body "$clean_root" "continuation-operator-$continuation_index" \
      "  ! grep -q x /etc/hosts $continuation_operator" \
      '    cat')"
  done
  : "$fixture_path"
}

# Build the boundary tree under $boundary_root: shapes that are DEAD in
# reality but that the static scan deliberately presumes live. The caller-named
# associative array is keyed by the bracketed limit identifier in the guard's
# header, and assert_documented_limits_are_all_pinned diffs the two lists, so a
# limit added to the header without a fixture (or the reverse) fails here.
# shellcheck disable=SC2034 # nameref: every write lands in the caller's array
create_boundary_tree_fixtures() {
  local -n boundary_limits_destination="$1"

  # Inside the body's FINAL compound statement the scan presumes the inversion
  # live, because which inner command runs last is data-dependent; these two
  # are the dead variants of that presumption.
  local final_compound_fixtures=()
  final_compound_fixtures+=("$(write_test_body "$boundary_root" final-group-inner-dead \
    '  true' \
    '  { ! true; true; }')")
  final_compound_fixtures+=("$(write_test_body "$boundary_root" final-if-inner-dead \
    '  true' \
    '  if true; then' \
    '    ! true' \
    '    true' \
    '  fi')")
  final_compound_fixtures+=("$(write_test_body "$boundary_root" final-select-inner-dead \
    '  true' \
    '  select choice in a; do' \
    '    ! true' \
    '    break' \
    '  done')")
  boundary_limits_destination["final-compound"]="${final_compound_fixtures[*]}"
  # case bodies are scanned opaquely (the region is tracked, never analyzed),
  # so a dead branch inversion passes.
  boundary_limits_destination["case-body"]="$(write_test_body "$boundary_root" case-branch-mid-dead \
    '  case x in x) ! true ;; esac' \
    '  true')"
  # Parenthesized contexts are presumed consumed; a process substitution
  # discards the status yet passes.
  boundary_limits_destination["parenthesized"]="$(write_test_body "$boundary_root" process-substitution-mid-dead \
    '  cat <(! true) >/dev/null' \
    '  true')"
  # Function BODIES are exempt (the call is what matters, and the common
  # single-command helper is live); a multi-command body hiding a dead
  # inversion passes.
  boundary_limits_destination["function-body"]="$(write_test_body "$boundary_root" function-body-inner-dead \
    '  f() { ! true; true; }' \
    '  f')"
  # File-scope helper functions are outside the scanned bodies entirely.
  boundary_limits_destination["file-scope-helper"]="$(write_bats_file "$boundary_root" file-scope-helper-inner-dead \
    'helper() {' \
    '  ! true' \
    '  true' \
    '}' \
    '@test "t" {' \
    '  helper' \
    '}')"
  # A `||` handler CONSUMES the status, so the handler decides whether the test
  # can fail. Measured under bats: `|| echo caught` passes with the refutation
  # violated, `|| { echo why; false; }` fails. The scan cannot tell which a
  # given handler is, so it presumes the handler may fail. These two are the
  # dead variants; the live spelling is in the clean tree and in the advice.
  local or_handler_fixtures=()
  or_handler_fixtures+=("$(write_test_body "$boundary_root" or-handler-cannot-fail \
    '  ! test -e /nope || echo caught' \
    '  true')")
  or_handler_fixtures+=("$(write_test_body "$boundary_root" or-handler-after-and \
    '  ! test -e /nope && echo held || echo violated' \
    '  true')")
  boundary_limits_destination["or-handler"]="${or_handler_fixtures[*]}"
  # The same presumption for the other consumer: an inversion that is the final
  # command of a condition selects a branch, and the branch decides. Measured:
  # `if ! true; then echo yes; fi` passes with the refutation violated.
  boundary_limits_destination["condition-consumer"]="$(write_test_body "$boundary_root" condition-branch-cannot-fail \
    '  if ! true; then echo yes; fi' \
    '  true')"
  # Only SCANNED_FILE_SUFFIXES are read. bats `load` accepts any path, so a
  # body in a helper with another suffix is invisible to the scan.
  boundary_limits_destination["unscanned-suffix"]="$(write_scan_fixture "$boundary_root" integration/helper-lib.sh \
    'setup() {' \
    '  ! true' \
    '  true' \
    '}')"
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

# guard_body_function_names -- the function names the GUARD treats as
# bats-executed bodies, read out of its own list.
guard_body_function_names() {
  sed -n '/^BATS_BODY_FUNCTION_NAMES = frozenset((/,/^))/p' "$GUARD" |
    grep -oE '"[a-z_]+"' | tr -d '"' | LC_ALL=C sort
}

# The guard's list and the wanted list must be the same set, and every wanted
# name must have a flagged fixture. A name quietly dropped from the guard takes
# its whole body out of the scan with nothing else to notice: dropping
# teardown_file and teardown_suite was measured to leave this suite green while
# two dead refutations went unreported. The FIRST attempt at this check derived
# the fixtures from the guard's own list, which meant a dropped name lost its
# fixture too and the check stayed green -- the same fail-open, one level up.
assert_guard_scans_every_bats_executed_body() {
  local -n flagged_expected_reference="$1"
  local body_name guard_names=()
  mapfile -t guard_names < <(guard_body_function_names)
  for body_name in "${BATS_EXECUTED_BODY_NAMES[@]}"; do
    printf '%s\n' "${guard_names[@]}" | grep -qxF "$body_name" ||
      record_failure "bats executes $body_name and the guard's body list does not name it, so those bodies go unscanned"
    [[ -n ${flagged_expected_reference["body_name_$body_name"]+set} ]] ||
      record_failure "bats executes $body_name with no flagged fixture pinning it"
  done
  for body_name in "${guard_names[@]}"; do
    printf '%s\n' "${BATS_EXECUTED_BODY_NAMES[@]}" | grep -qxF "$body_name" ||
      record_failure "the guard scans $body_name, which is not a body bats executes"
  done
}

# The guard's header, the boundary fixtures and the refusal assertions are
# three lists describing ONE set of limits, so enumerate and diff them: a limit
# documented without a fixture is an unverified claim, and a fixture without a
# documented limit is an undocumented blind spot.
assert_documented_limits_are_all_pinned() {
  local -n boundary_limits_reference="$1"
  local documented=() pinned=() identifier
  mapfile -t documented < <(
    sed -n 's/^#   - \[\([a-z][a-z0-9-]*\)\].*/\1/p' "$GUARD" | LC_ALL=C sort
  )
  mapfile -t pinned < <(
    printf '%s\n' "${!boundary_limits_reference[@]}" "${!REFUSAL_FIXTURE_WRITERS[@]}" |
      LC_ALL=C sort
  )
  if [[ ${#documented[@]} -eq 0 ]]; then
    record_failure "no bracketed limit identifiers found in the guard header (format changed?)"
    return 0
  fi
  for identifier in "${documented[@]}"; do
    printf '%s\n' "${pinned[@]}" | grep -qxF "$identifier" ||
      record_failure "guard header documents limit [$identifier] with no fixture pinning it"
  done
  for identifier in "${pinned[@]}"; do
    printf '%s\n' "${documented[@]}" | grep -qxF "$identifier" ||
      record_failure "fixture pins limit [$identifier] that the guard header does not document"
  done
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
# guard child only. The tool's OWN status is asserted, not merely a nonzero
# one: `-ne 0` is also satisfied by a guard that swallowed the tool error and
# then refused for some unrelated reason, which would report a fail-closed
# guard while the propagation had already been lost.
assert_python_failure_fails_guard() {
  local guard_output guard_status
  local -r stub_exit_status=7
  set +e
  guard_output="$(
    # shellcheck disable=SC2329,SC2317 # invoked indirectly: exported into the guard child
    python3() { return 7; }
    export -f python3
    bash "$GUARD" "$clean_root/test" 2>&1
  )"
  guard_status=$?
  set -e
  [[ $guard_status -eq $stub_exit_status ]] ||
    record_failure "a python3 failure must fail the guard with the tool's own status ($stub_exit_status), got $guard_status: $guard_output"
}

# A symlinked suite directory is still a suite directory: its bodies run under
# bats, so skipping them would hide real defects. The cycle back to the scan
# root proves the walk terminates and reports each file once.
assert_symlinked_suite_directory_is_scanned() {
  local guard_output guard_status
  mkdir -p "$symlink_root/test" "$symlink_root/outside"
  printf '@test "t" {\n  ! true\n  true\n}\n' >"$symlink_root/outside/dead.bats"
  ln -s "$symlink_root/outside" "$symlink_root/test/linked"
  ln -s "$symlink_root/test" "$symlink_root/test/cycle"
  run_guard guard_output guard_status "$symlink_root/test"
  [[ $guard_status -eq 1 ]] ||
    record_failure "a dead refutation behind a symlinked suite dir must be reported, got $guard_status: $guard_output"
  grep -qF 'linked/dead.bats:2' <<<"$guard_output" ||
    record_failure "the finding behind the symlink was not reported: $guard_output"
  [[ "$(grep -cF 'dead.bats:2' <<<"$guard_output")" == "1" ]] ||
    record_failure "the symlink cycle produced duplicate findings: $guard_output"
}

# --------------------------------------------------------------- refusals
#
# Input the scan cannot read correctly must be REFUSED (exit 2) with a
# diagnostic naming the path, never reported clean. Each writer below plants
# one such input in a private scan root and prints the SCAN-RELATIVE path the
# diagnostic has to name; the loop does the rest, so registering a refusal is
# one row in the two tables and one line in the guard's header.
#
# Every writer takes the root it must write into, and the fixture trees are
# scan targets only: nothing ever executes these files, which is why they are
# allowed to be invalid shell.

write_unterminated_case_refusal() {
  write_test_body "$1" unterminated-case \
    '  case x in' \
    '    x) true ;;'
}

write_unclosed_body_refusal() {
  write_bats_file "$1" unclosed-body \
    '@test "t" {' \
    '  ! true' \
    '  true'
}

write_unbalanced_compound_refusal() {
  # The braces balance while the `if` frame is still open, so the scan's two
  # bracket models disagree and its positional verdicts cannot be trusted.
  write_bats_file "$1" unbalanced-compound \
    '@test "t" {' \
    '  if true; then' \
    '    ! true' \
    '}'
}

write_heredoc_in_substitution_refusal() {
  write_test_body "$1" heredoc-in-substitution \
    '  x="$(cat <<EOF' \
    "it is data with an apostrophe: don't" \
    'EOF' \
    ')"' \
    '  true'
}

write_unterminated_quote_refusal() {
  write_test_body "$1" unterminated-quote \
    "  echo 'this quote never closes" \
    '  true'
}

write_unbalanced_parens_refusal() {
  write_test_body "$1" unbalanced-parens \
    '  x=$(printf %s hi' \
    '  true'
}

write_non_utf8_source_refusal() {
  local path="$1/test/integration/non-utf8.bats"
  mkdir -p "$(dirname "$path")"
  printf '@test "t" {\n  ! grep -q \xff /etc/hosts\n  true\n}\n' >"$path"
  printf 'integration/non-utf8.bats\n'
}

write_unreadable_file_refusal() {
  local relative_path
  relative_path="$(write_bats_file "$1" unreadable-file \
    '@test "t" {' \
    '  true' \
    '}')"
  chmod 000 "$1/test/$relative_path"
  printf '%s\n' "$relative_path"
}

write_unlistable_directory_refusal() {
  # A directory the walk cannot list. os.walk's default swallows the error,
  # which turned a tree holding a dead refutation into a green pass after one
  # chmod, so the scan collects walk errors and refuses instead.
  local hidden="$1/test/hidden"
  mkdir -p "$hidden"
  write_bats_file "$1" ok '@test "t" {' '  true' '}' >/dev/null
  printf '@test "t" {\n  ! true\n  true\n}\n' >"$hidden/dead.bats"
  chmod 000 "$hidden"
  if ls "$hidden" >/dev/null 2>&1; then
    chmod 755 "$hidden"
    record_failure "cannot test the unlistable-directory refusal: chmod 000 left $hidden listable (running as root?)"
    return 1
  fi
  printf 'hidden\n'
}

# The refusal set, keyed by the bracketed limit identifier in the guard's
# header. Registering a row here is what tells
# assert_documented_limits_are_all_pinned the limit exists, so a refusal the
# code can raise cannot stay undocumented and a documented one cannot stay
# unpinned. The DIAGNOSTIC is asserted, not just the exit code: these inputs
# overlap (an unterminated case leaves the body unclosed too), so a check that
# only looked at exit 2 would keep passing after the specific detection was
# removed.
#
# KEEP THE KEYS QUOTED. shfmt reads an UNQUOTED associative-array subscript as
# an arithmetic expression and rewrites `[unterminated-case]` into
# `[unterminated - case]`, which renames every limit on the next format run and
# breaks the diff against the guard's header.
declare -A REFUSAL_FIXTURE_WRITERS=(
  ["unterminated-case"]=write_unterminated_case_refusal
  ["unclosed-body"]=write_unclosed_body_refusal
  ["unbalanced-compound"]=write_unbalanced_compound_refusal
  ["heredoc-in-substitution"]=write_heredoc_in_substitution_refusal
  ["unterminated-quote"]=write_unterminated_quote_refusal
  ["unbalanced-parens"]=write_unbalanced_parens_refusal
  ["non-utf8-source"]=write_non_utf8_source_refusal
  ["unreadable-file"]=write_unreadable_file_refusal
  ["unlistable-directory"]=write_unlistable_directory_refusal
)
declare -A REFUSAL_DIAGNOSTICS=(
  ["unterminated-case"]='a case...esac opened in this bats-executed body'
  ["unclosed-body"]='this bats-executed body is never closed'
  ["unbalanced-compound"]='a compound command opened in this bats-executed body is never closed'
  ["heredoc-in-substitution"]='unterminated single quote'
  ["unterminated-quote"]='unterminated single quote'
  ["unbalanced-parens"]='unterminated (...)'
  ["non-utf8-source"]='cannot read'
  ["unreadable-file"]='cannot read'
  ["unlistable-directory"]='refusing to report a partial scan'
)

assert_unresolvable_input_refuses_the_scan() {
  local guard_output guard_status relative_path single_root identifier
  for identifier in "${!REFUSAL_FIXTURE_WRITERS[@]}"; do
    if [[ -z ${REFUSAL_DIAGNOSTICS[$identifier]+set} ]]; then
      record_failure "refusal limit [$identifier] has no expected diagnostic"
      continue
    fi
    single_root="$(mktemp -d)"
    if ! relative_path="$("${REFUSAL_FIXTURE_WRITERS[$identifier]}" "$single_root")"; then
      chmod -R u+rwX "$single_root" 2>/dev/null || true
      rm -rf "$single_root"
      continue # the writer already recorded why it could not build the input
    fi
    run_guard guard_output guard_status "$single_root/test"
    chmod -R u+rwX "$single_root" 2>/dev/null || true
    rm -rf "$single_root"
    [[ $guard_status -eq 2 ]] ||
      record_failure "limit [$identifier] must refuse the scan (exit 2), got $guard_status: $guard_output"
    grep -qF "$relative_path" <<<"$guard_output" ||
      record_failure "the refusal for [$identifier] must name the path $relative_path: $guard_output"
    grep -qF "${REFUSAL_DIAGNOSTICS[$identifier]}" <<<"$guard_output" ||
      record_failure "the refusal for [$identifier] must say why: $guard_output"
  done
}

# The liveness measurement is the ground truth every advice assertion rests
# on, so run the control first: a harness that answers the same way for
# everything makes each advice check pass for no reason at all.
assert_liveness_measurement_discriminates() {
  local measured
  measured="$(measure_refutation_liveness 'false')"
  [[ $measured == live ]] ||
    record_failure "the liveness measurement calls a failing body $measured, so it cannot discriminate"
  measured="$(measure_refutation_liveness 'true')"
  [[ $measured == dead ]] ||
    record_failure "the liveness measurement calls a passing body $measured, so it cannot discriminate"
}

# The failure message's advice, the shapes the guard accepts, and what the
# shell actually does are three statements of ONE contract, so all three are
# checked against each other rather than against the message's own wording:
#
#   1. every recommended spelling, with its refutation violated, must FAIL
#      (measured, not asserted);
#   2. the spelling the message warns about must PASS, or the warning is wrong;
#   3. the recommendation region must name every recommended spelling and NONE
#      of the discouraged ones, and the warning region the reverse.
#
# Checking (3) against the whole message instead of its regions is what let the
# highest-severity defect this guard exists to prevent come back: trading the
# recommendation and the warning leaves every substring of the honest message
# in place, so a message recommending the dead `|| echo` spelling satisfied a
# whole-output search word for word.
assert_advice_recommends_only_accepted_spellings() {
  local guard_output guard_status index marker measured
  local recommendation_region warning_region
  if [[ ${#RECOMMENDED_ADVICE_MARKERS[@]} -ne ${#RECOMMENDED_ADVICE_FIXTURE_LINES[@]} ]] ||
    [[ ${#RECOMMENDED_ADVICE_MARKERS[@]} -ne ${#RECOMMENDED_ADVICE_RUNNABLE_LINES[@]} ]]; then
    record_failure "the advice marker, fixture-line and runnable-line arrays must be index-aligned"
    return 0
  fi
  if [[ ${#DISCOURAGED_ADVICE_MARKERS[@]} -ne ${#DISCOURAGED_ADVICE_RUNNABLE_LINES[@]} ]]; then
    record_failure "the discouraged marker and runnable-line arrays must be index-aligned"
    return 0
  fi

  # (1) and (2): what the shell does with each spelling.
  for index in "${!RECOMMENDED_ADVICE_RUNNABLE_LINES[@]}"; do
    measured="$(measure_refutation_liveness "${RECOMMENDED_ADVICE_RUNNABLE_LINES[$index]}")"
    [[ $measured == live ]] ||
      record_failure "the message recommends a spelling that cannot fail the test: ${RECOMMENDED_ADVICE_MARKERS[$index]}"
  done
  for index in "${!DISCOURAGED_ADVICE_RUNNABLE_LINES[@]}"; do
    measured="$(measure_refutation_liveness "${DISCOURAGED_ADVICE_RUNNABLE_LINES[$index]}")"
    [[ $measured == dead ]] ||
      record_failure "the message warns about a spelling that CAN fail the test: ${DISCOURAGED_ADVICE_MARKERS[$index]}"
  done

  # Every recommended spelling must also pass the guard: a guard that flags the
  # shape it tells people to write trains them to switch it off.
  local advice_root
  advice_root="$(mktemp -d)"
  write_test_body "$advice_root" advice-spellings \
    "${RECOMMENDED_ADVICE_FIXTURE_LINES[@]}" \
    '  true' >/dev/null
  run_guard guard_output guard_status "$advice_root/test"
  rm -rf "$advice_root"
  [[ $guard_status -eq 0 ]] ||
    record_failure "the guard flags a spelling its own failure message recommends: $guard_output"

  # (3): the regions of the message that rejects the flagged tree.
  run_guard guard_output guard_status "$flagged_root/test"
  recommendation_region="$(grep -F "$ADVICE_RECOMMENDATION_LEAD_IN" <<<"$guard_output" || true)"
  warning_region="$(grep -F "$ADVICE_WARNING_LEAD_IN" <<<"$guard_output" || true)"
  if [[ -z $recommendation_region ]]; then
    record_failure "the failure message has no recommendation line ($ADVICE_RECOMMENDATION_LEAD_IN): $guard_output"
    return 0
  fi
  if [[ -z $warning_region ]]; then
    record_failure "the failure message has no warning line ($ADVICE_WARNING_LEAD_IN): $guard_output"
    return 0
  fi
  [[ $recommendation_region != "$warning_region" ]] ||
    record_failure "the recommendation and the warning must be separate lines, or trading them cannot be detected: $recommendation_region"

  for index in "${!RECOMMENDED_ADVICE_MARKERS[@]}"; do
    marker="${RECOMMENDED_ADVICE_MARKERS[$index]}"
    grep -qF "$marker" <<<"$recommendation_region" ||
      record_failure "the failure message no longer recommends: $marker"
  done
  for marker in "${DISCOURAGED_ADVICE_MARKERS[@]}"; do
    if grep -qF "$marker" <<<"$recommendation_region"; then
      record_failure "the failure message recommends a spelling measured unable to fail: $marker"
    fi
    grep -qF "$marker" <<<"$warning_region" ||
      record_failure "the failure message must warn that $marker cannot fail the test: $warning_region"
  done
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
  symlink_root="$(mktemp -d)"
  trap 'rm -rf "$flagged_root" "$clean_root" "$boundary_root" "$empty_root" \
               "$symlink_root"' EXIT

  # shellcheck disable=SC2034 # filled and read through namerefs by name
  local -A flagged_expected=()
  # shellcheck disable=SC2034 # filled and read through namerefs by name
  local -A boundary_limits=()
  create_flagged_tree_fixtures flagged_expected
  create_clean_tree_fixtures
  create_boundary_tree_fixtures boundary_limits

  assert_liveness_measurement_discriminates
  assert_flagged_tree_rejected flagged_expected
  assert_guard_scans_every_bats_executed_body flagged_expected
  assert_clean_tree_passes
  assert_boundary_tree_passes
  assert_documented_limits_are_all_pinned boundary_limits
  assert_empty_tree_passes
  assert_python_failure_fails_guard
  assert_symlinked_suite_directory_is_scanned
  assert_unresolvable_input_refuses_the_scan
  assert_advice_recommends_only_accepted_spellings

  report_failures dead-refutation-shapes
}

main "$@"
