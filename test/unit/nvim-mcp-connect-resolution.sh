#!/usr/bin/env bash
# nvim-mcp-connect.sh, the RESOLVING half: every case that ends with an
# instance chosen, or with the picker that refuses to choose between two. The
# refusals and the guards live in nvim-mcp-connect-refusals.sh, so that each
# file stays inside the one-second budget.
#
#   a) an injected socket that IS a verified in-tab candidate -> it is used
#   b) an injected socket that is NOT -> refused, naming the socket and the tab
#   c) a registry record whose identity matches           -> that socket is used
#   d) an identity MISMATCH  -> only that record is deleted, the next one wins
#   f) two candidates        -> picker, exit 4, both enumerated
#   g) two instances sharing ONE pane id -> both survive and both are enumerated
#   h) a dead-pid socket in the glob -> filtered by kill -0 BEFORE any RPC
#   q) a successful resolution leaves no probe file behind
#
# Cases a and b are one behavior, the injected pin SELECTING from the verified
# set, so they stay together here rather than splitting across the two files.
#
set -euo pipefail

# shellcheck source=test/unit/helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# --- a) an injected socket that IS a verified in-tab candidate ---------------
# Topology runs for the injected case too: the pin selects from the verified
# set rather than bypassing it.
setup_case injected-match
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/n1.sock"
printf '%s|w1:p2 4242\n' "$CASE/run/n1.sock" >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1 NVIM_MCP_SOCKET="$CASE/run/n1.sock"
[[ $RC -eq 0 ]] || fail "injected-match: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qF -- "--connect $CASE/run/n1.sock" "$CASE/exec" ||
  fail "injected-match: the server was not run against the pinned socket"
[[ -f $CASE/herdr-argv ]] || fail 'injected-match: topology was skipped for the injected path'

# --- b) an injected socket that is NOT a verified candidate ------------------
setup_case injected-mismatch
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/n1.sock"
{
  printf '%s|w1:p2 4242\n' "$CASE/run/n1.sock"
  printf '%s|w1:p9 9999\n' "$CASE/run/elsewhere.sock"
} >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1 NVIM_MCP_SOCKET="$CASE/run/elsewhere.sock"
[[ $RC -eq 3 ]] || fail "injected-mismatch: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qF "$CASE/run/elsewhere.sock" "$CASE/err" ||
  fail "injected-mismatch: the refusal does not name the socket asked for ($(cat "$CASE/err"))"
grep -qF "w1:t1" "$CASE/err" || fail "injected-mismatch: the refusal does not name the tab"
[[ ! -f $CASE/exec ]] || fail 'injected-mismatch: it connected anyway'

# --- c) a registry record whose identity matches -----------------------------
setup_case registry-hit
write_layout w1:t1 w1:p1 w1:p2
make_socket "$CASE/run/n1.sock"
record w1:p2 4242 "$CASE/run/n1.sock"
printf '%s|w1:p2 4242\n' "$CASE/run/n1.sock" >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 0 ]] || fail "registry-hit: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qF -- "--connect $CASE/run/n1.sock" "$CASE/exec" || fail 'registry-hit: wrong socket'

# --- d) identity mismatch deletes ONLY that record, the next one wins --------
# The stale record's socket ANSWERS (a different Neovim reused the path), which
# is exactly why presence is not identity.
setup_case identity-mismatch
write_layout w1:t1 w1:p1 w1:p2 w1:p3
record w1:p2 4242 "$CASE/run/stale.sock"
record w1:p3 4343 "$CASE/run/live.sock"
{
  printf '%s|w1:p9 9999\n' "$CASE/run/stale.sock"
  printf '%s|w1:p3 4343\n' "$CASE/run/live.sock"
} >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 0 ]] || fail "identity-mismatch: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qF -- "--connect $CASE/run/live.sock" "$CASE/exec" ||
  fail 'identity-mismatch: resolved the stale record'
[[ ! -e $CASE/registry/4242 ]] || fail 'identity-mismatch: the stale record was not deleted'
[[ -e $CASE/registry/4343 ]] || fail 'identity-mismatch: it deleted the good record too'

# --- f) two verified candidates are a picker, not a guess --------------------
setup_case picker
write_layout w1:t1 w1:p1 w1:p2 w1:p3
record w1:p2 4242 "$CASE/run/a.sock"
record w1:p3 4343 "$CASE/run/b.sock"
{
  printf '%s|w1:p2 4242\n' "$CASE/run/a.sock"
  printf '%s|w1:p3 4343\n' "$CASE/run/b.sock"
} >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 4 ]] || fail "picker: expected exit 4, got $RC ($(cat "$CASE/err"))"
grep -qF "$CASE/run/a.sock" "$CASE/err" || fail 'picker: candidate a is not enumerated'
grep -qF "$CASE/run/b.sock" "$CASE/err" || fail 'picker: candidate b is not enumerated'
[[ ! -f $CASE/exec ]] || fail 'picker: it guessed and ran the server anyway'

# --- g) a nested Neovim shares its parent's pane id and both survive ---------
# One record file per pid is what makes this two candidates; a single shared
# file keyed by pane would have kept only the last writer and picked it
# silently.
setup_case nested-pane
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/outer.sock"
record w1:p2 4343 "$CASE/run/inner.sock"
{
  printf '%s|w1:p2 4242\n' "$CASE/run/outer.sock"
  printf '%s|w1:p2 4343\n' "$CASE/run/inner.sock"
} >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 4 ]] || fail "nested-pane: expected exit 4, got $RC ($(cat "$CASE/err"))"
grep -qF "$CASE/run/outer.sock" "$CASE/err" || fail 'nested-pane: the outer instance is not enumerated'
grep -qF "$CASE/run/inner.sock" "$CASE/err" || fail 'nested-pane: the inner instance is not enumerated'

# --- h) a dead-pid socket is filtered before any RPC -------------------------
# No registry record at all, so the runtime-root glob is the source. The
# evaluation found 383 dead sockets and 0 live ones there.
setup_case dead-socket
write_layout w1:t1 w1:p1 w1:p4
mkdir -p "$CASE/run/aaa" "$CASE/run/bbb"
chmod 700 "$CASE/run/aaa" "$CASE/run/bbb"
dead_sock="$CASE/run/aaa/nvim.$dead_pid.0"
live_sock="$CASE/run/bbb/nvim.$$.0"
# BOTH are real sockets in a private directory, which is what the graveyard
# actually holds: Neovim leaves the socket file behind whenever it does not exit
# cleanly. So kill -0 on the pid in the filename is the ONLY thing that can tell
# these two apart before a connection is attempted.
make_socket "$dead_sock"
make_socket "$live_sock"
{
  printf '%s|w1:p4 %s\n' "$dead_sock" "$dead_pid"
  printf '%s|w1:p4 %s\n' "$live_sock" "$$"
} >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 0 ]] || fail "dead-socket: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qF -- "--connect $live_sock" "$CASE/exec" || fail 'dead-socket: wrong socket'
grep -qF "$dead_sock" "$CASE/probed" 2>/dev/null &&
  fail 'dead-socket: the dead socket was probed over RPC instead of filtered by kill -0'

# --- q) a successful resolution leaves no probe file behind -----------------
# exec REPLACES this process, so the EXIT trap never runs and the probe file has
# to be removed by hand first.
setup_case probe-file-cleanup
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/n1.sock"
printf '%s|w1:p2 4242\n' "$CASE/run/n1.sock" >"$CASE/identity"
# TMPDIR is the case's own directory, so the probe file is the only thing that
# can be in it.
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 0 ]] || fail "probe-file-cleanup: expected exit 0, got $RC ($(cat "$CASE/err"))"
left="$(find "$CASE/tmp" -type f | wc -l | tr -d ' ')"
[[ $left -eq 0 ]] ||
  fail "probe-file-cleanup: the probe file survived the exec ($left left in the case TMPDIR)"

printf 'PASS: %s (8 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
