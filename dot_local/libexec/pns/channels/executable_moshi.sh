#!/usr/bin/env bash
# pns channel: moshi (phone push).
#
# THE CHANNEL CONTRACT, shared by every executable in this directory:
#
#   input    one JSON event on stdin (fields rendered by relay.sh; `preview`
#            is the short form for channels with a length ceiling).
#   mode     "async": deliver in the background, print nothing.
#            "sync": deliver now, print one line saying what happened.
#   exit     always 0. A failed notification must never fail the caller.
#   absent   missing prerequisite (no key, no binary): exit 0 silently.
#
# relay.sh decides WHICH channels run; a channel only decides HOW to deliver.
set -uo pipefail

event="$(cat)"
auth_file="${RELAY_AUTH_FILE:-$HOME/.config/relay/auth.json}"
moshi_url="${RELAY_MOSHI_URL:-https://api.getmoshi.app/api/webhook}"

title="$(jq -r '.title // ""' <<<"$event" 2>/dev/null || true)"
preview="$(jq -r '.preview // ""' <<<"$event" 2>/dev/null || true)"

# The token goes on stdin, never argv: the process table is world-readable.
# `empty` when the key is absent, which is the silent-unavailable case.
body="$(jq -c --arg t "$title" --arg m "$preview" \
  'if .moshi_secret then {token: .moshi_secret, title: $t, message: $m} else empty end' \
  "$auth_file" 2>/dev/null || true)"
[[ -n $body ]] || exit 0

curl -fsS -m 10 -X POST "$moshi_url" -H 'Content-Type: application/json' \
  --data @- <<<"$body" >/dev/null 2>&1 || true
exit 0
