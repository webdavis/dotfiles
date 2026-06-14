#!/usr/bin/env bash
#
# osquery-tailscale-monitor.sh — polled every 60s by a launchd StartInterval agent.
# Pages on the off->on transition of `tailscale funnel` (a local port newly exposed
# to the PUBLIC internet). Funnel traffic tunnels through tailscaled, so osquery
# cannot see it as a listening port — polling the CLI is the only way to catch it.
# Silent in steady state.
set -euo pipefail

TAILSCALE="${OSQUERY_TAILSCALE_BIN:-/Applications/Tailscale.app/Contents/MacOS/Tailscale}"
STATE="${OSQUERY_TAILSCALE_STATE:-$HOME/.local/state/osquery-tailscale-funnel}"

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-alert-dispatch.sh"

mkdir -p "$(dirname "$STATE")"
[ -x "$TAILSCALE" ] || exit 0 # tailscale not installed → nothing to check

funnel=$("$TAILSCALE" funnel status 2>/dev/null || true)

# "No serve config" (or empty) = nothing exposed; anything else = an active funnel.
cur="inactive"
if [ -n "$funnel" ] && ! printf '%s' "$funnel" | grep -qi 'no serve config'; then
  cur="active"
fi

# First run: baseline silently — we cannot claim a transition with no prior state.
if [ ! -f "$STATE" ]; then
  printf '%s\n' "$cur" >"$STATE"
  exit 0
fi

prev=$(cat "$STATE" 2>/dev/null || true)
printf '%s\n' "$cur" >"$STATE"

# Page only on the inactive->active transition (a re-enable, not steady state).
if [ "$cur" = "active" ] && [ "$prev" = "inactive" ]; then
  body=$(printf '%s\n' \
    "**Tailscale Funnel is exposing a local service to the PUBLIC internet.**" \
    '- Did you set this up? If not, close it now: **tailscale funnel reset**' \
    '```' "$funnel" '```')
  send_alert CRIT "🔴 **CRITICAL**" "$body" "Sosumi"
fi
