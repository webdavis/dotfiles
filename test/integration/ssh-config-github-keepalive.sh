#!/usr/bin/env bash
# ssh-config-github-keepalive.sh, the managed ssh config must keep a GitHub
# connection alive while a slow pre-push hook runs.
#
# Git opens the connection to GitHub BEFORE running the pre-push hook. This
# repo's hook runs lint plus the whole test suite, several minutes, and GitHub
# closes the connection it considers idle in the meantime. When the hook finally
# returns, git writes into a dead socket and exits 141 with NO error text and no
# branch pushed, while the hook's own "checks passed" line is the last thing on
# screen. Traffic on an interval keeps the connection from going idle.
#
# Asserted through ssh's OWN parser (`ssh -G`), never by grepping the file: the
# question is what ssh RESOLVES for the name git actually connects to, and a
# block naming only the `github` alias resolves to nothing for `github.com`.
# That was the original defect and a grep for the keyword would have missed it.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIG="$REPO_ROOT/private_dot_ssh/config"
# The interval this repo sets. Pinned exactly, not merely "nonzero": ssh also
# reads the system-wide config, so a nonzero check could pass on someone else's
# setting while this file said nothing.
WANT_INTERVAL=20

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $CONFIG ]] || fail "missing managed ssh config: $CONFIG"
# ssh is system-shipped on every platform this repo targets, so its absence is a
# broken host rather than an optional tool to skip over.
command -v ssh >/dev/null 2>&1 || fail "ssh is not on PATH; it is system-shipped, so this is a broken host, not a reason to skip"

# resolved <host> <keyword> -> the value ssh computes, lowercased keyword match
resolved() {
  ssh -F "$CONFIG" -G "$1" 2>/dev/null | awk -v k="$2" 'tolower($1) == k { print $2; exit }'
}

# ---- 1: the name git actually connects to ----------------------------------
# Remotes are written git@github.com:owner/repo, so ssh matches `github.com`.
got="$(resolved github.com serveraliveinterval)"
[[ $got == "$WANT_INTERVAL" ]] ||
  fail "ssh resolves ServerAliveInterval=$got for github.com, want $WANT_INTERVAL; a push whose pre-push hook outlasts GitHub's idle timeout will die with exit 141 and no error"

# The block must genuinely match github.com, not just happen to set a keyword.
# User is the independent witness: it is git only if the Host pattern applied.
got="$(resolved github.com user)"
[[ $got == git ]] ||
  fail "ssh resolves User=$got for github.com, so the Host pattern does not cover the name git connects to (this is exactly how the keepalive went missing)"

# ---- 2: the alias keeps working --------------------------------------------
# `ssh github` is an established shorthand here; extending the pattern must not
# have cost it its settings.
got="$(resolved github user)"
[[ $got == git ]] || fail "the github alias no longer resolves User=git (got: $got)"
got="$(resolved github serveraliveinterval)"
[[ $got == "$WANT_INTERVAL" ]] ||
  fail "the github alias resolves ServerAliveInterval=$got, want $WANT_INTERVAL"

# ---- 3: the interval must survive long enough to cover the hook ------------
# Interval times CountMax is how long ssh tolerates silence. It has to exceed the
# hook's runtime or the client gives up mid-gate, trading one silent failure for
# another.
count_max="$(resolved github.com serveralivecountmax)"
[[ $count_max =~ ^[0-9]+$ ]] || fail "ServerAliveCountMax is not numeric: $count_max"
tolerated=$((WANT_INTERVAL * count_max))
[[ $tolerated -ge 600 ]] ||
  fail "keepalives tolerate only ${tolerated}s of silence (interval $WANT_INTERVAL x count $count_max); the pre-push gate runs longer than that, so the connection can still drop"

printf 'ssh-config-github-keepalive: OK (github.com resolves User=git with ServerAliveInterval=%s, alias intact, %ss tolerated)\n' \
  "$WANT_INTERVAL" "$tolerated"
