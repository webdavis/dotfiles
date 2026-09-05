#!/usr/bin/env bash
# nvim-mcp-connect.sh, the resolver that answers "which Neovim socket should this
# agent talk to" (spec 7.3, five steps). One behavior per case:
#
#   a) NVIM_MCP_SOCKET set              -> that socket is used, herdr never asked
#   b) registry hit, identity matches   -> that socket is used
#   c) identity MISMATCH                -> the entry is pruned and the next candidate wins
#   d) zero candidates                  -> refusal naming its reason, exit 3
#   e) two verified candidates          -> picker, exit 4, both enumerated
#   f) a dead-pid socket in the glob    -> filtered by kill -0 BEFORE any RPC
#   g) nvim missing from PATH           -> exit 2 naming nvim, nothing else touched
#   h) jq missing from PATH             -> exit 2 naming jq, nothing else touched
#
# herdr, nvim and the nvim-mcp binary are all fake executables in a per-case
# PATH, so no herdr server and no Neovim is ever contacted.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/libexec/nvim-mcp/executable_nvim-mcp-connect.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if ! command -v jq >/dev/null 2>&1; then
  printf 'SKIP: jq not on PATH; the resolver parses herdr JSON with jq\n'
  exit 0
fi
[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# A pid that is certainly not running, so kill -0 on it fails: the dead half of
# case (f). Searched for rather than assumed, because a hardcoded number can be
# alive on somebody's machine.
dead_pid=0
for candidate in 999331 999332 999333 4194301; do
  if ! kill -0 "$candidate" 2>/dev/null; then
    dead_pid="$candidate"
    break
  fi
done
[[ $dead_pid != 0 ]] || fail 'could not find a pid that is not running'

# setup_case <name> -- a private sandbox with stub herdr/nvim/nvim-mcp on PATH.
# Sets CASE (its directory) for the caller to fill in:
#   $CASE/layout.json   what the herdr stub prints for `pane layout`
#   $CASE/identity      "<socket>|<answer>" lines the nvim stub replies with
#   $CASE/registry      the resolver's registry file
#   $CASE/probed        every socket the nvim stub was asked about (one per line)
#   $CASE/exec          the argv the nvim-mcp stub was execed with
setup_case() {
  CASE="$work/$1"
  CASE_PATH="$CASE/bin:/usr/bin:/bin:/usr/sbin:/sbin"
  mkdir -p "$CASE/bin" "$CASE/run"
  : >"$CASE/identity"
  : >"$CASE/registry"
  printf '%s' '{"error":{"code":"pane_not_found","message":"pane not found"},"id":"cli:pane:layout"}' >"$CASE/layout.json"

  cat >"$CASE/bin/herdr" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$CASE/herdr-argv"
[[ "\$1 \$2" == "pane layout" ]] || exit 1
cat "$CASE/layout.json"
STUB

  # nvim --server <socket> --remote-expr <expr>: the identity probe. Every call
  # is logged BEFORE the answer is looked up, which is what case (f) asserts on.
  cat >"$CASE/bin/nvim" <<STUB
#!/bin/bash
sock=""
while [[ \$# -gt 0 ]]; do
  case "\$1" in
    --server)
      sock="\$2"
      shift 2
      ;;
    *) shift ;;
  esac
done
printf '%s\n' "\$sock" >>"$CASE/probed"
answer="\$(awk -F'|' -v s="\$sock" '\$1 == s { print \$2; exit }' "$CASE/identity")"
[[ -n \$answer ]] || exit 1
printf '%s' "\$answer"
STUB

  cat >"$CASE/bin/nvim-mcp" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >"$CASE/exec"
STUB

  chmod +x "$CASE/bin/herdr" "$CASE/bin/nvim" "$CASE/bin/nvim-mcp"
}

# run_case <env assignments...> -- runs the resolver in the current CASE, on
# CASE_PATH (the stub dir plus the system dirs unless a case narrowed it).
# Sets RC and leaves stdout in $CASE/out, stderr in $CASE/err.
run_case() {
  RC=0
  env -i \
    PATH="$CASE_PATH" \
    HOME="$CASE" \
    NVIM_MCP_REGISTRY="$CASE/registry" \
    NVIM_MCP_BIN="$CASE/bin/nvim-mcp" \
    XDG_RUNTIME_DIR="$CASE/run" \
    "$@" \
    bash "$SCRIPT" >"$CASE/out" 2>"$CASE/err" || RC=$?
}

# A herdr layout naming <tab> and the panes that follow.
write_layout() { # <tab> <pane...>
  local tab="$1"
  shift
  local panes=""
  local p
  for p in "$@"; do
    panes="$panes{\"pane_id\":\"$p\"},"
  done
  printf '{"id":"cli:pane:layout","result":{"layout":{"panes":[%s],"tab_id":"%s","workspace_id":"w1"},"type":"pane_layout"}}' \
    "${panes%,}" "$tab" >"$CASE/layout.json"
}

# --- a) an injected socket wins, and herdr is never asked --------------------
setup_case injected
printf '%s|w1:p9 4242\n' "$CASE/run/pinned.sock" >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1 NVIM_MCP_SOCKET="$CASE/run/pinned.sock"
[[ $RC -eq 0 ]] || fail "injected: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qF -- "--connect $CASE/run/pinned.sock" "$CASE/exec" ||
  fail "injected: the server was not run against the pinned socket ($(cat "$CASE/exec" 2>/dev/null))"
[[ ! -f $CASE/herdr-argv ]] || fail "injected: herdr was consulted ($(cat "$CASE/herdr-argv"))"

# --- b) a registry hit whose identity matches --------------------------------
setup_case registry-hit
write_layout w1:t1 w1:p1 w1:p2
printf 'w1:p2 4242 %s /repo\n' "$CASE/run/n1.sock" >"$CASE/registry"
printf '%s|w1:p2 4242\n' "$CASE/run/n1.sock" >"$CASE/identity"
: >"$CASE/run/n1.sock"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 0 ]] || fail "registry-hit: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qF -- "--connect $CASE/run/n1.sock" "$CASE/exec" ||
  fail "registry-hit: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"

# --- c) identity mismatch prunes the entry and the next candidate wins -------
# The stale entry's socket ANSWERS (a different Neovim reused the path), which
# is exactly why presence is not identity.
setup_case identity-mismatch
write_layout w1:t1 w1:p1 w1:p2 w1:p3
{
  printf 'w1:p2 4242 %s /repo\n' "$CASE/run/stale.sock"
  printf 'w1:p3 4343 %s /repo\n' "$CASE/run/live.sock"
} >"$CASE/registry"
{
  printf '%s|w1:p9 9999\n' "$CASE/run/stale.sock"
  printf '%s|w1:p3 4343\n' "$CASE/run/live.sock"
} >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 0 ]] || fail "identity-mismatch: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qF -- "--connect $CASE/run/live.sock" "$CASE/exec" ||
  fail "identity-mismatch: resolved the stale entry ($(cat "$CASE/exec" 2>/dev/null))"
grep -qF "stale.sock" "$CASE/registry" &&
  fail "identity-mismatch: the mismatched entry was not pruned ($(cat "$CASE/registry"))"

# --- d) zero candidates refuses, naming the reason ---------------------------
setup_case refusal
write_layout w1:t3 w1:p6
run_case HERDR_PANE_ID=w1:p6
[[ $RC -eq 3 ]] || fail "refusal: expected exit 3, got $RC"
grep -q "w1:t3" "$CASE/err" || fail "refusal: the reason does not name the tab ($(cat "$CASE/err"))"
grep -q "NVIM_MCP_SOCKET" "$CASE/err" || fail "refusal: the reason does not say how to fix it ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'refusal: the server was run anyway'

# --- e) two verified candidates are a picker, not a guess --------------------
setup_case picker
write_layout w1:t1 w1:p1 w1:p2 w1:p3
{
  printf 'w1:p2 4242 %s /repo\n' "$CASE/run/a.sock"
  printf 'w1:p3 4343 %s /repo\n' "$CASE/run/b.sock"
} >"$CASE/registry"
{
  printf '%s|w1:p2 4242\n' "$CASE/run/a.sock"
  printf '%s|w1:p3 4343\n' "$CASE/run/b.sock"
} >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 4 ]] || fail "picker: expected exit 4, got $RC ($(cat "$CASE/err"))"
grep -qF "$CASE/run/a.sock" "$CASE/err" || fail "picker: candidate a is not enumerated ($(cat "$CASE/err"))"
grep -qF "$CASE/run/b.sock" "$CASE/err" || fail "picker: candidate b is not enumerated ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'picker: it guessed and ran the server anyway'

# --- f) a dead-pid socket is filtered before any RPC -------------------------
# No registry at all, so the runtime-root glob is the source. The evaluation
# found 383 dead sockets and 0 live ones there; probing them costs 17 ms each,
# so kill -0 on the filename pid must come FIRST.
setup_case dead-socket
write_layout w1:t1 w1:p1 w1:p4
mkdir -p "$CASE/run/aaa" "$CASE/run/bbb"
dead_sock="$CASE/run/aaa/nvim.$dead_pid.0"
live_sock="$CASE/run/bbb/nvim.$$.0"
: >"$dead_sock"
: >"$live_sock"
{
  printf '%s|w1:p4 %s\n' "$dead_sock" "$dead_pid"
  printf '%s|w1:p4 %s\n' "$live_sock" "$$"
} >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 0 ]] || fail "dead-socket: expected exit 0, got $RC ($(cat "$CASE/err"))"
grep -qF -- "--connect $live_sock" "$CASE/exec" ||
  fail "dead-socket: wrong socket ($(cat "$CASE/exec" 2>/dev/null))"
grep -qF "$dead_sock" "$CASE/probed" 2>/dev/null &&
  fail 'dead-socket: the dead socket was probed over RPC instead of filtered by kill -0'

# --- g) nvim missing: exit 2 naming it, before herdr or the registry ---------
# Without this guard the first thing to fail is the herdr layout read, which
# reports "herdr did not report a layout" and sends the operator to debug herdr
# for a tool that is simply not installed.
setup_case no-nvim
write_layout w1:t1 w1:p1 w1:p2
printf 'w1:p2 4242 %s /repo\n' "$CASE/run/n1.sock" >"$CASE/registry"
cp "$CASE/registry" "$CASE/registry.before"
rm -f "$CASE/bin/nvim"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 2 ]] || fail "no-nvim: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -q "nvim" "$CASE/err" || fail "no-nvim: the message does not name nvim ($(cat "$CASE/err"))"
[[ ! -f $CASE/herdr-argv ]] || fail "no-nvim: herdr was consulted ($(cat "$CASE/herdr-argv"))"
cmp -s "$CASE/registry" "$CASE/registry.before" || fail 'no-nvim: the registry was rewritten'
[[ ! -f $CASE/exec ]] || fail 'no-nvim: the server was run anyway'

# --- h) jq missing: exit 2 naming it, before herdr or the registry -----------
# A PATH holding nothing but the stubs, because macOS ships jq in /usr/bin.
setup_case no-jq
CASE_PATH="$CASE/bin:/bin"
write_layout w1:t1 w1:p1 w1:p2
printf 'w1:p2 4242 %s /repo\n' "$CASE/run/n1.sock" >"$CASE/registry"
cp "$CASE/registry" "$CASE/registry.before"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 2 ]] || fail "no-jq: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -q "jq" "$CASE/err" || fail "no-jq: the message does not name jq ($(cat "$CASE/err"))"
[[ ! -f $CASE/herdr-argv ]] || fail "no-jq: herdr was consulted ($(cat "$CASE/herdr-argv"))"
cmp -s "$CASE/registry" "$CASE/registry.before" || fail 'no-jq: the registry was rewritten'
[[ ! -f $CASE/exec ]] || fail 'no-jq: the server was run anyway'

printf 'PASS: nvim-mcp-connect.sh\n'
