#!/usr/bin/env bash
# test-guard-runner.sh -- regression suite for test/tools/test-guard.sh (the
# placement / mode / symlink guard). Proves each guard rule catches its evasion:
#   F1  checked discovery: a find or sort that fails must FAIL the guard.
#   F4  symlink rejection: a symlinked test file AND a symlinked camp dir must
#       both fail the guard (a physical `find -type f` would skip them).
#   F5a bats placement: a *.bats must live DIRECTLY in a camp, same flat rule as
#       *.sh (nested or stray *.bats fails; flat *.bats passes).
#   plus the pre-existing placement/mode rules (nested *.sh, non-exec *.sh).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GUARD="$REPO_ROOT/test/tools/test-guard.sh"

fail() {
  printf 'FAIL: %b\n' "$*" >&2
  exit 1
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# guard <root> [env NAME=VAL ...] -- run the guard, capture rc + output.
guard() {
  local root="$1"
  shift
  set +e
  RC_OUT="$(env "$@" "$GUARD" "$root" 2>&1)"
  RC=$?
  set -e
}

mk_exec() { # <path>
  {
    printf '#!/usr/bin/env bash\n'
    printf 'exit 0\n'
  } >"$1"
  chmod +x "$1"
}

# ---- a clean tree passes (flat *.sh executable, flat *.bats, fixtures) -------
root="$work/clean/test"
mkdir -p "$root/unit" "$root/integration" "$root/e2e" "$root/fixtures/lib"
mk_exec "$root/unit/a.sh"
printf '#!/usr/bin/env bats\n@test "x" { true; }\n' >"$root/integration/suite.bats"
printf 'not executable, not run directly\n' >"$root/fixtures/lib/helper.sh"
guard "$root"
[[ $RC -eq 0 ]] || fail "clean tree should pass the guard (rc=$RC):\n$RC_OUT"

# ---- F4: a symlinked test FILE fails ----------------------------------------
root="$work/symfile/test"
mkdir -p "$root/unit"
mk_exec "$root/unit/real.sh"
ln -s real.sh "$root/unit/link.sh"
guard "$root"
[[ $RC -ne 0 ]] || fail "a symlinked test file must fail the guard (rc=0):\n$RC_OUT"
[[ $RC_OUT == *"symlink"* ]] || fail "expected a symlink rejection message:\n$RC_OUT"

# ---- F4: a symlinked camp DIR fails -----------------------------------------
root="$work/symdir/test"
mkdir -p "$root/unit" "$work/symdir/elsewhere"
mk_exec "$work/symdir/elsewhere/x.sh"
ln -s ../elsewhere "$root/e2e"
guard "$root"
[[ $RC -ne 0 ]] || fail "a symlinked camp dir must fail the guard (rc=0):\n$RC_OUT"
[[ $RC_OUT == *"symlink"* ]] || fail "expected a symlink rejection message for the camp dir:\n$RC_OUT"

# ---- F5a: a nested *.bats fails (flat-placement rule extends to bats) --------
root="$work/nestbats/test"
mkdir -p "$root/integration/sub"
printf '#!/usr/bin/env bats\n@test "x" { true; }\n' >"$root/integration/sub/suite.bats"
guard "$root"
[[ $RC -ne 0 ]] || fail "a nested *.bats must fail the guard (rc=0):\n$RC_OUT"
[[ $RC_OUT == *"nested"* ]] || fail "expected a nested-placement message for the bats file:\n$RC_OUT"

# ---- F5a: a stray *.bats directly under test/ fails -------------------------
root="$work/straybats/test"
mkdir -p "$root/unit"
mk_exec "$root/unit/a.sh"
printf '#!/usr/bin/env bats\n@test "x" { true; }\n' >"$root/stray.bats"
guard "$root"
[[ $RC -ne 0 ]] || fail "a stray *.bats under test/ must fail the guard (rc=0):\n$RC_OUT"
[[ $RC_OUT == *"outside"* ]] || fail "expected an outside-the-camps message for the stray bats:\n$RC_OUT"

# ---- placement: a nested *.sh fails -----------------------------------------
root="$work/nestsh/test"
mkdir -p "$root/unit/sub"
mk_exec "$root/unit/sub/a.sh"
guard "$root"
[[ $RC -ne 0 ]] || fail "a nested *.sh must fail the guard (rc=0):\n$RC_OUT"
[[ $RC_OUT == *"nested"* ]] || fail "expected a nested-placement message for the sh file:\n$RC_OUT"

# ---- mode: a non-executable camp *.sh fails ---------------------------------
root="$work/nonexec/test"
mkdir -p "$root/unit"
printf '#!/usr/bin/env bash\nexit 0\n' >"$root/unit/a.sh" # no chmod +x
guard "$root"
[[ $RC -ne 0 ]] || fail "a non-executable camp *.sh must fail the guard (rc=0):\n$RC_OUT"
[[ $RC_OUT == *"not executable"* ]] || fail "expected a not-executable message:\n$RC_OUT"

# ---- F1: a find that fails the FILE discovery must fail the guard ------------
# The shim passes the -type l symlink scan through, then fails the -name file
# discovery, so this exercises the files-discovery check specifically.
root="$work/pfind/test"
mkdir -p "$root/unit" "$work/pfind/bin"
mk_exec "$root/unit/a.sh"
{
  printf '#!/usr/bin/env bash\n'
  printf '/usr/bin/find "$@"\n'
  printf 'case " $* " in *" -name "*) exit 7 ;; esac\n'
  printf 'exit 0\n'
} >"$work/pfind/bin/find"
chmod +x "$work/pfind/bin/find"
guard "$root" "PATH=$work/pfind/bin:$PATH"
[[ $RC -ne 0 ]] || fail "partial find (exit 7) did not fail the guard:\n$RC_OUT"
[[ $RC_OUT == *"discovery failed"* ]] || fail "expected a discovery-failed message on find failure:\n$RC_OUT"

# ---- F1: a sort that fails the discovery pipeline must fail the guard --------
# sort is used only in the file discovery pipeline, not the symlink scan.
root="$work/psort/test"
mkdir -p "$root/unit" "$work/psort/bin"
mk_exec "$root/unit/a.sh"
{
  printf '#!/usr/bin/env bash\n'
  printf '/usr/bin/sort "$@"\nexit 7\n'
} >"$work/psort/bin/sort"
chmod +x "$work/psort/bin/sort"
guard "$root" "PATH=$work/psort/bin:$PATH"
[[ $RC -ne 0 ]] || fail "partial sort (exit 7) did not fail the guard:\n$RC_OUT"
[[ $RC_OUT == *"discovery failed"* ]] || fail "expected a discovery-failed message on sort failure:\n$RC_OUT"

echo "test-guard-runner: OK (symlink rejection, bats placement, checked discovery)"
