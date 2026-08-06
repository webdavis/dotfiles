#!/usr/bin/env bash
# relay: fan a notification to moshi (phone) + Hermes (Discord paper trail) + a
# clickable local macOS notification (focus the herdr pane on click). Each channel
# is isolated (|| true, backgrounded); always exits 0. Secret never on argv.
#
# Two mirrored narrowing flags: --local-only keeps the banner and drops both
# webhooks; --remote-only keeps only the hermes leg (no banner, no phone) and is
# the LOG path the weekly unattended jobs use. --remote-only additionally posts
# SYNCHRONOUSLY and prints the delivery outcome, because an undelivered log entry
# is invisible in a way an undelivered alert is not. See the hermes block below.
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
# The DECISION CORE. Every branch below that decides what happens (rather than
# doing it) lives in helpers/event.sh as a pure function, which is what makes
# those decisions testable one behavior at a time without spawning this script.
# shellcheck source=dot_local/libexec/pns/helpers/event.sh
source "${PNS_HELPERS_DIR:-$pns_dir/helpers}/event.sh"

# Endpoints and credentials belong to the channels now, each reading the same
# env names (RELAY_AUTH_FILE, RELAY_MOSHI_URL, RELAY_HERMES_URL) with the same
# defaults. The engine deliberately knows none of them: that is what makes it
# destination-agnostic.

title="$(pns_title "$agent" "$state" "$project")"
message="$(pns_message "$branch" "$detail" "$state")"
# Phone push and macOS banner clip long summaries mid-sentence; pre-trim to the
# last full sentence within ~260 chars so they end cleanly. Discord keeps the
# full text. This stays here rather than in the core because it forks python.
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

# Presence gating applies to the phone alone: at the desk the banner and the
# Discord entry suffice, so the push is skipped; away, it is added. The
# fail-open rules and the reasons behind them live with the verdict, in
# helpers/event.sh. RELAY_IDLE_SECS overrides the probe (tests and manual
# runs); RELAY_IOREG points it at a stub.
# The idle PROBE is impure and lives here; the VERDICT is pns_wants_phone.
#
# The probe runs ONLY when a phone push could actually fire. Both narrowing
# flags suppress the moshi leg outright, so under either one the answer is
# unused, and it is an unbounded pipe to ioreg with no timeout. On the
# --remote-only path that is not merely wasted work: the POST there is
# SYNCHRONOUS, so a wedged ioreg holds the weekly job before the delivery it
# exists to report, after the week's guard is spent and while the caller still
# holds its serialize lock. HIDIdleTime is input-idle, which is what works
# under the never-sleep power policy.
idle_secs="${RELAY_IDLE_SECS:-}"
if [[ -z $local_only && -z $remote_only && -z ${RELAY_FORCE_PHONE:-} && -z $idle_secs ]]; then
  idle_ns="$("${RELAY_IOREG:-/usr/sbin/ioreg}" -c IOHIDSystem 2>/dev/null | grep -m1 HIDIdleTime | awk '{print $NF}' || true)"
  [[ $idle_ns =~ ^[0-9]+$ ]] && idle_secs=$((idle_ns / 1000000000))
fi
want_phone=""
pns_wants_phone "$idle_secs" "${RELAY_DESK_IDLE_SECS:-600}" \
  "$local_only" "$remote_only" "${RELAY_FORCE_PHONE:-}" && want_phone=1

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
