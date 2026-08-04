#!/usr/bin/env bash
#
# report-plugin-updates.sh -- say what Claude Code's own plugin auto-update
# changed. READ ONLY: it installs nothing, updates nothing and writes nothing
# outside its own state directory.
#
# WHY THIS EXISTS. Claude Code refreshes a marketplace and its installed plugins
# at startup by itself, so this machine needs no plugin updater. What it does
# not do is leave a record: plugins move version silently, and the operator has
# no way to answer "what changed on this machine last week" when something
# starts behaving differently. This helper is the record, on the same
# #unattended-upgrades channel and in the same entry shape as the weekly
# Homebrew upgrade and the weekly skills update. Act on alerts, read records.
#
# THE SOURCE OF TRUTH is ~/.claude/plugins/installed_plugins.json, which Claude
# Code maintains. Verified against the live file on 2026-08-03 (schema
# version 2): a `plugins` object keyed by `<name>@<marketplace>`, each holding an
# ARRAY of install records, each record carrying `scope`, `installPath`,
# `version`, `installedAt`, `lastUpdated`, and `gitCommitSha` when the
# marketplace publishes one. Two sibling files were checked and rejected:
# known_marketplaces.json records marketplaces rather than plugin versions, and
# plugin-catalog-cache.json is a catalog of what is AVAILABLE, not what is
# installed.
#
# WHAT REACHES THE CHANNEL: plugin ids and version fingerprints, nothing else.
# Never an installPath (an absolute home path), never a marketplace source URL.
#
# Usage: report-plugin-updates.sh [--scheduled]
#   --scheduled  marks this as the LaunchAgent's run. ONLY a scheduled run posts
#                an entry, advances the success marker or moves the snapshot,
#                mirroring homebrew-weekly-upgrade.sh and update-skills.sh.
#                Without the marker an operator running this by hand on a
#                Wednesday would post a weekly entry and a dead LaunchAgent
#                would look alive, inverting the one signal the record carries.
#                A manual run still prints the comparison to stdout, which is
#                what makes it useful for checking the helper by hand.
#
# ERREXIT IS ON, and every failure this helper means to TOLERATE is guarded where
# it happens rather than by leaving errexit off: the relay call inside alert(),
# the host lookup that falls back to unknown-host, and the library's own
# bookkeeping write, which warns and returns 0 by design so a weekly job never
# dies over its marker file. Everything else that fails stops the run, because
# the alternative is this record continuing past a broken step and posting an
# entry assembled from whatever survived.
set -euo pipefail

# The state Claude Code owns, and the state this helper owns. Both are
# overridable so the tests can point the whole mechanism at a sandbox.
INSTALLED_PLUGINS_FILE="${REPORT_PLUGIN_UPDATES_STATE_FILE:-$HOME/.claude/plugins/installed_plugins.json}"
STATE_DIR="${REPORT_PLUGIN_UPDATES_STATE_DIR:-$HOME/.local/state/report-plugin-updates}"
SNAPSHOT_FILE="$STATE_DIR/installed-plugins.snapshot"
LOG_SUCCESS_MARKER="$STATE_DIR/last-success-at"
LOG_WEEK_GUARD="$STATE_DIR/log-week-claims"

# The relay script by ABSOLUTE path. A LaunchAgent's PATH carries no
# ~/.local/bin, so a bare `relay.sh` would never be found under launchd and
# every alert would vanish exactly when it mattered.
RELAY="${REPORT_PLUGIN_UPDATES_RELAY:-$HOME/.local/bin/relay.sh}"

# jq by absolute path for the same reason, with the same env seam.
JQ="${REPORT_PLUGIN_UPDATES_JQ:-/opt/homebrew/bin/jq}"
[[ -x $JQ ]] || JQ="$(command -v jq 2>/dev/null || printf 'jq')"

# The agent name every message from this helper carries, on both routes.
AGENT_NAME='report-plugin-updates'

# An unknown argument is an ERROR, never a silent fallthrough: a typo'd marker in
# the plist would otherwise run every week and quietly post nothing, which looks
# exactly like a dead LaunchAgent.
SCHEDULED=""
for arg in "$@"; do
  case "$arg" in
    --scheduled) SCHEDULED=1 ;;
    *)
      printf 'usage: report-plugin-updates.sh [--scheduled]\n%s: unknown argument: %s\n' \
        "$AGENT_NAME" "$arg" >&2
      exit 2
      ;;
  esac
done

# The shared entry shape. A missing library is LOUD and never silent: this whole
# helper is a record, and a record that quietly stops being posted is the
# invisibility it exists to end.
UNATTENDED_LOG_LIB="$(dirname "${BASH_SOURCE[0]}")/unattended-log-lib.sh"
UNATTENDED_LOG_AVAILABLE=""
if [[ -r $UNATTENDED_LOG_LIB ]]; then
  # shellcheck source=dot_local/bin/unattended-log-lib.sh
  source "$UNATTENDED_LOG_LIB"
  UNATTENDED_LOG_AVAILABLE=1
else
  printf '%s: WARNING %s is missing; no record will be posted (run chezmoi apply)\n' \
    "$AGENT_NAME" "$UNATTENDED_LOG_LIB" >&2
fi

# The entry's opening lines, from ONE clock reading captured at START-UP, before
# this run can rewrite its own marker. Read later, the gap would be zero on every
# successful run and the timestamp would sit hours from the gap printed under it.
LOG_ENTRY_HEADER=""
if [[ -n $UNATTENDED_LOG_AVAILABLE ]]; then
  LOG_ENTRY_HEADER="$(unattended_log_entry_header "$LOG_SUCCESS_MARKER")"
fi

# What the entry says it cannot tell you, restated on every entry rather than
# assumed known. A record implying a completeness it does not have is worse than
# no record, and BOTH gaps below are real on this machine today.
# shellcheck disable=SC2016 # the backticks are Discord code-span syntax, not a substitution
readonly RECORD_CAVEAT='Versions are what Claude Code records in installed_plugins.json for USER-scope installs; a project-scope plugin is not tracked here. A plugin whose marketplace publishes neither a version nor a commit reports the literal `unknown`, so its updates cannot appear in this list at all.'

readonly RECORD_LABEL='Claude Code plugins'
readonly RECORD_SOURCE_DESCRIPTION='reading ~/.claude/plugins/installed_plugins.json'

# mark_success_or_exit -- record this run, and stop if the record did not land.
#
# The library's own writer warns and returns 0 whatever happened, on purpose: no
# weekly job should die over its bookkeeping file. That leaves this helper to
# check the result, because the marker is what the NEXT entry measures its gap
# from, and an entry claiming a gap from a run that never happened is a lie in
# the only field a reader uses to judge whether this channel is alive. Errexit
# cannot catch this one for us, which is exactly why it is spelled out.
mark_success_or_exit() {
  unattended_log_mark_success "$LOG_SUCCESS_MARKER"
  [[ -f $LOG_SUCCESS_MARKER && -s $LOG_SUCCESS_MARKER ]] && return 0
  printf '%s: this run finished but could not record itself at %s; the next entry would measure its gap from a run that never happened\n' \
    "$AGENT_NAME" "$LOG_SUCCESS_MARKER" >&2
  exit 1
}

# alert <state> <detail> -- the EXISTING relay route, so this lands in the
# priority channel beside every other alert on this machine. Best effort: a
# missing relay never changes the outcome, and a failure to notify is stated.
alert() {
  local state="$1" detail="$2"
  if [[ ! -x $RELAY ]]; then
    printf '%s: relay.sh is not executable at %s; this alert was NOT delivered\n' \
      "$AGENT_NAME" "$RELAY" >&2
    return 0
  fi
  "$RELAY" --agent "$AGENT_NAME" --state "$state" \
    --project "$(unattended_log_host 2>/dev/null || printf 'unknown-host')" \
    --detail "$detail" 9>&- || true
  return 0
}

# record_entry <class> <body> -- post ONE entry, and say whether THIS run
# delivered it.
#
# RETURNS 0 ONLY WHEN THE GATEWAY ACCEPTED IT. Everything downstream of a
# delivered entry (the snapshot move, the success marker) hangs off that answer,
# which is what stops a change from being consumed by a run that reported it
# nowhere: the snapshot still holds the old versions, so the next run finds the
# same change and reports it again.
record_entry() {
  local class="$1" body="$2" detail
  [[ -n $SCHEDULED ]] || return 1
  if ! unattended_log_claim_week "$LOG_WEEK_GUARD" "$class"; then
    printf '%s: this ISO week already has a %s entry; not posting again\n' "$AGENT_NAME" "$class"
    return 1
  fi
  detail="$(printf '%s\n%s' "$LOG_ENTRY_HEADER" "$body")"
  # Claimed BEFORE the attempt so two overlapping runs cannot both post, and
  # GIVEN BACK when the attempt failed so the week is never marked done with
  # nothing sent. A broken record channel cannot report itself, so it is also
  # said once a week on the ALERT route, which is the one that buzzes.
  if UNATTENDED_LOG_RELAY="$RELAY" unattended_log_post "$AGENT_NAME" "$class" \
    "$(unattended_log_host)" "$detail"; then
    return 0
  fi
  unattended_log_release_week "$LOG_WEEK_GUARD" "$class"
  printf '%s: the entry was NOT delivered; this week stays unclaimed so a later run retries\n' \
    "$AGENT_NAME" >&2
  UNATTENDED_LOG_RELAY="$RELAY" unattended_log_alert_delivery_failure "$LOG_WEEK_GUARD" "$AGENT_NAME"
  return 1
}

# plugin_snapshot -- "<plugin id><TAB><version fingerprint>" lines, the input
# shape unattended_log_change_line reads, sorted so two readings compare.
#
# THE FINGERPRINT IS THE FIELD THAT ACTUALLY MOVES, in this order: `version` when
# the marketplace publishes a real one; otherwise `gitCommitSha`, which is the
# same fact under another name; otherwise the literal `unknown`, which the caveat
# names. `lastUpdated` was considered as a third fallback and rejected: measured
# on the live file 2026-08-03, six plugins carried the same lastUpdated to the
# second as their marketplace's own, so a plain marketplace refresh would have
# reported all six as changed every week and trained the reader to skip the
# channel. A record nobody reads is the failure this whole channel exists to end.
#
# REFUSES A DEGRADED FILE rather than reporting its emptiness as a quiet week.
# `{}` and `{"plugins": {}}` both parse, and both would otherwise render as
# "0 of 0 tracked entries changed", which is indistinguishable from a machine
# whose plugins are all steady. A missing file is refused for the same reason,
# and the exit status carries it: pipefail hands the jq failure back through the
# sort.
#
# EXACTLY ONE DOCUMENT, which is why this slurps. jq reads its input as a
# SEQUENCE of JSON values, so a file holding the inventory twice (what a crashed
# writer, an interrupted rewrite, or two writers racing leave behind) parses
# without complaint and this key-walk emits rows from BOTH copies. The snapshot
# then carries two fingerprints for one plugin id, and a machine where nothing
# moved reports a version transition every week until someone notices. Slurping
# and counting is the single-value test (the same shape as
# .chezmoiscripts/run_before_12-quarantine-unparseable-claude-settings.sh).
# `== 1` and not that script's `<= 1`: it tolerates a zero-value file because the
# template it protects treats an empty file as an absent one, whereas an empty
# inventory here is a reading nobody can compare against.
#
# NO USER-SCOPE RECORDS IS AN ANSWER, not a failure, which is why `-e` is not on
# that jq. `-e` exits 4 when a filter produced no output at all, and the filter
# produces none on a file that parses perfectly and says every installed plugin
# is project-scope. Uninstalling the last user-scope plugin while a project-scope
# one remains would then raise plugin-state-unreadable every week from then on
# and report the removal never, which inverts this record twice over: silent
# about a real change, loud about a file that is fine. The degraded shapes above
# are refused by their own error() calls, so the exit status still carries them.
#
# EVERY RECORD IS SHAPE-CHECKED BEFORE THE SCOPE FILTER, which is the order that
# matters. Selecting on `.scope == "user"` first makes a record whose key reads
# `scop` (or holds a number, or is not an object at all) simply drop out of the
# reading, and a plugin that drops out of a reading is reported as REMOVED. A
# typo in a file this helper only ever reads must not be announced as software
# leaving the machine, so a record it cannot interpret refuses the whole run.
#
# STDERR IS THE CALLER'S TO REDIRECT, and is deliberately not merged into stdout
# here. Merged, any line jq ever writes to stderr on an otherwise SUCCESSFUL run
# would be sorted into the snapshot as a row with no tab, which
# unattended_log_change_line reads as a plugin name with an empty fingerprint and
# reports as newly added. A fabricated change is the one thing this record must
# never produce, so the two streams stay apart.
plugin_snapshot() {
  # shellcheck disable=SC2016 # a jq program: $id is a jq binding, not a shell variable
  "$JQ" -rs '
    if length != 1 then
      error("installed_plugins.json holds \(length) top-level JSON documents; exactly one is expected")
    else .[0] end
    | if (.plugins? | type) != "object" then
      error("installed_plugins.json has no plugins object")
    elif (.plugins | length) == 0 then
      error("installed_plugins.json records no installed plugins")
    else . end
    | .plugins
    | to_entries[]
    | .key as $id
    | (if (.value | type) == "array" then .value else error("the entry for \($id) is not an array of install records") end)
    | .[]
    | (if type == "object" then . else error("an install record for \($id) is not an object") end)
    | (if (.scope | type) == "string" then . else error("an install record for \($id) carries no scope string") end)
    | select(.scope == "user")
    | [$id, (if ((.version | type) == "string" and .version != "" and .version != "unknown")
             then .version
             elif ((.gitCommitSha | type) == "string" and .gitCommitSha != "")
             then .gitCommitSha
             else "unknown" end)]
    | @tsv
  ' "$INSTALLED_PLUGINS_FILE" | sort
}

# write_snapshot <reading> -- replace the snapshot ALL AT ONCE, or say so.
#
# A plain copy onto the live snapshot truncates it before it holds the new
# content, so a run interrupted between those two moments (a full disk, a reboot,
# a killed launchd job) leaves a short file behind, and the next run reads every
# plugin missing from it as newly added. A fabricated change list is the one
# thing this record must never produce, and it is worse than no record at all.
#
# The staging file is a SIBLING in the state directory, not in TMPDIR: rename(2)
# is atomic only within one filesystem, and nothing guarantees TMPDIR is on this
# one. Both callers go through here, so the baseline write and the post-delivery
# write get the same guarantee.
write_snapshot() {
  local reading="$1" staged=""
  if staged="$(mktemp "$SNAPSHOT_FILE.XXXXXX" 2>/dev/null)" && [[ -n $staged ]] &&
    cp "$reading" "$staged" && mv -f "$staged" "$SNAPSHOT_FILE"; then
    return 0
  fi
  [[ -n $staged ]] && rm -f "$staged"
  return 1
}

mkdir -p "$STATE_DIR" 2>/dev/null || {
  printf '%s: the state directory %s could not be created; nothing can be compared or remembered\n' \
    "$AGENT_NAME" "$STATE_DIR" >&2
  alert plugin-record-broken \
    "$(printf 'The Claude Code plugin record on %s could not create its state directory at %s, so it can neither compare against last week nor remember this week. Until that is fixed this machine keeps NO record of what its plugins update to.' \
      "$(unattended_log_host 2>/dev/null || printf 'unknown-host')" "$STATE_DIR")"
  exit 1
}

# The snapshot has to be a REGULAR FILE, and anything else is refused here rather
# than discovered later. A DIRECTORY at that path (a stray mkdir, a restore that
# recreated the tree wrong, an interrupted move) makes the absent-snapshot test
# below true on every run: each one takes its copy INSIDE the directory, finds no
# baseline next time, and exits 0 having recorded nothing. That is the shape of
# failure this record exists to end, a machine that looks healthy while saying
# nothing, and it never resolves on its own.
if [[ -e $SNAPSHOT_FILE && ! -f $SNAPSHOT_FILE ]]; then
  printf '%s: the snapshot path %s is not a regular file; nothing can be compared or remembered\n' \
    "$AGENT_NAME" "$SNAPSHOT_FILE" >&2
  alert plugin-record-broken \
    "$(printf 'The Claude Code plugin record on %s cannot use its snapshot path %s, which exists but is not a regular file. Every run would read it as a first run, write inside it and report nothing. Remove whatever is at that path.' \
      "$(unattended_log_host 2>/dev/null || printf 'unknown-host')" "$SNAPSHOT_FILE")"
  exit 1
fi

# Without the library there is no entry shape, no week guard and no gap figure,
# so this helper can do nothing but exit. That has to be ALERTED rather than
# logged, because the symptom of leaving it at a stderr line is a channel that
# quietly stops receiving entries, which is the precise ambiguity the record was
# built to remove. The alert route does not go through the library, so it still
# works here.
if [[ -z $UNATTENDED_LOG_AVAILABLE ]]; then
  alert plugin-record-broken \
    "$(printf 'The Claude Code plugin record on %s could not load %s, so it can post nothing at all. Its channel is silent for a reason that has nothing to do with the plugins it reports on. Run chezmoi apply.' \
      "${HOSTNAME:-unknown-host}" "$UNATTENDED_LOG_LIB")"
  exit 1
fi

printf '=== %s %s ===\n' "$AGENT_NAME" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# The workspace holds ONE file, this run's reading. The previous reading is the
# snapshot, which lives in the state directory across runs by design.
work_dir=""
if ! work_dir="$(mktemp -d "${TMPDIR:-/tmp}/report-plugin-updates.XXXXXX" 2>/dev/null)" ||
  [[ -z $work_dir ]]; then
  printf '%s: the workspace could not be created (mktemp -d failed); nothing was compared\n' \
    "$AGENT_NAME" >&2
  alert plugin-record-broken \
    "$(printf 'The Claude Code plugin record on %s could not create a workspace (mktemp -d failed), so nothing was compared and the snapshot was left alone. Its next run reports the whole gap.' \
      "$(unattended_log_host 2>/dev/null || printf 'unknown-host')")"
  exit 1
fi
trap 'rm -rf "$work_dir"' EXIT
current="$work_dir/current"
read_error="$work_dir/read-error"

# A FAILED READING IS LOUD AND TERMINAL. Every other ending of this script leaves
# a record somewhere; this one refuses to post a change list at all, because the
# only change list it could build from an unreadable file is "nothing changed",
# and that is the exact defect the record exists to prevent. The snapshot is left
# untouched, so once the file is readable again the whole gap is reported.
if ! plugin_snapshot >"$current" 2>"$read_error"; then
  printf '%s: %s could not be read as an installed-plugin inventory:\n%s\n' \
    "$AGENT_NAME" "$INSTALLED_PLUGINS_FILE" "$(cat "$read_error")" >&2
  alert plugin-state-unreadable \
    "$(printf 'The Claude Code plugin record on %s could not read %s as an installed-plugin inventory, so it reported NOTHING rather than a false quiet week. The snapshot was left alone, so the next successful run reports the whole gap. jq said: %s' \
      "$(unattended_log_host 2>/dev/null || printf 'unknown-host')" \
      "$INSTALLED_PLUGINS_FILE" "$(tr '\n' ' ' <"$read_error")")"
  exit 1
fi

printf 'read %d user-scope plugin(s) from %s\n' "$(wc -l <"$current" | tr -d ' ')" "$INSTALLED_PLUGINS_FILE"

# FIRST RUN: record the baseline and say nothing. There is no previous reading to
# compare against, and treating an absent snapshot as an empty one would announce
# every installed plugin as newly added, on a machine where nothing happened.
if [[ ! -f $SNAPSHOT_FILE ]]; then
  printf '%s: no snapshot yet; recording this reading as the baseline and posting nothing\n' "$AGENT_NAME"
  if [[ -z $SCHEDULED ]]; then
    printf '%s: this is not a scheduled run, so the baseline was NOT written\n' "$AGENT_NAME"
    exit 0
  fi
  # A reader that cannot remember what it read is a reader that reports nothing,
  # every week, for as long as the write keeps failing: the next run finds no
  # snapshot, records another baseline and stays quiet. That has to reach the
  # route that buzzes, because the run log is the one place nobody looks and the
  # symptom on the record channel is indistinguishable from a healthy machine.
  if ! write_snapshot "$current"; then
    printf '%s: the baseline could not be written to %s\n' "$AGENT_NAME" "$SNAPSHOT_FILE" >&2
    alert plugin-record-broken \
      "$(printf 'The Claude Code plugin record on %s read its plugins but could not persist the baseline to %s. Until that write succeeds every run records a baseline afresh and reports NOTHING, which looks exactly like a week in which nothing moved.' \
        "$(unattended_log_host 2>/dev/null || printf 'unknown-host')" "$SNAPSHOT_FILE")"
    exit 1
  fi
  mark_success_or_exit
  exit 0
fi

# The one sentence describing what moved, in the shape the other two weekly jobs
# use. Both readings succeeded by this point, which is why the ok flag is a
# literal: the failure path above exits rather than reaching here.
if ! change_line="$(unattended_log_change_section 1 "$SNAPSHOT_FILE" "$current" \
  "$RECORD_LABEL" "$RECORD_CAVEAT" versions "$RECORD_SOURCE_DESCRIPTION")"; then
  printf '%s: the two readings could not be compared; nothing was posted and the snapshot was left alone\n' \
    "$AGENT_NAME" >&2
  exit 1
fi
printf '%s\n' "$change_line"

if ! record_entry completed "$change_line"; then
  printf '%s: no entry was delivered by this run; the snapshot stays where it is\n' "$AGENT_NAME"
  exit 0
fi

# ONLY NOW. The snapshot moves after the entry is delivered and never before, so
# a change consumed by a run that told nobody is reported by the next one
# instead. The success marker follows the same rule, so the gap in the next entry
# is measured from a run that actually posted.
if ! write_snapshot "$current"; then
  printf '%s: the entry was delivered but the snapshot at %s could not be updated; the next entry will repeat this change\n' \
    "$AGENT_NAME" "$SNAPSHOT_FILE" >&2
  alert plugin-record-broken \
    "$(printf 'The Claude Code plugin record on %s delivered this week entry but could not persist the new reading to %s, so every later run re-reports the same change. The state directory needs attention.' \
      "$(unattended_log_host 2>/dev/null || printf 'unknown-host')" "$SNAPSHOT_FILE")"
  exit 1
fi
mark_success_or_exit
exit 0
