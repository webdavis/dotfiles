#!/usr/bin/env bash
# stat-order.sh. test/validate-tests.sh must reject any control flow that
# tries a BSD-form stat before a GNU-form stat in a scanned test file. The BSD
# form (the `-f` variant) reached first runs before the GNU form (the `-c`
# variant); on Linux CI (GNU coreutils), and on macOS whenever the nix dev
# shell puts GNU coreutils first on PATH, the `-f` variant means "filesystem
# status" and SUCCEEDS with the wrong output, so the GNU form never runs and
# the test silently reads garbage. This broke CI twice as a `||` fallback
# chain, then a third time as an `if`-gated probe the chain-only scan could
# not see. The property is ORDER, not one syntax: the portable idiom is
# GNU-first (the `-c` variant first) in any shape -- chain, `if` probe, `&&`
# probe, case branch, or a variable holding the command. A capability-gated
# bare BSD form with no GNU form after it in scope (e.g. a find-exec in a
# GNU-probed else-branch) must stay allowed. This drives the guard against a
# scratch fixture tree.
#
# Self-immunity trick: the two stat tokens are assembled from the variables
# below, GNU token declared FIRST, and never written as a literal BSD-first
# sequence in THIS file. `just test` runs the real guard over test/, which
# scans this very file in raw-text order, so declaring the GNU token before
# the BSD token is what keeps the fixtures honest without the guard flagging
# its own test. The gnu-token-first boundary fixture below pins this exact
# escape hatch.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=helpers/find-repo-root.sh
source "$here/helpers/find-repo-root.sh"
# shellcheck source=helpers/capture-output.sh
source "$here/helpers/capture-output.sh"
# shellcheck source=helpers/write-probe-scripts.sh
source "$here/helpers/write-probe-scripts.sh"
# shellcheck source=helpers/report-test-failures.sh
source "$here/helpers/report-test-failures.sh"

REPO_ROOT="$(find_repo_root)" || exit 1
GUARD="$REPO_ROOT/test/validate-tests.sh"

# The GNU form and the BSD form, as tokens (neither assignment carries `||`).
gnu_form='stat -c'
bsd_form='stat -f'

# Whitespace variants (a tab, multiple spaces): legal shell spellings of the
# same commands, derived from the tokens so no literal appears here either.
bsd_form_tab="${bsd_form/ /$'\t'}"
gnu_form_wide="${gnu_form/ /   }"

# GNU long-option spellings. These literals are safe to write here: only a BSD
# form inside a `||` segment can trip the guard, never a GNU form.
gnu_long_form='stat --format'
gnu_printf_form='stat --printf'

# Set in main; global so the EXIT trap can still see them after main returns.
flagged_root=""
clean_root=""
no_candidate_root=""
eval_boundary_root=""

# probe <root> <name> <body-line>... -- write an executable probe into
# <root>/test/unit and print its path (a thin wrapper over the shared helper).
probe() {
  local root="$1" name="$2"
  shift 2
  write_probe_in_suite "$root/test" unit "$name" "$@"
}

# run_guard <output-variable-name> <status-variable-name> <scanned-root>
# Run the guard against the scanned root, writing its output and exit code into
# the two caller-named variables (forwarded to capture_output's namerefs).
run_guard() {
  local output_variable_name="$1" status_variable_name="$2"
  capture_output "$output_variable_name" "$status_variable_name" bash "$GUARD" "$3"
}

# create_flagged_tree_fixtures <associative-array-name>
# Build the flagged tree's fixtures under $flagged_root and record each fixture
# path in the caller-named associative array (nameref), keyed by fixture name,
# for the assertions that grep the guard's rejection output. The offending
# line usually sits on physical line 2 (right after the shebang), so most
# flagged files are reported at ":2"; the case and masked-function fixtures
# place it deeper and assert their own line number.
# shellcheck disable=SC2034 # nameref: every write lands in the caller's array
create_flagged_tree_fixtures() {
  local -n flagged_fixture_destination="$1"

  # BSD-first chain on one line -- MUST be flagged.
  flagged_fixture_destination["bsd_single"]="$(probe "$flagged_root" bsd-single \
    "perms() { $bsd_form '%Lp' \"\$1\" 2>/dev/null || $gnu_form '%a' \"\$1\"; }")"

  # BSD-first chain split across a backslash continuation -- MUST be flagged.
  flagged_fixture_destination["bsd_split"]="$(probe "$flagged_root" bsd-split \
    "perms() { $bsd_form '%Lp' \"\$1\" \\" \
    "  || $gnu_form '%a' \"\$1\"; }")"

  # An earlier GNU call must NOT mask a later BSD-first chain on the same
  # logical line: the second command substitution is a BSD-first fallback chain
  # and MUST be flagged (per-chain analysis, not first-global-occurrence).
  flagged_fixture_destination["masked_bsd"]="$(probe "$flagged_root" masked-bsd \
    "a=\$($gnu_form '%a' .); b=\$($bsd_form '%Lp' . || $gnu_form '%a' .)")"

  # BSD-first chain spelled with a tab between stat and -f -- MUST be flagged
  # (legal token spacing must not bypass the scan).
  flagged_fixture_destination["bsd_tab"]="$(probe "$flagged_root" bsd-tab \
    "perms() { $bsd_form_tab '%Lp' \"\$1\" || $gnu_form '%a' \"\$1\"; }")"

  # A chain with ONLY a long-option GNU form AFTER the BSD form -- MUST still
  # be flagged (the long options count as GNU forms, not as absolution).
  flagged_fixture_destination["bsd_then_long"]="$(probe "$flagged_root" bsd-then-long \
    "perms() { $bsd_form '%Lp' \"\$1\" || $gnu_long_form=%a \"\$1\"; }")"

  # A BSD-first chain inside a COMMENT -- MUST be flagged: the scan reads raw
  # text on purpose, since a commented-out example gets copy-pasted.
  flagged_fixture_destination["commented_chain"]="$(probe "$flagged_root" commented-chain \
    "# copy-paste bait: $bsd_form '%Lp' . || $gnu_form '%a' .")"

  # A BSD-first chain inside \$root/fixtures/ -- MUST be flagged: the placement
  # check exempts fixtures/, but the stat scan reads every text file below the
  # scanned root (a sourced fixture lib carries the same trap).
  local fixtures_lib="$flagged_root/test/fixtures/stat-lib.sh"
  mkdir -p "$flagged_root/test/fixtures"
  {
    printf '#!/usr/bin/env bash\n'
    printf '%s\n' "perms() { $bsd_form '%Lp' \"\$1\" || $gnu_form '%a' \"\$1\"; }"
  } >"$fixtures_lib"
  flagged_fixture_destination["fixtures_lib"]="$fixtures_lib"

  # An `if`-gated BSD probe with the GNU form after the `fi` -- MUST be
  # flagged. This is the exact shape that broke CI a third time: no `||`
  # anywhere, yet the BSD form is tried first and its SUCCESS short-circuits
  # the GNU form.
  flagged_fixture_destination["if_gated"]="$(probe "$flagged_root" if-gated \
    "if $bsd_form '%Lp' . 2>/dev/null; then" \
    "  exit 0" \
    "fi" \
    "$gnu_form '%a' .")"

  # A `&&`-early-exit BSD probe with the GNU form on the next line -- MUST be
  # flagged (same order, third spelling).
  flagged_fixture_destination["and_exit"]="$(probe "$flagged_root" and-exit \
    "$bsd_form '%Lp' . 2>/dev/null && exit 0" \
    "$gnu_form '%a' .")"

  # A case dispatch whose BSD branch precedes its GNU branch -- MUST be
  # flagged: a uname gate still picks the BSD form on macOS while the nix
  # shell has put GNU stat first on PATH. Reported at the BSD branch, line 3.
  # shellcheck disable=SC2016 # the fixture wants a literal $(uname), run-time expanded
  flagged_fixture_destination["case_dispatch"]="$(probe "$flagged_root" case-dispatch \
    'case "$(uname)" in' \
    "  Darwin) $bsd_form '%Lp' . ;;" \
    "  *) $gnu_form '%a' . ;;" \
    "esac")"

  # A variable assigned the BSD command, used in a chain with a GNU fallback
  # -- MUST be flagged at the assignment: the token declaration is where the
  # BSD form enters the file first.
  flagged_fixture_destination["variable_token"]="$(probe "$flagged_root" variable-token \
    "stat_command='$bsd_form'" \
    "\$stat_command '%Lp' . || $gnu_form '%a' .")"

  # The same BSD token feeding eval / sh -c assembled chains -- MUST be
  # flagged at the assignment. (Formerly documented out of scope; the ordered
  # raw-text scan sees the token declaration precede the GNU form. A token
  # file that declares the GNU token FIRST remains the documented escape
  # hatch; see the boundary tree.)
  flagged_fixture_destination["eval_assembled"]="$(probe "$flagged_root" eval-assembled \
    "bsd_token='$bsd_form'" \
    "eval \"\$bsd_token '%Lp' . || $gnu_form '%a' .\"")"
  flagged_fixture_destination["sh_c_assembled"]="$(probe "$flagged_root" sh-c-assembled \
    "gated_command='$bsd_form'" \
    "sh -c \"\$gated_command '%Lp' . || $gnu_form '%a' .\"")"

  # A GNU-first function must NOT absolve a BSD-first probe in a LATER
  # function: scopes reset at each function definition line, so the second
  # function is judged on its own and MUST be flagged at its BSD line (4).
  flagged_fixture_destination["masked_function"]="$(probe "$flagged_root" masked-function \
    "good() { $gnu_form '%a' .; }" \
    "bad() {" \
    "  if $bsd_form '%Lp' . 2>/dev/null; then" \
    "    return 0" \
    "  fi" \
    "  $gnu_form '%a' ." \
    "}")"

  # The flagged tree also carries passing single-line and split GNU-first
  # fixtures so one guard run proves the scan flags only the BSD-first chains
  # and leaves the GNU-first ones untouched.
  flagged_fixture_destination["gnu_single_mixed"]="$(probe "$flagged_root" gnu-single-mixed \
    "perms() { $gnu_form '%a' \"\$1\" 2>/dev/null || $bsd_form '%Lp' \"\$1\"; }")"
  flagged_fixture_destination["gnu_split_mixed"]="$(probe "$flagged_root" gnu-split-mixed \
    "perms() { $gnu_form '%a' \"\$1\" \\" \
    "  || $bsd_form '%Lp' \"\$1\"; }")"
}

# Build the clean tree's fixtures under $clean_root: every spelling the guard
# must leave alone. Nothing greps their paths later, so the captured paths are
# referenced here only to consume the probe helper's stdout.
create_clean_tree_fixtures() {
  # GNU-first chain on one line -- MUST pass.
  local gnu_single
  gnu_single="$(probe "$clean_root" gnu-single \
    "perms() { $gnu_form '%a' \"\$1\" 2>/dev/null || $bsd_form '%Lp' \"\$1\"; }")"

  # GNU-first chain split across a backslash continuation -- MUST pass (the
  # false-positive case).
  local gnu_split
  gnu_split="$(probe "$clean_root" gnu-split \
    "perms() { $gnu_form '%a' \"\$1\" \\" \
    "  || $bsd_form '%Lp' \"\$1\"; }")"

  # Capability-gated bare BSD form, no `||` chain -- MUST pass.
  local bare_bsd
  bare_bsd="$(probe "$clean_root" bare-bsd \
    "find . -exec $bsd_form '%N %m' {} \\; | sort")"

  # Fully clean file (no stat at all) -- MUST pass.
  local clean_file
  clean_file="$(probe "$clean_root" clean \
    "printf 'no stat calls here\\n'")"

  # Two safe GNU-first chains on one logical line -- MUST pass (the per-chain
  # split must not cross-contaminate neighbouring chains).
  local double_safe
  double_safe="$(probe "$clean_root" double-safe \
    "x=\$($gnu_form '%a' . || $bsd_form '%Lp' .); y=\$($gnu_form '%s' . || $bsd_form '%z' .)")"

  # GNU-first chain with multi-space GNU form and tab BSD fallback -- MUST pass
  # (the GNU form must be recognized through the same whitespace tolerance).
  local gnu_wide
  gnu_wide="$(probe "$clean_root" gnu-wide \
    "perms() { $gnu_form_wide '%a' \"\$1\" || $bsd_form_tab '%Lp' \"\$1\"; }")"

  # GNU-first chains spelled with the long options (`--format=` attached,
  # `--printf` with a separate argument) -- MUST pass: they are correct
  # GNU-first fallbacks, exactly like `-c`.
  local gnu_long_attached gnu_long_separate
  gnu_long_attached="$(probe "$clean_root" gnu-long-attached \
    "perms() { $gnu_long_form=%a \"\$1\" || $bsd_form '%Lp' \"\$1\"; }")"
  gnu_long_separate="$(probe "$clean_root" gnu-long-separate \
    "size() { $gnu_printf_form '%s' \"\$1\" || $bsd_form '%z' \"\$1\"; }")"

  # An `if`-gated GNU probe with the BSD form after the `fi` -- MUST pass:
  # the correct order in the same shape the flagged if-gated fixture uses.
  local gnu_first_if
  gnu_first_if="$(probe "$clean_root" gnu-first-if \
    "if $gnu_form '%a' . 2>/dev/null; then" \
    "  exit 0" \
    "fi" \
    "$bsd_form '%Lp' .")"

  # A GNU capability probe gating a bare BSD call in the else-branch -- MUST
  # pass: this is the allowed capability-gate pattern (the GNU form appears
  # first, and the BSD call carries no chain).
  local gnu_probe_gated_bsd
  gnu_probe_gated_bsd="$(probe "$clean_root" gnu-probe-gated-bsd \
    "if $gnu_form '%n' . >/dev/null 2>&1; then" \
    "  find . -exec $gnu_form '%n %a' {} +" \
    "else" \
    "  find . -exec $bsd_form '%N %Lp' {} +" \
    "fi")"

  : "$gnu_single" "$gnu_split" "$bare_bsd" "$clean_file" "$double_safe" "$gnu_wide"
  : "$gnu_long_attached" "$gnu_long_separate" "$gnu_first_if" "$gnu_probe_gated_bsd"
}

# Build the no-candidate tree (not a single stat call) under $no_candidate_root.
create_no_candidate_fixture() {
  local no_candidate_probe
  no_candidate_probe="$(probe "$no_candidate_root" no-stat-anywhere \
    "printf 'not a single stat call below this root\\n'")"
  : "$no_candidate_probe"
}

# Build the documented out-of-scope fixtures under $eval_boundary_root: the
# shapes the scan is KNOWN not to catch, pinned so a future "improvement" that
# silently widens or narrows the scan's scope shows up as a test change.
create_boundary_tree_fixtures() {
  # A masking GNU call inside the same unseparated segment: the ordered scan
  # sees the GNU form first and absolves the BSD-first chain after it.
  local same_segment_mask
  same_segment_mask="$(probe "$eval_boundary_root" same-segment-mask \
    "a=\$($gnu_form '%a' .) b=\$($bsd_form '%Lp' . || $gnu_form '%a' .)")"

  # An UNRELATED GNU call earlier in the same scope absolves a later
  # BSD-first probe: the scan tracks form order, not data flow, so it cannot
  # tell a real capability gate from a coincidental earlier GNU call. This is
  # the scope-level analog of the same-segment mask.
  local scope_mask
  scope_mask="$(probe "$eval_boundary_root" scope-mask \
    "a=\"\$($gnu_form '%s' .)\"" \
    "if $bsd_form '%Lp' . 2>/dev/null; then exit 0; fi" \
    "$gnu_form '%a' .")"

  # Runtime assembly with the GNU token declared FIRST: the raw-text scan
  # reads declaration order, so a BSD-first chain assembled at run time from
  # GNU-token-first declarations is invisible. This is the exact escape hatch
  # THIS test file relies on for self-immunity; pinned so it cannot silently
  # close (closing it means this test flags itself).
  local gnu_token_first_assembly
  gnu_token_first_assembly="$(probe "$eval_boundary_root" gnu-token-first-assembly \
    "gnu_token='$gnu_form'" \
    "bsd_token='$bsd_form'" \
    "eval \"\$bsd_token '%Lp' . || \$gnu_token '%a' .\"")"

  # A case dispatch listing the GNU branch BEFORE a BSD fallback branch: the
  # scan does not understand dispatch conditions, so this passes even though
  # the `*)` branch still runs the BSD form on macOS under a GNU-first PATH.
  local reversed_case_dispatch
  # shellcheck disable=SC2016 # the fixture wants a literal $(uname), run-time expanded
  reversed_case_dispatch="$(probe "$eval_boundary_root" reversed-case-dispatch \
    'case "$(uname)" in' \
    "  Linux) $gnu_form '%a' . ;;" \
    "  *) $bsd_form '%Lp' . ;;" \
    "esac")"

  : "$same_segment_mask" "$scope_mask" "$gnu_token_first_assembly" "$reversed_case_dispatch"
}

# assert_flagged_tree_rejected <associative-array-name>
# The flagged tree is rejected, and stderr names each BSD-first chain at line 2
# while leaving the GNU-first fixtures unmentioned.
assert_flagged_tree_rejected() {
  local -n flagged_fixture_paths_reference="$1"
  local guard_output guard_status
  run_guard guard_output guard_status "$flagged_root/test"
  if [[ $guard_status -eq 0 ]]; then
    record_failure "flagged tree (BSD-first single + split) was NOT rejected (guard exit 0)"
    return 0
  fi
  grep -qiE 'stat|bsd|gnu-first' <<<"$guard_output" ||
    record_failure "rejection message does not mention the stat rule: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["bsd_single"]}:2" <<<"$guard_output" ||
    record_failure "BSD-first single-line chain not reported at :2: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["bsd_split"]}:2" <<<"$guard_output" ||
    record_failure "BSD-first split chain not reported at :2: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["masked_bsd"]}:2" <<<"$guard_output" ||
    record_failure "BSD-first chain masked by an earlier GNU call not reported at :2: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["bsd_tab"]}:2" <<<"$guard_output" ||
    record_failure "tab-spelled BSD-first chain not reported at :2: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["bsd_then_long"]}:2" <<<"$guard_output" ||
    record_failure "BSD-first chain with only a long-option GNU fallback not reported at :2: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["commented_chain"]}:2" <<<"$guard_output" ||
    record_failure "BSD-first chain inside a comment not reported at :2: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["fixtures_lib"]}:2" <<<"$guard_output" ||
    record_failure "BSD-first chain inside fixtures/ not reported at :2: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["if_gated"]}:2" <<<"$guard_output" ||
    record_failure "if-gated BSD probe (the third CI breakage shape) not reported at :2: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["and_exit"]}:2" <<<"$guard_output" ||
    record_failure "and-exit BSD probe not reported at :2: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["case_dispatch"]}:3" <<<"$guard_output" ||
    record_failure "case dispatch with the BSD branch first not reported at :3: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["variable_token"]}:2" <<<"$guard_output" ||
    record_failure "BSD command held in a variable not reported at its assignment (:2): $guard_output"
  grep -qF "${flagged_fixture_paths_reference["eval_assembled"]}:2" <<<"$guard_output" ||
    record_failure "eval-assembled chain fed by a BSD token not reported at the assignment (:2): $guard_output"
  grep -qF "${flagged_fixture_paths_reference["sh_c_assembled"]}:2" <<<"$guard_output" ||
    record_failure "sh -c assembled chain fed by a BSD token not reported at the assignment (:2): $guard_output"
  grep -qF "${flagged_fixture_paths_reference["masked_function"]}:4" <<<"$guard_output" ||
    record_failure "BSD-first probe in a later function masked by an earlier GNU-first function not reported at :4: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["gnu_single_mixed"]}" <<<"$guard_output" &&
    record_failure "GNU-first single-line chain was wrongly reported: $guard_output"
  grep -qF "${flagged_fixture_paths_reference["gnu_split_mixed"]}" <<<"$guard_output" &&
    record_failure "GNU-first split chain was wrongly reported: $guard_output"
  return 0
}

# A tree of only passing fixtures (GNU-first single, GNU-first split, bare
# capability-gated BSD, fully clean) exits 0.
assert_clean_tree_passes() {
  local guard_output guard_status
  run_guard guard_output guard_status "$clean_root/test"
  [[ $guard_status -eq 0 ]] ||
    record_failure "clean tree (GNU-first, bare BSD, no-stat) was wrongly rejected: $guard_output"
}

# Fail-closed, part 1: a tree with NO stat candidates at all still exits 0.
# grep reporting "no match" (exit 1) is a pass, distinct from a tool error
# (exit above 1), which the next two assertions pin as a failure.
assert_no_candidate_tree_passes() {
  local guard_output guard_status
  run_guard guard_output guard_status "$no_candidate_root/test"
  [[ $guard_status -eq 0 ]] ||
    record_failure "no-candidate tree (grep exit 1) was wrongly rejected: $guard_output"
}

# Fail-closed, part 2: a grep tool error (exit above 1) must FAIL the guard,
# never silently yield an empty candidate list and a green pass. The exported
# function shadows grep inside the guard child only.
assert_grep_failure_fails_guard() {
  local guard_output guard_status
  set +e
  guard_output="$(
    # shellcheck disable=SC2329,SC2317 # invoked indirectly: exported into the guard child
    grep() { return 7; }
    export -f grep
    bash "$GUARD" "$clean_root/test" 2>&1
  )"
  guard_status=$?
  set -e
  if [[ $guard_status -eq 0 ]]; then
    record_failure "grep failure (exit 7) did not fail the guard (fails open)"
  else
    grep -qi 'grep' <<<"$guard_output" ||
      record_failure "grep-failure rejection does not name grep: $guard_output"
  fi
}

# Fail-closed, part 3: an awk tool error must FAIL the guard; a failure inside
# a process substitution would otherwise never reach the parent. The clean tree
# has stat candidates (the bare BSD fixture), so awk is reached.
assert_awk_failure_fails_guard() {
  local guard_output guard_status
  set +e
  guard_output="$(
    # shellcheck disable=SC2329,SC2317 # invoked indirectly: exported into the guard child
    awk() { return 7; }
    export -f awk
    bash "$GUARD" "$clean_root/test" 2>&1
  )"
  guard_status=$?
  set -e
  if [[ $guard_status -eq 0 ]]; then
    record_failure "awk failure (exit 7) did not fail the guard (fails open)"
  else
    grep -qi 'awk' <<<"$guard_output" ||
      record_failure "awk-failure rejection does not name awk: $guard_output"
  fi
}

# The documented boundary: the same-segment mask, the scope-level mask, the
# gnu-token-first assembly, and the reversed case dispatch are out of scope
# and MUST pass.
assert_out_of_scope_cases_pass() {
  local guard_output guard_status
  run_guard guard_output guard_status "$eval_boundary_root/test"
  [[ $guard_status -eq 0 ]] ||
    record_failure "documented out-of-scope cases (same-segment mask, scope mask, gnu-token-first assembly, reversed case dispatch) must pass: $guard_output"
}

main() {
  [[ -x $GUARD ]] || {
    printf 'stat-order: guard missing or not executable: %s\n' "$GUARD" >&2
    exit 1
  }

  flagged_root="$(mktemp -d)"
  clean_root="$(mktemp -d)"
  no_candidate_root="$(mktemp -d)"
  eval_boundary_root="$(mktemp -d)"
  trap 'rm -rf "$flagged_root" "$clean_root" "$no_candidate_root" "$eval_boundary_root"' EXIT

  # shellcheck disable=SC2034 # filled and read through namerefs by name
  local -A flagged_fixture_paths=()
  create_flagged_tree_fixtures flagged_fixture_paths
  create_clean_tree_fixtures
  create_no_candidate_fixture
  create_boundary_tree_fixtures

  assert_flagged_tree_rejected flagged_fixture_paths
  assert_clean_tree_passes
  assert_no_candidate_tree_passes
  assert_grep_failure_fails_guard
  assert_awk_failure_fails_guard
  assert_out_of_scope_cases_pass

  report_failures stat-order
}

main "$@"
