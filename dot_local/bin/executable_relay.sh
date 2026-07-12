#!/usr/bin/env bash
# relay: fan a notification to moshi (phone) + Hermes (Discord paper trail) + a
# clickable local macOS notification (focus the herdr pane on click). Each channel
# is isolated (|| true, backgrounded); always exits 0. Secret never on argv.
set -euo pipefail

agent="" state="" project="" branch="" detail="" pane="" local_only=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --agent | --state | --project | --branch | --detail | --pane)
      # A value-taking flag with no value (typically as the last argument) must NOT abort
      # this always-exit-0 notification path: warn, ignore the flag, and keep going. Without
      # the guard, `shift 2` on a single remaining argument fails under set -e and kills the
      # whole run, silently dropping every channel.
      if [[ $# -lt 2 ]]; then
        printf 'relay: %s given without a value; ignoring\n' "$1" >&2
        shift
        continue
      fi
      case "$1" in
        --agent) agent="$2" ;;
        --state) state="$2" ;;
        --project) project="$2" ;;
        --branch) branch="$2" ;;
        --detail) detail="$2" ;;
        --pane) pane="$2" ;;
      esac
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

title="${agent:-relay} · ${state:-done}${project:+ · $project}"
# Body is the summary itself (branch-prefixed), not a redundant "state — project" header the title already
# carries -- so the phone push / macOS banner spend their short preview on the summary, not boilerplate.
message="${branch:+($branch) }${detail:-${state:-done}}"
# Phone push + macOS banner clip long summaries mid-sentence; pre-trim to the last full sentence within
# ~260 chars so they end cleanly. Discord (#relay) keeps the full text below -- it has no length ceiling.
preview="$(printf '%s' "$message" | python3 -c 'import re, sys
s = sys.stdin.read()
if len(s) <= 260:
    sys.stdout.write(s)
else:
    cut = 0
    for m in re.finditer(r"[.!?](?= |$)", s):
        if m.end() <= 260:
            cut = m.end()
    sys.stdout.write(s[:cut] if cut else s[:259].rstrip() + "…")' 2>/dev/null || printf '%s' "$message")"

# Presence gating for the phone push only: at the desk (recent keyboard/mouse input) the local banner +
# Discord log suffice, so skip moshi; away (idle past the threshold) add it. Fail-safe: if idle is unknown
# (probe failed) treat as away so a push is never dropped. RELAY_IDLE_SECS overrides the probe (test/manual);
# RELAY_FORCE_PHONE=1 always pushes. HIDIdleTime is input-idle -> works under the never-sleep power policy.
desk_idle="${RELAY_DESK_IDLE_SECS:-600}"
# Fail OPEN on ANY uncertainty. Validate the raw nanosecond field AND the threshold as plain decimal
# digits BEFORE any arithmetic, because two silent-drop traps hide here: a PRESENT-but-garbled
# HIDIdleTime line awk-coerces to 0 ("actively typing") and suppresses the push, and a non-numeric
# threshold aborts the bash arithmetic comparison under set -u (rc=1, dropping every channel). So we
# pull the raw $NF (not a pre-divided int), require all-digits on it and on the threshold, and only then
# compare. Any invalid or absent value = presence unknown = want_phone stays 1 (treat as "user away").
# RELAY_IDLE_SECS overrides the probe; RELAY_IOREG overrides the probe binary (tests point it at a stub).
want_phone=1
if [[ -z ${RELAY_FORCE_PHONE:-} ]]; then
  idle_secs="${RELAY_IDLE_SECS:-}"
  if [[ -z $idle_secs ]]; then
    idle_ns="$("${RELAY_IOREG:-/usr/sbin/ioreg}" -c IOHIDSystem 2>/dev/null | grep -m1 HIDIdleTime | awk '{print $NF}' || true)"
    [[ $idle_ns =~ ^[0-9]+$ ]] && idle_secs=$((idle_ns / 1000000000))
  fi
  if [[ $idle_secs =~ ^[0-9]+$ && $desk_idle =~ ^[0-9]+$ && $idle_secs -lt $desk_idle ]]; then want_phone=""; fi
fi

# moshi -- token read from the 0600 file by jq; body sent on stdin (never on argv)
moshi_body="$(jq -c --arg t "$title" --arg m "$preview" \
  'if .moshi_secret then {token: .moshi_secret, title: $t, message: $m} else empty end' "$auth_file" 2>/dev/null || true)"
if [[ -n $moshi_body && -z $local_only && -n $want_phone ]]; then
  (curl -fsS -m 10 -X POST "$moshi_url" -H 'Content-Type: application/json' --data @- <<<"$moshi_body" >/dev/null 2>&1 || true) &
fi

# hermes -- body carries no secret; HMAC key read from the file by python (never argv/env); body on stdin
hermes_body="$(jq -cn --arg a "$agent" --arg s "$state" --arg p "$project" --arg d "$message" \
  '{agent: $a, state: $s, project: $p, detail: $d}' || true)"
sig="$(printf '%s' "$hermes_body" | python3 -c 'import hmac, hashlib, json, sys
secret = json.load(open(sys.argv[1])).get("hermes_secret") or ""
sys.stdout.write(hmac.new(secret.encode(), sys.stdin.buffer.read(), hashlib.sha256).hexdigest() if secret else "")' "$auth_file" 2>/dev/null || true)"
if [[ -n $hermes_body && -n $sig && -z $local_only ]]; then
  (curl -fsS -m 10 -X POST "$hermes_url" -H 'Content-Type: application/json' \
    -H "X-Webhook-Signature: $sig" --data @- <<<"$hermes_body" >/dev/null 2>&1 || true) &
fi

# local clickable notification -> focus the exact herdr pane on click
if command -v terminal-notifier >/dev/null 2>&1; then
  exec_cmd=":"
  [[ -n $pane ]] && exec_cmd="herdr agent focus $pane"
  (terminal-notifier -title "$title" -message "$preview" -sound default \
    -activate com.mitchellh.ghostty -execute "$exec_cmd" >/dev/null 2>&1 || true) &
fi

exit 0
