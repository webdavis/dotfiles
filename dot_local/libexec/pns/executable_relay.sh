#!/usr/bin/env bash
# relay: the pns engine. Renders one event and fans it out to the channel
# plugins (phone, Discord, macOS banner). Always exits 0; a notification must
# never fail the work it reports on.
#
# Narrowing flags: --local-only keeps only the banner; --remote-only keeps
# only the Discord leg, posts synchronously, and prints the outcome, because
# an undelivered log entry is invisible in a way an undelivered alert is not.
set -euo pipefail

agent="" state="" project="" branch="" detail="" pane="" local_only="" remote_only=""
is_relay_flag() {
  case "$1" in
    --agent | --state | --project | --branch | --detail | --pane | --local-only | --remote-only) return 0 ;;
    *) return 1 ;;
  esac
}
while [[ $# -gt 0 ]]; do
  case "$1" in
    --agent | --state | --project | --branch | --detail | --pane)
      # A value-taking flag with no value must NOT abort this always-exit-0 notification path: warn,
      # ignore the flag, and keep going. "No value" means either the flag is the last argument, OR its
      # next token is itself a RECOGNIZED option (e.g. `--pane --local-only`) -- consuming that token as
      # the value would silently drop the real flag (here, leak to external channels the caller asked to
      # keep local). In both cases we do NOT consume the next token. (An unrecognized next token like
      # `--bogus` is still taken as the value -- the unknown-flag leniency below is deliberately retained.)
      if [[ $# -lt 2 ]] || is_relay_flag "$2"; then
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
    --remote-only)
      remote_only=1
      shift
      ;;
    *) shift ;;
  esac
done

pns_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Decisions live in helpers/event.sh as pure functions, so tests call them
# directly instead of spawning this script.
# shellcheck source=dot_local/libexec/pns/helpers/event.sh
source "${PNS_HELPERS_DIR:-$pns_dir/helpers}/event.sh"
# The probes (idle, phone attention) are shared with hooks/moshi-gate.sh so
# the two can never disagree about where the operator is.
# shellcheck source=dot_local/libexec/pns/helpers/presence.sh
source "${PNS_HELPERS_DIR:-$pns_dir/helpers}/presence.sh"

# Endpoints and credentials belong to the channels; the engine knows none of
# them.

title="$(pns_title "$agent" "$state" "$project")"
message="$(pns_message "$branch" "$detail" "$state")"
# Pre-trim the preview to the last full sentence within ~260 chars: the phone
# and banner clip mid-sentence otherwise. Discord keeps the full text.
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

# Presence gating applies to the phone leg only: at the desk the banner is
# enough, away adds the push. The idle probe runs only when its answer could
# matter, because it is an unbounded pipe to ioreg and a wedged probe on a
# path that cannot use the answer would stall the caller for nothing.
idle_secs="${RELAY_IDLE_SECS:-}"
if [[ -z $local_only && -z $remote_only && -z ${RELAY_SKIP_PHONE:-} &&
  -z ${RELAY_FORCE_PHONE:-} && -z $idle_secs ]]; then
  idle_secs="$(pns_idle_secs)"
fi
# RELAY_SKIP_PHONE means the caller already put this event on the phone by
# another route, so only the phone leg is dropped, and it beats
# RELAY_FORCE_PHONE: "I already sent it" is more specific than an override.
want_phone=""
if [[ -z ${RELAY_SKIP_PHONE:-} ]] && pns_wants_phone "$idle_secs" "${RELAY_DESK_IDLE_SECS:-120}" \
  "$local_only" "$remote_only" "${RELAY_FORCE_PHONE:-}"; then
  want_phone=1
fi

# THE ATTENTION OVERRIDE. In the band between "just typed" and "away", the
# idle clock says desk, but the operator may be right next to it watching from
# their phone. A phone demonstrably in hand (mosh bytes moving, or a Back Tap
# marker) flips the verdict to away. The band check confines the probes to the
# one case where they can change the answer.
#
# Caller intent is never overridden: the narrowing flags and RELAY_SKIP_PHONE
# state what the caller wants delivered, so no probe may resurrect a leg they
# dropped.
if [[ -z $want_phone && -z $local_only && -z $remote_only && -z ${RELAY_SKIP_PHONE:-} ]] &&
  pns_attention_band "$idle_secs" "${RELAY_DESK_IDLE_SECS:-120}" && pns_phone_attention; then
  want_phone=1
fi

# ---------------------------------------------------------------------------
# Channel dispatch
#
# Everything above is the ENGINE: it renders the event and decides WHICH
# channels fire (the narrowing flags, presence gating). Everything below hands
# that event to plugins in channels/, one JSON object on stdin each. A channel
# decides only HOW to deliver and whether it can; it always exits 0 and says
# nothing unless asked to deliver synchronously. channels/moshi.sh carries the
# full contract.
#
# This forecasts the SP3 Rust architecture deliberately: core stays
# destination-agnostic so the published crate is not wired to one person's
# stack, and a channel stays an executable taking JSON on stdin so it can be
# written in any language. Adding a destination means dropping a file in
# channels/ and routing to it here, not editing delivery code.
# ---------------------------------------------------------------------------
channels_dir="${PNS_CHANNELS_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/channels}"

# send <channel> <mode>: hand the event to one channel. A channel that is
# missing is simply not installed, which is not an error.
send() {
  local channel="$channels_dir/$1.sh" mode="$2"
  [[ -x $channel ]] || return 0
  # The pane is SANITIZED HERE, once, rather than in each channel. A channel may
  # be written in any language, so it cannot be expected to share this guard,
  # and duplicating the regex per channel is how one copy gets tightened and the
  # others rot. An unsafe id is dropped from the event and the banner simply
  # does not focus a pane.
  local safe_pane="$pane"
  if [[ -n $pane ]] && ! pns_pane_is_safe "$pane"; then
    safe_pane=""
    printf 'relay: dropped a pane id with shell metacharacters; no channel will focus a pane\n' >&2
  fi
  jq -cn --arg a "$agent" --arg s "$state" --arg p "$project" --arg b "$branch" \
    --arg d "$detail" --arg t "$title" --arg m "$message" --arg v "$preview" \
    --arg n "$safe_pane" --arg o "$mode" \
    '{agent: $a, state: $s, project: $p, branch: $b, detail: $d,
      title: $t, message: $m, preview: $v, pane: $n, mode: $o}' |
    "$channel" || true
}

plan="$(pns_channel_plan "$local_only" "$remote_only" "$want_phone")"
if [[ -z $plan ]]; then
  # A legitimate verdict, and one that must be SAID: a silent exit here is
  # indistinguishable from a delivered notification.
  printf 'relay: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent\n'
else
  while read -r channel mode; do
    [[ -n $channel ]] && send "$channel" "$mode"
  done <<<"$plan"
fi

exit 0
