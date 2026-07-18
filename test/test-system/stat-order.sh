#!/usr/bin/env bash
# stat-order.sh. test/validate-tests.sh must reject a BSD-first stat
# fallback chain in a scanned test file. The BSD form (the `-f` variant) placed
# first in a chain runs before the GNU form (the `-c` variant); on Linux CI (GNU
# coreutils) the `-f` variant means "filesystem status" and SUCCEEDS with the
# wrong output, so the fallback never fires and the test silently reads garbage.
# Two CI failures (PRs #49, #50) came from exactly this. The portable idiom is
# GNU-first (the `-c` variant first). A capability-gated bare BSD form with no
# chain (e.g. a find-exec in a GNU-probed else-branch) is not a fallback chain
# and must stay allowed. This drives the guard against a scratch fixture tree.
#
# Self-immunity trick: the two stat tokens are assembled from the variables
# below, never written as a literal BSD-first chain in THIS file. `just test`
# runs the real guard over test/, which scans this very file; keeping every line
# that also carries `||` off the literal tokens lets the fixtures stay honest
# without the guard flagging its own test.
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

run_guard() { # <scanned-root>: sets captured_output and captured_status
  capture_output bash "$GUARD" "$1"
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

  # Fixtures. The chain always starts on physical line 2 (right after the
  # shebang), so a flagged file is reported at ":2".
  #
  # (a) BSD-first chain on one line -- MUST be flagged.
  local bsd_single
  bsd_single="$(probe "$flagged_root" bsd-single \
    "perms() { $bsd_form '%Lp' \"\$1\" 2>/dev/null || $gnu_form '%a' \"\$1\"; }")"

  # (b) GNU-first chain on one line -- MUST pass.
  local gnu_single
  gnu_single="$(probe "$clean_root" gnu-single \
    "perms() { $gnu_form '%a' \"\$1\" 2>/dev/null || $bsd_form '%Lp' \"\$1\"; }")"

  # (c) BSD-first chain split across a backslash continuation -- MUST be flagged.
  local bsd_split
  bsd_split="$(probe "$flagged_root" bsd-split \
    "perms() { $bsd_form '%Lp' \"\$1\" \\" \
    "  || $gnu_form '%a' \"\$1\"; }")"

  # (d) GNU-first chain split the same way -- MUST pass (the false-positive case).
  local gnu_split
  gnu_split="$(probe "$clean_root" gnu-split \
    "perms() { $gnu_form '%a' \"\$1\" \\" \
    "  || $bsd_form '%Lp' \"\$1\"; }")"

  # (e) Capability-gated bare BSD form, no `||` chain -- MUST pass.
  local bare_bsd
  bare_bsd="$(probe "$clean_root" bare-bsd \
    "find . -exec $bsd_form '%N %m' {} \\; | sort")"

  # (f) Fully clean file (no stat at all) -- MUST pass.
  local clean_file
  clean_file="$(probe "$clean_root" clean \
    "printf 'no stat calls here\\n'")"

  # (g) An earlier GNU call must NOT mask a later BSD-first chain on the same
  # logical line: the second command substitution is a BSD-first fallback chain
  # and MUST be flagged (per-chain analysis, not first-global-occurrence).
  local masked_bsd
  masked_bsd="$(probe "$flagged_root" masked-bsd \
    "a=\$($gnu_form '%a' .); b=\$($bsd_form '%Lp' . || $gnu_form '%a' .)")"

  # (h) Two safe GNU-first chains on one logical line -- MUST pass (the per-chain
  # split must not cross-contaminate neighbouring chains).
  local double_safe
  double_safe="$(probe "$clean_root" double-safe \
    "x=\$($gnu_form '%a' . || $bsd_form '%Lp' .); y=\$($gnu_form '%s' . || $bsd_form '%z' .)")"

  # (i) BSD-first chain spelled with a tab between stat and -f -- MUST be flagged
  # (legal token spacing must not bypass the scan).
  local bsd_tab
  bsd_tab="$(probe "$flagged_root" bsd-tab \
    "perms() { $bsd_form_tab '%Lp' \"\$1\" || $gnu_form '%a' \"\$1\"; }")"

  # (j) GNU-first chain with multi-space GNU form and tab BSD fallback -- MUST
  # pass (the GNU form must be recognized through the same whitespace tolerance).
  local gnu_wide
  gnu_wide="$(probe "$clean_root" gnu-wide \
    "perms() { $gnu_form_wide '%a' \"\$1\" || $bsd_form_tab '%Lp' \"\$1\"; }")"

  # (k) GNU-first chains spelled with the long options (`--format=` attached,
  # `--printf` with a separate argument) -- MUST pass: they are correct GNU-first
  # fallbacks, exactly like `-c`.
  local gnu_long_attached gnu_long_separate
  gnu_long_attached="$(probe "$clean_root" gnu-long-attached \
    "perms() { $gnu_long_form=%a \"\$1\" || $bsd_form '%Lp' \"\$1\"; }")"
  gnu_long_separate="$(probe "$clean_root" gnu-long-separate \
    "size() { $gnu_printf_form '%s' \"\$1\" || $bsd_form '%z' \"\$1\"; }")"

  # (l) A chain with ONLY a long-option GNU form AFTER the BSD form -- MUST still
  # be flagged (the long options count as GNU forms, not as absolution).
  local bsd_then_long
  bsd_then_long="$(probe "$flagged_root" bsd-then-long \
    "perms() { $bsd_form '%Lp' \"\$1\" || $gnu_long_form=%a \"\$1\"; }")"

  # (m) A BSD-first chain inside a COMMENT -- MUST be flagged: the scan reads raw
  # text on purpose, since a commented-out example gets copy-pasted.
  local commented_chain
  commented_chain="$(probe "$flagged_root" commented-chain \
    "# copy-paste bait: $bsd_form '%Lp' . || $gnu_form '%a' .")"

  # (n) A BSD-first chain inside \$root/fixtures/ -- MUST be flagged: the
  # placement check exempts fixtures/, but the stat scan reads every text file
  # below the scanned root (a sourced fixture lib carries the same trap).
  local fixtures_lib="$flagged_root/test/fixtures/stat-lib.sh"
  mkdir -p "$flagged_root/test/fixtures"
  {
    printf '#!/usr/bin/env bash\n'
    printf '%s\n' "perms() { $bsd_form '%Lp' \"\$1\" || $gnu_form '%a' \"\$1\"; }"
  } >"$fixtures_lib"

  # The flagged tree also carries the passing single-line and split-passing
  # fixtures so one guard run proves the scan flags only the BSD-first chains and
  # leaves the GNU-first ones untouched.
  local gnu_single_mixed gnu_split_mixed
  gnu_single_mixed="$(probe "$flagged_root" gnu-single-mixed \
    "perms() { $gnu_form '%a' \"\$1\" 2>/dev/null || $bsd_form '%Lp' \"\$1\"; }")"
  gnu_split_mixed="$(probe "$flagged_root" gnu-split-mixed \
    "perms() { $gnu_form '%a' \"\$1\" \\" \
    "  || $bsd_form '%Lp' \"\$1\"; }")"

  # Assertion 1: the flagged tree is rejected, and stderr names each BSD-first
  # chain at line 2 while leaving the GNU-first fixtures unmentioned.
  run_guard "$flagged_root/test"
  if [[ $captured_status -eq 0 ]]; then
    record_failure "flagged tree (BSD-first single + split) was NOT rejected (guard exit 0)"
  else
    grep -qiE 'stat|bsd|gnu-first' <<<"$captured_output" ||
      record_failure "rejection message does not mention the stat rule: $captured_output"
    grep -qF "$bsd_single:2" <<<"$captured_output" ||
      record_failure "BSD-first single-line chain not reported at :2: $captured_output"
    grep -qF "$bsd_split:2" <<<"$captured_output" ||
      record_failure "BSD-first split chain not reported at :2: $captured_output"
    grep -qF "$masked_bsd:2" <<<"$captured_output" ||
      record_failure "BSD-first chain masked by an earlier GNU call not reported at :2: $captured_output"
    grep -qF "$bsd_tab:2" <<<"$captured_output" ||
      record_failure "tab-spelled BSD-first chain not reported at :2: $captured_output"
    grep -qF "$bsd_then_long:2" <<<"$captured_output" ||
      record_failure "BSD-first chain with only a long-option GNU fallback not reported at :2: $captured_output"
    grep -qF "$commented_chain:2" <<<"$captured_output" ||
      record_failure "BSD-first chain inside a comment not reported at :2: $captured_output"
    grep -qF "$fixtures_lib:2" <<<"$captured_output" ||
      record_failure "BSD-first chain inside fixtures/ not reported at :2: $captured_output"
    grep -qF "$gnu_single_mixed" <<<"$captured_output" &&
      record_failure "GNU-first single-line chain was wrongly reported: $captured_output"
    grep -qF "$gnu_split_mixed" <<<"$captured_output" &&
      record_failure "GNU-first split chain was wrongly reported: $captured_output"
  fi

  # Assertion 2: a tree of only passing fixtures (GNU-first single, GNU-first
  # split, bare capability-gated BSD, fully clean) exits 0.
  run_guard "$clean_root/test"
  [[ $captured_status -eq 0 ]] ||
    record_failure "clean tree (GNU-first, bare BSD, no-stat) was wrongly rejected: $captured_output"

  # Assertion 3 (fail-closed, part 1): a tree with NO stat candidates at all still
  # exits 0. grep reporting "no match" (exit 1) is a pass, distinct from a tool
  # error (exit above 1), which the next two assertions pin as a failure.
  local no_candidate_probe
  no_candidate_probe="$(probe "$no_candidate_root" no-stat-anywhere \
    "printf 'not a single stat call below this root\\n'")"
  run_guard "$no_candidate_root/test"
  [[ $captured_status -eq 0 ]] ||
    record_failure "no-candidate tree (grep exit 1) was wrongly rejected: $captured_output"
  : "$no_candidate_probe"

  # Assertion 4 (fail-closed, part 2): a grep tool error (exit above 1) must FAIL
  # the guard, never silently yield an empty candidate list and a green pass. The
  # exported function shadows grep inside the guard child only.
  set +e
  captured_output="$(
    # shellcheck disable=SC2329,SC2317 # invoked indirectly: exported into the guard child
    grep() { return 7; }
    export -f grep
    bash "$GUARD" "$clean_root/test" 2>&1
  )"
  captured_status=$?
  set -e
  if [[ $captured_status -eq 0 ]]; then
    record_failure "grep failure (exit 7) did not fail the guard (fails open)"
  else
    grep -qi 'grep' <<<"$captured_output" ||
      record_failure "grep-failure rejection does not name grep: $captured_output"
  fi

  # Assertion 5 (fail-closed, part 3): an awk tool error must FAIL the guard; a
  # failure inside a process substitution would otherwise never reach the parent.
  # The clean tree has stat candidates (the bare BSD fixture), so awk is reached.
  set +e
  captured_output="$(
    # shellcheck disable=SC2329,SC2317 # invoked indirectly: exported into the guard child
    awk() { return 7; }
    export -f awk
    bash "$GUARD" "$clean_root/test" 2>&1
  )"
  captured_status=$?
  set -e
  if [[ $captured_status -eq 0 ]]; then
    record_failure "awk failure (exit 7) did not fail the guard (fails open)"
  else
    grep -qi 'awk' <<<"$captured_output" ||
      record_failure "awk-failure rejection does not name awk: $captured_output"
  fi

  # Assertion 6 (documented boundaries, pinned): the scan's guarantee is LITERAL
  # chains in raw text, analyzed per chain segment. Out of scope and MUST pass:
  # a BSD-first chain assembled at run time (eval over a variable holding the BSD
  # form, or a string handed to `sh -c`), and a GNU stat call that masks a later
  # BSD-first chain packed into the SAME segment (no `;`/`&&` between them; the
  # segment split is a documented approximation). These tests exist so a future
  # "improvement" that silently widens the scan's scope shows up as a test change.
  local eval_probe sh_c_probe same_segment_mask
  eval_probe="$(probe "$eval_boundary_root" eval-assembled \
    "bsd_token='$bsd_form'" \
    "eval \"\$bsd_token '%Lp' . || $gnu_form '%a' .\"")"
  sh_c_probe="$(probe "$eval_boundary_root" sh-c-assembled \
    "gated_command='$bsd_form'" \
    "sh -c \"\$gated_command '%Lp' . || $gnu_form '%a' .\"")"
  same_segment_mask="$(probe "$eval_boundary_root" same-segment-mask \
    "a=\$($gnu_form '%a' .) b=\$($bsd_form '%Lp' . || $gnu_form '%a' .)")"
  run_guard "$eval_boundary_root/test"
  [[ $captured_status -eq 0 ]] ||
    record_failure "documented out-of-scope cases (eval, sh -c, same-segment mask) must pass: $captured_output"
  : "$eval_probe" "$sh_c_probe" "$same_segment_mask"

  # Reference the passing fixture paths so shellcheck sees them used; they double
  # as a manifest of what the clean tree contains.
  : "$gnu_single" "$gnu_split" "$bare_bsd" "$clean_file" "$double_safe" "$gnu_wide"
  : "$gnu_long_attached" "$gnu_long_separate"

  report_failures stat-order
}

main "$@"
