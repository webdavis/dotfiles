#!/usr/bin/env bash
# herdr-jump.sh — the workspace create-or-focus helper must validate its args
# (house rule: unknown/missing CLI args -> usage to stderr, exit non-zero) and
# guard its `herdr | jq` pipeline so a down/erroring server fails cleanly rather
# than dying under set -e with no message or spewing jq errors.
#
# Cases:
#   a) no args / wrong arg count -> usage on stderr, non-zero exit
#   b) herdr server down (rc=3)  -> clean one-line error, non-zero exit, no jq spew
#   b) herdr error JSON (rc=0)   -> clean one-line error, non-zero exit, no jq spew
#   c) happy path, label exists  -> `workspace focus <id>` recorded, no create
#   c) happy path, label absent  -> `workspace create ... --focus` recorded, no focus
#
# The script is driven via HERDR_BIN_PATH pointing at a stub `herdr`, so the test
# never touches the live server.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_herdr-jump.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

if ! command -v jq >/dev/null 2>&1; then
  printf 'SKIP: jq not on PATH; herdr-jump queries herdr via jq\n'
  exit 0
fi
[[ -f $SCRIPT ]] || fail "missing script: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Stub `herdr` for the given mode at $work/<name>/herdr, recording argv to
# $work/<name>/rec. Echo the stub path.
make_stub() {
  local name="$1" mode="$2"
  local dir="$work/$name"
  mkdir -p "$dir"
  local rec="$dir/rec"
  case "$mode" in
    down)
      cat >"$dir/herdr" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$rec"
exit 3
STUB
      ;;
    errorjson)
      cat >"$dir/herdr" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$rec"
[[ "\$1 \$2" == "workspace list" ]] && printf '%s' '{"id":"x","error":{"code":-1}}'
exit 0
STUB
      ;;
    happy)
      cat >"$dir/herdr" <<STUB
#!/bin/bash
printf '%s\n' "\$*" >>"$rec"
if [[ "\$1 \$2" == "workspace list" ]]; then
  printf '%s' '{"result":{"workspaces":[{"label":"homelab","workspace_id":"w15"},{"label":"dotfiles","workspace_id":"wW"}]}}'
fi
exit 0
STUB
      ;;
    *) fail "unknown stub mode: $mode" ;;
  esac
  chmod +x "$dir/herdr"
  printf '%s' "$dir/herdr"
}

# run <stub-path> <args...> -> sets RC, ERR
run() {
  local stub="$1"
  shift
  local e="$work/err"
  RC=0
  HERDR_BIN_PATH="$stub" bash "$SCRIPT" "$@" >/dev/null 2>"$e" || RC=$?
  ERR="$(cat "$e")"
}

# --- a) arg validation -----------------------------------------------------
down_stub="$(make_stub down down)"

run "$down_stub" # zero args
[[ $RC -ne 0 ]] || fail "no-args: expected non-zero exit, got 0"
grep -qi 'usage' <<<"$ERR" || fail "no-args: expected usage on stderr, got: $ERR"

run "$down_stub" onlylabel # one arg
[[ $RC -ne 0 ]] || fail "one-arg: expected non-zero exit, got 0"
grep -qi 'usage' <<<"$ERR" || fail "one-arg: expected usage on stderr, got: $ERR"

run "$down_stub" a b c # too many
[[ $RC -ne 0 ]] || fail "three-args: expected non-zero exit, got 0"
grep -qi 'usage' <<<"$ERR" || fail "three-args: expected usage on stderr, got: $ERR"

# --- b) server down / error JSON -> clean failure, no jq spew --------------
run "$down_stub" homelab /tmp/homelab
[[ $RC -ne 0 ]] || fail "server-down: expected non-zero exit, got 0"
! grep -qiE 'jq: error|cannot iterate' <<<"$ERR" || fail "server-down: jq error leaked: $ERR"
[[ -n $ERR ]] || fail "server-down: expected a clean one-line error on stderr, got none"

err_stub="$(make_stub errorjson errorjson)"
run "$err_stub" homelab /tmp/homelab
[[ $RC -ne 0 ]] || fail "error-json: expected non-zero exit, got 0"
! grep -qiE 'jq: error|cannot iterate' <<<"$ERR" || fail "error-json: jq error leaked: $ERR"
[[ -n $ERR ]] || fail "error-json: expected a clean one-line error on stderr, got none"

# --- c) happy path: existing label -> focus, no create ---------------------
happy_stub="$(make_stub happy-focus happy)"
run "$happy_stub" homelab /tmp/homelab
[[ $RC -eq 0 ]] || fail "happy-focus: expected exit 0, got $RC (stderr: $ERR)"
rec="$work/happy-focus/rec"
grep -qx 'workspace focus w15' "$rec" || fail "happy-focus: did not focus w15 (rec: $(cat "$rec"))"
! grep -q 'workspace create' "$rec" || fail "happy-focus: created a workspace instead of focusing (rec: $(cat "$rec"))"

# --- c) happy path: missing label -> create --focus, no focus --------------
happy_stub="$(make_stub happy-create happy)"
run "$happy_stub" brandnew /tmp/brandnew
[[ $RC -eq 0 ]] || fail "happy-create: expected exit 0, got $RC (stderr: $ERR)"
rec="$work/happy-create/rec"
grep -q 'workspace create --cwd /tmp/brandnew --label brandnew --focus' "$rec" ||
  fail "happy-create: did not create with expected argv (rec: $(cat "$rec"))"
! grep -q 'workspace focus' "$rec" || fail "happy-create: focused instead of creating (rec: $(cat "$rec"))"

printf 'PASS: herdr-jump validates args, guards its pipeline, and preserves create-or-focus\n'
