#!/usr/bin/env bash
# relay-agent: build an agent state message from a hook payload, hand to relay.sh.
# Arg 1 = state (done|blocked|asked|plan-ready). Always exits 0.
set -euo pipefail
# The DECISION CORE, same library relay.sh sources. The helpers directory is
# derived by parameter expansion rather than the `cd`/`dirname` pair relay.sh
# uses: every harness invokes this hook by absolute path, and a notification
# path that already forks jq and python should not pay a subshell and an exec
# to learn where it lives.
#
# A core that is not there ENDS THE RUN AT 0, and says why on stderr first (the
# `source` failure names the path it wanted). Without that clause this line is
# a new way for the hook to exit non-zero, which is the one thing it must never
# do; and there is nothing lost by stopping, because relay.sh sources the same
# library and would fail on the very next line anyway.
# shellcheck source=dot_local/libexec/pns/helpers/event.sh
source "${PNS_HELPERS_DIR:-${BASH_SOURCE[0]%/*}/../helpers}/event.sh" || exit 0
state="${1:-done}"
# The engine binary by absolute path; RELAY_BIN is the test seam.
relay="${RELAY_BIN:-$HOME/.local/libexec/pns/pns}"
input="$(cat 2>/dev/null || true)"
cwd="$(printf '%s' "$input" | jq -r '.cwd // empty' 2>/dev/null || true)"
transcript="$(printf '%s' "$input" | jq -r '.transcript_path // empty' 2>/dev/null || true)"
agent="${RELAY_AGENT:-claude}"
codex_bin="${CODEX_BIN:-codex}"
project="${cwd##*/}"
branch=""
[[ -n $cwd && -d $cwd ]] && branch="$(git -C "$cwd" branch --show-current 2>/dev/null || true)"
# The assistant text of the transcript's last turn, or empty for anything this
# cannot read. Only the TAIL is read: the extraction needs the last user turn
# and what follows, never the whole file, and a long session grows the
# transcript past 200MB. Measured 2026-08-05: each slurp of the full file held
# ~33MB resident and minutes of CPU, and a stop-hook loop piled up 33
# concurrent jq processes. 4MB of tail parses in well under a second (a cut
# first line is dropped by fromjson? by design).
reply_from_transcript() {
  tail -c 4000000 "$1" | jq -rs -R '[ split("\n")[] | select(length > 0) | fromjson? | select(type=="object") ] as $a
    | ([ $a | to_entries[] | select(.value.type=="user" and ((.value.message.content|type)=="string" or ((.value.message.content[0]?.type)=="text"))) | .key ] | last // -1) as $s
    | [ $a[$s+1:][] | select(.type=="assistant") | .message.content[]? | select(.type=="text") | .text ] | join("\n\n")' 2>/dev/null || true
}
# The harness has not always flushed the assistant's final text by the time the
# Stop hook runs. Live capture 2026-08-12: the single read came back empty, the
# summarizer was skipped, and the notification shipped with no --detail at all.
# So an empty result is RE-READ inside a bounded window. What an expired window
# proves is only that nothing readable arrived in time: a turn that really said
# nothing, a transcript that could not be read, and one that would not parse all
# leave the same empty string, and all three are reported the same way. The
# bound is what keeps those cases from delaying every notification.
#
# The window is measured on the FLATTENED reply, not the raw extraction: an
# assistant block carrying only whitespace is non-empty raw and empty once
# flattened, which is the same missing-summary symptom through another door.
#
# The attempt count is VALIDATED before it is believed, and falls back to the
# default rather than to no retries. Measured 2026-08-12: `[[ $attempt -lt abc ]]`
# evaluates `abc` as a variable name in arithmetic context, which under `set -u`
# is an unbound-variable error and exits the hook 1, on the one path whose whole
# contract is exiting 0. The INTERVAL needs no such guard: a value sleep refuses
# fails the sleep, and the guarded sleep below breaks the loop and carries on.
reply_reread_attempts=4
[[ ${PNS_REPLY_REREAD_ATTEMPTS:-} =~ ^[0-9]+$ ]] && reply_reread_attempts="$PNS_REPLY_REREAD_ATTEMPTS"
reply_reread_interval="${PNS_REPLY_REREAD_INTERVAL:-0.15}"
# The harness's OWN copy of the final text, which is why the transcript read
# above is the fallback rather than the source. Claude Code documents that a
# Stop hook can fire before the transcript write completes, and recommends this
# field instead; version 2.1.226 builds the Stop payload with it, carrying the
# last assistant message joined and trimmed, and omits it when there is none.
# Absent, or present and empty, falls through to the file.
payload_reply="$(printf '%s' "$input" | jq -r '.last_assistant_message // empty' 2>/dev/null || true)"
detail=""
if [[ $state == "done" ]] && [[ -n $payload_reply || (-n $transcript && -f $transcript) ]]; then
  # one line, trimmed, last 8000 chars at most
  reply="$(pns_flatten_reply "$payload_reply")"
  if [[ -z $reply && -n $transcript && -f $transcript ]]; then
    reply="$(pns_flatten_reply "$(reply_from_transcript "$transcript")")"
    attempt=0
    while [[ -z $reply && $attempt -lt $reply_reread_attempts ]]; do
      # A sleep that FAILS must not fail the hook: bare, `set -e` turns a
      # killed or refused sleep into a non-zero exit on the one path whose
      # whole contract is exiting 0.
      sleep "$reply_reread_interval" || break
      reply="$(pns_flatten_reply "$(reply_from_transcript "$transcript")")"
      attempt=$((attempt + 1))
    done
  fi
  used_codex=""
  # Codex-primary: one cheap `codex exec` summarizes the whole turn + classifies it as "STATE|SUMMARY";
  # STATE may override 'done' (e.g. asking). It runs in a stripped, dedicated CODEX_HOME (minimal config:
  # fast model + low reasoning, live auth symlinked, NO hooks/plugins) -- which cuts codex's skill/plugin/
  # hook load (~9s -> ~3s) and means this run has no Stop hook: a hard guarantee against a
  # relay->codex->relay loop, on top of the RELAY_SUMMARIZING guard. RELAY_CODEX_HOME overrides the path
  # (tests point it at a temp dir). On any miss (re-entry, codex absent, timeout, bad output) it falls back.
  if [[ -z ${RELAY_SUMMARIZING:-} && -n $reply ]] && command -v "$codex_bin" >/dev/null 2>&1; then
    codex_home="${RELAY_CODEX_HOME:-$HOME/.config/relay/codex-home}"
    # Keep the relay Codex home private: it symlinks the live Codex auth, so create the dir and its
    # config under umask 077 (owner-only). --ephemeral below also stops session transcripts from being
    # persisted at all, so retained files no longer exist -- this is the belt to that suspenders.
    (umask 077 && mkdir -p "$codex_home") 2>/dev/null || true
    [[ -f "$codex_home/config.toml" ]] || (umask 077 && printf 'model = "gpt-5.5"\nmodel_reasoning_effort = "low"\n' >"$codex_home/config.toml") 2>/dev/null || true
    ln -sf "$HOME/.codex/auth.json" "$codex_home/auth.json" 2>/dev/null || true
    cmd=()
    if command -v gtimeout >/dev/null 2>&1; then
      cmd=(gtimeout 30)
    elif command -v timeout >/dev/null 2>&1; then
      cmd=(timeout 30)
    fi
    # The turn transcript (up to 8000 chars of assistant output) is fed on STDIN, never as an argv
    # positional where `ps` could read it: the `-` positional tells `codex exec` to read the prompt from
    # stdin. --ephemeral runs without persisting any session file, so the transcript is not retained on disk.
    prompt="Summarize this AI coding agent's last turn for a brief phone notification, then classify it.
Output EXACTLY one line and nothing else: STATE|SUMMARY
STATE is one of: done (finished its work), asking (wants you to answer or choose), blocked (needs permission/input to continue).
SUMMARY is two or three sentences, up to 320 characters, plain text, no newlines, covering what was done plus any decision or question raised.

Turn:
$reply"
    cmd+=("$codex_bin" exec --ephemeral --skip-git-repo-check -C "$codex_home" -s read-only -)
    out="$(RELAY_SUMMARIZING=1 CODEX_HOME="$codex_home" "${cmd[@]}" <<<"$prompt" 2>/dev/null || true)"
    line="$(printf '%s\n' "$out" | grep -E '^(done|asking|blocked)\|' | tail -1 || true)"
    # The SUMMARY half is what makes the line usable, not the state half. A
    # matched state with nothing after the pipe used to count as a hit, which
    # skipped the trim below and shipped a title-only notification over a turn
    # that had text (live 2026-08-12). It must carry at least one non-blank
    # character: a summary of spaces renders just as blank as no summary, the
    # same equivalence the reply path already draws. Anything less falls
    # through exactly as if the condenser had failed, empty $line included.
    if [[ ${line#*|} =~ [^[:space:]] ]]; then
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

# --- moshi-hook interposition -----------------------------------------------
# On a BLOCKING event this hook hands the payload to moshi-hook and returns
# what moshi returns, so the operator can approve or deny from the phone, while
# pns keeps the presence gate moshi has none of. Non-blocking events are pns's
# own (moshi-hook is deliberately not installed for these two harnesses, see
# run_once_after_60), so forwarding one would push the same notification twice
# and buy nothing: moshi cannot round-trip a decision the harness is not
# waiting for.
#
# THE EXIT CONTRACT AND ITS ONE EXCEPTION. Everything above this line is a
# notification, and a notification that cannot be delivered must never fail the
# turn it reports on, which is why this hook exits 0 on every other path. The
# forwarded path is the exception: there the exit code is the OPERATOR'S
# DECISION, and a `|| true` on it would answer the permission prompt for them.
# Do not "fix" the asymmetry.
moshi_sub=""
gate="${PNS_MOSHI_GATE:-${BASH_SOURCE[0]%/*}/moshi-gate.sh}"
if [[ $state == blocked && -x $gate ]] &&
  command -v "${MOSHI_HOOK_BIN:-/opt/homebrew/bin/moshi-hook}" >/dev/null 2>&1; then
  # The harness name reaches this hook from a config file, so it is MATCHED
  # against the harnesses pns registers itself for rather than pasted into a
  # subcommand. Whether to forward is decided here; the gate re-checks the
  # shape because pi and omp reach it without passing through this hook.
  case "$agent" in
    claude | codex) moshi_sub="$agent-hook" ;;
  esac
fi
if [[ -n $moshi_sub ]]; then
  # The notification goes out FIRST and with the phone leg suppressed: moshi is
  # about to raise the actionable card, so relay's own push would be the same
  # event a second time, and a round trip that waits on a human must not hold
  # the banner and the paper trail behind it. relay's stdout is moved to stderr
  # because this hook's stdout now belongs to moshi's reply.
  RELAY_SKIP_PHONE=1 "$relay" "${args[@]}" >&2 || true
  # The payload is WRITTEN BACK, byte for byte: this hook consumed stdin at the
  # top, and a consumed-but-not-forwarded stream leaves moshi with an empty
  # parse, after which it silently does nothing.
  status=0
  printf '%s' "$input" | "$gate" "$moshi_sub" || status=$?
  exit "$status"
fi
"$relay" "${args[@]}" || true
exit 0
