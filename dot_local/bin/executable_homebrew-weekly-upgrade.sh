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
LOCKFILE="${HOMEBREW_WEEKLY_LOCKFILE:-$HOME/.local/state/homebrew-weekly-upgrade.lock}"

# Serialize: one weekly upgrade at a time, via the KERNEL. The Monday-noon
# LaunchAgent and an ad-hoc `just brew-upgrade` must never run concurrent
# brew/mas/cleanup/tailscaled operations. macOS ships /usr/bin/lockf
# (flock(2)-backed): open $LOCKFILE on fd 9 and test-acquire with `lockf -s -t 0`
# (non-blocking; exit 75 = EX_TEMPFAIL when another process already holds it).
# The kernel releases the lock automatically when the fd closes (normal exit or
# crash), so there is no stale-lock class. Non-darwin hosts (no /usr/bin/lockf)
# proceed unlocked: the contending scheduled runs are darwin-only. Absolute path
# because a stripped PATH would not carry /usr/bin. (House precedent: the same
# kernel-lock shape guards ~/.local/bin/update-skills.sh.)
acquire_lock() {
  [[ -x /usr/bin/lockf ]] || return 0
  mkdir -p "$(dirname "$LOCKFILE")" 2>/dev/null || return 1
  exec 9>>"$LOCKFILE" || return 1
  /usr/bin/lockf -s -t 0 9
}

# Aggregate exit status: a failing step is logged and the run continues, but the
# helper exits non-zero when any step failed (an all-failed run must not exit 0).
weekly_upgrade_failures=0

run() {
  # run "<label>" cmd args... -- print a section header, run, log the outcome,
  # count a failure, and continue regardless of exit status.
  local label="$1"
  shift
  printf '== %s ==\n' "$label"
  if "$@"; then
    printf '   ok: %s\n' "$label"
  else
    printf '   FAILED (exit %d): %s\n' "$?" "$label" >&2
    weekly_upgrade_failures=$((weekly_upgrade_failures + 1))
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

if ! acquire_lock; then
  printf 'homebrew-weekly-upgrade: another run holds the lock; deferring (exit 75).\n' >&2
  exit 75
fi

printf '=== homebrew-weekly-upgrade %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

run "brew update" "$BREW" update
run "brew outdated" "$BREW" outdated
run "mas outdated" "$MAS" outdated
run "brew upgrade" "$BREW" upgrade
run "tailscaled refresh (if upgraded)" refresh_tailscaled
run "mas upgrade" "$MAS" upgrade
run "brew cleanup" "$BREW" cleanup

printf '=== done %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

if [[ $weekly_upgrade_failures -gt 0 ]]; then
  printf '=== %d step(s) failed; see FAILED lines above ===\n' "$weekly_upgrade_failures" >&2
  exit 1
fi
