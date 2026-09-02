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
# saying when the job ran, when it last succeeded, and what it changed -- or TWO
# in the one week that starts by deferring and later completes, because a week
# the job actually finished must not be left with "deferred, nothing attempted"
# as its newest message (see unattended_log_claim_week). Failures keep going to
# the existing alert route so they land in the priority channel. Act on one,
# record the other.
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
# unwritable guard, an absent pns.sh: each produces a stated line. A quiet
# no-op in this file would read downstream as a delivered entry, which is the
# precise failure mode the whole record exists to end.

# The webhook route name, which is also the URL path segment hermes serves it on
# (.platforms.webhook.extra.routes.<name> in the encrypted hermes config). The
# apply-time status check reads this same name out of this file rather than
# repeating it, because a rename in one place alone is a 404 on every entry
# forever and nothing else would notice.
UNATTENDED_LOG_ROUTE="unattended-upgrades"

# The gateway URL is pns's business for THIS caller: the post below names the
# ROUTE with --channel and pns derives the endpoint from its own default
# gateway (DEFAULT_HERMES_URL in dot_local/share/pns/src/channels/hermes.rs).
# That is NOT the only copy of the host:port, though: uu's own config template
# hardcodes the identical gateway URL separately, in its [records] block at
# dot_config/uu/private_config.toml.tmpl, because uu posts its own weekly
# record straight to hermes rather than through this library. Deriving one
# from the other is a design question, not settled here. run_after_68 probes
# the live hermes config's real port, which is the check that catches a drift
# between either hardcoded copy and the real gateway.

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

# The ISO year-week the guard is keyed on. Empty when the clock cannot be read,
# which every caller treats as "do not gate", never as "already claimed".
unattended_log_week() { date +%G-%V 2>/dev/null || true; }

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
#
# BASE 10 IS FORCED on both epochs. Bash arithmetic reads a leading zero as
# OCTAL, so a marker holding `0837000000` (a truncated or half-written one, and
# two of the ten digits do it) raises "value too great for base" -- from a line
# that runs at START-UP in both weekly jobs, one of them under set -e, which
# would end the run before its lock, its alert or its record.
unattended_log_gap_line() {
  local marker="$1" now="${2:-}" recorded_epoch="" recorded_iso="" delta
  if [[ ! -r $marker ]]; then
    printf 'last successful run: NEVER RECORDED on this machine'
    return 0
  fi
  read -r recorded_epoch recorded_iso <"$marker" 2>/dev/null || true
  if [[ ! $recorded_epoch =~ ^[0-9]+$ ]]; then
    printf 'last successful run: UNKNOWN (the record at %s is unreadable)' "$marker"
    return 0
  fi
  [[ -n $now ]] || now="$(date +%s)"
  if [[ ! $now =~ ^[0-9]+$ ]]; then
    printf 'last successful run: %s (elapsed UNKNOWN; this clock could not be read)' "${recorded_iso:-$recorded_epoch}"
    return 0
  fi
  delta=$((10#$now - 10#$recorded_epoch))
  printf 'last successful run: %s (%s ago)' "${recorded_iso:-$recorded_epoch}" "$(unattended_log_elapsed "$delta")"
}

# unattended_log_entry_header <marker-file> -- the two lines every entry opens
# with: when THIS run started, and the gap to the previous successful one. Both
# come from ONE clock reading, which is the whole reason this is a function
# rather than two lines in each caller: they used to be sampled at different
# instants (the gap at start-up, the timestamp at delivery), so a two-hour run
# printed timestamps seven days and two hours apart above a gap reading seven
# days, and a reader cannot tell which of the two figures to believe.
#
# ONE `date` call yields both fields. Deriving the timestamp from the epoch
# afterwards would need `date -r` on BSD and `date -d @` on GNU, which is the
# portability trap the marker format itself already sidesteps (see the note at
# the top of this file).
#
# It is captured at START-UP, before the run can rewrite its own marker; a header
# taken at delivery would report the gap as zero on every successful run.
unattended_log_entry_header() {
  local marker="$1" now_epoch="" now_iso=""
  read -r now_epoch now_iso < <(date -u '+%s %Y-%m-%dT%H:%M:%SZ' 2>/dev/null) || true
  if [[ ! $now_epoch =~ ^[0-9]+$ || -z $now_iso ]]; then
    # Named, not hidden: an entry with no timestamp at all still beats one
    # carrying a confident wrong time.
    printf 'run at UNKNOWN (this clock could not be read)\n%s' "$(unattended_log_gap_line "$marker")"
    return 0
  fi
  printf 'run at %s\n%s' "$now_iso" "$(unattended_log_gap_line "$marker" "$now_epoch")"
}

# unattended_log_claim_week <guard-dir> <class> -- returns 0 when THIS entry
# should be emitted, 1 when the week already has it. `class` is `completed`,
# `deferred`, or `delivery-alert`.
#
# WHY A GUARD AT ALL: entries are emitted on the deferral and refusal exits as
# well as on the tail, and update-skills fires 24 hourly Monday slots, so an
# ordinary week would post up to 24 entries without this.
#
# WHY THE CLAIM IS A FILE CREATION, not a read-then-write of one guard file: the
# read-then-write shape is a race every scheduled slot can enter. Two slots that
# both read an unclaimed week both write it and both post; measured, 200 of 200
# concurrent pairs claimed the same fresh week. And these slots genuinely do
# overlap: a contending run posts its "another run holds the lock" entry while
# the holder is still working. So the claim is `set -o noclobber` on a token file
# named for the week and the class, which is O_EXCL: the kernel grants it to
# exactly one caller, and the ones that lose can tell "already claimed" from
# "could not write" by whether the token exists.
#
# WHY THE CLASS IS IN THE FILE NAME, not in the file's contents: contents have to
# be parsed, and a guard holding an unrecognised class used to wedge the whole
# week (anything that was not literally `deferred` was treated as completed, so
# both claim types were refused, with no warning and no rewrite -- a week that
# posted nothing while the guard asserted it had). A name this function does not
# recognise is simply not one of its tokens.
#
# WHY COMPLETED MAY FOLLOW DEFERRED, and only in that direction: a week whose
# first slot deferred and whose twelfth slot succeeded must not leave
# "deferred, nothing attempted" as its newest message. A reader taking the newest
# message at face value -- which is the entire design -- would conclude the job
# is stuck in a week it actually finished. So a week is capped at two entries,
# one per class, with the newest being the truer outcome; the reverse (a late
# deferral burying an earlier completion) is refused. `delivery-alert` is not an
# entry on this channel at all: it is the once-a-week alert that the channel
# itself is broken, and it takes a token here for the same reason.
#
# FAIL OPEN on an unwritable or unusable guard: emitting up to 24 entries once is
# noisy and visible, while suppressing them is invisible, and invisible is the
# failure this record exists to prevent. The condition is stated either way.
unattended_log_claim_week() {
  local guard="$1" class="$2" week token stale
  week="$(unattended_log_week)"
  if [[ -z $week ]]; then
    printf 'unattended-log: WARNING could not read the ISO week; emitting this entry ungated\n' >&2
    return 0
  fi
  case "$class" in
    deferred | completed | delivery-alert) ;;
    *)
      printf 'unattended-log: WARNING unrecognised entry class %s; emitting this entry ungated rather than trusting a guard that cannot describe it\n' "$class" >&2
      return 0
      ;;
  esac
  # A deferral must never bury a completed entry for the same week.
  [[ $class == "deferred" && -e "$guard/$week.completed" ]] && return 1
  mkdir -p "$guard" 2>/dev/null || true
  token="$guard/$week.$class"
  if ! (set -o noclobber && : >"$token") 2>/dev/null; then
    [[ -e $token ]] && return 1
    printf 'unattended-log: WARNING could not write the weekly guard at %s; this week may post one entry per scheduled slot\n' "$token" >&2
    return 0
  fi
  # Keep the guard readable as "what did THIS week do": drop other weeks' tokens.
  for stale in "$guard"/*; do
    [[ -e $stale ]] || continue
    [[ ${stale##*/} == "$week".* ]] && continue
    rm -f "$stale" 2>/dev/null || true
  done
  return 0
}

# unattended_log_release_week <guard-dir> <class> -- give the claim back, so a
# later slot retries. Called when the entry the claim was taken for was NOT
# delivered: a guard that outlives a failed delivery silences the remaining 23
# slots and leaves the week with no entry while asserting it has one.
unattended_log_release_week() {
  local guard="$1" class="$2" week
  week="$(unattended_log_week)"
  [[ -n $week ]] || return 0
  rm -f "$guard/$week.$class" 2>/dev/null || true
  return 0
}

# How many changed names an entry lists before it summarizes the rest. Discord
# caps a message at 2000 characters, and a whole-store or whole-Cellar move would
# otherwise blow past it, taking the gap figure with it.
UNATTENDED_LOG_NAME_CAP=12

# __unattended_log_lookup <snapshot-file> <name> -- print that name's
# fingerprint; exit non-zero when the name is not in the file at all (which is
# not the same as an empty fingerprint, and the caller depends on the
# difference: absent means added or removed).
#
# The name reaches awk through the ENVIRONMENT, never through -v. awk processes
# escape sequences in a -v VALUE, so a name holding a literal backslash-n (which
# is exactly what `jq @tsv` emits for a newline, and a doubled backslash for a
# backslash) never matched itself: the lookup missed, and the entry was reported
# as removed AND re-added every single week. ENVIRON does no such processing.
# A skill name is not a secret, so passing it in the environment is fine here.
__unattended_log_lookup() {
  UNATTENDED_LOG_WANT="$2" awk -F'\t' '
    $1 == ENVIRON["UNATTENDED_LOG_WANT"] { print $2; found = 1; exit }
    END { exit !found }' "$1" 2>/dev/null
}

# __unattended_log_code <text> -- render third-party text as a Discord inline
# code span.
#
# Every name and version in an entry is chosen by whoever published the package,
# and it lands in a channel whose entire value is that its contents read as
# trustworthy machine records. Unquoted, a version string of
# `[urgent: click here](https://evil.example)` renders as a CLICKABLE LINK the
# operator never authored. A code span renders it literally, so the two
# characters that could close the span early -- a backtick, and any control
# character -- are removed first.
__unattended_log_code() {
  local text="${1//\`/}"
  text="$(printf '%s' "$text" | tr -d '[:cntrl:]')"
  # shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
  printf '`%s`' "$text"
}

# unattended_log_change_tuples <before-file> <after-file>
#
# The raw diff between two snapshots, one line per name that MOVED, tab
# separated:
#
#   <name>	<added|removed|changed>	<before-fingerprint>	<after-fingerprint>
#
# The absent side of an add or a remove is the empty string. A pair with nothing
# between them prints nothing at all.
#
# WHY THIS IS SEPARATE FROM THE SENTENCE ABOVE IT. The Discord entry is one
# consumer; the other is the osquery file-integrity page, which asks whether a
# recorded upgrade plausibly explains a file whose content hash left its
# known-good manifest. That question needs the transition as DATA, and it is
# asked days after the run that answered it, so the weekly Homebrew job persists
# these lines to disk. Both readings come from this one walk, so the channel and
# the page can never disagree about what a week did.
#
# A TAB inside a fingerprint is DELETED before it is emitted. The tuple is tab
# separated and is read back field by field, so a fingerprint carrying one would
# shift the state into a version column. Deleted rather than replaced, because
# the code-span quoting already deletes every control character, so the rendered
# summary is unchanged by this.
unattended_log_change_tuples() {
  local before="$1" after="$2" name fingerprint_after fingerprint_before
  while IFS=$'\t' read -r name fingerprint_after; do
    [[ -n $name ]] || continue
    if ! fingerprint_before="$(__unattended_log_lookup "$before" "$name")"; then
      printf '%s\t%s\t%s\t%s\n' "$name" added "" "${fingerprint_after//$'\t'/}"
    elif [[ $fingerprint_before != "$fingerprint_after" ]]; then
      printf '%s\t%s\t%s\t%s\n' "$name" changed \
        "${fingerprint_before//$'\t'/}" "${fingerprint_after//$'\t'/}"
    fi
  done <"$after"
  while IFS=$'\t' read -r name fingerprint_before; do
    [[ -n $name ]] || continue
    if ! __unattended_log_lookup "$after" "$name" >/dev/null; then
      printf '%s\t%s\t%s\t%s\n' "$name" removed "${fingerprint_before//$'\t'/}" ""
    fi
  done <"$before"
}

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
# Added and removed names count as changes and are marked as such, and BOTH count
# toward the total: counting only the after-rows renders the impossible
# "2 of 0 tracked entries changed" on an emptied snapshot. A removal is the
# single most worth-seeing line here: something left without being asked to.
#
# EVERY interpolated name and fingerprint goes through __unattended_log_code,
# because all of them are chosen by whoever published the package, and the
# channel's whole value is that its contents read as trustworthy machine records.
# See that function for what it prevents.
unattended_log_change_line() {
  local before="$1" after="$2" label="$3" caveat="$4" style="$5"
  local total=0 name state fingerprint_before fingerprint_after shown
  local -a changed=()
  # The TOTAL is the tracked population, which the tuples deliberately do not
  # describe: they list only what MOVED. It is the after-rows plus the removals,
  # because counting the after-rows alone renders the impossible "2 of 0 tracked
  # entries changed" on an emptied snapshot.
  while IFS=$'\t' read -r name _; do
    [[ -n $name ]] || continue
    total=$((total + 1))
  done <"$after"
  while IFS=$'\t' read -r name state fingerprint_before fingerprint_after; do
    [[ -n $name ]] || continue
    case "$state" in
      added) changed+=("$(__unattended_log_code "$name") (added)") ;;
      removed)
        changed+=("$(__unattended_log_code "$name") (removed)")
        total=$((total + 1))
        ;;
      *)
        if [[ $style == "versions" ]]; then
          changed+=("$(__unattended_log_code "$name") $(__unattended_log_code "$fingerprint_before") -> $(__unattended_log_code "$fingerprint_after")")
        else
          changed+=("$(__unattended_log_code "$name")")
        fi
        ;;
    esac
  done < <(unattended_log_change_tuples "$before" "$after")

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

# unattended_log_change_section <ok-flag> <before> <after> <label> <caveat>
#   <style> <source-description>
#
# One section of an entry, with the ONE thing a change list must never do built
# in: when the reading it depends on failed, it says so instead of comparing two
# files nothing could fill. Two empty snapshots render as "0 of N tracked entries
# changed", which is indistinguishable from a quiet week on a subject this run
# could not inspect at all -- the same shape as every other defect this record
# exists to end. Shared by both weekly jobs so the two read alike.
unattended_log_change_section() {
  local ok="$1" before="$2" after="$3" label="$4" caveat="$5" style="$6" source_description="$7"
  if [[ -z $ok ]]; then
    printf '%s: NOT COMPARED -- %s failed on this run, so nothing here says what changed; it says this run could not read what is installed.' \
      "$label" "$source_description"
    return 0
  fi
  unattended_log_change_line "$before" "$after" "$label" "$caveat" "$style"
}

# unattended_log_post <agent> <state> <project> <detail> -- deliver one entry.
#
# --remote-only is not optional here. An unattended Monday-slot run is idle past
# pns.sh's desk threshold by definition, so the default fan-out would buzz the
# phone and pop a macOS banner for every weekly heartbeat, including the ones
# that say nothing changed. That is the noise a separate channel was chosen to
# avoid. --local-only is the wrong lever: it suppresses the hermes leg, which IS
# the log. --remote-only also makes pns POST synchronously and print the
# delivery outcome, so a 401 or a 404 lands in this job's run log instead of
# being discarded into /dev/null while the channel stays empty.
#
# 9>&- closes the caller's serialize-lock fd for pns and everything it spawns.
# pns detaches channels that outlive this whole run, and a kernel flock is held
# until the LAST copy of the fd closes, so a detached child that inherited it
# keeps the lock held after the job exited and the next scheduled slot defers
# over a competing run that does not exist. Closing an fd that was never opened
# is a no-op.
#
# RETURNS THE OUTCOME: 0 when the gateway accepted the entry, non-zero for every
# other ending. The caller needs it because the week guard is claimed around this
# call: a refused delivery that reported success would mark the week done,
# silence the other 23 slots, and leave the week with no entry at all while the
# guard asserted it had one.
#
# The outcome is read from pns's STDOUT line rather than from its exit status,
# and deliberately so: pns exits 0 whatever the gateway answered, which is the
# contract that keeps a broken record channel from breaking the job it reports
# on. That line is pns's stated interface for --remote-only, pinned on both
# sides (test/unit/pns-remote-only.sh writes it, this file reads it). It is
# also echoed onward, so the caller's run log keeps it either way.
#
# Never ABORTS the caller: a non-zero return is a fact to act on, and both
# callers do, but neither treats it as fatal.
# unattended_engine
# The engine the weekly jobs post through: an explicit override wins (the
# test seam), else the binary by absolute path. The callers' -x guards are
# what refuse a half-provisioned machine, loudly.
unattended_engine() {
  local override="${UNATTENDED_LOG_ENGINE:-}"
  if [[ -n $override ]]; then
    printf '%s' "$override"
    return 0
  fi
  printf '%s' "${HOME:-}/.local/libexec/pns/pns"
}

unattended_log_post() {
  local agent="$1" state="$2" project="$3" detail="$4" outcome
  local pns_script
  pns_script="$(unattended_engine)"
  if [[ ! -x $pns_script ]]; then
    printf 'unattended-log: no executable pns engine at %s; this entry was NOT delivered (run chezmoi apply)\n' "$pns_script"
    return 1
  fi
  # stdout captured (it is the outcome), stderr left alone so pns's own
  # warnings still reach this job's run log unmangled.
  outcome="$("$pns_script" --remote-only --channel "$UNATTENDED_LOG_ROUTE" \
    --agent "$agent" --state "$state" --project "$project" --detail "$detail" 9>&- || true)"
  [[ -n $outcome ]] && printf '%s\n' "$outcome"
  grep -q '^pns: posted HTTP 2' <<<"$outcome"
}

# unattended_log_alert_delivery_failure <guard-dir> <agent> -- say on the ALERT
# route that the record channel itself is broken. At most once per ISO week.
#
# WHY THIS EXISTS: pns already prints `post FAILED HTTP 401` into the job's run
# log, and that is very nearly no better than silence. This whole design rests on
# the operator NOT going looking (it is why drift-watch was rejected: a passive
# signal goes unnoticed), and a broken record channel is the one failure the
# record channel cannot report on itself. So it goes to the EXISTING route, which
# lands in the priority channel with a banner and a phone push, exactly like
# every other thing on this machine that needs acting on.
#
# NO PNS_HERMES_URL override, which is what makes this the alert route: pns's
# default is the alert webhook. No --remote-only either; this one should buzz.
# There is no recursion risk: the alert path is fire-and-forget and never
# re-enters this library.
#
# THE WEEK IS CLAIMED ONLY WHEN AN ALERT CAN ACTUALLY BE ATTEMPTED, which is the
# same rule the record delivery follows by giving its claim back on a refusal. A
# claim taken before the pns was found spent the week's one alert on a call
# that sent nothing: a pns restored on Tuesday had every remaining slot's alert
# suppressed by that token, so the week lost BOTH halves at once, the record and
# the alert saying the record could not be posted. The claim cannot cover the
# delivery itself -- this route is fire-and-forget and its outcome is never
# observed -- so it covers exactly what is knowable, that an attempt was made.
unattended_log_alert_delivery_failure() {
  local guard="$1" agent="$2"
  local pns_script
  pns_script="$(unattended_engine)"
  if [[ ! -x $pns_script ]]; then
    printf 'unattended-log: no executable pns engine at %s; the broken-record-channel alert was NOT delivered either, and this week stays unclaimed so a later run retries it\n' "$pns_script"
    return 0
  fi
  unattended_log_claim_week "$guard" delivery-alert || return 0
  "$pns_script" --agent "$agent" --state log-channel-broken \
    --project "$(unattended_log_host)" \
    --detail "$(printf 'The weekly record from %s could NOT be delivered to the unattended-upgrades channel (this job'"'"'s run log carries the HTTP status). Until this is fixed that channel is silent for a reason that has nothing to do with the jobs it reports on, so its silence means nothing. Check that the hermes gateway is up and that it serves the unattended-upgrades route.' "$agent")" 9>&- || true
  return 0
}
