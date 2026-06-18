#!/usr/bin/env bash
# Claude Code Stop hook: POST a "done" push notification to moshi (getmoshi.app).
#
# Hook input: JSON on stdin with { session_id, transcript_path, cwd,
# permission_mode, hook_event_name }. Wired async in modify_settings.json so it
# never delays turn completion. Always exits 0.
#
# modify_settings.json injects MOSHI_TOKEN (vaulted in KeePassXC) into the hook's
# environment. No-ops if MOSHI_TOKEN is unset, e.g. before an interactive
# `chezmoi apply` with KeePassXC unlocked. The token reaches jq via the
# environment and curl via stdin, so it never appears in this script's argv.

set -euo pipefail

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
