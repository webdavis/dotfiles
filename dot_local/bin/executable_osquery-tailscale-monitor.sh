#!/usr/bin/env bash
#
# osquery-tailscale-monitor.sh — polled every 60s by a launchd StartInterval agent.
# Pages on the off->on transition of `tailscale funnel` (a local port newly exposed
# to the PUBLIC internet). Funnel traffic tunnels through tailscaled, so osquery
# cannot see it as a listening port — polling the CLI is the only way to catch it.
# Silent in steady state.
set -euo pipefail

# Resolution order: explicit override → PATH (the headless homebrew formula this
# machine runs — the LaunchAgent PATH includes /opt/homebrew/bin) → the GUI-app
# path (the future tailscale-app cask). See CLAUDE.md §Tailscale.
TAILSCALE="${OSQUERY_TAILSCALE_BIN:-$(command -v tailscale || echo /Applications/Tailscale.app/Contents/MacOS/Tailscale)}"
STATE="${OSQUERY_TAILSCALE_STATE:-$HOME/.local/state/osquery-tailscale-funnel}"

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-alert-dispatch.sh"

mkdir -p "$(dirname "$STATE")"

# A monitoring gap is itself a security event: a missing binary warns once (on the
# transition into "missing", never every 60s) instead of silently disabling the
# funnel pager — the regression that left this monitor dead on the formula install.
if [ ! -x "$TAILSCALE" ]; then
  prev=$(cat "$STATE" 2>/dev/null || true)
  printf '%s\n' "missing" >"$STATE"
  echo "WARN: no tailscale binary ($TAILSCALE) — funnel monitoring is blind" >&2
  if [ "$prev" != "missing" ]; then
    send_alert WARN "⚠️ **WARNING**" "**Tailscale funnel monitoring is not running** — no tailscale binary found. Public-exposure paging is blind until this is fixed." "Funk"
  fi
  exit 0
fi

funnel=$("$TAILSCALE" funnel status 2>/dev/null || true)

# "No serve config" (or empty) = nothing exposed; anything else = an active funnel.
cur="inactive"
if [ -n "$funnel" ] && ! printf '%s' "$funnel" | grep -qi 'no serve config'; then
  cur="active"
fi

# First run (no prior state): baseline silently ONLY when inactive. A funnel found
# ALREADY active at the first observation (or after the state file is lost) is a
# pre-existing PUBLIC exposure — accepting it as the silent baseline would hide exactly
# the event this monitor exists to catch. Treat a missing state as prev="inactive" so an
# initial active observation flows into the page path below. (FX7)
prev="inactive"
if [ -f "$STATE" ]; then
  prev=$(cat "$STATE" 2>/dev/null || true)
fi
printf '%s\n' "$cur" >"$STATE"

# Page on the inactive->active transition (a re-enable, not steady state), on
# missing->active (a funnel found running when monitoring recovers from a blind window),
# and on the initial active observation (prev defaulted to inactive above): each is a
# public exposure the operator has not yet been told about.
if [ "$cur" = "active" ] && { [ "$prev" = "inactive" ] || [ "$prev" = "missing" ]; }; then
  body=$(printf '%s\n' \
    "**Tailscale Funnel is exposing a local service to the PUBLIC internet.**" \
    '- Did you set this up? If not, close it now: **tailscale funnel reset**' \
    '```' "$funnel" '```')
  send_alert CRIT "🔴 **CRITICAL**" "$body" "Sosumi"
fi
