# shellcheck shell=bash
# unattended-log-lib.sh, the shared entry shape for the weekly UNATTENDED jobs
# (update-skills.sh and homebrew-weekly-upgrade.sh). Sourced, never executed, so
# it carries no shebang and no executable bit, exactly like macos-defaults-lib.sh
# beside it. Each caller sources it as
#   source "$(dirname "${BASH_SOURCE[0]}")/unattended-log-lib.sh"
# which resolves in BOTH the chezmoi source tree (dot_local/bin/) and the applied
# ~/.local/bin/ layout, because this file carries no executable_ or dot_ prefix.
#
# WHAT THIS IS FOR. The weekly jobs upgrade things unattended. When a machine
# later misbehaves there is nothing to investigate against, because a clean week
# and a dead LaunchAgent produce identical silence. These entries are that
# record: a separate Discord channel that receives ONE message per week per job
# saying when the job ran, when it last succeeded, and what it changed. Failures
# keep going to the existing alert route so they land in the priority channel.
# Act on one, record the other.
#
# WHY EVERY ENTRY STATES ITS OWN GAP, instead of the channel being a heartbeat
# you count. `man launchd.plist`, under StartCalendarInterval, verbatim:
#
#   "Unlike cron which skips job invocations when the computer is asleep, launchd
#    will start the job the next time the computer wakes up. If multiple
#    intervals transpire before the computer is woken, those events will be
#    coalesced into one event upon wake from sleep."
#
# So a live, healthy job can legitimately produce ONE entry covering three weeks,
# and an absent entry cannot distinguish a dead LaunchAgent from a laptop that
# was closed for two Mondays. Counting messages measures nothing. Worse, absence
# is the most passive signal there is, and the operator's standing ruling on the
# plugin-updater task rejected drift-watch for exactly that reason: passive
# alerts go unnoticed. So the newest entry carries its own gap
# ("last successful run: 2026-07-10T12:00:00Z (23d 0h ago)"), which reads the
# same under coalescing, sleep, shutdown, and a wedged deferral loop.
#
# WHY THE TIMESTAMP IS STORED AS EPOCH PLUS ISO. The marker file holds both
# fields on one line: "<epoch-seconds> <iso-8601-utc>". The epoch is what the gap
# arithmetic uses, so nothing ever has to PARSE a timestamp back. That matters
# here: BSD date (the macOS host and the CI runner) needs
# `date -j -f <format>` while GNU date needs `date -d`, and the flake devshell
# and the host disagree about which one `date` is. Storing the number removes the
# question. The ISO field is for the human reading the entry.
#
# NOTHING HERE IS EVER SILENT. A missing marker, an unparseable marker, an
# unwritable guard, an absent relay.sh: each produces a stated line. A quiet
# no-op in this file would read downstream as a delivered entry, which is the
# precise failure mode the whole record exists to end.

# The webhook route name, which is also the URL path segment hermes serves it on
# (.platforms.webhook.extra.routes.<name> in the encrypted hermes config). The
# apply-time status check reads this same name out of this file rather than
# repeating it, because a rename in one place alone is a 404 on every entry
# forever and nothing else would notice.
UNATTENDED_LOG_ROUTE="unattended-upgrades"

# The gateway URL this library posts to. UNATTENDED_LOG_HERMES_URL overrides it
# (tests). The default host:port must match .platforms.webhook.extra.{host,port}
# in the hermes config; run_after_68 probes the live config's real port, which is
# the check that actually catches a drift here.
unattended_log_url() {
  printf '%s' "${UNATTENDED_LOG_HERMES_URL:-http://127.0.0.1:8644/webhooks/$UNATTENDED_LOG_ROUTE}"
}

# The machine this entry is about. The channel aggregates unattended jobs, and
# the daemon-host role is expected to move to a second Mac, so an entry that does
# not name its host is not investigable.
unattended_log_host() {
  local host
  host="$(hostname -s 2>/dev/null || true)"
  [[ -n $host ]] || host="${HOSTNAME:-unknown-host}"
  printf '%s' "$host"
}

# ISO 8601 UTC, the one timestamp format this repo uses. BSD date has no -Is, so
# the format is spelled out rather than relying on a GNU shorthand.
unattended_log_now_iso() { date -u +%Y-%m-%dT%H:%M:%SZ; }

# unattended_log_elapsed <seconds> -- a gap a human reads at a glance. Units
# shift with magnitude; the boundaries are pinned by test because an off-by-one
# at 86400 turns "1d 0h" into "24h 0m".
#
# A NEGATIVE gap means the recorded timestamp is in the future, i.e. the clock
# moved backwards (a restored backup, an NTP correction, a timezone-naive edit).
# Rendering that as a small positive number would be a confident lie, so it is
# named instead.
unattended_log_elapsed() {
  local seconds="${1:-}"
  [[ $seconds =~ ^-?[0-9]+$ ]] || {
    printf 'unknown'
    return 0
  }
  if [[ $seconds -lt 0 ]]; then
    printf 'unknown (the recorded timestamp is in the FUTURE; this clock moved backwards)'
    return 0
  fi
  if [[ $seconds -lt 60 ]]; then
    printf '%ds' "$seconds"
  elif [[ $seconds -lt 3600 ]]; then
    printf '%dm' "$((seconds / 60))"
  elif [[ $seconds -lt 86400 ]]; then
    printf '%dh %dm' "$((seconds / 3600))" "$(((seconds % 3600) / 60))"
  else
    printf '%dd %dh' "$((seconds / 86400))" "$(((seconds % 86400) / 3600))"
  fi
}

# unattended_log_mark_success <marker-file> -- record THIS moment as the job's
# last successful run. Best-effort: a job must not fail because it could not
# write its own bookkeeping, but a failure to write is stated, because the next
# entry would otherwise report a gap measured from a run that did not happen.
unattended_log_mark_success() {
  local marker="$1" dir
  dir="$(dirname "$marker")"
  mkdir -p "$dir" 2>/dev/null || true
  if ! printf '%s %s\n' "$(date +%s)" "$(unattended_log_now_iso)" >"$marker" 2>/dev/null; then
    printf 'unattended-log: WARNING could not record the successful-run timestamp at %s; the next entry will report a stale or absent gap\n' "$marker" >&2
  fi
}

# unattended_log_gap_line <marker-file> -- the one line that makes an entry
# legible on its own. Three states, three distinct sentences, no state that
# renders as a plausible small gap.
unattended_log_gap_line() {
  local marker="$1" recorded_epoch="" recorded_iso="" now delta
  if [[ ! -r $marker ]]; then
    printf 'last successful run: NEVER RECORDED on this machine'
    return 0
  fi
  read -r recorded_epoch recorded_iso <"$marker" 2>/dev/null || true
  if [[ ! $recorded_epoch =~ ^[0-9]+$ ]]; then
    printf 'last successful run: UNKNOWN (the record at %s is unreadable)' "$marker"
    return 0
  fi
  now="$(date +%s)"
  if [[ ! $now =~ ^[0-9]+$ ]]; then
    printf 'last successful run: %s (elapsed UNKNOWN; this clock could not be read)' "${recorded_iso:-$recorded_epoch}"
    return 0
  fi
  delta=$((now - recorded_epoch))
  printf 'last successful run: %s (%s ago)' "${recorded_iso:-$recorded_epoch}" "$(unattended_log_elapsed "$delta")"
}

# unattended_log_claim_week <guard-file> <class> -- returns 0 when THIS entry
# should be emitted, 1 when the week already has it. `class` is `completed` or
# `deferred`.
#
# WHY A GUARD AT ALL: entries are emitted on the deferral and refusal exits as
# well as on the tail, and update-skills fires 24 hourly Monday slots, so an
# ordinary week would post up to 24 entries without this.
#
# WHY COMPLETED MAY FOLLOW DEFERRED, and only in that direction: a week whose
# first slot deferred and whose twelfth slot succeeded must not leave
# "deferred, nothing attempted" as its newest message. A reader taking the newest
# message at face value -- which is the entire design -- would conclude the job
# is stuck in a week it actually finished. So the guard records the CLASS as well
# as the week and allows exactly one upgrade, capping a week at two entries while
# guaranteeing the newest one is the truer outcome. The reverse (a late deferral
# burying an earlier completion) is refused.
#
# FAIL OPEN on an unwritable or unreadable guard: emitting up to 24 entries once
# is noisy and visible, while suppressing them is invisible, and invisible is the
# failure this record exists to prevent. The condition is stated either way.
unattended_log_claim_week() {
  local guard="$1" class="$2" week recorded_week="" recorded_class="" dir
  week="$(date +%G-%V 2>/dev/null || true)"
  if [[ -z $week ]]; then
    printf 'unattended-log: WARNING could not read the ISO week; emitting this entry ungated\n' >&2
    return 0
  fi
  if [[ -r $guard ]]; then
    read -r recorded_week recorded_class <"$guard" 2>/dev/null || true
  fi
  if [[ $recorded_week == "$week" ]]; then
    if [[ $recorded_class != "deferred" || $class != "completed" ]]; then
      return 1
    fi
  fi
  dir="$(dirname "$guard")"
  mkdir -p "$dir" 2>/dev/null || true
  if ! printf '%s %s\n' "$week" "$class" >"$guard" 2>/dev/null; then
    printf 'unattended-log: WARNING could not write the weekly guard at %s; this week may post one entry per scheduled slot\n' "$guard" >&2
  fi
  return 0
}

# How many changed names an entry lists before it summarizes the rest. Discord
# caps a message at 2000 characters, and a whole-store or whole-Cellar move would
# otherwise blow past it, taking the gap figure with it.
UNATTENDED_LOG_NAME_CAP=12

# unattended_log_change_line <before-file> <after-file> <label> <caveat> <style>
#
# One sentence describing what moved between two snapshots. Both files hold
# "<name><TAB><fingerprint>" lines. It lives here rather than in either producer
# because the two weekly jobs must report in the SAME shape; two copies of this
# would drift and the channel would read as two different logs.
#
# `style` decides whether the fingerprint is worth printing:
#   versions  the fingerprint is a human-meaningful version, so a change renders
#             as "name old -> new" (Homebrew formulae, clawhub-installed skills).
#   opaque    the fingerprint is a content hash that tells a reader nothing, so a
#             change renders as the bare name (the npx skills lane).
#
# `caveat` is what this subject CANNOT tell you, restated on every entry rather
# than assumed known. A record implying a completeness it does not have is worse
# than no record.
#
# Added and removed names count as changes and are marked as such. A removal is
# the single most worth-seeing line here: something left without being asked to.
unattended_log_change_line() {
  local before="$1" after="$2" label="$3" caveat="$4" style="$5"
  local total=0 name fingerprint_after fingerprint_before shown
  local -a changed=()
  while IFS=$'\t' read -r name fingerprint_after; do
    [[ -n $name ]] || continue
    total=$((total + 1))
    fingerprint_before="$(awk -F'\t' -v want="$name" '$1 == want { print $2; exit }' "$before" 2>/dev/null || true)"
    if [[ -z $fingerprint_before ]]; then
      changed+=("$name (added)")
    elif [[ $fingerprint_before != "$fingerprint_after" ]]; then
      if [[ $style == "versions" ]]; then
        changed+=("$name $fingerprint_before -> $fingerprint_after")
      else
        changed+=("$name")
      fi
    fi
  done <"$after"
  while IFS=$'\t' read -r name fingerprint_before; do
    [[ -n $name ]] || continue
    awk -F'\t' -v want="$name" '$1 == want { found = 1 } END { exit !found }' "$after" 2>/dev/null ||
      changed+=("$name (removed)")
  done <"$before"

  if [[ ${#changed[@]} -eq 0 ]]; then
    printf '%s: 0 of %d tracked entries changed. %s' "$label" "$total" "$caveat"
    return 0
  fi
  shown="$(printf '%s, ' "${changed[@]:0:UNATTENDED_LOG_NAME_CAP}")"
  shown="${shown%, }"
  if [[ ${#changed[@]} -gt $UNATTENDED_LOG_NAME_CAP ]]; then
    shown="$shown, and $((${#changed[@]} - UNATTENDED_LOG_NAME_CAP)) more"
  fi
  printf '%s: %d of %d tracked entries changed (%s). %s' \
    "$label" "${#changed[@]}" "$total" "$shown" "$caveat"
}

# unattended_log_post <agent> <state> <project> <detail> -- deliver one entry.
#
# --remote-only is not optional here. An unattended Monday-slot run is idle past
# relay.sh's desk threshold by definition, so the default fan-out would buzz the
# phone and pop a macOS banner for every weekly heartbeat, including the ones
# that say nothing changed. That is the noise a separate channel was chosen to
# avoid. --local-only is the wrong lever: it suppresses the hermes leg, which IS
# the log. --remote-only also makes relay POST synchronously and print the
# delivery outcome, so a 401 or a 404 lands in this job's run log instead of
# being discarded into /dev/null while the channel stays empty.
#
# 9>&- closes the caller's serialize-lock fd for relay and everything it spawns.
# relay detaches channels that outlive this whole run, and a kernel flock is held
# until the LAST copy of the fd closes, so a detached child that inherited it
# keeps the lock held after the job exited and the next scheduled slot defers
# over a competing run that does not exist. Closing an fd that was never opened
# is a no-op.
#
# NEVER fails the caller: a record that cannot be delivered must not break the
# job it is reporting on.
unattended_log_post() {
  local agent="$1" state="$2" project="$3" detail="$4"
  local relay_script="${UNATTENDED_LOG_RELAY:-$HOME/.local/bin/relay.sh}"
  if [[ ! -x $relay_script ]]; then
    printf 'unattended-log: relay.sh is not executable at %s; this entry was NOT delivered (run chezmoi apply)\n' "$relay_script"
    return 0
  fi
  RELAY_HERMES_URL="$(unattended_log_url)" "$relay_script" --remote-only \
    --agent "$agent" --state "$state" --project "$project" --detail "$detail" 9>&- || true
  return 0
}
