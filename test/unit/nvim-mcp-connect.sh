#!/usr/bin/env bash
# nvim-mcp-connect.sh, the resolver that answers "which Neovim socket should this
# agent talk to" (spec 7.3). One behavior per case:
#
#   a) an injected socket that IS a verified in-tab candidate -> it is used
#   b) an injected socket that is NOT -> refused, naming the socket and the tab
#   c) a registry record whose identity matches           -> that socket is used
#   d) an identity MISMATCH  -> only that record is deleted, the next one wins
#   e) zero candidates       -> refusal naming its reason, exit 3
#   f) two candidates        -> picker, exit 4, both enumerated
#   g) two instances sharing ONE pane id -> both survive and both are enumerated
#   h) a dead-pid socket in the glob -> filtered by kill -0 BEFORE any RPC
#   i) nvim missing from PATH -> exit 2, the whole diagnostic, nothing touched
#   j) jq missing from PATH   -> exit 2, the whole diagnostic, nothing touched
#   k) a symlinked registry   -> exit 2, refused before any read or prune
#   l) a registry looser than 0700 -> exit 2, refused the same way
#   m) a probe that never answers  -> bounded, and not a candidate
#   n) an oversized reply     -> never reaches the server
#   o) a multi-line reply     -> never reaches the server
#   p) a correct identity then padding then garbage -> never reaches the server
#   q) a successful resolution leaves no probe file behind
#   r) a record naming a TCP endpoint  -> refused, exit 3, never connected to
#   s) a socket outside a private tree -> refused, exit 3, never connected to
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
if [[ ! -x /usr/bin/perl ]]; then
  printf 'SKIP: /usr/bin/perl is absent, and the socket cases need a real unix socket\n'
  exit 0
fi
[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"
JQ_PATH="$(command -v jq)"

# make_socket <path> -- a real, bound, listening unix socket. System perl,
# because bash cannot make one and the resolver's `-S` test wants the real
# thing rather than a regular file standing in for it.
make_socket() {
  /usr/bin/perl -MIO::Socket::UNIX -e \
    'IO::Socket::UNIX->new(Local => $ARGV[0], Listen => 1) or die $!' "$1"
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# A pid that is certainly not running, so kill -0 on it fails: the dead half of
# case (h). Searched for rather than assumed, because a hardcoded number can be
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
#   $CASE/identity      "<socket>|<reply>" lines the nvim stub answers with
#   $CASE/hang          sockets the nvim stub never answers on
#   $CASE/registry      the registry DIRECTORY, one file per instance pid
#   $CASE/probed        every socket the nvim stub was asked about
#   $CASE/exec          the argv the nvim-mcp stub was execed with
setup_case() {
  CASE="$work/$1"
  CASE_PATH="$CASE/bin:/usr/bin:/bin:/usr/sbin:/sbin"
  mkdir -p "$CASE/bin" "$CASE/run" "$CASE/registry" "$CASE/tmp"
  # 0700, because the resolver refuses a socket outside a private tree.
  chmod 700 "$CASE/registry" "$CASE/run"
  : >"$CASE/identity"
  : >"$CASE/hang"
  printf '%s' '{"error":{"code":"pane_not_found"},"id":"cli:pane:layout"}' >"$CASE/layout.json"

  cat >"$CASE/bin/herdr" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$CASE/herdr-argv"
[[ "\$1 \$2" == "pane layout" ]] || exit 1
cat "$CASE/layout.json"
STUB

  # nvim --server <socket> --remote-expr <expr>: the identity probe. Every call
  # is logged BEFORE the answer is looked up, which is what case (h) asserts on.
  # %b so an identity reply can carry an escaped newline (case o).
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
if grep -qxF "\$sock" "$CASE/hang"; then
  sleep 3
  exit 0
fi
answer="\$(awk -F'|' -v s="\$sock" '\$1 == s { print \$2; exit }' "$CASE/identity")"
[[ -n \$answer ]] || exit 1
if [[ \$answer == @* ]]; then
  cat "\${answer#@}"
  exit 0
fi
printf '%b' "\$answer"
STUB

  cat >"$CASE/bin/nvim-mcp" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >"$CASE/exec"
STUB

  chmod +x "$CASE/bin/herdr" "$CASE/bin/nvim" "$CASE/bin/nvim-mcp"
}

# private_path <name>=<target>... -- a PATH holding ONLY what is named, plus a
# bash and an env of its own, so an absence fixture cannot be invalidated by
# whatever another host happens to keep in /usr/bin.
private_path() {
  mkdir -p "$CASE/pathbin"
  ln -s /bin/bash "$CASE/pathbin/bash"
  ln -s /usr/bin/env "$CASE/pathbin/env"
  ln -s /usr/bin/mktemp "$CASE/pathbin/mktemp"
  local spec
  for spec in "$@"; do
    ln -s "${spec#*=}" "$CASE/pathbin/${spec%%=*}"
  done
  CASE_PATH="$CASE/pathbin"
}

# record <pane> <pid> <socket> -- one registry file, named for the pid, the way
# the VimEnter autocmd writes it.
record() {
  printf '%s %s %s /repo\n' "$1" "$2" "$3" >"$CASE/registry/$2"
}

# A herdr layout naming <tab> and the panes that follow.
write_layout() { # <tab> <pane...>
  local tab="$1"
  shift
  local panes="" p
  for p in "$@"; do
    panes="$panes{\"pane_id\":\"$p\"},"
  done
  printf '{"id":"cli:pane:layout","result":{"layout":{"panes":[%s],"tab_id":"%s","workspace_id":"w1"},"type":"pane_layout"}}' \
    "${panes%,}" "$tab" >"$CASE/layout.json"
}

# run_case <env assignments...> -- runs the resolver in the current CASE, on
# CASE_PATH. Sets RC; stdout is $CASE/out, stderr is $CASE/err.
run_case() {
  RC=0
  env -i \
    PATH="$CASE_PATH" \
    HOME="$CASE" \
    NVIM_MCP_REGISTRY="$CASE/registry" \
    NVIM_MCP_BIN="$CASE/bin/nvim-mcp" \
    XDG_RUNTIME_DIR="$CASE/run" \
    TMPDIR="$CASE/tmp" \
    NVIM_MCP_PROBE_DEADLINE=0.1 \
    "$@" \
    bash "$SCRIPT" >"$CASE/out" 2>"$CASE/err" || RC=$?
}

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

# --- e) zero candidates refuses, naming the reason ---------------------------
setup_case refusal
write_layout w1:t3 w1:p6
run_case HERDR_PANE_ID=w1:p6
[[ $RC -eq 3 ]] || fail "refusal: expected exit 3, got $RC"
grep -q "w1:t3" "$CASE/err" || fail 'refusal: the reason does not name the tab'
grep -q "NVIM_MCP_SOCKET" "$CASE/err" || fail 'refusal: the reason does not say how to fix it'
[[ ! -f $CASE/exec ]] || fail 'refusal: the server was run anyway'

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
# The dead one stays a plain file on purpose: kill -0 filters it before
# anything looks at what kind of file it is.
: >"$dead_sock"
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

# --- i) nvim missing: exit 2 naming it, before herdr or the registry ---------
# Without this guard the first thing to fail is the herdr layout read, which
# reports "herdr did not report a layout" and sends the operator to debug herdr
# for a tool that is simply not installed.
setup_case no-nvim
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/n1.sock"
private_path "herdr=$CASE/bin/herdr" "jq=$JQ_PATH" "nvim-mcp=$CASE/bin/nvim-mcp"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 2 ]] || fail "no-nvim: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -qxF 'nvim-mcp-connect: nvim is not on PATH, and the resolver needs it' "$CASE/err" ||
  fail "no-nvim: wrong diagnostic ($(cat "$CASE/err"))"
[[ ! -f $CASE/herdr-argv ]] || fail 'no-nvim: herdr was consulted'
[[ -e $CASE/registry/4242 ]] || fail 'no-nvim: the registry was pruned'
[[ ! -f $CASE/exec ]] || fail 'no-nvim: the server was run anyway'

# --- j) jq missing: exit 2 naming it, before herdr or the registry -----------
setup_case no-jq
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/n1.sock"
private_path "herdr=$CASE/bin/herdr" "nvim=$CASE/bin/nvim" "nvim-mcp=$CASE/bin/nvim-mcp"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 2 ]] || fail "no-jq: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -qxF 'nvim-mcp-connect: jq is not on PATH, and the resolver needs it' "$CASE/err" ||
  fail "no-jq: wrong diagnostic ($(cat "$CASE/err"))"
[[ ! -f $CASE/herdr-argv ]] || fail 'no-jq: herdr was consulted'
[[ -e $CASE/registry/4242 ]] || fail 'no-jq: the registry was pruned'

# --- k) a symlinked registry is refused before any read or prune -------------
# The resolver DELETES records, so a symlink here would aim that delete
# somewhere else entirely.
setup_case registry-symlink
write_layout w1:t1 w1:p1 w1:p2
mkdir -p "$CASE/elsewhere"
rmdir "$CASE/registry"
ln -s "$CASE/elsewhere" "$CASE/registry"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 2 ]] || fail "registry-symlink: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -q "symlink" "$CASE/err" || fail "registry-symlink: the message does not say why ($(cat "$CASE/err"))"
[[ ! -f $CASE/herdr-argv ]] || fail 'registry-symlink: herdr was consulted anyway'
[[ ! -f $CASE/exec ]] || fail 'registry-symlink: it connected anyway'

# --- l) a registry looser than 0700 is refused the same way ------------------
setup_case registry-mode
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/n1.sock"
printf '%s|w1:p2 4242\n' "$CASE/run/n1.sock" >"$CASE/identity"
chmod 755 "$CASE/registry"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 2 ]] || fail "registry-mode: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -q "0700" "$CASE/err" || fail "registry-mode: the message does not name the mode ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'registry-mode: it connected anyway'

# --- m) a probe that never answers is bounded and is not a candidate --------
setup_case probe-hangs
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/n1.sock"
printf '%s\n' "$CASE/run/n1.sock" >"$CASE/hang"
start="$SECONDS"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 3 ]] || fail "probe-hangs: expected exit 3, got $RC ($(cat "$CASE/err"))"
(($((SECONDS - start)) < 2)) || fail 'probe-hangs: the probe was not bounded'
[[ ! -f $CASE/exec ]] || fail 'probe-hangs: it connected to a socket that never answered'

# --- n) an oversized reply never reaches the server -------------------------
# The 128-byte cap and the "<pane> <decimal pid>" grammar are belt and braces
# here: the comparison against the record's own pane and pid already rejects
# this reply, so these two cases pin the OUTCOME, not which of the three checks
# produced it.
setup_case oversized-reply
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/n1.sock"
printf '%s|%s 4242\n' "$CASE/run/n1.sock" "$(printf 'w1:p2%.0s' {1..60})" >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 3 ]] || fail "oversized-reply: expected exit 3, got $RC ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'oversized-reply: it connected on an over-long reply'

# --- o) a multi-line reply never reaches the server -------------------------
setup_case multiline-reply
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/n1.sock"
printf '%s|w1:p2 4242\\nw1:p2 4242\n' "$CASE/run/n1.sock" >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 3 ]] || fail "multiline-reply: expected exit 3, got $RC ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'multiline-reply: it connected on a two-line reply'

# --- p) a valid identity followed by padding and garbage --------------------
# The byte cap used to TRUNCATE what had already been captured, so a reply that
# opened with the right identity, then newlines, then anything at all, lost its
# suffix to `head` and its newlines to the command substitution and was read as
# a clean identity.
setup_case padded-reply
write_layout w1:t1 w1:p1 w1:p2
record w1:p2 4242 "$CASE/run/n1.sock"
{
  printf 'w1:p2 4242'
  head -c 4096 /dev/zero | tr '\0' '\n'
  head -c 1048576 /dev/zero | tr '\0' 'G'
} >"$CASE/padded.bin"
printf '%s|@%s\n' "$CASE/run/n1.sock" "$CASE/padded.bin" >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 3 ]] || fail "padded-reply: expected exit 3, got $RC ($(cat "$CASE/err"))"
[[ ! -f $CASE/exec ]] || fail 'padded-reply: a padded reply was read as a clean identity'

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

# --- r) a record naming a TCP endpoint --------------------------------------
# `nvim --listen` takes a TCP address as readily as a path, and Neovim trusts
# every RPC peer, so another account can watch a TCP listener, learn the
# identity it reports, and rebind the port after a crash to replay it. The
# resolver will not connect to one.
setup_case tcp-endpoint
write_layout w1:t1 w1:p1 w1:p2
printf 'w1:p2 4242 127.0.0.1:6666 /repo\n' >"$CASE/registry/4242"
printf '127.0.0.1:6666|w1:p2 4242\n' >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 3 ]] || fail "tcp-endpoint: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qF '127.0.0.1:6666' "$CASE/err" || fail "tcp-endpoint: the refusal does not name it ($(cat "$CASE/err"))"
[[ ! -s $CASE/probed ]] || fail 'tcp-endpoint: it was connected to before being refused'
[[ ! -f $CASE/exec ]] || fail 'tcp-endpoint: the server was run against it'

# --- s) a real socket outside a caller-owned 0700 tree ----------------------
# A socket any account can reach is one any account can rebind after the owner
# dies, and the identity it then reports is whatever its owner chooses.
setup_case loose-socket
write_layout w1:t1 w1:p1 w1:p2
mkdir -p "$CASE/open"
chmod 755 "$CASE/open"
make_socket "$CASE/open/n1.sock"
record w1:p2 4242 "$CASE/open/n1.sock"
printf '%s|w1:p2 4242\n' "$CASE/open/n1.sock" >"$CASE/identity"
run_case HERDR_PANE_ID=w1:p1
[[ $RC -eq 3 ]] || fail "loose-socket: expected exit 3, got $RC ($(cat "$CASE/err"))"
grep -qF "$CASE/open" "$CASE/err" || fail "loose-socket: the refusal does not name it ($(cat "$CASE/err"))"
[[ ! -s $CASE/probed ]] || fail 'loose-socket: it was connected to before being refused'
[[ ! -f $CASE/exec ]] || fail 'loose-socket: the server was run against it'

printf 'PASS: nvim-mcp-connect.sh (19 cases)\n'
