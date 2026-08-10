#!/usr/bin/env bash
# pns channel: hermes (Discord, via the hermes gateway). The channel contract
# is documented in channels/moshi.sh.
#
# RELAY_HERMES_URL picks the Discord channel, so #relay and
# #unattended-upgrades are one script with two configurations.
#
# Sync mode exists here because the weekly job records must be able to SEE a
# delivery failure: a 401 or 404 swallowed silently leaves the Discord channel
# empty, and an empty channel looks like the jobs stopped running. Sync posts
# print the HTTP status; the caller's run log keeps it.
set -uo pipefail

event="$(cat)"
auth_file="${RELAY_AUTH_FILE:-$HOME/.config/relay/auth.json}"
hermes_url="${RELAY_HERMES_URL:-http://127.0.0.1:8644/webhooks/relay}"

mode="$(jq -r '.mode // "async"' <<<"$event" 2>/dev/null || true)"
# The full message, not the preview: Discord has no length ceiling.
body="$(jq -c '{agent: (.agent // ""), state: (.state // ""), project: (.project // ""), detail: (.message // "")}' \
  <<<"$event" 2>/dev/null || true)"

# The signing key stays inside python: it never reaches argv or the environment.
sig="$(printf '%s' "$body" | python3 -c 'import hmac, hashlib, json, sys
secret = json.load(open(sys.argv[1])).get("hermes_secret") or ""
sys.stdout.write(hmac.new(secret.encode(), sys.stdin.buffer.read(), hashlib.sha256).hexdigest() if secret else "")' \
  "$auth_file" 2>/dev/null || true)"

if [[ -z $body || -z $sig ]]; then
  # No key means unavailable. Sync callers are told; async stays silent.
  [[ $mode == sync ]] &&
    printf 'relay: post SKIPPED -- no hermes signing key in %s; nothing was sent\n' "$auth_file"
  exit 0
fi

if [[ $mode == sync ]]; then
  deadline="${RELAY_REMOTE_TIMEOUT:-5}"
  [[ $deadline =~ ^[0-9]+$ ]] || deadline=5
  # No -f: it would collapse every HTTP error into exit 22, and the CODE is
  # the point. -w prints 000 when no response arrived at all.
  code="$(curl -sS -o /dev/null -m "$deadline" -w '%{http_code}' \
    -X POST "$hermes_url" -H 'Content-Type: application/json' \
    -H "X-Webhook-Signature: $sig" --data @- <<<"$body" 2>/dev/null || true)"
  case "$code" in
    2??) printf 'relay: posted HTTP %s\n' "$code" ;;
    000) printf 'relay: post FAILED HTTP 000 (no response; is the hermes gateway up?)\n' ;;
    '') printf 'relay: post FAILED (curl reported no HTTP status at all)\n' ;;
    *) printf 'relay: post FAILED HTTP %s\n' "$code" ;;
  esac
  exit 0
fi

curl -fsS -m 10 -X POST "$hermes_url" -H 'Content-Type: application/json' \
  -H "X-Webhook-Signature: $sig" --data @- <<<"$body" >/dev/null 2>&1 || true
exit 0
