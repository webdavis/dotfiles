#!/usr/bin/env bash
# runner-discovery.sh -- regression suite for test/run-test-suite.sh (the shared
# integration/e2e camp runner). Proves the correctness rules the gate leans on
# cannot silently regress:
#   F1  checked discovery: a find or sort that fails mid-discovery must FAIL the
#       camp, never green-gate a truncated list.
#   F2  fd 3 closed for children: a test that drains fd 3 must NOT swallow the
#       tests queued behind it.
#   F5b per-camp bats: the camp's own *.bats suites run and their failure
#       propagates (not only in the aggregate `test`).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUN_CAMP="$REPO_ROOT/test/run-test-suite.sh"

fail() {
  printf 'FAIL: %b\n' "$*" >&2
  exit 1
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

mk_sh() { # <camp-dir> <name> <body-line...>
  local dir="$1" name="$2"
  shift 2
  {
    printf '#!/usr/bin/env bash\n'
    printf '%s\n' "$@"
  } >"$dir/$name.sh"
  chmod +x "$dir/$name.sh"
}

# run_camp <camp> [env NAME=VAL ...] -- run the camp runner, capture rc + output.
run_camp() {
  local camp="$1"
  shift
  set +e
  RC_OUT="$(env "$@" "$RUN_CAMP" "$camp" 2>&1)"
  RC=$?
  set -e
}

# ---- F2: an fd-3-draining test must not swallow the failing test behind it ---
camp="$work/fd3"
mkdir -p "$camp"
# a-drain sorts first; it slurps fd 3 (if the child inherits it) then exits 0.
mk_sh "$camp" a-drain 'cat <&3 >/dev/null 2>&1 || true' 'exit 0'
# z-fail sorts last; it must still run and fail the camp.
mk_sh "$camp" z-fail 'exit 1'
run_camp "$camp"
[[ $RC -ne 0 ]] || fail "fd-3 drain swallowed the failing test; the camp green-gated (rc=0):\n$RC_OUT"
[[ $RC_OUT == *"z-fail.sh"* ]] || fail "z-fail never ran (header absent), so fd 3 was not closed for children:\n$RC_OUT"

# ---- F1: a find that fails the *.sh discovery must fail the camp -------------
# The shim fails ONLY the `-name '*.sh'` discovery (not the later `*.bats` one),
# so the test isolates the *.sh discovery: an unchecked (process-substitution)
# *.sh discovery would swallow this failure and green-gate.
camp="$work/pfind"
mkdir -p "$camp/bin"
mk_sh "$camp" a 'exit 0'
cat >"$camp/bin/find" <<'SHIM'
#!/usr/bin/env bash
/usr/bin/find "$@"
case " $* " in *".sh "*) exit 7 ;; esac
exit 0
SHIM
chmod +x "$camp/bin/find"
run_camp "$camp" "PATH=$camp/bin:$PATH"
[[ $RC -ne 0 ]] || fail "partial *.sh find (exit 7) did not fail the camp:\n$RC_OUT"
[[ $RC_OUT == *"discovery failed"* ]] || fail "expected a discovery-failed message on find failure:\n$RC_OUT"

# ---- F1: a sort that fails the *.sh discovery must fail the camp -------------
# The shim fails ONLY the FIRST sort (the *.sh discovery pipeline; the *.bats
# discovery sort is the second), isolating the *.sh discovery the same way. Its
# counter lives beside the shim (${0%/*}), so the heredoc needs no interpolation.
camp="$work/psort"
mkdir -p "$camp/bin"
mk_sh "$camp" a 'exit 0'
cat >"$camp/bin/sort" <<'SHIM'
#!/usr/bin/env bash
count="${0%/*}/count"
n=$(($(cat "$count" 2>/dev/null || echo 0) + 1))
printf '%s' "$n" >"$count"
/usr/bin/sort "$@"
[[ $n -eq 1 ]] && exit 7
exit 0
SHIM
chmod +x "$camp/bin/sort"
run_camp "$camp" "PATH=$camp/bin:$PATH"
[[ $RC -ne 0 ]] || fail "partial *.sh sort (exit 7) did not fail the camp:\n$RC_OUT"
[[ $RC_OUT == *"discovery failed"* ]] || fail "expected a discovery-failed message on sort failure:\n$RC_OUT"

# ---- F5b: the camp's own *.bats run, and their failure propagates -----------
# The stub records its argv to $BATS_ARGV (passed through the runner's env) and
# exits nonzero, modeling a failing suite without needing real bats/parallel.
camp="$work/bats"
mkdir -p "$camp/failbin" "$camp/passbin"
mk_sh "$camp" a 'exit 0'
printf '#!/usr/bin/env bats\n@test "x" { false; }\n' >"$camp/suite.bats"
cat >"$camp/failbin/bats" <<'SHIM'
#!/usr/bin/env bash
printf '%s\n' "$@" >"$BATS_ARGV"
exit 1
SHIM
chmod +x "$camp/failbin/bats"
run_camp "$camp" "PATH=$camp/failbin:$PATH" "BATS_ARGV=$camp/bats.argv"
[[ $RC -ne 0 ]] || fail "a failing camp bats suite did not fail the camp:\n$RC_OUT"
grep -q 'suite.bats' "$camp/bats.argv" 2>/dev/null ||
  fail "the runner did not invoke bats on the camp's suite (argv: $(cat "$camp/bats.argv" 2>/dev/null || echo none))"

# ---- F5b positive: a passing camp bats suite leaves the camp green ----------
cat >"$camp/passbin/bats" <<'SHIM'
#!/usr/bin/env bash
exit 0
SHIM
chmod +x "$camp/passbin/bats"
run_camp "$camp" "PATH=$camp/passbin:$PATH"
[[ $RC -eq 0 ]] || fail "camp with passing .sh and passing bats should be green (rc=$RC):\n$RC_OUT"

echo "runner-discovery: OK (checked discovery, fd-3 isolation, per-camp bats)"
