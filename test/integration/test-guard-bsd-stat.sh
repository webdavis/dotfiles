#!/usr/bin/env bash
# test-guard-bsd-stat.sh. scripts/test-guard.sh must reject a BSD-first stat
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

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GUARD="$REPO_ROOT/scripts/test-guard.sh"

[[ -x $GUARD ]] || {
  printf 'test-guard-bsd-stat: FAIL -- guard missing or not executable: %s\n' "$GUARD" >&2
  exit 1
}

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

failures=0
report_failure() {
  printf 'test-guard-bsd-stat: FAIL -- %s\n' "$*" >&2
  failures=$((failures + 1))
}

flagged_root="$(mktemp -d)"
clean_root="$(mktemp -d)"
no_candidate_root="$(mktemp -d)"
trap 'rm -rf "$flagged_root" "$clean_root" "$no_candidate_root"' EXIT

# Writes an executable scratch probe (shebang on line 1, the given body lines
# after) into <root>/test/unit and echoes its path. Each argument after <name>
# is one physical line, so a two-line body forms a backslash continuation.
write_probe() { # <root> <name> <body-line>...
  local camp="$1/test/unit"
  local file="$camp/$2.sh"
  mkdir -p "$camp"
  shift 2
  {
    printf '#!/usr/bin/env bash\n'
    printf '%s\n' "$@"
  } >"$file"
  chmod +x "$file"
  printf '%s\n' "$file"
}

run_guard() { # <scanned-root>: sets $guard_output and $guard_status
  set +e
  guard_output="$(bash "$GUARD" "$1" 2>&1)"
  guard_status=$?
  set -e
}

# Fixtures. The chain always starts on physical line 2 (right after the shebang),
# so a flagged file is reported at ":2".
#
# (a) BSD-first chain on one line -- MUST be flagged.
bsd_single="$(write_probe "$flagged_root" bsd-single \
  "perms() { $bsd_form '%Lp' \"\$1\" 2>/dev/null || $gnu_form '%a' \"\$1\"; }")"

# (b) GNU-first chain on one line -- MUST pass.
gnu_single="$(write_probe "$clean_root" gnu-single \
  "perms() { $gnu_form '%a' \"\$1\" 2>/dev/null || $bsd_form '%Lp' \"\$1\"; }")"

# (c) BSD-first chain split across a backslash continuation -- MUST be flagged (FX11).
bsd_split="$(write_probe "$flagged_root" bsd-split \
  "perms() { $bsd_form '%Lp' \"\$1\" \\" \
  "  || $gnu_form '%a' \"\$1\"; }")"

# (d) GNU-first chain split the same way -- MUST pass (the FX11 false-positive case).
gnu_split="$(write_probe "$clean_root" gnu-split \
  "perms() { $gnu_form '%a' \"\$1\" \\" \
  "  || $bsd_form '%Lp' \"\$1\"; }")"

# (e) Capability-gated bare BSD form, no `||` chain -- MUST pass.
bare_bsd="$(write_probe "$clean_root" bare-bsd \
  "find . -exec $bsd_form '%N %m' {} \\; | sort")"

# (f) Fully clean file (no stat at all) -- MUST pass.
clean_file="$(write_probe "$clean_root" clean \
  "printf 'no stat calls here\\n'")"

# (g) An earlier GNU call must NOT mask a later BSD-first chain on the same
# logical line: the second command substitution is a BSD-first fallback chain
# and MUST be flagged (per-chain analysis, not first-global-occurrence).
masked_bsd="$(write_probe "$flagged_root" masked-bsd \
  "a=\$($gnu_form '%a' .); b=\$($bsd_form '%Lp' . || $gnu_form '%a' .)")"

# (h) Two safe GNU-first chains on one logical line -- MUST pass (the per-chain
# split must not cross-contaminate neighbouring chains).
double_safe="$(write_probe "$clean_root" double-safe \
  "x=\$($gnu_form '%a' . || $bsd_form '%Lp' .); y=\$($gnu_form '%s' . || $bsd_form '%z' .)")"

# (i) BSD-first chain spelled with a tab between stat and -f -- MUST be flagged
# (legal token spacing must not bypass the scan).
bsd_tab="$(write_probe "$flagged_root" bsd-tab \
  "perms() { $bsd_form_tab '%Lp' \"\$1\" || $gnu_form '%a' \"\$1\"; }")"

# (j) GNU-first chain with multi-space GNU form and tab BSD fallback -- MUST pass
# (the GNU form must be recognized through the same whitespace tolerance).
gnu_wide="$(write_probe "$clean_root" gnu-wide \
  "perms() { $gnu_form_wide '%a' \"\$1\" || $bsd_form_tab '%Lp' \"\$1\"; }")"

# (k) GNU-first chains spelled with the long options (`--format=` attached,
# `--printf` with a separate argument) -- MUST pass: they are correct GNU-first
# fallbacks, exactly like `-c`.
gnu_long_attached="$(write_probe "$clean_root" gnu-long-attached \
  "perms() { $gnu_long_form=%a \"\$1\" || $bsd_form '%Lp' \"\$1\"; }")"
gnu_long_separate="$(write_probe "$clean_root" gnu-long-separate \
  "size() { $gnu_printf_form '%s' \"\$1\" || $bsd_form '%z' \"\$1\"; }")"

# (l) A chain with ONLY a long-option GNU form AFTER the BSD form -- MUST still
# be flagged (the long options count as GNU forms, not as absolution).
bsd_then_long="$(write_probe "$flagged_root" bsd-then-long \
  "perms() { $bsd_form '%Lp' \"\$1\" || $gnu_long_form=%a \"\$1\"; }")"

# The flagged tree also carries the passing single-line and split-passing fixtures
# so one guard run proves the scan flags only the BSD-first chains and leaves the
# GNU-first ones untouched.
gnu_single_mixed="$(write_probe "$flagged_root" gnu-single-mixed \
  "perms() { $gnu_form '%a' \"\$1\" 2>/dev/null || $bsd_form '%Lp' \"\$1\"; }")"
gnu_split_mixed="$(write_probe "$flagged_root" gnu-split-mixed \
  "perms() { $gnu_form '%a' \"\$1\" \\" \
  "  || $bsd_form '%Lp' \"\$1\"; }")"

# Assertion 1: the flagged tree is rejected, and stderr names each BSD-first
# chain at line 2 while leaving the GNU-first fixtures unmentioned.
run_guard "$flagged_root/test"
if [[ $guard_status -eq 0 ]]; then
  report_failure "flagged tree (BSD-first single + split) was NOT rejected (guard exit 0)"
else
  grep -qiE 'stat|bsd|gnu-first' <<<"$guard_output" ||
    report_failure "rejection message does not mention the stat rule: $guard_output"
  grep -qF "$bsd_single:2" <<<"$guard_output" ||
    report_failure "BSD-first single-line chain not reported at :2: $guard_output"
  grep -qF "$bsd_split:2" <<<"$guard_output" ||
    report_failure "BSD-first split chain not reported at :2: $guard_output"
  grep -qF "$masked_bsd:2" <<<"$guard_output" ||
    report_failure "BSD-first chain masked by an earlier GNU call not reported at :2: $guard_output"
  grep -qF "$bsd_tab:2" <<<"$guard_output" ||
    report_failure "tab-spelled BSD-first chain not reported at :2: $guard_output"
  grep -qF "$bsd_then_long:2" <<<"$guard_output" ||
    report_failure "BSD-first chain with only a long-option GNU fallback not reported at :2: $guard_output"
  grep -qF "$gnu_single_mixed" <<<"$guard_output" &&
    report_failure "GNU-first single-line chain was wrongly reported: $guard_output"
  grep -qF "$gnu_split_mixed" <<<"$guard_output" &&
    report_failure "GNU-first split chain was wrongly reported: $guard_output"
fi

# Assertion 2: a tree of only passing fixtures (GNU-first single, GNU-first split,
# bare capability-gated BSD, fully clean) exits 0.
run_guard "$clean_root/test"
[[ $guard_status -eq 0 ]] ||
  report_failure "clean tree (GNU-first, bare BSD, no-stat) was wrongly rejected: $guard_output"

# Assertion 3 (fail-closed, part 1): a tree with NO stat candidates at all still
# exits 0. grep reporting "no match" (exit 1) is a pass, distinct from a tool
# error (exit above 1), which the next two assertions pin as a failure.
no_candidate_probe="$(write_probe "$no_candidate_root" no-stat-anywhere \
  "printf 'not a single stat call below this root\\n'")"
run_guard "$no_candidate_root/test"
[[ $guard_status -eq 0 ]] ||
  report_failure "no-candidate tree (grep exit 1) was wrongly rejected: $guard_output"
: "$no_candidate_probe"

# Assertion 4 (fail-closed, part 2): a grep tool error (exit above 1) must FAIL
# the guard, never silently yield an empty candidate list and a green pass. The
# exported function shadows grep inside the guard child only.
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
  report_failure "grep failure (exit 7) did not fail the guard (fails open)"
else
  grep -qi 'grep' <<<"$guard_output" ||
    report_failure "grep-failure rejection does not name grep: $guard_output"
fi

# Assertion 5 (fail-closed, part 3): an awk tool error must FAIL the guard; a
# failure inside a process substitution would otherwise never reach the parent.
# The clean tree has stat candidates (the bare BSD fixture), so awk is reached.
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
  report_failure "awk failure (exit 7) did not fail the guard (fails open)"
else
  grep -qi 'awk' <<<"$guard_output" ||
    report_failure "awk-failure rejection does not name awk: $guard_output"
fi

# Reference the passing fixture paths so shellcheck sees them used; they double as
# a manifest of what the clean tree contains.
: "$gnu_single" "$gnu_split" "$bare_bsd" "$clean_file" "$double_safe" "$gnu_wide"
: "$gnu_long_attached" "$gnu_long_separate"

if ((failures > 0)); then
  printf 'test-guard-bsd-stat: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi
printf 'test-guard-bsd-stat: PASS -- guard flags BSD-first stat chains, allows GNU-first and bare BSD\n'
