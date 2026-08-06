#!/usr/bin/env bash
# pns channel: hermes (the Discord paper trail). See channels/moshi.sh for the
# contract every channel in this directory implements.
#
# This channel is ROUTE-PARAMETERIZED: the gateway decides which Discord channel
# an event lands in from the route in RELAY_HERMES_URL, so #relay (agent state)
# and #unattended-upgrades (the weekly job records) are one plugin with two
# configurations rather than two plugins.
#
# It is also the only channel that implements SYNC delivery, and that asymmetry
# is deliberate. On the alert path a discarded HTTP status is affordable, since
# the banner and the phone already told the operator. On the LOG path it is not:
# a 401 (wrong key) or 404 (route in the config but not loaded by the running
# gateway) swallowed into /dev/null leaves the channel permanently empty, and an
# empty channel reads as "the job stopped running", which is the exact inversion
# this record exists to prevent. So sync posts with a short deadline and prints
# the outcome, which the caller's LaunchAgent run log keeps.
set -uo pipefail

event="$(cat)"
auth_file="${RELAY_AUTH_FILE:-$HOME/.config/relay/auth.json}"
hermes_url="${RELAY_HERMES_URL:-http://127.0.0.1:8644/webhooks/relay}"

mode="$(jq -r '.mode // "async"' <<<"$event" 2>/dev/null || true)"
# The full message, not the preview: Discord has no length ceiling, so this is
# the one channel that keeps the whole summary.
body="$(jq -c '{agent: (.agent // ""), state: (.state // ""), project: (.project // ""), detail: (.message // "")}' \
  <<<"$event" 2>/dev/null || true)"

# The body carries no secret; the HMAC key is read from the file by python and
# never reaches argv or the environment.
sig="$(printf '%s' "$body" | python3 -c 'import hmac, hashlib, json, sys
secret = json.load(open(sys.argv[1])).get("hermes_secret") or ""
sys.stdout.write(hmac.new(secret.encode(), sys.stdin.buffer.read(), hashlib.sha256).hexdigest() if secret else "")' \
  "$auth_file" 2>/dev/null || true)"

if [[ -z $body || -z $sig ]]; then
  # Unavailable. On the sync path say so: a silent exit there is
  # indistinguishable from a delivered entry.
  [[ $mode == sync ]] &&
    printf 'relay: post SKIPPED -- no hermes signing key in %s; nothing was sent\n' "$auth_file"
  exit 0
fi

if [[ $mode == sync ]]; then
  deadline="${RELAY_REMOTE_TIMEOUT:-5}"
  [[ $deadline =~ ^[0-9]+$ ]] || deadline=5
  # No -f: it collapses any HTTP >= 400 into a bare exit 22, and the CODE is the
  # whole point here. -w '%{http_code}' reports 000 when no response arrived.
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
