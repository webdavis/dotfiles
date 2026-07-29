#!/usr/bin/env bash
# no-dead-refutation-in-bats.sh, a REPO-WIDE guard: a bats test may not assert
# absence with a bare `! command` anywhere except as its final statement.
#
# The mechanism, measured rather than assumed:
#
#   @test "A" { ! grep -qF PRESENT "$f"; }              -> FAILS (correct)
#   @test "B" { ! grep -qF PRESENT "$f"; true; }        -> PASSES (assertion dead)
#   @test "C" { ! grep -qF PRESENT "$f"
#               ! grep -qF ABSENT  "$f"; }              -> PASSES (first one dead)
#
# bash's `set -e` does not abort on an inverted command, and bats takes the
# body's LAST status as the result. So a bare `!` in final position works, and
# anywhere else it is a comment with a grep in it. Nine such assertions were live
# on main, including one asserting that alert data is never sent off-box.
#
# The rule is positional, so this guard is too. It does not ban `! cmd`; it bans
# relying on one whose status nothing consumes. `if ! cmd; then`, `! cmd || fail`
# and a final-statement `! cmd` are all fine.
#
# Why a guard and not just the fix: a dead assertion is invisible in a green run.
# It reads as coverage, it costs a reviewer real attention to spot, and the next
# test copied from a neighbour inherits it.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -d $REPO_ROOT/test ]] || fail "no test directory under $REPO_ROOT"

report="$(
  python3 - "$REPO_ROOT" <<'PY'
import os, re, sys
root = sys.argv[1]
dead = []
for dirpath, _, names in os.walk(os.path.join(root, "test")):
    for name in sorted(names):
        if not name.endswith(".bats"):
            continue
        path = os.path.join(dirpath, name)
        lines = open(path).read().splitlines()
        index = 0
        while index < len(lines):
            if not lines[index].lstrip().startswith("@test "):
                index += 1
                continue
            depth = lines[index].count("{") - lines[index].count("}")
            body, cursor = [], index + 1
            while cursor < len(lines) and depth > 0:
                depth += lines[cursor].count("{") - lines[cursor].count("}")
                if depth > 0:
                    body.append((cursor + 1, lines[cursor]))
                cursor += 1
            # Join backslash continuations: one logical statement can span lines,
            # and treating the first line as "not last" would misreport it.
            statements, buffer, start = [], "", None
            for number, text in body:
                stripped = text.strip()
                if not stripped or stripped.startswith("#"):
                    continue
                if start is None:
                    start = number
                piece = stripped.rstrip("\\")
                buffer = (buffer + " " + piece) if buffer else piece
                if stripped.endswith("\\"):
                    continue
                statements.append((start, buffer))
                buffer, start = "", None
            if buffer:
                statements.append((start, buffer))
            last = statements[-1][0] if statements else None
            for number, statement in statements:
                if not re.match(r"^!\s", statement):
                    continue
                # A status something else consumes is not dead.
                if "||" in statement or "&&" in statement or "; then" in statement:
                    continue
                if number != last:
                    dead.append(f"{os.path.relpath(path, root)}:{number}  {statement[:70]}")
            index = cursor
for entry in dead:
    print(entry)
PY
)"

if [[ -n $report ]]; then
  printf 'FAIL: %s bats assertion(s) invert a command mid-body, where nothing consumes the status:\n' \
    "$(printf '%s\n' "$report" | wc -l | tr -d ' ')" >&2
  printf '%s\n' "$report" | sed 's/^/  /' >&2
  printf 'An inverted command only decides the test as its LAST statement. Anywhere else it cannot fail. Wrap it: if cmd; then echo "why this is wrong"; false; fi, or use a refute helper.\n' >&2
  exit 1
fi

printf 'no-dead-refutation-in-bats: OK (no bats assertion inverts a command where its status is discarded)\n'
