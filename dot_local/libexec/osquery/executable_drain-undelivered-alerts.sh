#!/usr/bin/env bash
#
# drain-undelivered-alerts.sh, run on a timer by launchd (StartInterval 300).
# Sources the shared dispatch library and drains the undelivered-alerts SQLite
# store: every stored CRIT page that has not yet reached the hermes #priority
# webhook is replayed in occurrence order, and each delivered row is removed.
# Nothing else runs this drain on a schedule, so without it a page stored during
# a gateway outage would sit undelivered until a producer happened to fire again.
#
# A single-instance lock guards the case where one drain runs longer than the
# 300-second timer interval and the next tick fires while it is still going. The
# two runs would otherwise read the same row snapshot and POST every page twice.
# The lock lets exactly one drain run at a time; an overlapping run exits 0
# immediately, because the drain that holds the lock already sweeps every stored
# row and a second concurrent drain has nothing to add.
#
# Exit status is always 0: a drain is a best-effort background sweep, and a
# failure inside it must never surface as a launchd job error. The library's
# retry_undelivered_alerts is itself set -e-safe (an empty store, a missing
# database, or a malformed row is a quiet no-op), so all this wrapper adds is the
# single-instance lock and the always-zero exit.

set -euo pipefail

# The shared dispatch library provides retry_undelivered_alerts and the SQLite
# store helpers. Source it from the same deployed path the three producers
# (results-alerter, firewall-gatekeeper-monitor, uptime-watchdog) use, so all
# four agree on one implementation of the store and its drain.
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

# The single-instance lock file sits beside the store it guards, so every
# drainer invocation contends on one lock no matter what launched it. The
# default is derived from the store path (itself overridable for tests), so
# there is never a second path to keep in sync with the first.
OSQUERY_DRAIN_LOCK_FILE="${OSQUERY_DRAIN_LOCK_FILE:-${OSQUERY_UNDELIVERED_ALERTS_DB}.drain.lock}"

# Take the single-instance lock and report whether this run may proceed (0 to
# proceed, nonzero to skip). Uses the kernel lock /usr/bin/lockf on a held file
# descriptor: the kernel releases it on ANY exit, normal or crash, so a drain
# SIGKILLed mid-run can never wedge the lock and block every later drain (there
# is no stale-lock state to clean up). The acquire is non-blocking (-t 0): an
# overlapping run fails to take the lock and returns nonzero, so the caller skips
# rather than queueing behind the running drain. House precedent: hue-pulse.sh,
# homebrew-weekly-upgrade.sh, and update-skills.sh all guard with this same
# kernel-lock shape. The lockf binary path is overridable (OSQUERY_DRAIN_LOCKF_BIN)
# so the platform-fallback and fail-closed paths can be exercised in tests.
#
# The lock is MUTUAL EXCLUSION, so on a genuine setup error it must fail CLOSED
# (return nonzero, the caller skips this sweep), NEVER fall through and run the
# sweep unlocked: two overlapping launchd runs sweeping at once would each read
# the same row snapshot and double-POST every page, the exact race this lock
# exists to prevent. A skipped sweep loses nothing; the next 300-second tick
# retries. The ONE exception is a host with no lockf at all (any non-darwin box,
# e.g. Linux CI): there is no kernel lock to take, so the drain proceeds unlocked
# by design, matching the library's darwin-only runtime.
take_single_instance_lock() {
  local lockf_bin="${OSQUERY_DRAIN_LOCKF_BIN:-/usr/bin/lockf}"
  # No lockf available: the documented non-darwin fallback. Proceed unlocked so
  # the drain still runs; there is no lock to fail closed on.
  [[ -x $lockf_bin ]] || return 0
  # From here the lock is REQUIRED. Any failure to set it up fails CLOSED.
  local lock_directory
  lock_directory="$(dirname "$OSQUERY_DRAIN_LOCK_FILE")"
  mkdir -p "$lock_directory" 2>/dev/null || return 1
  exec 9>>"$OSQUERY_DRAIN_LOCK_FILE" 2>/dev/null || return 1
  "$lockf_bin" -s -t 0 9
}

# main -- take the single-instance lock, drain the store once, and exit 0.
main() {
  if ! take_single_instance_lock; then
    # Another drain already holds the lock and covers every stored row; skip.
    return 0
  fi
  retry_undelivered_alerts
  return 0
}

main "$@"
