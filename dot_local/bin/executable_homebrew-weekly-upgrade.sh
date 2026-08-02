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
# It also RELAYS, on both of this machine's channels. A failing step alerts on
# the existing relay route so it lands in the priority channel, and every
# SCHEDULED run posts a record of what it upgraded to the separate
# #unattended-upgrades channel. Act on one, record the other. Before this the
# helper relayed nothing at all: a weekly upgrade could fail every step and the
# only trace was a line in a log nobody opens.
#
# Usage: homebrew-weekly-upgrade.sh [--scheduled]
#   --scheduled  marks this as the LaunchAgent's run. ONLY a scheduled run posts
#                a weekly record, mirroring update-skills.sh. Without the marker,
#                an operator running `just brew-upgrade` on a Wednesday would
#                post a weekly entry and a dead LaunchAgent would look alive,
#                which inverts the one signal the record carries. Failures alert
#                either way: a failure is a failure whoever started it.
#
# brew/mas are overridable (HOMEBREW_WEEKLY_BREW / HOMEBREW_WEEKLY_MAS) so the
# test harness can inject mocks; default to absolute Homebrew paths.
set -uo pipefail

BREW="${HOMEBREW_WEEKLY_BREW:-/opt/homebrew/bin/brew}"
MAS="${HOMEBREW_WEEKLY_MAS:-/opt/homebrew/bin/mas}"
TS="${HOMEBREW_WEEKLY_TAILSCALED:-/opt/homebrew/opt/tailscale/bin/tailscaled}"
LOCKFILE="${HOMEBREW_WEEKLY_LOCKFILE:-$HOME/.local/state/homebrew-weekly-upgrade.lock}"
STATE_DIR="${HOMEBREW_WEEKLY_STATE_DIR:-$HOME/.local/state/homebrew-weekly-upgrade}"
LOG_SUCCESS_MARKER="$STATE_DIR/last-success-at"
LOG_WEEK_GUARD="$STATE_DIR/last-log-week"

# An unknown argument is an ERROR, never a silent fallthrough: a typo'd marker in
# the plist would otherwise run every week and quietly post nothing, which looks
# exactly like a dead LaunchAgent.
SCHEDULED=""
for arg in "$@"; do
  case "$arg" in
    --scheduled) SCHEDULED=1 ;;
    *)
      printf 'usage: homebrew-weekly-upgrade.sh [--scheduled]\nhomebrew-weekly-upgrade: unknown argument: %s\n' "$arg" >&2
      exit 2
      ;;
  esac
done

# The weekly record. Shared with update-skills.sh so the two jobs report in the
# same shape; see unattended-log-lib.sh for why every entry states its own gap.
# A missing library is LOUD and never fatal: upgrading matters more than
# bookkeeping, but a silently absent record is the invisibility it exists to end.
UNATTENDED_LOG_LIB="$(dirname "${BASH_SOURCE[0]}")/unattended-log-lib.sh"
UNATTENDED_LOG_AVAILABLE=""
if [[ -r $UNATTENDED_LOG_LIB ]]; then
  # shellcheck source=dot_local/bin/unattended-log-lib.sh
  source "$UNATTENDED_LOG_LIB"
  UNATTENDED_LOG_AVAILABLE=1
else
  printf 'homebrew-weekly-upgrade: WARNING %s is missing; no weekly record will be posted (run chezmoi apply)\n' \
    "$UNATTENDED_LOG_LIB" >&2
fi

# Captured BEFORE anything can rewrite the marker, so a successful run reports
# the gap to the PREVIOUS success rather than to itself.
LOG_GAP_LINE=""
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  LOG_GAP_LINE="$(unattended_log_gap_line "$LOG_SUCCESS_MARKER")"
fi

# The relay script by ABSOLUTE path. The LaunchAgent's PATH is
# /opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin, with no
# ~/.local/bin in it, so a bare `relay.sh` would never be found under launchd and
# every alert would vanish exactly when it mattered.
RELAY="${HOMEBREW_WEEKLY_RELAY:-$HOME/.local/bin/relay.sh}"

# weekly_alert <state> <detail> -- the EXISTING relay route, so this lands in the
# priority channel beside every other alert on this machine. Best effort: a
# missing relay never fails the upgrade, and a failure to notify is stated.
weekly_alert() {
  local state="$1" detail="$2"
  if [[ ! -x $RELAY ]]; then
    printf 'homebrew-weekly-upgrade: relay.sh is not executable at %s; this alert was NOT delivered\n' "$RELAY" >&2
    return 0
  fi
  "$RELAY" --agent homebrew-weekly-upgrade --state "$state" \
    --project "$(unattended_log_host 2>/dev/null || printf 'unknown-host')" --detail "$detail" 9>&- || true
  return 0
}

# weekly_record <class> <body> -- the LOG route. Gated on --scheduled (above) and
# on one entry per ISO week.
weekly_record() {
  local class="$1" body="$2" detail
  [[ -n $SCHEDULED ]] || return 0
  [[ -n $UNATTENDED_LOG_AVAILABLE ]] || return 0
  if ! unattended_log_claim_week "$LOG_WEEK_GUARD" "$class"; then
    printf 'homebrew-weekly-upgrade: this ISO week already has a %s-or-better record; not posting again\n' "$class"
    return 0
  fi
  detail="$(printf 'run at %s\n%s\n%s' "$(unattended_log_now_iso)" "$LOG_GAP_LINE" "$body")"
  UNATTENDED_LOG_RELAY="$RELAY" unattended_log_post homebrew-weekly-upgrade "$class" \
    "$(unattended_log_host)" "$detail"
  return 0
}

# __homebrew_package_snapshot <kind> -- "<name><TAB><version>" lines, the input
# shape unattended_log_change_line reads. Homebrew and the App Store BOTH report
# real version numbers, so unlike the npx skills lane this record can name the
# exact transition. Always exits 0: a record that cannot be computed must not
# fail the upgrade it is recording.
__homebrew_package_snapshot() {
  case "$1" in
    brew)
      # `brew list --versions` prints "<name> <version> [<version>...]"; the
      # remainder is joined so a formula keeping two versions installed reads as
      # one fingerprint rather than being truncated to the first.
      "$BREW" list --versions 2>/dev/null |
        awk 'NF >= 2 { name = $1; $1 = ""; sub(/^[ \t]+/, ""); printf "%s\t%s\n", name, $0 }' |
        sort || true
      ;;
    mas)
      # `mas list` prints "<id> <Name> (<version>)". The id is the stable key but
      # the NAME is what a reader recognizes, so the name is the key here and the
      # id is dropped.
      "$MAS" list 2>/dev/null |
        sed -E 's/^[0-9]+[[:space:]]+(.*)[[:space:]]+\(([^)]*)\)[[:space:]]*$/\1\t\2/' |
        grep -F "$(printf '\t')" | sort || true
      ;;
  esac
  return 0
}

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
# The LABELS of the failed steps, kept as an array (never a space-joined string),
# because they are what makes the alert actionable: "the weekly upgrade failed"
# is not something an operator can do anything with, "brew upgrade, brew cleanup"
# is.
weekly_upgrade_failed_steps=()

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
    weekly_upgrade_failed_steps+=("$label")
  fi
}

# Re-copy the tailscaled binary into the system daemon if brew just upgraded it (the
# daemon runs a root-owned copy in /usr/local/bin that `brew upgrade` does not touch).
# Guarded so it only fires when the binary actually changed -- no needless weekly VPN
# restart -- and only when tailscale is installed. sudo is passwordless here via the
# user's sudo config; if that ever changes the step just logs and continues.
#
# shellcheck disable=SC2329,SC2317 # invoked indirectly, as an argument to run()
refresh_tailscaled() {
  [[ -x $TS ]] || return 0
  cmp -s "$TS" /usr/local/bin/tailscaled 2>/dev/null && return 0
  sudo -n "$TS" install-system-daemon
}

if ! acquire_lock; then
  printf 'homebrew-weekly-upgrade: another run holds the lock; deferring (exit 75).\n' >&2
  # A deferral is NOT a failure: nothing was attempted, so it is recorded rather
  # than alerted. Without an entry, a week spent entirely in contention would
  # leave the channel empty, which reads as a dead LaunchAgent.
  weekly_record deferred "nothing was attempted: another homebrew-weekly-upgrade run already holds the serialize lock, so this run deferred (exit 75)."
  exit 75
fi

printf '=== homebrew-weekly-upgrade %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

snapshot_dir=""
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  snapshot_dir="$(mktemp -d)"
  trap 'rm -rf "$snapshot_dir"' EXIT
  __homebrew_package_snapshot brew >"$snapshot_dir/brew.before"
  __homebrew_package_snapshot mas >"$snapshot_dir/mas.before"
fi

run "brew update" "$BREW" update
run "brew outdated" "$BREW" outdated
run "mas outdated" "$MAS" outdated
run "brew upgrade" "$BREW" upgrade
run "tailscaled refresh (if upgraded)" refresh_tailscaled
run "mas upgrade" "$MAS" upgrade
run "brew cleanup" "$BREW" cleanup

printf '=== done %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# The weekly RECORD, posted whether or not anything moved. A run that upgraded
# nothing is precisely where the gap figure is the only information the entry
# carries, so suppressing the empty entry would throw away the reason the channel
# exists. The failed-step count rides along rather than being implied by silence.
if [[ -n $snapshot_dir ]]; then
  __homebrew_package_snapshot brew >"$snapshot_dir/brew.after"
  __homebrew_package_snapshot mas >"$snapshot_dir/mas.after"
  weekly_record completed "$(printf '%s\n%s\nfailed steps: %d' \
    "$(unattended_log_change_line "$snapshot_dir/brew.before" "$snapshot_dir/brew.after" \
      'formulae and casks' \
      'Versions are what brew list --versions reports; a formula reinstalled at the same version does not appear here, and a cask Homebrew tracks only as latest reports that literal string rather than a version.' \
      versions)" \
    "$(unattended_log_change_line "$snapshot_dir/mas.before" "$snapshot_dir/mas.after" \
      'App Store apps' \
      'Versions are what mas list reports, keyed by app name.' \
      versions)" \
    "$weekly_upgrade_failures")"
fi

if [[ $weekly_upgrade_failures -gt 0 ]]; then
  printf '=== %d step(s) failed; see FAILED lines above ===\n' "$weekly_upgrade_failures" >&2
  # ALERT, on the existing route, so this lands in the priority channel. The
  # failed step names are the whole point: "the weekly upgrade failed" is not
  # something an operator can act on, "brew upgrade failed" is.
  weekly_alert upgrade-failed \
    "$(printf 'The weekly Homebrew upgrade finished with %d failed step(s): %s. Full output: ~/.local/log/homebrew/weekly-upgrade.log' \
      "$weekly_upgrade_failures" "${weekly_upgrade_failed_steps[*]}")"
  exit 1
fi

# A fully clean run is what "last successful run" has to mean, so the marker is
# written here and nowhere else. A failing run deliberately leaves it alone, so
# the gap keeps growing until a run actually succeeds.
[[ -n $UNATTENDED_LOG_AVAILABLE ]] && unattended_log_mark_success "$LOG_SUCCESS_MARKER"
exit 0
