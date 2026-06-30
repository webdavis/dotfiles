#!/usr/bin/env bash
# relay-agent: build an agent state message from a hook payload, hand to relay.sh.
# Arg 1 = state (done|blocked|asked|plan-ready). Always exits 0.
set -euo pipefail
state="${1:-done}"
relay="${RELAY_BIN:-$HOME/.local/bin/relay.sh}"
input="$(cat 2>/dev/null || true)"
cwd="$(printf '%s' "$input" | jq -r '.cwd // empty' 2>/dev/null || true)"
transcript="$(printf '%s' "$input" | jq -r '.transcript_path // empty' 2>/dev/null || true)"
agent="${RELAY_AGENT:-claude}"
codex_bin="${CODEX_BIN:-codex}"
project="${cwd##*/}"
branch=""
[[ -n $cwd && -d $cwd ]] && branch="$(git -C "$cwd" branch --show-current 2>/dev/null || true)"
detail=""
if [[ $state == "done" && -n $transcript && -f $transcript ]]; then
  reply="$(jq -rs '[.[] | select(.type=="assistant" and (.message.content? != null))
    | .message.content[] | select(.type=="text") | .text] | last // empty' "$transcript" 2>/dev/null || true)"
  reply="$(printf '%s' "$reply" | tr '\n\r\t' '   ' | tr -s ' ' | sed 's/^ *//; s/ *$//')"
  used_codex=""
  # Codex-primary: one cheap `codex exec` returns "STATE|SUMMARY"; STATE may override 'done' (e.g. asking),
  # SUMMARY is a real one-line summary instead of a lead-sentence guess. Guarded by RELAY_SUMMARIZING: if a
  # codex exec ever fired its own Stop hook -> relay-agent -> codex, the re-entry sees the marker, skips
  # codex, and falls back -- so it can never loop, regardless of how Codex evolves.
  if [[ -z ${RELAY_SUMMARIZING:-} && -n $reply ]] && command -v "$codex_bin" >/dev/null 2>&1; then
    cmd=()
    if command -v gtimeout >/dev/null 2>&1; then
      cmd=(gtimeout 30)
    elif command -v timeout >/dev/null 2>&1; then
      cmd=(timeout 30)
    fi
    cmd+=("$codex_bin" exec -s read-only "Classify this AI coding agent's final message and summarize it for a brief phone notification.
Output EXACTLY one line and nothing else: STATE|SUMMARY
STATE is one of: done (finished its work), asking (wants you to answer or choose), blocked (needs permission/input to continue).
SUMMARY is at most 120 characters, plain text, no newlines.

Message:
$reply")
    out="$(RELAY_SUMMARIZING=1 "${cmd[@]}" 2>/dev/null || true)"
    line="$(printf '%s\n' "$out" | grep -E '^(done|asking|blocked)\|' | tail -1 || true)"
    if [[ -n $line ]]; then
      state="${line%%|*}"
      detail="${line#*|}"
      used_codex=1
    fi
  fi
  if [[ -z $used_codex ]]; then
    detail="$(printf '%s' "$reply" | python3 -c 'import sys, re
s = sys.stdin.read().strip()
if len(s) <= 240:
    sys.stdout.write(s)
else:
    cut = 0
    for m in re.finditer(r"[.!?](?= [A-Z])", s):
        if m.end() <= 240:
            cut = m.end()
        else:
            break
    if cut:
        sys.stdout.write(s[:cut])
    else:
        head = s[:240]
        sp = head.rfind(" ")
        sys.stdout.write(head[:sp] + "…" if sp > 0 else head + "…")' 2>/dev/null || true)"
  fi
else
  detail="$(printf '%s' "$input" | jq -r '.message // .detail // empty' 2>/dev/null || true)"
fi
args=(--agent "$agent" --state "$state" --project "$project")
[[ -n $branch ]] && args+=(--branch "$branch")
[[ -n $detail ]] && args+=(--detail "$detail")
[[ -n ${HERDR_PANE_ID:-} ]] && args+=(--pane "$HERDR_PANE_ID")
"$relay" "${args[@]}" || true
exit 0
