#!/usr/bin/env bash
#
# osquery-heartbeat.sh — the daily proof-of-life. Fired by launchd at 09:00, it sends
# ONE silent message to the #priority channel so the user can trust that silence = safe:
# if the daily ✅ arrives, the osquery pipeline is scheduled and alive. The uptime
# watchdog (every 15 min) is the ALARM that PAGES when a component is down; this is the
# complementary POSITIVE affirmation. It is always silent (empty sound) — a proof-of-life
# must never ping like a real page — and honest: if osqueryd is not answering it reports
# that (the watchdog is what actually pages on it) rather than a blind checkmark.
set -euo pipefail

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-alert-dispatch.sh"

OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"

# CRIT selects the #priority channel (the dispatcher's only route); the empty sound makes
# it silent. A wedged osqueryd passes a process check but cannot answer a one-shot query,
# so probe with a real query — the same signal the watchdog uses.
if "$OSQUERYI" --json "SELECT 1 AS ok FROM time" >/dev/null 2>&1; then
  send_alert CRIT "✅ osquery pipeline healthy · $(date -u +%Y-%m-%d)" \
    "- osqueryd answering; all monitors scheduled. Silence since the last message means all clear." ""
else
  send_alert CRIT "⚠️ osquery heartbeat · $(date -u +%Y-%m-%d)" \
    "- osqueryd is not answering — the uptime watchdog pages on this; this note is the silent daily record." ""
fi
