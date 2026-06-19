#!/usr/bin/env bash
# Claude Code Stop hook: POST a "done" push notification to moshi (getmoshi.app).
#
# Hook input: JSON on stdin with { session_id, transcript_path, cwd,
# permission_mode, hook_event_name }. Wired async in modify_settings.json so it
# never delays turn completion. Always exits 0.
#
# Reads the webhook secret from ~/.config/moshi/setting.json (a 0600 file chezmoi
# renders from the KeePassXC entry "Moshi :: Webhook Secret") into MOSHI_TOKEN and
# exports it, so the jq body below reads it from the environment — never from a
# command line. The secret therefore never lands in any process's argv (no `ps`
# exposure). No-ops if the file is missing or the secret is empty, e.g. before an
# interactive `chezmoi apply` with KeePassXC unlocked.

set -euo pipefail

MOSHI_TOKEN="$(jq -r '.webhook_secret // empty' "$HOME/.config/moshi/setting.json" 2>/dev/null || true)"
export MOSHI_TOKEN
[[ -n ${MOSHI_TOKEN:-} ]] || exit 0

input=$(cat)
cwd=$(printf '%s' "$input" | jq -r '.cwd // empty' 2>/dev/null || true)
project=${cwd##*/}
message="Claude finished${project:+ in $project}"

body=$(jq -cn --arg m "$message" \
  '{token: env.MOSHI_TOKEN, title: "Done", message: $m}') || exit 0

curl -fsS -m 10 -X POST https://api.getmoshi.app/api/webhook \
  -H 'Content-Type: application/json' \
  --data @- <<<"$body" >/dev/null 2>&1 || true

exit 0
