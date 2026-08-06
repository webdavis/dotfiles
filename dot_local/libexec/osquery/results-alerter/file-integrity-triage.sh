#!/usr/bin/env bash
#
# file-integrity-triage.sh - a sourced helper for results-alerter.sh. Functions
# only, no main. DISPLAY-ONLY: nothing here changes a severity, suppresses a
# finding, or returns non-zero.
#
# WHY IT EXISTS. The file-integrity page fires whenever a watched file diverges
# from its root-owned known-good manifest, and every one of those pages read the
# same: a path, a verb, and a next step. A routine vendor update and a tamper
# were byte-identical bodies, so triage started from zero every time - open a
# terminal, hash the file by hand, read the manifest by hand, then try to
# remember whether anything upgraded itself lately. This puts the three facts
# that separate those two cases INTO the page:
#
#   recorded  the sha256 the governing manifest binds to this exact path, short
#   ondisk    the sha256 the file carries right now, short
#   upgrade   whether a RECORDED unattended upgrade plausibly explains it
#
# Equal hashes are informative on their own: the verdict paged, so something
# diverged, and if the content agrees then the divergence is the mode, the owner,
# or the manifest being unusable at all.
#
# WHAT IT NEVER DOES. It never says a change is safe and it never dismisses one.
# A correlated upgrade is a LEAD, and the rendered line says so in as many words,
# because the record it reads is not a trust input: it lives in the
# operator-writable state dir, so whoever tampered with a manifested file could
# also write a record claiming an upgrade explains it. The page still pages, at
# the same tier, with the same next step.
#
# A PAGE IS NEVER LOST TO A CORRELATION FAILURE. Every function here returns 0,
# on every input. The caller runs under `set -euo pipefail` and has ALREADY
# decided to page by the time it asks, so a missing record, a malformed record,
# an unreadable manifest or a vanished file each cost the page one LINE and
# nothing more. Each of those states renders its own sentence rather than an
# empty field, and the ones that mean something is broken also say so on stderr,
# which lands in the alerter's launchd log.
#
# THE MAPPING FROM A FILE TO THE PACKAGE THAT SHIPS IT IS NOT KNOWABLE HERE, and
# this is the limit to keep in mind when reading a page. The upgrade record lists
# package NAMES and version transitions, not file lists; asking Homebrew which
# formula owns a path would mean forking brew from the alert path, and a keg that
# has since been cleaned up cannot answer for the version it replaced anyway. So
# the only correlation this makes is a NAME match against the flagged file's
# basename, which is a lead and is labelled as one. When nothing matches, the
# line states what the recent run DID change instead of claiming a mapping it
# cannot verify.
#
# WHICH PAGES AN UPGRADE CAN EVEN EXPLAIN, stated so the correlation is not read
# as broader than it is. Both known-good manifests are derived from chezmoi
# INTENT, so on a healthy machine no Homebrew upgrade can move a manifested file
# and the honest answer is almost always the no-match line. The case where the
# correlation earns its place is the managed-bin FAIL-SAFE: when that manifest is
# missing, empty or untrustworthy, _managed_bin_is_tracked deliberately tracks
# EVERYTHING under ~/.local/bin, which is where the self-updating third-party
# shims live (herdr, mise, bob, hermes, yt-dlp, and the pipx and uv symlinks).
# Every one of their updates then pages, and several of them are Homebrew
# formulae. The hash pair below is what carries the other pages: it says whether
# the content diverged at all, or whether the mode, the owner or the manifest
# itself is what the verdict actually tripped on.
#
# NO APOSTROPHES IN ANY STRING HERE. Every value reaches the page through
# render-page.sh, whose jq program is bash SINGLE-QUOTED; an apostrophe in a
# rendered string ends that quoting and breaks the renderer.

# The upgrade record, written by ~/.local/libexec/unattended-upgrades/homebrew-weekly-upgrade.sh TWICE per
# run whose package listing could be read: the run line alone before the first
# brew step, then the whole thing again with the package rows once the run is
# done. Both carry the same timestamp, so the two are one record at two levels of
# detail; the pre-upgrade publish is what makes the record cover the window it
# describes, since a watched file rewritten in the first seconds of a run would
# otherwise be correlated against the PREVIOUS week. Keep this literal in sync
# with the producer; test/unit/osquery-file-integrity-triage.sh pins them equal,
# because a rename in one alone leaves this answering no-record forever, which
# reads exactly like a quiet month of upgrades.
#
# Format, tab separated. Line 1 is the run that produced it:
#
#   <epoch-seconds>	<iso-8601-utc>
#
# and every later line is one package that moved:
#
#   <name>	<added|removed|changed>	<before-version>	<after-version>
#
# The absent side of an add or a remove is the empty string. LINE 1 ALONE has two
# readings, which is why the sentence rendered for it states what the run
# recorded rather than what it did: a run still in flight has compared nothing
# yet, and a finished run that moved nothing dates the last time anything
# upgraded at all.
OSQUERY_UPGRADE_RECORD="${OSQUERY_UPGRADE_RECORD:-$HOME/.local/state/homebrew-weekly-upgrade/last-upgrade-changes.tsv}"

# How recent a recorded upgrade must be to be offered as an explanation.
#
# THREE DAYS, and the number is chosen against the two clocks that bound it. The
# upgrade job runs weekly, so a window of seven days or more could hold two runs
# and the answer would stop being unambiguous; the record keeps only the newest
# run for the same reason. At the other end, the page itself is fast (an event is
# judged within seconds, and the scheduled audit within two 15-minute ticks), so
# the honest causal gap between an upgrade and its page is minutes. Three days is
# the slack between those: it survives a machine that slept through Monday and a
# launchd interval coalesced into a later wake, without ever letting a
# fortnight-old upgrade be waved at a page it cannot explain.
OSQUERY_UPGRADE_RECORD_WINDOW_DAYS=3

# A record longer than this is refused whole rather than walked. The real file
# lists the packages one weekly run moved (tens, at a whole-Cellar move maybe a
# few hundred), and this runs on the alert path, where an unbounded read of a
# file anyone can append to would be a way to stall paging.
OSQUERY_UPGRADE_RECORD_MAX_ROWS=500

# The most bytes that may be read, and the size past which the record is refused
# whole. The row cap above bounds how many LINES are walked; it does not bound
# the read itself, because a single line with no newline in it can be as large as
# the disk allows and would be read in full before the first row was counted. At
# 500 rows this leaves over 500 bytes per row, and the real file averages well
# under a hundred.
OSQUERY_UPGRADE_RECORD_MAX_BYTES=262144

# How many package names an unmatched line lists before it counts the rest. The
# whole sentence is capped at 240 characters by the renderer, so a long list
# would be truncated mid-name and take the timestamp with it.
OSQUERY_UPGRADE_RECORD_NAME_CAP=5

# _file_integrity_short <sha256>: the first twelve hex characters of a plausible
# sha256, or the empty string. Long enough to read back off a terminal against
# `shasum -a 256`, short enough to sit in a page line beside its twin. Validated
# rather than sliced blind, so a stat error string or a truncated manifest column
# renders as nothing rather than as a plausible-looking digest.
_file_integrity_short() {
  local hash="${1,,}"
  [[ $hash =~ ^[0-9a-f]{64}$ ]] || return 0
  printf '%s' "${hash:0:12}"
  return 0
}

# _file_integrity_recorded_hash <target>: the short sha256 the governing manifest
# binds to this exact path, or a stated reason there is none. Never a guess.
#
# The manifest CHOICE is reused from pipeline-verdict.sh rather than re-derived:
# which of the two known-good lists governs a path is a security decision that
# already has one owner and one test. Checked by NAME, because this helper is
# sourced beside that one and a partial deploy must report a broken lookup rather
# than resolve every path to the wrong list.
_file_integrity_recorded_hash() {
  local target="$1" manifest hash path
  if ! declare -F _pipeline_manifest_for >/dev/null 2>&1; then
    printf 'manifest lookup unavailable'
    return 0
  fi
  manifest="$(_pipeline_manifest_for "$target")"
  if [[ ! -r $manifest || ! -s $manifest ]]; then
    printf 'manifest unreadable'
    return 0
  fi
  # Four whitespace-separated columns with the PATH LAST, so a path holding
  # spaces is taken whole by the final field; the same idiom the verdict and the
  # audit read this file with. `|| [[ -n $hash ]]` keeps a final line with no
  # trailing newline in scope.
  while read -r hash _ _ path || [[ -n $hash ]]; do
    [[ -n $path && $path == "$target" ]] || continue
    hash="$(_file_integrity_short "$hash")"
    if [[ -n $hash ]]; then
      printf '%s' "$hash"
      return 0
    fi
  done <"$manifest"
  # No tuple for this path is not a defect to hide: it is the fail-safe shape the
  # verdict pages on (a file the manifest can never vouch for), and naming it is
  # what tells the operator the manifest is not merely disagreeing.
  printf 'not in the manifest'
  return 0
}

# _file_integrity_disk_hash <target>: the short sha256 the file carries right
# now, or a stated reason there is none. Links are named, never followed: a
# symlink standing where a manifested file belongs is itself the finding, and
# hashing through it would report the referent bytes as if they were the watched
# ones.
_file_integrity_disk_hash() {
  local target="$1" hash
  if [[ -L $target ]]; then
    printf 'a symbolic link'
    return 0
  fi
  if [[ ! -e $target ]]; then
    printf 'absent'
    return 0
  fi
  if [[ ! -f $target ]]; then
    printf 'not a regular file'
    return 0
  fi
  hash="$(shasum -a 256 -- "$target" 2>/dev/null)"
  hash="$(_file_integrity_short "${hash%% *}")"
  if [[ -n $hash ]]; then
    printf '%s' "$hash"
    return 0
  fi
  printf 'unreadable'
  return 0
}

# _file_integrity_upgrade_line <target>: one sentence saying whether a recorded
# unattended upgrade plausibly explains this file changing.
#
# Five endings, each a fact rather than a verdict:
#   a name match inside the window  -> the transition, its timestamp, and the
#                                      explicit statement that a name match is
#                                      not proof
#   no name match inside the window -> what the run DID change, or that it
#                                      changed nothing
#   the newest record is older      -> which window it fell out of, and when it
#                                      is from
#   no record on this machine       -> said plainly
#   the record cannot be parsed     -> said plainly, and warned about on stderr
#
# A record whose timestamp is in the FUTURE (a restored backup, a clock
# correction) falls out of the window arm and is shown with its timestamp, which
# is the honest reading: it is not in the last three days, and the operator can
# see why at a glance.
_file_integrity_upgrade_line() {
  local target="$1" record="$OSQUERY_UPGRADE_RECORD"
  local basename_of_target="${target##*/}"
  local epoch iso now age window rows=0 bytes snapshot=""
  local row rest name state version_before version_after
  local matched="" matched_before="" matched_after=""
  local -a changed_names=()

  if [[ ! -e $record ]]; then
    printf 'no upgrade record on this machine'
    return 0
  fi
  # WHAT MAY BE READ AT ALL. The record path is not a trust input: it lives in
  # the operator-writable state dir, so anything can be standing there. A
  # readable FIFO with no writer is the shape that matters, because `read` on it
  # blocks FOREVER, and it blocks while the alerter holds its single-instance
  # lock, so the page is never sent and the cursor never advances: every page on
  # the machine stops behind one named pipe. A character device does the same by
  # never yielding a newline. A regular file is the only shape that cannot, so it
  # is the only shape accepted. SYMLINKS ARE FOLLOWED, deliberately: `-f` judges
  # the final target, so a link to a regular file is read (a normal way to place
  # state) and a link to a pipe or a device is refused exactly like the bare one.
  if [[ ! -f $record || ! -r $record ]]; then
    printf 'osquery file-integrity triage: the upgrade record at %s is not a readable regular file; this page carries no correlation\n' \
      "$record" >&2
    printf 'the upgrade record could not be read'
    return 0
  fi
  # ONE OPEN, ONE SNAPSHOT, BOUNDED. Both shapes (the run line and the package
  # rows) are parsed from these same bytes. Two opens could straddle the atomic
  # rename the producer publishes with, and would then pair one generation's
  # timestamp with the other generation's transitions: a correlation that no run
  # ever produced, rendered as if it had. The read is capped because this is the
  # alert path. The type check above already refuses the shapes that block
  # forever, so what is left to bound is size, and a byte cap bounds that
  # directly; a watchdog child (the shape ssh-hardening --verify uses to bound a
  # blocking `sshd -G`) would buy nothing here, since there is no external
  # process to signal, only this shell reading a local file. A record over the
  # cap is refused WHOLE rather than parsed from its first bytes, for the same
  # reason an unrecognised row refuses the whole record: a partial reading
  # rendered as a complete one is the failure this correlation exists to avoid.
  snapshot="$(head -c "$((OSQUERY_UPGRADE_RECORD_MAX_BYTES + 1))" -- "$record" 2>/dev/null)" || snapshot=""
  bytes="$(printf '%s' "$snapshot" | LC_ALL=C wc -c)" || bytes=""
  bytes="${bytes//[[:space:]]/}"
  if [[ ! $bytes =~ ^[0-9]+$ ]] || ((bytes > OSQUERY_UPGRADE_RECORD_MAX_BYTES)); then
    printf 'osquery file-integrity triage: the upgrade record at %s could not be read within %s bytes; this page carries no correlation\n' \
      "$record" "$OSQUERY_UPGRADE_RECORD_MAX_BYTES" >&2
    printf 'the upgrade record could not be read'
    return 0
  fi
  # The first line dates the record, taken from the snapshot rather than from a
  # second open of the file.
  row="${snapshot%%$'\n'*}"
  epoch="${row%%$'\t'*}"
  iso="${row#*$'\t'}"
  if [[ ! $epoch =~ ^[0-9]{1,11}$ ]] ||
    [[ ! $iso =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]]; then
    printf 'osquery file-integrity triage: the upgrade record at %s does not start with a run timestamp; this page carries no correlation\n' \
      "$record" >&2
    printf 'the upgrade record could not be read'
    return 0
  fi

  now="${EPOCHSECONDS:-$(date +%s)}"
  [[ $now =~ ^[0-9]{1,11}$ ]] || now=0
  window=$((OSQUERY_UPGRADE_RECORD_WINDOW_DAYS * 86400))
  age=$((10#$now - 10#$epoch))
  if ((age < 0)) || ((age > window)); then
    printf 'no recorded upgrade in the last %s days; the newest record is from %s' \
      "$OSQUERY_UPGRADE_RECORD_WINDOW_DAYS" "$iso"
    return 0
  fi

  while IFS= read -r row; do
    rows=$((rows + 1))
    ((rows == 1)) && continue # the run timestamp, already read above
    # FIELD DECODING BY EXPANSION, NOT BY `read`. A tab is one of the three IFS
    # WHITESPACE characters, so `IFS=$'\t' read -r name state before after`
    # treats a RUN of tabs as a single delimiter and drops the empty column the
    # producer writes for the absent side of an add or a remove. An `added` row
    # (name, added, EMPTY, version) decoded as (name, added, version, EMPTY) and
    # rendered backwards: a package that had just appeared read as one that had
    # just been removed. Splitting on each delimiter in turn keeps an empty
    # column exactly where the producer put it. A row with too few columns lands
    # a copy of the tail in `state`, which the recognised-state check below
    # refuses along with every other row shape this cannot describe.
    name="${row%%$'\t'*}"
    rest="${row#*$'\t'}"
    state="${rest%%$'\t'*}"
    rest="${rest#*$'\t'}"
    version_before="${rest%%$'\t'*}"
    version_after="${rest#*$'\t'}"
    [[ -n $name ]] || continue
    if ((rows > OSQUERY_UPGRADE_RECORD_MAX_ROWS)); then
      printf 'osquery file-integrity triage: the upgrade record at %s lists more than %s rows; this page carries no correlation\n' \
        "$record" "$OSQUERY_UPGRADE_RECORD_MAX_ROWS" >&2
      printf 'the upgrade record could not be read'
      return 0
    fi
    case "$state" in
      added | removed | changed) ;;
      *)
        # A row this cannot describe makes the WHOLE record untrustworthy rather
        # than one row skippable: a partial reading rendered as a complete one is
        # the failure this correlation exists to avoid.
        printf 'osquery file-integrity triage: the upgrade record at %s holds a row with no recognised state; this page carries no correlation\n' \
          "$record" >&2
        printf 'the upgrade record could not be read'
        return 0
        ;;
    esac
    changed_names+=("$name")
    if [[ -z $matched && $name == "$basename_of_target" ]]; then
      matched="$name"
      matched_before="${version_before:-none}"
      matched_after="${version_after:-none}"
    fi
  done <<<"$snapshot"

  if [[ -n $matched ]]; then
    printf 'recorded upgrade: %s %s -> %s at %s (the name matches this file, which is not proof)' \
      "$matched" "$matched_before" "$matched_after" "$iso"
    return 0
  fi
  if [[ ${#changed_names[@]} -eq 0 ]]; then
    # "recorded no package change", not "changed nothing". The producer publishes
    # the run line before its first brew step and fills in the rows afterwards,
    # so this shape is read by two different runs: a week that genuinely moved
    # nothing, and a run still in flight that has not compared anything yet. The
    # first wording is true of both; the second would claim a completed run about
    # one that is still going.
    printf 'no recorded upgrade names this file; the run at %s recorded no package change' "$iso"
    return 0
  fi
  # Names only, no versions: the whole sentence shares a 240-character cap with
  # the timestamp, and a truncated list that swallowed the date would be worse
  # than a shorter one.
  local shown
  shown="$(printf '%s, ' "${changed_names[@]:0:OSQUERY_UPGRADE_RECORD_NAME_CAP}")"
  shown="${shown%, }"
  if [[ ${#changed_names[@]} -gt $OSQUERY_UPGRADE_RECORD_NAME_CAP ]]; then
    shown="$shown, and $((${#changed_names[@]} - OSQUERY_UPGRADE_RECORD_NAME_CAP)) more"
  fi
  printf 'no recorded upgrade names this file; the run at %s changed: %s' "$iso" "$shown"
  return 0
}

# file_integrity_triage <target>: the three facts as one compact JSON object, for
# the router to attach to a finding and the renderer to print.
#
# Built with `jq -n --arg`, never by interpolating into a JSON string: two of the
# three values carry text chosen by whoever published a package. A jq that cannot
# run at all yields an empty object, so the renderer falls back to its own
# placeholders and the page survives that too.
file_integrity_triage() {
  local target="$1"
  jq -cn \
    --arg recorded "$(_file_integrity_recorded_hash "$target")" \
    --arg ondisk "$(_file_integrity_disk_hash "$target")" \
    --arg upgrade "$(_file_integrity_upgrade_line "$target")" \
    '{recorded: $recorded, ondisk: $ondisk, upgrade: $upgrade}' 2>/dev/null || printf '{}'
  return 0
}
