#!/usr/bin/env bash
#
# homebrew-weekly-upgrade.sh -- run by the com.webdavis.homebrew-weekly-upgrade
# LaunchAgent every Monday at 12:00 (when the operator is present). Upgrades
# Homebrew formulae + casks + Mac App Store apps, then cleans up. Prints a
# sectioned, timestamped report to stdout; the LaunchAgent routes that to
# ~/.local/log/homebrew/weekly-upgrade.log. Resilient: a failing step is logged
# but never aborts the rest, and cleanup always runs. No Gatekeeper/quarantine
# stripping -- present-time "Open?" prompts are acceptable (operator is here).
#
# brew/mas are overridable (HOMEBREW_WEEKLY_BREW / HOMEBREW_WEEKLY_MAS) so the
# test harness can inject mocks; default to absolute Homebrew paths.
set -uo pipefail

BREW="${HOMEBREW_WEEKLY_BREW:-/opt/homebrew/bin/brew}"
MAS="${HOMEBREW_WEEKLY_MAS:-/opt/homebrew/bin/mas}"
TS="${HOMEBREW_WEEKLY_TAILSCALED:-/opt/homebrew/opt/tailscale/bin/tailscaled}"

run() {
  # run "<label>" cmd args... -- print a section header, run, log the outcome,
  # and continue regardless of exit status.
  local label="$1"
  shift
  printf '== %s ==\n' "$label"
  if "$@"; then
    printf '   ok: %s\n' "$label"
  else
    printf '   FAILED (exit %d): %s\n' "$?" "$label" >&2
  fi
}

# Re-copy the tailscaled binary into the system daemon if brew just upgraded it (the
# daemon runs a root-owned copy in /usr/local/bin that `brew upgrade` does not touch).
# Guarded so it only fires when the binary actually changed -- no needless weekly VPN
# restart -- and only when tailscale is installed. sudo is passwordless here via the
# user's sudo config; if that ever changes the step just logs and continues.
refresh_tailscaled() {
  [[ -x $TS ]] || return 0
  cmp -s "$TS" /usr/local/bin/tailscaled 2>/dev/null && return 0
  sudo -n "$TS" install-system-daemon
}

printf '=== homebrew-weekly-upgrade %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

run "brew update" "$BREW" update
run "brew outdated" "$BREW" outdated
run "mas outdated" "$MAS" outdated
run "brew upgrade" "$BREW" upgrade
run "tailscaled refresh (if upgraded)" refresh_tailscaled
run "mas upgrade" "$MAS" upgrade
run "brew cleanup" "$BREW" cleanup

printf '=== done %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
