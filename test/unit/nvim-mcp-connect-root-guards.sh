#!/usr/bin/env bash
set -euo pipefail

# shellcheck source-path=SCRIPTDIR
# shellcheck source=helpers/nvim-mcp-connect.sh
source "$(dirname "${BASH_SOURCE[0]}")/helpers/nvim-mcp-connect.sh"

# --- n) nvim answers the run-dir query with nothing usable -------------------
# An empty XDG_RUNTIME_DIR is one way to get there: Neovim then reports an
# empty stdpath("run") and starts no server at all.
setup_case no-run-dir
me term_a
: >"$CASE/rundir"
run_case
[[ $RC -eq 2 ]] || fail "no-run-dir: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -qF 'run dir' "$CASE/err" || fail "no-run-dir: the fault does not say what was missing ($(cat "$CASE/err"))"
[[ ! -e $CASE/probed ]] || fail 'no-run-dir: something was probed with no root to look in'
[[ ! -f $CASE/exec ]] || fail 'no-run-dir: the server was run anyway'

# --- o) a run root that is not this user's private directory ----------------
# Neovim falls back to <temp>/nvim.<random> when nvim.<user> is mis-owned, and
# <temp> can be a shared /tmp: a socket there is one any account can pre-create.
setup_case loose-root
me term_a
live "$(sock term_a)"
chmod 755 "$RUN"
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 2 ]] || fail "loose-root: expected exit 2, got $RC ($(cat "$CASE/err"))"
grep -qF '0700' "$CASE/err" || fail "loose-root: the fault does not name the mode ($(cat "$CASE/err"))"
grep -qF "$RUN" "$CASE/err" || fail "loose-root: the fault does not name the root ($(cat "$CASE/err"))"
[[ ! -e $CASE/probed ]] || fail 'loose-root: a socket in an untrusted root was probed'
[[ ! -f $CASE/exec ]] || fail 'loose-root: it connected anyway'

# --- p) a socket path longer than a unix socket allows -----------------------
# sun_path is 104 bytes on macOS (108 on Linux), NUL included. A root deep
# enough pushes the name past it; the bind would fail with a bare "invalid
# argument" on the editor's side and a probe here would find nothing, so the
# resolver says what happened instead of refusing as if no Neovim existed.
setup_case long-root
RUN="$CASE/run/$(printf 'x%.0s' {1..60})"
mkdir -p "$RUN"
chmod 700 "$RUN"
me term_a
run_case XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 5 ]] || fail "long-root: expected exit 5, got $RC ($(cat "$CASE/err"))"
grep -qF "$(printf '%s' "$(sock term_a)" | wc -c | tr -d ' ') bytes" "$CASE/err" ||
  fail "long-root: the message does not name the length ($(cat "$CASE/err"))"
grep -qF 'allow 103' "$CASE/err" || fail "long-root: the message does not name the limit ($(cat "$CASE/err"))"
[[ ! -e $CASE/probed ]] || fail 'long-root: a path over the limit was probed'
[[ ! -f $CASE/exec ]] || fail 'long-root: it connected anyway'

# UTF-8 characters consume more bytes than Bash's character count. The path
# fits by characters but exceeds sun_path in bytes, and must be refused first.
setup_case utf8-root
RUN="$CASE/run/$(printf 'é%.0s' {1..35})"
mkdir -p "$RUN"
chmod 700 "$RUN"
me term_a
run_case LC_ALL=en_US.UTF-8 XDG_RUNTIME_DIR="$RUN"
[[ $RC -eq 5 ]] || fail "utf8-root: expected exit 5, got $RC ($(cat "$CASE/err"))"
grep -qF "$(printf '%s' "$(sock term_a)" | wc -c | tr -d ' ') bytes" "$CASE/err" || fail 'utf8-root: wrong byte count'
[[ ! -e $CASE/probed && ! -f $CASE/exec ]] || fail 'utf8-root: overlong path was used'

printf 'PASS: %s (4 cases)\n' "$(basename "${BASH_SOURCE[0]}")"
