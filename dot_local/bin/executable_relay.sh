#!/usr/bin/env bash
# relay: fan a notification to moshi (phone) + Hermes (Discord paper trail) + a
# clickable local macOS notification (focus the herdr pane on click). Each channel
# is isolated (|| true, backgrounded); always exits 0. Secret never on argv.
set -euo pipefail

agent="" state="" project="" branch="" detail="" pane="" local_only=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --agent)
      agent="${2:-}"
      shift 2
      ;;
    --state)
      state="${2:-}"
      shift 2
      ;;
    --project)
      project="${2:-}"
      shift 2
      ;;
    --branch)
      branch="${2:-}"
      shift 2
      ;;
    --detail)
      detail="${2:-}"
      shift 2
      ;;
    --pane)
      pane="${2:-}"
      shift 2
      ;;
    --local-only)
      local_only=1
      shift
      ;;
    *) shift ;;
  esac
done

auth_file="${RELAY_AUTH_FILE:-$HOME/.config/relay/auth.json}"
moshi_url="${RELAY_MOSHI_URL:-https://api.getmoshi.app/api/webhook}"
hermes_url="${RELAY_HERMES_URL:-http://127.0.0.1:8644/webhooks/relay}"

loc="$project"
[[ -n $branch ]] && loc="${loc:+$loc }($branch)"
title="${agent:-relay} · ${state:-done}${project:+ · $project}"
message="${state:-done}${loc:+ — $loc}"
[[ -n $detail ]] && message="$message"$'\n'"$detail"

# moshi -- token read from the 0600 file by jq; body sent on stdin (never on argv)
moshi_body="$(jq -c --arg t "$title" --arg m "$message" \
  'if .moshi_secret then {token: .moshi_secret, title: $t, message: $m} else empty end' "$auth_file" 2>/dev/null || true)"
if [[ -n $moshi_body && -z $local_only ]]; then
  (curl -fsS -m 10 -X POST "$moshi_url" -H 'Content-Type: application/json' --data @- <<<"$moshi_body" >/dev/null 2>&1 || true) &
fi

# hermes -- body carries no secret; HMAC key read from the file by python (never argv/env); body on stdin
hermes_body="$(jq -cn --arg a "$agent" --arg s "$state" --arg p "$project" --arg d "$message" \
  '{agent: $a, state: $s, project: $p, detail: $d}')"
sig="$(printf '%s' "$hermes_body" | python3 -c 'import hmac, hashlib, json, sys
secret = json.load(open(sys.argv[1])).get("hermes_secret") or ""
sys.stdout.write(hmac.new(secret.encode(), sys.stdin.buffer.read(), hashlib.sha256).hexdigest() if secret else "")' "$auth_file" 2>/dev/null || true)"
if [[ -n $sig && -z $local_only ]]; then
  (curl -fsS -m 10 -X POST "$hermes_url" -H 'Content-Type: application/json' \
    -H "X-Webhook-Signature: $sig" --data @- <<<"$hermes_body" >/dev/null 2>&1 || true) &
fi

# local clickable notification -> focus the exact herdr pane on click
if command -v terminal-notifier >/dev/null 2>&1; then
  exec_cmd=":"
  [[ -n $pane ]] && exec_cmd="herdr agent focus $pane"
  (terminal-notifier -title "$title" -message "$message" -sound default \
    -activate com.mitchellh.ghostty -execute "$exec_cmd" >/dev/null 2>&1 || true) &
fi

exit 0
