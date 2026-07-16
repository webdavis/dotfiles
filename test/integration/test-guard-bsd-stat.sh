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

failures=0
report_failure() {
  printf 'test-guard-bsd-stat: FAIL -- %s\n' "$*" >&2
  failures=$((failures + 1))
}

flagged_root="$(mktemp -d)"
clean_root="$(mktemp -d)"
trap 'rm -rf "$flagged_root" "$clean_root"' EXIT

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

# Reference the passing fixture paths so shellcheck sees them used; they double as
# a manifest of what the clean tree contains.
: "$gnu_single" "$gnu_split" "$bare_bsd" "$clean_file"

if ((failures > 0)); then
  printf 'test-guard-bsd-stat: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi
printf 'test-guard-bsd-stat: PASS -- guard flags BSD-first stat chains, allows GNU-first and bare BSD\n'
