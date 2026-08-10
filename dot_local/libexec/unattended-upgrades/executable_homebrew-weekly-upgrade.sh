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
# The osquery converge tool, run right after the upgrade group below. The
# osquery CASK reinstalls the vendor package and wipes our config, packs and
# flags out of /var/osquery, and this job is the only thing on the machine that
# upgrades that cask, so the wipe and its repair belong in the same run.
OSQUERY_CONVERGE="${HOMEBREW_WEEKLY_OSQUERY_CONVERGE:-$HOME/.local/libexec/osquery/osquery-converge.sh}"
LOCKFILE="${HOMEBREW_WEEKLY_LOCKFILE:-$HOME/.local/state/homebrew-weekly-upgrade.lock}"
STATE_DIR="${HOMEBREW_WEEKLY_STATE_DIR:-$HOME/.local/state/homebrew-weekly-upgrade}"
LOG_SUCCESS_MARKER="$STATE_DIR/last-success-at"
LOG_WEEK_GUARD="$STATE_DIR/log-week-claims"
# The durable record of what THIS run moved, for the osquery file-integrity page
# to correlate against days later. Keep this literal in sync with
# OSQUERY_UPGRADE_RECORD in
# ~/.local/libexec/osquery/results-alerter/file-integrity-triage.sh (the
# consumer); test/unit/osquery-file-integrity-triage.sh pins them equal, because
# a rename in one alone leaves that page answering no-record forever, which reads
# exactly like a quiet month of upgrades.
UPGRADE_RECORD="$STATE_DIR/last-upgrade-changes.tsv"

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
# same shape; see helpers/log-entries.sh for why every entry states its own gap.
# A missing library is LOUD and never fatal: upgrading matters more than
# bookkeeping, but a silently absent record is the invisibility it exists to end.
UNATTENDED_LOG_LIB="$(dirname "${BASH_SOURCE[0]}")/helpers/log-entries.sh"
UNATTENDED_LOG_AVAILABLE=""
if [[ -r $UNATTENDED_LOG_LIB ]]; then
  # shellcheck source=dot_local/libexec/unattended-upgrades/helpers/log-entries.sh
  source "$UNATTENDED_LOG_LIB"
  UNATTENDED_LOG_AVAILABLE=1
else
  printf 'homebrew-weekly-upgrade: WARNING %s is missing; no weekly record will be posted (run chezmoi apply)\n' \
    "$UNATTENDED_LOG_LIB" >&2
fi

# The entry's opening lines (this run's timestamp and the gap to the previous
# success), from ONE clock reading, captured BEFORE anything can rewrite the
# marker: a gap read later would be the run's own timestamp, and a timestamp read
# later would sit hours away from the gap printed under it.
LOG_ENTRY_HEADER=""
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  LOG_ENTRY_HEADER="$(unattended_log_entry_header "$LOG_SUCCESS_MARKER")"
fi

# The upgrade record's timestamp, read ONCE here and used by both the record this
# run publishes before it starts and the one it finalizes after (see
# write_upgrade_record). It dates the moment the run BEGAN, which is what makes
# the record cover the window it describes: the epoch is what the consumer does
# arithmetic on and the ISO string is what it renders, and two `date` calls could
# disagree about which run they belong to.
UPGRADE_RECORD_EPOCH=""
UPGRADE_RECORD_ISO=""
read -r UPGRADE_RECORD_EPOCH UPGRADE_RECORD_ISO < <(date -u '+%s %Y-%m-%dT%H:%M:%SZ' 2>/dev/null) || true

# The relay script by ABSOLUTE path. The LaunchAgent's PATH is
# /opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin, with no
# ~/.local/bin in it, so a bare `relay.sh` would never be found under launchd and
# every alert would vanish exactly when it mattered.
RELAY="${HOMEBREW_WEEKLY_RELAY:-$HOME/.local/libexec/pns/relay.sh}"

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
# on the weekly claim, which admits one entry per class per ISO week: one, or two
# in a week that defers before it completes.
weekly_record() {
  local class="$1" body="$2" detail
  [[ -n $SCHEDULED ]] || return 0
  [[ -n $UNATTENDED_LOG_AVAILABLE ]] || return 0
  if ! unattended_log_claim_week "$LOG_WEEK_GUARD" "$class"; then
    printf 'homebrew-weekly-upgrade: this ISO week already has a %s-or-better record; not posting again\n' "$class"
    return 0
  fi
  detail="$(printf '%s\n%s' "$LOG_ENTRY_HEADER" "$body")"
  # Claimed BEFORE the attempt, so two overlapping runs cannot both post, and
  # GIVEN BACK when the attempt failed, so a week is never marked done with
  # nothing sent. A broken record channel cannot report itself, so it is also
  # said once a week on the ALERT route, which is the one that lands in the
  # priority channel.
  if ! UNATTENDED_LOG_RELAY="$RELAY" unattended_log_post homebrew-weekly-upgrade "$class" \
    "$(unattended_log_host)" "$detail"; then
    unattended_log_release_week "$LOG_WEEK_GUARD" "$class"
    printf 'homebrew-weekly-upgrade: the weekly record was NOT delivered; this week stays unclaimed so a later run retries\n' >&2
    UNATTENDED_LOG_RELAY="$RELAY" unattended_log_alert_delivery_failure "$LOG_WEEK_GUARD" homebrew-weekly-upgrade
  fi
  return 0
}

# __homebrew_package_snapshot <kind> -- "<name><TAB><version>" lines, the input
# shape unattended_log_change_line reads. Homebrew and the App Store BOTH report
# real version numbers, so unlike the npx skills lane this record can name the
# exact transition.
#
# RETURNS THE SOURCE COMMAND'S STATUS (pipefail carries it through the
# transform). It used to swallow every failure and hand back an empty file, so a
# `brew list --versions` that failed rendered as "0 of 0 tracked entries
# changed": a clean week, on a machine whose package manager could not be
# queried at all. An EMPTY answer from a command that SUCCEEDED is a different
# fact and stays a success, which is why the mas transform ends in an awk filter
# rather than a grep: grep exits 1 on no match, so a machine with no App Store
# apps would otherwise report its truthful nothing as a failure every week.
__homebrew_package_snapshot() {
  case "$1" in
    brew)
      # `brew list --versions` prints "<name> <version> [<version>...]"; the
      # remainder is joined so a formula keeping two versions installed reads as
      # one fingerprint rather than being truncated to the first.
      "$BREW" list --versions 2>/dev/null |
        awk 'NF >= 2 { name = $1; $1 = ""; sub(/^[ \t]+/, ""); printf "%s\t%s\n", name, $0 }' |
        sort
      ;;
    mas)
      # `mas list` prints "<id> <Name> (<version>)". The id is the stable key but
      # the NAME is what a reader recognizes, so the name is the key here and the
      # id is dropped.
      "$MAS" list 2>/dev/null |
        sed -E 's/^[0-9]+[[:space:]]+(.*)[[:space:]]+\(([^)]*)\)[[:space:]]*$/\1\t\2/' |
        awk -F'\t' 'NF >= 2' | sort
      ;;
    *)
      printf 'homebrew-weekly-upgrade: unknown snapshot kind: %s\n' "$1" >&2
      return 1
      ;;
  esac
}

# write_upgrade_record <started|completed> -- persist what this run moved, so
# something days later can ask whether an upgrade plausibly explains a file that
# changed.
#
# PUBLISHED TWICE, ONE RUN. `started` writes the run line alone before the first
# brew step; `completed` rewrites it with the package rows once the after
# snapshot is taken. Both carry the SAME timestamp, so the two are one record at
# two levels of detail rather than two runs. It was written only at the end
# before, and that left the whole upgrade window uncovered: a watched file
# rewritten in the first seconds of a run pages while the newest record on disk
# is still the previous week's, so the correlation answers "no recorded upgrade
# in the last 3 days" about a file an upgrade is moving right then.
#
# THE LIMIT, since this is a lead and its limits are what keep it honest: a page
# fired mid-run reads a record that dates the run and names NOTHING, because
# nothing has been compared yet. That renders as the no-match line, which states
# what the run recorded rather than claiming a mapping; it is the same sentence a
# genuinely empty week produces. Naming the packages mid-run would mean diffing
# the Cellar on every page, which is the fork-brew-from-the-alert-path this
# deliberately does not do.
#
# WHO READS IT. The osquery file-integrity page fires when a watched file leaves
# its known-good manifest, and a vendor update and a tamper used to render the
# same body. The page now carries a correlation line built from this file. The
# record is a LEAD there and is labelled as one: it lives in this
# operator-writable state dir, so it is not a trust input, and nothing about it
# can suppress or downgrade a page.
#
# NOT GATED ON --scheduled, unlike the Discord entry. That gate exists because
# the entry is a LIVENESS signal, and an ad-hoc run posting one would make a dead
# LaunchAgent look alive. This record answers a different question, and an
# operator running `just brew-upgrade` on a Wednesday moves exactly as many
# package files as Monday noon does.
#
# BREW ONLY. App Store apps install into /Applications, which no known-good
# manifest covers and no file-integrity watch reads, so a mas transition could
# never explain one of these pages and listing it would only pad the line.
#
# ONE CLOCK READING for both timestamp fields AND for both phases (the house
# idiom from unattended_log_entry_header), taken once near the top of this script
# into UPGRADE_RECORD_EPOCH / UPGRADE_RECORD_ISO: the epoch is what the reader
# does arithmetic on and the ISO string is what it renders, and separate `date`
# calls could disagree, which here would also make one run look like two.
#
# Written to a temp file and moved into place, so a reader mid-write sees the
# previous record whole rather than a torn one. Best effort throughout: upgrading
# matters more than bookkeeping, and every failure is stated rather than silent.
write_upgrade_record() {
  local phase="$1"
  case "$phase" in
    started | completed) ;;
    *)
      printf 'homebrew-weekly-upgrade: WARNING unknown upgrade-record phase: %s; no record was written\n' "$phase" >&2
      return 0
      ;;
  esac
  [[ -n $brew_snapshot_ok ]] || {
    printf 'homebrew-weekly-upgrade: the package listing could not be read, so this run adds no package rows to the upgrade record; the correlation line will name nothing\n' >&2
    return 0
  }
  if [[ ! $UPGRADE_RECORD_EPOCH =~ ^[0-9]+$ || -z $UPGRADE_RECORD_ISO ]]; then
    printf 'homebrew-weekly-upgrade: WARNING this clock could not be read, so no upgrade record was written for this run\n' >&2
    return 0
  fi
  mkdir -p "$STATE_DIR" 2>/dev/null || true
  if ! {
    printf '%s\t%s\n' "$UPGRADE_RECORD_EPOCH" "$UPGRADE_RECORD_ISO"
    # An `if`, not a `&&` list: the group's status is what the caller tests, and
    # a false test as the last statement would report a written record as failed.
    if [[ $phase == completed ]]; then
      unattended_log_change_tuples "$snapshot_dir/brew.before" "$snapshot_dir/brew.after"
    fi
  } >"$UPGRADE_RECORD.tmp" 2>/dev/null; then
    printf 'homebrew-weekly-upgrade: WARNING could not write the upgrade record at %s; the file-integrity page will report no recorded upgrade\n' \
      "$UPGRADE_RECORD" >&2
    rm -f "$UPGRADE_RECORD.tmp" 2>/dev/null || true
    return 0
  fi
  mv -f "$UPGRADE_RECORD.tmp" "$UPGRADE_RECORD" 2>/dev/null ||
    printf 'homebrew-weekly-upgrade: WARNING could not install the upgrade record at %s; the file-integrity page will report no recorded upgrade\n' \
      "$UPGRADE_RECORD" >&2
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
# kernel-lock shape guards ~/.local/libexec/unattended-upgrades/agent-skills/update-skills.sh.)
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

# Put OUR files back into /var/osquery if the osquery cask upgrade wiped them,
# and restart the daemon if it did. This is the whole reason the converge tool
# has a second caller: an upgrade here runs with nobody present, so without this
# the machine could sit for a week running a root daemon with no detection
# config and nothing would say so.
#
# The tool is idempotent and silent when nothing drifted, so a week that did not
# touch osquery adds one exit-0 line to the log and nothing else. A failure is
# counted by run() like any other step, which alerts on the priority route with
# the step named.
#
# A tool that is not deployed is stated and NOT counted as a failed step: this
# job's business is upgrading, it would alert every week for a condition only an
# apply can fix, and a silently absent converge is what the warning prevents.
#
# shellcheck disable=SC2329,SC2317 # invoked indirectly, as an argument to run()
converge_osquery() {
  if [[ ! -x $OSQUERY_CONVERGE ]]; then
    printf 'homebrew-weekly-upgrade: WARNING %s is not executable, so /var/osquery was NOT converged after this upgrade; run chezmoi apply\n' \
      "$OSQUERY_CONVERGE" >&2
    return 0
  fi
  "$OSQUERY_CONVERGE"
}

# TWO different facts, and they used to collapse into one. `lockf` answers 75
# (EX_TEMPFAIL) when another process holds the lock; anything else means this run
# could not even OPEN the lock file, e.g. its directory is not writable. Reporting
# the second as the first posts a record blaming a holder that does not exist,
# tells nobody, and exits 75 ("try again later") for a condition that will still
# be there next week.
weekly_lock_rc=0
acquire_lock || weekly_lock_rc=$?
if [[ $weekly_lock_rc -eq 75 ]]; then
  printf 'homebrew-weekly-upgrade: another run holds the lock; deferring (exit 75).\n' >&2
  # A deferral is NOT a failure: nothing was attempted, so it is recorded rather
  # than alerted. Without an entry, a week spent entirely in contention would
  # leave the channel empty, which reads as a dead LaunchAgent.
  weekly_record deferred "nothing was attempted: another homebrew-weekly-upgrade run already holds the serialize lock, so this run deferred (exit 75)."
  exit 75
elif [[ $weekly_lock_rc -ne 0 ]]; then
  printf 'homebrew-weekly-upgrade: the serialize lock at %s could not be OPENED (rc %d); nothing ran.\n' \
    "$LOCKFILE" "$weekly_lock_rc" >&2
  weekly_alert lock-unavailable \
    "$(printf 'The weekly Homebrew upgrade could not OPEN its serialize lock at %s (rc %d), so nothing ran. This is not another run holding it: check that the directory is writable.' \
      "$LOCKFILE" "$weekly_lock_rc")"
  weekly_record deferred \
    "$(printf 'nothing was attempted: the serialize lock at %s could not be OPENED (rc %d), which is not another run holding it. An alert was also attempted on the priority route; that path is fire-and-forget, so its delivery was not observed.' \
      "$LOCKFILE" "$weekly_lock_rc")"
  exit 1
fi

printf '=== homebrew-weekly-upgrade %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

snapshot_dir=""
# Per-lane readability. A lane is "not compared" when EITHER of its two readings
# failed, because half a comparison is not one.
brew_snapshot_ok=1
mas_snapshot_ok=1
# What could not be read, per lane. A NOT COMPARED line has to name the thing
# that actually broke: when the workspace itself could not be allocated, both
# package commands were fine and pointing the operator at them wastes the one
# actionable sentence in the entry.
brew_snapshot_source='brew list --versions'
mas_snapshot_source='mas list'
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  # GUARDED, because this script deliberately does not run under errexit. A
  # failed mktemp (an absent, unwritable or full TMPDIR) left snapshot_dir empty,
  # every upgrade step ran anyway, and the record block below, guarded on that
  # same variable, was skipped whole: the upgrade happened, the success marker
  # was written, the run exited 0, and NEITHER channel said a word about the
  # week. A workspace that cannot be allocated costs the comparison, never the
  # entry.
  #
  # The template is spelled out rather than left to a bare `mktemp -d` for the
  # reason update-skills.sh spells out its fork-clone template: macOS mktemp
  # ignores TMPDIR entirely in the bare form (measured on macOS 26.2 / Darwin
  # 25.2), so the location is neither redirectable nor testable, and anything
  # this ever fails to clean up should carry the name of the job that made it.
  if snapshot_dir="$(mktemp -d "${TMPDIR:-/tmp}/homebrew-weekly-record.XXXXXX" 2>/dev/null)" &&
    [[ -n $snapshot_dir ]]; then
    trap 'rm -rf "$snapshot_dir"' EXIT
    __homebrew_package_snapshot brew >"$snapshot_dir/brew.before" || brew_snapshot_ok=""
    __homebrew_package_snapshot mas >"$snapshot_dir/mas.before" || mas_snapshot_ok=""
    # Published BEFORE the first brew step, so the record covers the window it
    # describes rather than appearing only once the window has closed.
    write_upgrade_record started
  else
    snapshot_dir=""
    brew_snapshot_ok=""
    mas_snapshot_ok=""
    brew_snapshot_source='creating the snapshot workspace (mktemp -d)'
    mas_snapshot_source="$brew_snapshot_source"
    printf 'homebrew-weekly-upgrade: WARNING the snapshot workspace could not be created (mktemp -d failed); the upgrade still runs, and this entry will say that nothing could be compared\n' >&2
  fi
fi

run "brew update" "$BREW" update
run "brew outdated" "$BREW" outdated
run "mas outdated" "$MAS" outdated
run "brew upgrade" "$BREW" upgrade
run "tailscaled refresh (if upgraded)" refresh_tailscaled
# Immediately after the brew group, not at the end of the run: between a cask
# upgrade wiping /var/osquery and this repairing it, the root daemon is running
# without our detection config, and `mas upgrade` plus `brew cleanup` can take
# minutes. The window is the monitoring gap, so it is kept as short as the
# ordering allows.
run "osquery config converge (after upgrade)" converge_osquery
run "mas upgrade" "$MAS" upgrade
run "brew cleanup" "$BREW" cleanup

printf '=== done %s ===\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# The weekly RECORD, posted whether or not anything moved. A run that upgraded
# nothing is precisely where the gap figure is the only information the entry
# carries, so suppressing the empty entry would throw away the reason the channel
# exists. The failed-step count rides along rather than being implied by silence.
#
# The entry is gated on the LIBRARY being available, never on the snapshots: a
# run with no workspace to compare in still has a class, a host, a timestamp, a
# gap and a failed-step count to report, and those are what a reader checks
# first.
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  if [[ -n $snapshot_dir ]]; then
    __homebrew_package_snapshot brew >"$snapshot_dir/brew.after" || brew_snapshot_ok=""
    __homebrew_package_snapshot mas >"$snapshot_dir/mas.after" || mas_snapshot_ok=""
    write_upgrade_record completed
  fi
  weekly_record completed "$(printf '%s\n%s\nfailed steps: %d' \
    "$(unattended_log_change_section "$brew_snapshot_ok" \
      "$snapshot_dir/brew.before" "$snapshot_dir/brew.after" \
      'formulae and casks' \
      'Versions are what brew list --versions reports; a formula reinstalled at the same version does not appear here, and a cask Homebrew tracks only as latest reports that literal string rather than a version.' \
      versions "$brew_snapshot_source")" \
    "$(unattended_log_change_section "$mas_snapshot_ok" \
      "$snapshot_dir/mas.before" "$snapshot_dir/mas.after" \
      'App Store apps' \
      'Versions are what mas list reports, keyed by app name.' \
      versions "$mas_snapshot_source")" \
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
