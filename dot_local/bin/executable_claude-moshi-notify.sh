#!/usr/bin/env bash
# Claude Code Stop hook: POST a "done" push notification to moshi (getmoshi.app).
#
# Hook input: JSON on stdin with { session_id, transcript_path, cwd,
# permission_mode, hook_event_name }. Wired async in modify_settings.json so it
# never delays turn completion. Always exits 0.
#
# Reads the webhook secret from ~/.config/moshi/auth.json (a 0600 file chezmoi
# renders from the KeePassXC entry "Moshi :: Webhook Secret") into MOSHI_TOKEN and
# exports it, so the jq body below reads it from the environment — never from a
# command line. The secret therefore never lands in any process's argv (no `ps`
# exposure). No-ops if the file is missing or the secret is empty, e.g. before an
# interactive `chezmoi apply` with KeePassXC unlocked.
#
# The push is enriched with: the project (cwd basename) plus the current git
# branch when cwd is a repo, and a short snippet of Claude's final assistant
# message parsed from the transcript JSONL. Each transcript line is a JSON event;
# the final reply is the last assistant event carrying a "text" content block
# (trailing non-assistant metadata events are skipped by the filter). Any failure
# to read or parse the transcript degrades gracefully — the push still goes out
# with whatever context is available, and the hook always exits 0.

set -euo pipefail

MOSHI_TOKEN="$(jq -r '.webhook_secret // empty' "$HOME/.config/moshi/auth.json" 2>/dev/null || true)"
export MOSHI_TOKEN
[[ -n ${MOSHI_TOKEN:-} ]] || exit 0

input=$(cat)
cwd=$(printf '%s' "$input" | jq -r '.cwd // empty' 2>/dev/null || true)
transcript=$(printf '%s' "$input" | jq -r '.transcript_path // empty' 2>/dev/null || true)

project=${cwd##*/}

# Git branch, only when cwd is inside a work tree (suppress all git noise).
branch=""
if [[ -n $cwd && -d $cwd ]]; then
  branch=$(git -C "$cwd" branch --show-current 2>/dev/null || true)
fi

# Final assistant message snippet: last assistant event's text content. jq slurps
# the JSONL (-s) and takes the last text block; a 25MB transcript parses in ~0.2s,
# acceptable for an async hook. Any read/parse error yields an empty snippet.
snippet=""
if [[ -n $transcript && -f $transcript ]]; then
  snippet=$(jq -rs '
    [ .[]
      | select(.type == "assistant" and (.message.content? != null))
      | .message.content[]
      | select(.type == "text")
      | .text
    ] | last // empty
  ' "$transcript" 2>/dev/null || true)
fi

# Collapse whitespace/newlines to single spaces, trim, then truncate to a push-
# friendly length with an ellipsis. tr/sed/awk are coreutils — dependency-light.
if [[ -n $snippet ]]; then
  snippet=$(printf '%s' "$snippet" | tr '\n\r\t' '   ' | tr -s ' ' | sed 's/^ *//; s/ *$//')
  max=240
  if [[ ${#snippet} -gt $max ]]; then
    snippet="${snippet:0:max}…"
  fi
fi

# Title: prefer "Claude · <project>", fall back to a bare "Claude".
title="Claude"
[[ -n $project ]] && title="Claude · $project"

# Message: "<project> (<branch>)" location line, then the snippet on a new line.
location=$project
[[ -n $branch ]] && location="${location:+$location }($branch)"
if [[ -z $location ]]; then
  message="Claude finished"
else
  message="Done in $location"
fi
[[ -n $snippet ]] && message="$message"$'\n'"$snippet"

body=$(jq -cn --arg t "$title" --arg m "$message" \
  '{token: env.MOSHI_TOKEN, title: $t, message: $m}') || exit 0

curl -fsS -m 10 -X POST https://api.getmoshi.app/api/webhook \
  -H 'Content-Type: application/json' \
  --data @- <<<"$body" >/dev/null 2>&1 || true

exit 0
