#!/usr/bin/env bash
# test-guard-bsd-stat.sh. scripts/test-guard.sh must reject a BSD-first stat
# fallback chain in a test file. The BSD form (the `-f` variant) placed first in
# a chain runs before the GNU form (the `-c` variant); on Linux CI (GNU
# coreutils) the `-f` variant means "filesystem status" and SUCCEEDS with the
# wrong output, so the fallback never fires and the test silently reads garbage.
# Two CI failures (PRs #49, #50) came from exactly this. The portable idiom is
# GNU-first (the `-c` variant first). A capability-gated bare BSD form with no
# chain (e.g. a find-exec in a GNU-probed else-branch) is not a fallback chain
# and must stay allowed. This drives the guard against scratch camps.
#
# The stat tokens are assembled from variables on purpose: a literal BSD-first
# chain written here would be flagged by the very guard under test (it scans this
# file too). Keeping the two `stat` tokens off any line that also carries `||`
# lets the fixtures stay honest without tripping the guard on itself.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1 && pwd)"
GUARD="$REPO_ROOT/scripts/test-guard.sh"

# The GNU form and the BSD form, as tokens (neither line below carries `||`).
gnu='stat -c'
bsd='stat -f'

fails=0
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  fails=$((fails + 1))
}

# A valid scratch camp (executable *.sh in test/unit) so only the stat rule can
# differentiate a pass from a fail.
mk_scratch() { # <body-line> -> echoes the scratch root
  local root
  root="$(mktemp -d)"
  mkdir -p "$root/test/unit"
  {
    printf '#!/usr/bin/env bash\n'
    printf '%s\n' "$1"
  } >"$root/test/unit/probe.sh"
  chmod +x "$root/test/unit/probe.sh"
  printf '%s\n' "$root"
}

run_guard() { # <scratch-root>: sets $status and $out
  set +e
  out="$(bash "$GUARD" "$1/test" 2>&1)"
  status=$?
  set -e
}

# 1) BSD-first chain: MUST be rejected.
root="$(mk_scratch "perms() { $bsd '%Lp' \"\$1\" 2>/dev/null || $gnu '%a' \"\$1\"; }")"
run_guard "$root"
if [[ $status -eq 0 ]]; then
  fail "BSD-first fallback chain was NOT rejected (guard exit 0)"
else
  grep -qiE 'stat|bsd|gnu-first' <<<"$out" || fail "rejection message does not mention the stat rule: $out"
fi
rm -rf "$root"

# 2) GNU-first chain: MUST pass.
root="$(mk_scratch "perms() { $gnu '%a' \"\$1\" 2>/dev/null || $bsd '%Lp' \"\$1\"; }")"
run_guard "$root"
[[ $status -eq 0 ]] || fail "GNU-first fallback chain was wrongly rejected: $out"
rm -rf "$root"

# 3) Capability-gated bare BSD form (no chain): MUST pass (not a fallback chain).
root="$(mk_scratch "find . -exec $bsd '%N %m' {} \\; | sort")"
run_guard "$root"
[[ $status -eq 0 ]] || fail "ungated bare BSD form (no chain) was wrongly rejected: $out"
rm -rf "$root"

if ((fails > 0)); then
  printf '%d assertion(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'PASS: guard rejects BSD-first stat chains, allows GNU-first and ungated bare form\n'
