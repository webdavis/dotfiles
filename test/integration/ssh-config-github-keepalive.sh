#!/usr/bin/env bash
# ssh-config-github-keepalive.sh, the managed ssh config must keep a GitHub
# connection alive while a slow pre-push hook runs.
#
# Git starts the remote helper and reads the ref advertisement BEFORE running
# the pre-push hook, so the connection sits idle for the hook's whole runtime.
# Idle long enough and GitHub closes it; when the hook returns, git writes into
# a dead socket and exits 141 with NO error text and no branch pushed, while the
# hook's own "checks passed" line is the last thing on screen. Traffic on an
# interval keeps the connection from going idle.
#
# This config is MACHINE-WIDE while the hook is per-repository: the user-wide
# pre-push dispatcher runs whatever hook a repository ships. So this asserts a
# machine-wide property and is deliberately not sized to any one repository's
# hook (the dotfiles hook takes seconds today, and its own header carries that
# measurement; either way it is not what these numbers protect). The reasoning
# is recorded in private_dot_ssh/config, next to the setting.
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
# Floor on interval x CountMax, the span of server silence ssh rides out before
# it declares the connection dead. This is NOT what defeats GitHub's idle close
# (the interval is), so the floor is not sized to any hook's runtime: it keeps a
# future edit from shrinking the tolerance to where a transient stall kills the
# push, which is the same silent failure the keepalive exists to prevent.
MINIMUM_TOLERATED_SILENCE_SECONDS=600

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

# ---- 3: ssh must ride out a stall rather than tear the connection down ------
# Interval times CountMax is how long ssh keeps going without a reply from the
# server. Too small and a transient stall drops the push mid-hook, trading one
# silent failure for another.
count_max="$(resolved github.com serveralivecountmax)"
[[ $count_max =~ ^[0-9]+$ ]] || fail "ServerAliveCountMax is not numeric: $count_max"
tolerated=$((WANT_INTERVAL * count_max))
[[ $tolerated -ge $MINIMUM_TOLERATED_SILENCE_SECONDS ]] ||
  fail "keepalives tolerate only ${tolerated}s of server silence (interval $WANT_INTERVAL x count $count_max), want at least ${MINIMUM_TOLERATED_SILENCE_SECONDS}s; below that a stall drops the connection on its own"

printf 'ssh-config-github-keepalive: OK (github.com resolves User=git with ServerAliveInterval=%s, alias intact, %ss tolerated)\n' \
  "$WANT_INTERVAL" "$tolerated"
