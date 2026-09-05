#!/usr/bin/env bash
# What the registering autocmd must REFUSE to do, checked against real Neovim
# because every one of these is about how Neovim's own file operations behave.
#
#   a) an existing registry left at 0777 is not written into
#   b) a name planted at the temp path is not followed, so the file it points
#      at is neither truncated nor published as a record
#   c) a --listen pathname carrying a newline is not recorded at all, because a
#      record is one line and the resolver would read only as far as the newline
#   d) a --listen pathname carrying a space or a tab is refused for the same
#      reason: the resolver reads a record as whitespace-separated fields
#   e) every later operation goes through the CANONICAL registry path, so
#      removing the alias it was reached by does not strand the record
#   f) exit cleanup does not delete a file named for our pid when registration
#      was refused
#
# Case c is not hypothetical: Neovim binds such a pathname and answers RPC on
# it, so without this the resolver probes a truncated name, finds nothing, and
# deletes the record of a healthy instance.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SOURCE="$REPO_ROOT/dot_config/nvim/lua/config/autocmds.lua"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if ! command -v nvim >/dev/null 2>&1; then
  printf 'SKIP: nvim is not on PATH, and these are its own file operations\n'
  exit 0
fi
[[ -f $SOURCE ]] || fail "missing source: $SOURCE"

# Short, because these bind real unix sockets and sun_path is 104 bytes.
work="$(mktemp -d /tmp/nmcs.XXXXXX)"
pids=()
cleanup() {
  local p
  for p in ${pids[@]+"${pids[@]}"}; do
    kill "$p" 2>/dev/null || true
  done
  rm -rf "$work"
}
trap cleanup EXIT

awk '/^-- Tell the nvim-mcp resolver/,0' "$SOURCE" >"$work/block.lua"
[[ -s $work/block.lua ]] || fail 'could not find the registry block in autocmds.lua'
# The marker autocmd is registered AFTER the block's, so it runs after it in
# the same VimEnter. Without it every case here, all of which expect no record
# to appear, would have to wait out a timeout instead of a signal.
{
  printf 'augroup = function(n) return vim.api.nvim_create_augroup("nvim_config_" .. n, { clear = true }) end\n'
  printf 'dofile("%s/block.lua")\n' "$work"
  printf 'vim.api.nvim_create_autocmd("VimEnter", { callback = function() vim.fn.writefile({ "done" }, vim.env.NMC_DONE) end })\n'
} >"$work/init.lua"

# start_nvim <case> <socket> [extra nvim args...] -- one headless instance under
# $work/<case> as its state home, left running until the case is done. `exec` so
# the recorded pid is Neovim itself; stdout to /dev/null so an inherited pipe
# cannot outlive this script. Sets REGISTRY and ERRLOG for the caller.
start_nvim() {
  local case_name="$1" socket="$2"
  shift 2
  REGISTRY="$work/$case_name/nvim-mcp/registry"
  ERRLOG="$work/err.$case_name"
  local done="$work/done.$case_name"
  (
    cd "$work" &&
      exec env HERDR_PANE_ID=w1:p2 XDG_STATE_HOME="$work/$case_name" NMC_DONE="$done" \
        nvim --clean --headless -u "$work/init.lua" "$@" --listen "$socket" \
        >/dev/null 2>"$ERRLOG"
  ) &
  pids+=("$!")
  # Wait for VimEnter to have run, not for a record to appear: every case here
  # expects NO record, so polling for one would always wait out the timeout.
  local waited=0
  while [[ $waited -lt 100 ]]; do
    if [[ -e $done ]]; then
      return 0
    fi
    sleep 0.02
    waited=$((waited + 1))
  done
  fail "$case_name: Neovim never reached VimEnter ($(cat "$ERRLOG" 2>/dev/null))"
}

records_in() { # <dir>
  find "$1" -type f 2>/dev/null | wc -l | tr -d ' ' || true
}

# quit_nvim <socket> -- a CLEAN exit, so VimLeavePre runs. A signal from the
# trap would not prove anything about the cleanup path.
quit_nvim() {
  nvim --server "$1" --remote-send '<Cmd>qa!<CR>' 2>/dev/null || true
  local waited=0
  while [[ -e $1 && $waited -lt 50 ]]; do
    sleep 0.02
    waited=$((waited + 1))
  done
}

# --- a) an existing 0777 registry is refused ---------------------------------
mkdir -p "$work/loose/nvim-mcp/registry"
chmod 777 "$work/loose/nvim-mcp/registry"
start_nvim loose "$work/loose.sock"
[[ "$(records_in "$REGISTRY")" == 0 ]] ||
  fail "loose: registered into a 0777 registry ($(find "$REGISTRY" -type f | tr '\n' ' '))"
grep -q 'is not mode 0700' "$ERRLOG" ||
  fail "loose: did not refuse for the reason under test ($(cat "$ERRLOG"))"

# --- b) a symlink planted at the temp name is not followed -------------------
# Planted from --cmd, which runs before the init that registers the autocmd, so
# the link is in place for this instance's own pid before VimEnter fires.
mkdir -p "$work/planted/nvim-mcp/registry"
chmod 700 "$work/planted/nvim-mcp/registry"
printf 'VICTIM CONTENT\n' >"$work/victim"
plant="lua local d = '$work/planted/nvim-mcp/registry' vim.uv.fs_symlink('$work/victim', d .. '/' .. vim.fn.getpid() .. '.tmp')"
start_nvim planted "$work/planted.sock" --cmd "$plant"
[[ "$(cat "$work/victim")" == 'VICTIM CONTENT' ]] ||
  fail "planted: the symlink was followed and its target was truncated ($(cat "$work/victim"))"
[[ "$(records_in "$REGISTRY")" == 0 ]] ||
  fail 'planted: a record was published through the planted name'
# The refusal here is O_EXCL declining an existing name, which is silent by
# design; what must not appear is any complaint about the registry itself.
grep -q 'not registering' "$ERRLOG" &&
  fail "planted: refused for an unrelated reason ($(cat "$ERRLOG"))"

# --- c) a --listen pathname carrying a newline is not recorded ---------------
mkdir -p "$work/newline/nvim-mcp/registry"
chmod 700 "$work/newline/nvim-mcp/registry"
newline_sock="$work/a"$'\n'"b.sock"
start_nvim newline "$newline_sock"
[[ -S $newline_sock ]] || fail 'newline: Neovim did not bind the pathname, so the case proves nothing'
[[ "$(records_in "$REGISTRY")" == 0 ]] ||
  fail "newline: a record was written for a pathname that cannot be read back ($(cat "$REGISTRY"/* 2>/dev/null))"
grep -q 'whitespace or NUL' "$ERRLOG" ||
  fail "newline: did not refuse for the reason under test ($(cat "$ERRLOG"))"

# --- d) a space or a tab in the listen pathname is refused -------------------
# Same failure as the newline and the same cause: a record is read as
# whitespace-separated fields, so the resolver takes the pathname only as far
# as the first space, probes a name nothing answers on, and deletes the record
# of a healthy instance.
for whitespace_kind in space tab; do
  mkdir -p "$work/ws-$whitespace_kind/nvim-mcp/registry"
  chmod 700 "$work/ws-$whitespace_kind/nvim-mcp/registry"
  if [[ $whitespace_kind == space ]]; then
    ws_sock="$work/a b.sock"
  else
    ws_sock="$work/a"$'\t'"b.sock"
  fi
  start_nvim "ws-$whitespace_kind" "$ws_sock"
  [[ -S $ws_sock ]] || fail "ws-$whitespace_kind: Neovim did not bind it, so the case proves nothing"
  [[ "$(records_in "$REGISTRY")" == 0 ]] ||
    fail "ws-$whitespace_kind: a record was written for a pathname that cannot be read back"
  grep -q 'whitespace or NUL' "$ERRLOG" ||
    fail "ws-$whitespace_kind: did not refuse for the reason under test ($(cat "$ERRLOG"))"
done

# --- e) later operations go through the canonical registry path --------------
# The configured registry is reached through an alias that sits in a directory
# other accounts can write. Validating one pathname and then opening, renaming
# and deleting through another leaves a window in which that alias can be
# swapped. Removing the alias after registration is the deterministic form of
# the same question: a resolver that kept the canonical path still finds its
# own record, and one that re-derives from the configured string does not.
mkdir -p "$work/shared" "$work/canon"
chmod 777 "$work/shared"
chmod 700 "$work/canon"
ln -s "$work/canon" "$work/shared/state"
start_nvim_at() { # <state home> <socket>
  REGISTRY="$1/nvim-mcp/registry"
  ERRLOG="$work/err.canonical"
  local done_marker="$work/done.canonical"
  (
    cd "$work" &&
      exec env HERDR_PANE_ID=w1:p2 XDG_STATE_HOME="$1" NMC_DONE="$done_marker" \
        nvim --clean --headless -u "$work/init.lua" --listen "$2" \
        >/dev/null 2>"$ERRLOG"
  ) &
  pids+=("$!")
  local waited=0
  while [[ ! -e $done_marker && $waited -lt 100 ]]; do
    sleep 0.02
    waited=$((waited + 1))
  done
}
start_nvim_at "$work/shared/state" "$work/canon.sock"
canonical_registry="$work/canon/nvim-mcp/registry"
[[ "$(records_in "$canonical_registry")" == 1 ]] ||
  fail "canonical: expected one record under the resolved directory ($(cat "$ERRLOG"))"
rm "$work/shared/state"
quit_nvim "$work/canon.sock"
[[ "$(records_in "$canonical_registry")" == 0 ]] ||
  fail 'canonical: the record was left behind, so the delete went through the configured path'

# --- f) refused registration arms no cleanup ---------------------------------
# The registry is an alias to a directory that already holds a file named for
# this instance's pid. Registration is refused, and the exit must not then
# delete somebody else's file just because it shares that name.
mkdir -p "$work/decoy" "$work/refused/nvim-mcp"
chmod 700 "$work/decoy"
# The REGISTRY itself is the alias, so its own leaf is a symlink and
# registration is refused outright.
ln -s "$work/decoy" "$work/refused/nvim-mcp/registry"
plant_decoy="lua vim.fn.writefile({ 'not ours' }, '$work/decoy/' .. vim.fn.getpid())"
start_nvim refused "$work/refused.sock" --cmd "$plant_decoy"
grep -q 'not registering' "$ERRLOG" ||
  fail "refused: registration was not refused, so the case proves nothing ($(cat "$ERRLOG"))"
decoy_count="$(records_in "$work/decoy")"
quit_nvim "$work/refused.sock"
[[ "$(records_in "$work/decoy")" == "$decoy_count" ]] ||
  fail 'refused: exit deleted a file named for our pid even though nothing was registered'

printf 'PASS: nvim-mcp-registry-safety.sh (7 cases)\n'
