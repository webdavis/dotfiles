#!/usr/bin/env bash
# What the registering autocmd must REFUSE to do, checked against real Neovim
# because every one of these is about how Neovim's own file operations behave.
#
#   a) an existing registry left at 0777 is not written into
#   b) a name planted at the temp path is not followed, so the file it points
#      at is neither truncated nor published as a record
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

printf 'PASS: nvim-mcp-registry-safety.sh (2 cases)\n'
