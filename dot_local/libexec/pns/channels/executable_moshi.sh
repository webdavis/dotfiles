#!/usr/bin/env bash
# pns channel: moshi (phone push).
#
# THE CHANNEL CONTRACT, shared by every executable in this directory and the
# shape SP3 implements in Rust:
#
#   input     one JSON event object on stdin, with the fields relay.sh renders:
#             agent, state, project, branch, detail, title, message, preview,
#             pane, mode. `preview` is the pre-trimmed short form; a channel
#             with a length ceiling uses it, one without uses `message`.
#   mode      "async" (deliver in the background, say nothing) or "sync"
#             (deliver now, print the outcome). Core chooses; a channel that
#             cannot honour sync still must not block.
#   exit      ALWAYS 0. A channel never fails its caller: a notification that
#             cannot be delivered must not break the work being reported on.
#   absent    a channel whose prerequisite is missing (no key, no binary) exits
#             0 silently. That is how a plugin declares itself unavailable.
#   output    nothing on the async path. On the sync path, one line saying what
#             happened, because a silent exit there is indistinguishable from a
#             delivered message.
#
# Core decides WHICH channels run (the narrowing flags, presence gating); a
# channel only decides HOW to deliver and whether it can.
set -uo pipefail

event="$(cat)"
auth_file="${RELAY_AUTH_FILE:-$HOME/.config/relay/auth.json}"
moshi_url="${RELAY_MOSHI_URL:-https://api.getmoshi.app/api/webhook}"

title="$(jq -r '.title // ""' <<<"$event" 2>/dev/null || true)"
preview="$(jq -r '.preview // ""' <<<"$event" 2>/dev/null || true)"

# Token read from the 0600 file by jq and sent on stdin, never on argv: the
# process table is world-readable. `empty` when the key is absent, which is the
# unavailable-plugin case.
body="$(jq -c --arg t "$title" --arg m "$preview" \
  'if .moshi_secret then {token: .moshi_secret, title: $t, message: $m} else empty end' \
  "$auth_file" 2>/dev/null || true)"
[[ -n $body ]] || exit 0

curl -fsS -m 10 -X POST "$moshi_url" -H 'Content-Type: application/json' \
  --data @- <<<"$body" >/dev/null 2>&1 || true
exit 0
