#!/usr/bin/env bash
# Two Neovims starting at the same moment against an EMPTY registry must BOTH
# register themselves.
#
# `vim.fn.mkdir(dir, "p")` is not race free: it tests for the directory and then
# creates it, so when two instances both find it missing the loser gets E739 out
# of the create and never writes its record. One record then survives where
# there are two live editors, and because the resolver's runtime-root fallback
# only runs when the registry yields NOTHING, that single record suppresses
# discovery and is selected silently instead of raising the picker.
#
# FOUR fresh registries, not one. Which instance loses the create is a
# scheduling outcome, so a single pair reproduced the loss only about one run in
# three; four consecutive pairs over a warm cache made it reliable, and after
# the fix every pair passes whichever instance wins.
#
# Real Neovim, because the race is inside Neovim's own mkdir. The registering
# block is extracted from the config and run under a bare harness, so this costs
# eight `--clean` startups (about 20 ms each) rather than a plugin tree.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SOURCE="$REPO_ROOT/dot_config/nvim/lua/config/autocmds.lua"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if ! command -v nvim >/dev/null 2>&1; then
  printf 'SKIP: nvim is not on PATH, and the registry race lives inside its mkdir\n'
  exit 0
fi
[[ -f $SOURCE ]] || fail "missing source: $SOURCE"

work="$(mktemp -d)"
pids=()

reap() {
  local p
  for p in ${pids[@]+"${pids[@]}"}; do
    kill "$p" 2>/dev/null || true
  done
  pids=()
}

trap 'reap; rm -rf "$work"' EXIT

# The registering block is the tail of the file, from its own header comment.
# `augroup` is the one local it borrows from the surrounding file.
awk '/^-- Tell the nvim-mcp resolver/,0' "$SOURCE" >"$work/block.lua"
[[ -s $work/block.lua ]] || fail 'could not find the registry block in autocmds.lua'
printf 'augroup = function(n) return vim.api.nvim_create_augroup("nvim_config_" .. n, { clear = true }) end\ndofile("%s/block.lua")\n' \
  "$work" >"$work/init.lua"

for attempt in 1 2 3 4; do
  registry="$work/state$attempt/nvim-mcp/registry"
  [[ ! -e $registry ]] || fail "attempt $attempt: the registry existed before the run"

  # Both instances share one pane id on purpose: that is the nested-Neovim
  # shape, and it is the case a per-pane key would also have collapsed.
  #
  # `exec` so the recorded pid IS Neovim rather than the subshell around it, or
  # the reap kills a wrapper and leaves the editor running. stdout goes to
  # /dev/null for the same reason: an inherited pipe Neovim holds open outlives
  # this script and hangs whatever is reading it.
  for socket in a b; do
    (
      cd "$work" &&
        exec env HERDR_PANE_ID=w1:p2 XDG_STATE_HOME="$work/state$attempt" \
          nvim --clean --headless -u "$work/init.lua" \
          --listen "$work/$attempt.$socket.sock" \
          >/dev/null 2>"$work/err.$attempt.$socket"
    ) &
    pids+=("$!")
  done

  # Poll rather than sleep a fixed slice: a healthy pair registers in about
  # 60 ms. `|| true` because under pipefail a find over a directory that does
  # not exist yet fails the whole assignment, and errexit would end the poll on
  # its first pass, before either instance had a chance to create it. `if`
  # rather than `&&` for the break, because a false test as the loop's last
  # command is the loop's status and would end the run silently.
  records=0
  for _ in $(seq 1 40); do
    records="$(find "$registry" -type f 2>/dev/null | wc -l | tr -d ' ' || true)"
    if [[ $records -ge 2 ]]; then
      break
    fi
    sleep 0.02
  done

  errors="$(cat "$work"/err."$attempt".* 2>/dev/null | tr '\n' ' ' || true)"
  [[ $records -eq 2 ]] ||
    fail "attempt $attempt: expected 2 records, found $records ($errors)"

  # Two DISTINCT pids under one pane, which is what makes them two candidates.
  recorded="$(find "$registry" -type f -exec basename {} \; | sort | tr '\n' ' ')"
  [[ "$(printf '%s' "$recorded" | wc -w | tr -d ' ')" == 2 ]] ||
    fail "attempt $attempt: expected two distinct pid records, got: $recorded"
  grep -qh '^w1:p2 ' "$registry"/* ||
    fail "attempt $attempt: a record does not name the pane it registered under"

  reap
done

printf 'PASS: nvim-mcp-registry-concurrent.sh (4 pairs, both instances register on a fresh registry)\n'
