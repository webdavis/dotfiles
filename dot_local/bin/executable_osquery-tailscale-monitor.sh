#!/usr/bin/env bash
#
# osquery-tailscale-monitor.sh - polled every 60s by a launchd StartInterval agent. Pages on the
# off->on transition of `tailscale funnel` (a local port newly exposed to the PUBLIC internet).
# Funnel traffic tunnels through tailscaled, so osquery cannot see it as a listening port; polling
# the CLI is the only way to catch it. Silent in steady state.
#
# R2-5: a MONITORING GAP of a public-exposure detector is itself CRIT (a blind funnel monitor is an
# undetectable public-exposure risk). So a status-command failure is NOT swallowed into "inactive",
# a corrupt/gap prior state is NOT trusted as a baseline, the state file is written atomically, and
# every blindness path (missing binary, failed/empty status) dispatches at CRIT so it reaches the
# remote #priority channel (the dispatcher POSTs only CRIT - a WARN was dropped before the POST).
set -euo pipefail

# Resolution order: explicit override -> PATH (the headless homebrew formula this machine runs) ->
# the GUI-app path (the future tailscale-app cask). See CLAUDE.md the Tailscale section.
TAILSCALE="${OSQUERY_TAILSCALE_BIN:-$(command -v tailscale || echo /Applications/Tailscale.app/Contents/MacOS/Tailscale)}"
STATE="${OSQUERY_TAILSCALE_STATE:-$HOME/.local/state/osquery-tailscale-funnel}"

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-alert-dispatch.sh"

mkdir -p "$(dirname "$STATE")"

# Prior state is JSON {funnel: active|inactive, monitor: ok|missing|unavailable}. Read + validate;
# an unreadable/corrupt state yields empty fields (treated as no trustworthy baseline).
prev_funnel=""
prev_monitor=""
if [ -f "$STATE" ]; then
  prev_funnel=$(jq -r '.funnel // empty' <"$STATE" 2>/dev/null || echo "")
  prev_monitor=$(jq -r '.monitor // empty' <"$STATE" 2>/dev/null || echo "")
fi
# Only active/inactive is a valid funnel baseline (R2-5b): a corrupt/absent value is NOT a baseline.
case "$prev_funnel" in active | inactive) ;; *) prev_funnel="" ;; esac

# Atomically persist state (temp + rename), owner-only.
write_state() {
  (
    umask 077
    jq -cn --arg funnel "$1" --arg monitor "$2" '{funnel:$funnel, monitor:$monitor}' >"$STATE.tmp"
  ) && mv -f "$STATE.tmp" "$STATE" && chmod 600 "$STATE"
}

# Dispatch a CRIT monitoring-gap page ONCE on entering a gap (from a healthy monitor), staying
# silent while already blind, and preserve the prior valid funnel state so a later recovery can
# detect a transition against a real baseline. <gap-kind> <human-detail>.
gap_alert() {
  local kind="$1" detail="$2"
  # Page once: only when transitioning FROM a healthy monitor (ok/empty) INTO a gap.
  if [ "$prev_monitor" = "ok" ] || [ -z "$prev_monitor" ]; then
    send_alert CRIT "🔴 **CRITICAL**" \
      "**Tailscale funnel monitoring is BLIND - public-exposure paging is not running.**"$'\n'"- $detail"$'\n'"- A funnel could be opened to the PUBLIC internet without a page while this is blind. **Fix now.**" \
      "Sosumi" "tailscale-gap:$kind" || true
  fi
  write_state "${prev_funnel:-inactive}" "$kind" # preserve the last valid funnel state
}

# A missing binary is a monitoring gap (the regression that left this monitor dead on the formula
# install). CRIT so it reaches the remote channel, page-once.
if [ ! -x "$TAILSCALE" ]; then
  echo "WARN: no tailscale binary ($TAILSCALE) - funnel monitoring is blind" >&2
  gap_alert missing "No tailscale binary found at $TAILSCALE."
  exit 0
fi

# Run the status command, capturing its exit code (do NOT `|| true` it into a false inactive).
rc=0
funnel=$("$TAILSCALE" funnel status 2>/dev/null) || rc=$?

# A failed command, or an EMPTY output we cannot classify, is a gap - not a silent "inactive".
# bt holds a literal backtick so the command renders as Discord inline-code without a lint
# conflict (shfmt would single-quote a no-expansion string, then SC2016 flags the bare backticks).
bt='`'
if [ "$rc" -ne 0 ]; then
  gap_alert unavailable "${bt}tailscale funnel status${bt} exited $rc - the funnel state is unreadable."
  exit 0
fi
if [ -z "$funnel" ]; then
  gap_alert unavailable "${bt}tailscale funnel status${bt} returned no output - the funnel state is unreadable."
  exit 0
fi

# Classify the (non-empty) output: an explicit "no serve config" is inactive; anything else is an
# exposed funnel. Both are valid states, so the monitor is healthy (monitor=ok).
cur="active"
if printf '%s' "$funnel" | grep -qi 'no serve config'; then
  cur="inactive"
fi
write_state "$cur" ok

# Page on a funnel found active when the prior VALID baseline was not already active: a fresh
# inactive->active transition, an initial active observation (no baseline), or an active reading on
# recovery from a blind window (prev_funnel was cleared by the gap). An active steady state (prev
# baseline already active) is silent. R2-5: an active reading after a gap MUST page, never be
# accepted as a silent new baseline.
if [ "$cur" = "active" ] && [ "$prev_funnel" != "active" ]; then
  body=$(printf '%s\n' \
    "**Tailscale Funnel is exposing a local service to the PUBLIC internet.**" \
    '- Did you set this up? If not, close it now: **tailscale funnel reset**' \
    '```' "$funnel" '```')
  send_alert CRIT "🔴 **CRITICAL**" "$body" "Sosumi" "tailscale-funnel-active" || true
fi
