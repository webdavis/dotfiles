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
project="${cwd##*/}"
branch=""
[[ -n $cwd && -d $cwd ]] && branch="$(git -C "$cwd" branch --show-current 2>/dev/null || true)"
detail=""
if [[ $state == "done" && -n $transcript && -f $transcript ]]; then
  detail="$(jq -rs '[.[] | select(.type=="assistant" and (.message.content? != null))
    | .message.content[] | select(.type=="text") | .text] | last // empty' "$transcript" 2>/dev/null || true)"
  detail="$(printf '%s' "$detail" | tr '\n\r\t' '   ' | tr -s ' ' | sed 's/^ *//; s/ *$//')"
  detail="$(printf '%s' "$detail" | python3 -c 'import sys, re
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
else
  detail="$(printf '%s' "$input" | jq -r '.message // .detail // empty' 2>/dev/null || true)"
fi
args=(--agent "$agent" --state "$state" --project "$project")
[[ -n $branch ]] && args+=(--branch "$branch")
[[ -n $detail ]] && args+=(--detail "$detail")
[[ -n ${HERDR_PANE_ID:-} ]] && args+=(--pane "$HERDR_PANE_ID")
"$relay" "${args[@]}" || true
exit 0
