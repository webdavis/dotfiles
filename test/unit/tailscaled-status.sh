#!/usr/bin/env bash
# tailscaled-status.sh — the Tailscale status reminder (run_onchange_after_66)
# must classify on `tailscale status --json`'s .BackendState, not on grepped text.
# It renders the REAL chezmoiscript with the host chezmoi and runs the rendered
# body against a fake `tailscale` binary (TAILSCALE_BIN) per BackendState, plus a
# connection failure and an unparseable-output case. Asserts, for every case:
#
#   Running          -> silent success   (no stderr, exit 0)
#   Starting         -> transient note    (informational, no action)
#   NeedsLogin       -> auth reminder     (sudo tailscale up --accept-dns=true)
#   NeedsMachineAuth -> tailnet-admin note (awaits approval)
#   Stopped          -> stopped note       (sudo tailscale up)
#   <unknown state>  -> UNKNOWN note, state printed verbatim (never "daemon missing")
#   rc != 0          -> "daemon is not running" reminder with the full formula path
#   garbage stdout   -> same "daemon is not running" reminder (unparseable JSON)
#
# Every case must exit 0 (a reminder must never abort `chezmoi apply`), and the
# five real states must NOT be misclassified as a missing daemon — the exact bug
# the audit flagged in the old grep-based script.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/.chezmoiscripts/run_onchange_after_66-tailscaled-status.sh.tmpl"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Host-tool guards: plain test/*.sh scripts run outside the Nix shell.
for tool in chezmoi jq; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'SKIP: %s not on PATH; cannot render/run the status reminder\n' "$tool"
    exit 0
  fi
done
[[ -f $SCRIPT ]] || fail "missing template: $SCRIPT"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Render the darwin-only script once (scratch HOME, CI=1 — same mechanics as the
# treefmt rendered-template lint and the herdr build-script test). Empty render
# == non-darwin host: skip.
rendered="$work/rendered.sh"
render_home="$(mktemp -d)"
HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$SCRIPT" >"$rendered" || fail "chezmoi failed to render $SCRIPT"
rm -rf "$render_home"
if [[ ! -s $rendered ]]; then
  printf 'SKIP: empty render (non-darwin host); nothing to exercise\n'
  exit 0
fi

# Write a fake `tailscale` at $1/tailscale that, for `status --json`, cats the
# payload file $2 and exits with code $3. Any other invocation exits non-zero —
# the reminder must depend only on the JSON contract, never on bare `status`.
make_fake() {
  local dir="$1" payload_file="$2" json_rc="$3"
  cat >"$dir/tailscale" <<STUB
#!/bin/bash
if [[ "\$1" == "status" && "\$2" == "--json" ]]; then
  cat "$payload_file"
  exit $json_rc
fi
exit 1
STUB
  chmod +x "$dir/tailscale"
}

# run_case <name> <payload> <json_rc>
# Populates RC (script exit) and ERR (script stderr).
run_case() {
  local name="$1" payload="$2" json_rc="$3"
  local dir="$work/$name"
  mkdir -p "$dir"
  printf '%s' "$payload" >"$dir/payload"
  make_fake "$dir" "$dir/payload" "$json_rc"
  local errf="$dir/err"
  RC=0
  TAILSCALE_BIN="$dir/tailscale" bash "$rendered" >/dev/null 2>"$errf" || RC=$?
  ERR="$(cat "$errf")"
}

failures=0
# check <case> <PASS|already-failed> ...  — records + prints per-assertion result.
report() {
  local status="$1" msg="$2"
  if [[ $status == ok ]]; then
    printf '  ok   %s\n' "$msg"
  else
    printf '  FAIL %s\n' "$msg"
    failures=$((failures + 1))
  fi
}

# Realistic .BackendState payloads (shape grounded in a live `tailscale status
# --json` read on 2026-07-10: Version 1.98.8, top-level BackendState/HaveNodeKey).
RUNNING='{"Version":"1.98.8-t05a918293","BackendState":"Running","HaveNodeKey":true,"AuthURL":""}'
STARTING='{"Version":"1.98.8","BackendState":"Starting","HaveNodeKey":true,"AuthURL":""}'
NEEDSLOGIN='{"Version":"1.98.8","BackendState":"NeedsLogin","HaveNodeKey":false,"AuthURL":"https://login.tailscale.com/a/deadbeef"}'
NEEDSMACHINEAUTH='{"Version":"1.98.8","BackendState":"NeedsMachineAuth","HaveNodeKey":true,"AuthURL":""}'
STOPPED='{"Version":"1.98.8","BackendState":"Stopped","HaveNodeKey":true,"AuthURL":""}'
UNKNOWN='{"Version":"1.98.8","BackendState":"Frobnicating","HaveNodeKey":true,"AuthURL":""}'
GARBAGE='tailscaled is not running, so your query cannot be answered'

# Assertions on the last run_case. a0 = exit 0; am = stderr matches; an = stderr
# must NOT match; asilent = no stderr.
a0() {
  if [[ $RC -eq 0 ]]; then report ok "$1: exits 0"; else report bad "$1: exits 0 (got rc=$RC, stderr: $ERR)"; fi
}
am() {
  if grep -qi -- "$2" <<<"$ERR"; then report ok "$1: mentions '$2'"; else report bad "$1: mentions '$2' (stderr: $ERR)"; fi
}
an() {
  if grep -qi -- "$2" <<<"$ERR"; then report bad "$1: must NOT mention '$2' (stderr: $ERR)"; else report ok "$1: does not mention '$2'"; fi
}
asilent() {
  if [[ -z $ERR ]]; then report ok "$1: silent (no stderr)"; else report bad "$1: silent (stderr: $ERR)"; fi
}

printf 'tailscaled-status cases:\n'

run_case running "$RUNNING" 0
a0 Running
asilent Running

run_case starting "$STARTING" 0
a0 Starting
am Starting starting
an Starting "unknown backendstate" # discriminate: falling through to the unknown handler still greps 'starting'
an Starting install-system-daemon

run_case needslogin "$NEEDSLOGIN" 0
a0 NeedsLogin
am NeedsLogin "accept-dns=true"
an NeedsLogin install-system-daemon

run_case needsmachineauth "$NEEDSMACHINEAUTH" 0
a0 NeedsMachineAuth
am NeedsMachineAuth "tailnet admin"
an NeedsMachineAuth install-system-daemon

run_case stopped "$STOPPED" 0
a0 Stopped
am Stopped stopped
an Stopped "unknown backendstate" # discriminate: falling through to the unknown handler still greps 'stopped'
an Stopped install-system-daemon

run_case unknown "$UNKNOWN" 0
a0 unknown-state
am unknown-state "unknown backendstate"
am unknown-state Frobnicating
an unknown-state install-system-daemon

run_case connfail "" 1
a0 connection-failure
am connection-failure "is not running"
am connection-failure install-system-daemon

run_case garbage "$GARBAGE" 0
a0 garbage-stdout
am garbage-stdout "is not running"
am garbage-stdout install-system-daemon

if [[ $failures -gt 0 ]]; then
  printf 'tailscaled-status: %d assertion(s) FAILED\n' "$failures" >&2
  exit 1
fi
printf 'tailscaled-status: OK (5 states + unknown + connection failure + unparseable, all exit 0)\n'
