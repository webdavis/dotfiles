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
  reply="$(jq -rs -R '[ splits("\n") | select(length > 0) | fromjson? ] as $a
    | ([ $a | to_entries[] | select(.value.type=="user" and ((.value.message.content|type)=="string" or ((.value.message.content[0]?.type)=="text"))) | .key ] | last // -1) as $s
    | [ $a[$s+1:][] | select(.type=="assistant") | .message.content[]? | select(.type=="text") | .text ] | join("\n\n")' "$transcript" 2>/dev/null || true)"
  reply="$(printf '%s' "$reply" | tr '\n\r\t' '   ' | tr -s ' ' | sed 's/^ *//; s/ *$//')"
  [[ ${#reply} -le 8000 ]] || reply="${reply: -8000}" # cap only long turns; the offset empties short strings
  used_codex=""
  # Codex-primary: one cheap `codex exec` summarizes the whole turn + classifies it as "STATE|SUMMARY";
  # STATE may override 'done' (e.g. asking). It runs in a stripped, dedicated CODEX_HOME (minimal config:
  # fast model + low reasoning, live auth symlinked, NO hooks/plugins) -- which cuts codex's skill/plugin/
  # hook load (~9s -> ~3s) and means this run has no Stop hook: a hard guarantee against a
  # relay->codex->relay loop, on top of the RELAY_SUMMARIZING guard. RELAY_CODEX_HOME overrides the path
  # (tests point it at a temp dir). On any miss (re-entry, codex absent, timeout, bad output) it falls back.
  if [[ -z ${RELAY_SUMMARIZING:-} && -n $reply ]] && command -v "$codex_bin" >/dev/null 2>&1; then
    codex_home="${RELAY_CODEX_HOME:-$HOME/.config/relay/codex-home}"
    mkdir -p "$codex_home" 2>/dev/null || true
    [[ -f "$codex_home/config.toml" ]] || printf 'model = "gpt-5.5"\nmodel_reasoning_effort = "low"\n' >"$codex_home/config.toml" 2>/dev/null || true
    ln -sf "$HOME/.codex/auth.json" "$codex_home/auth.json" 2>/dev/null || true
    cmd=()
    if command -v gtimeout >/dev/null 2>&1; then
      cmd=(gtimeout 30)
    elif command -v timeout >/dev/null 2>&1; then
      cmd=(timeout 30)
    fi
    cmd+=("$codex_bin" exec --skip-git-repo-check -C "$codex_home" -s read-only "Summarize this AI coding agent's last turn for a brief phone notification, then classify it.
Output EXACTLY one line and nothing else: STATE|SUMMARY
STATE is one of: done (finished its work), asking (wants you to answer or choose), blocked (needs permission/input to continue).
SUMMARY is two or three sentences, up to 320 characters, plain text, no newlines, covering what was done plus any decision or question raised.

Turn:
$reply")
    out="$(RELAY_SUMMARIZING=1 CODEX_HOME="$codex_home" "${cmd[@]}" 2>/dev/null || true)"
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
