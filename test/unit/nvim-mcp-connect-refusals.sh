#!/usr/bin/env bash
# nvim-mcp-connect.sh, the REFUSING half: every case that ends with the resolver
# declining to connect, and the guards that run before it looks at anything.
# The resolutions live in nvim-mcp-connect-resolution.sh, so that each file
# stays inside the one-second budget.
#
#   e) zero candidates       -> refusal naming its reason, exit 3
#   i) nvim missing from PATH -> exit 2, the whole diagnostic, nothing touched
#   j) jq missing from PATH   -> exit 2, the whole diagnostic, nothing touched
#   k) a symlinked registry   -> exit 2, refused before any read or prune
#   l) a registry looser than 0700 -> exit 2, refused the same way
#   m) a probe that never answers  -> bounded, and not a candidate
#   n) an oversized reply     -> never reaches the server
#   o) a multi-line reply     -> never reaches the server
#   p) a correct identity then padding then garbage -> never reaches the server
#   r) a record naming a TCP endpoint  -> refused, exit 3, never connected to
#   s) a socket outside a private tree -> refused, exit 3, never connected to
#
set -euo pipefail

# shellcheck source=test/unit/helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# --- e) zero candidates refuses, naming the reason ---------------------------
setup_case refusal
write_layout w1:t3 w1:p6
run_case HERDR_PANE_ID=w1:p6
[[ $RC -eq 3 ]] || fail "refusal: expected exit 3, got $RC"
grep -q "w1:t3" "$CASE/err" || fail 'refusal: the reason does not name the tab'
grep -q "NVIM_MCP_SOCKET" "$CASE/err" || fail 'refusal: the reason does not say how to fix it'
[[ ! -f $CASE/exec ]] || fail 'refusal: the server was run anyway'

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

printf 'PASS: %s (11 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
