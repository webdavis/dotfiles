#!/usr/bin/env bash
#
# results-alerter.sh - the osquery alerter ENTRY, fired by launchd (WatchPaths)
# whenever ~/.local/log/osquery/osqueryd.results.log changes. It reads the new
# results-log rows since its cursor, runs them through the decomposed pipeline
# (normalize -> route -> render), delivers a confirmed-CRITICAL batch as one
# #priority page via alert-dispatch.sh, and advances its cursor ONLY after the
# batch is durably delivered-or-spooled - so a crash between read and checkpoint
# re-reads the same rows (at-least-once), never skips them.
#
# The pipeline stages are sourced single-responsibility helpers under
# results-alerter/; this file is the entry that owns the snapshot-to-delivery
# transaction. ONLY main calls send_alert, the cursor checkpoint (_checkpoint),
# or exit.
#
# No startup drain: the scheduled drainer (drain-undelivered-alerts.sh) owns
# delivery liveness - it sweeps the write-ahead undelivered-alerts store on its
# own timer - so the alerter stays single-responsibility (detect and page) and
# never blocks detection behind a delivery retry. A page that cannot be delivered
# now is persisted by send_alert's write-ahead store and delivered by that drainer
# later.

set -euo pipefail

LOG="${OSQUERY_RESULTS_LOG:-$HOME/.local/log/osquery/osqueryd.results.log}"
STATE="${OSQUERY_RESULTS_OFFSET:-$HOME/.local/state/osquery-results-offset}"

# The dispatch library and the six pipeline helpers, from the libexec home (the
# same deployed path the other consumers source; literal so the relocation guard
# can assert it).
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/results-alerter/normalize.sh"
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/results-alerter/route.sh"
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/results-alerter/allowlist-verdict.sh"
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/results-alerter/pipeline-verdict.sh"
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/results-alerter/digest-store.sh"
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/results-alerter/render-page.sh"

# The single-instance lock file sits beside the cursor it guards, so every alerter
# invocation contends on one lock no matter what launched it (a WatchPaths burst can
# fire several). Overridable for tests.
OSQUERY_RESULTS_LOCK_FILE="${OSQUERY_RESULTS_LOCK_FILE:-${STATE}.lock}"

# _take_single_instance_lock: hold fd 9 open on the lock file and take a nonblocking
# kernel lock on it (/usr/bin/lockf, the house pattern used by the drainer,
# hue-pulse, homebrew-weekly-upgrade). Return 0 to proceed, nonzero to skip. The
# kernel releases the lock on ANY exit (normal or crash), so there is no stale-lock
# state to clean up. This is MUTUAL EXCLUSION: on a genuine setup error it fails
# CLOSED (nonzero -> the caller skips), never runs unlocked - two overlapping runs
# would each read the same cursor+snapshot, double-send the banner, and race the
# checkpoint. The ONE exception is a host without lockf (any non-darwin box, e.g.
# Linux CI): there is no kernel lock to take, so the run proceeds unlocked by design.
_take_single_instance_lock() {
  local lockf_bin="${OSQUERY_RESULTS_LOCKF_BIN:-/usr/bin/lockf}"
  [[ -x $lockf_bin ]] || return 0
  mkdir -p "$(dirname "$OSQUERY_RESULTS_LOCK_FILE")" 2>/dev/null || return 1
  exec 9>>"$OSQUERY_RESULTS_LOCK_FILE" 2>/dev/null || return 1
  "$lockf_bin" -s -t 0 9
}

# _checkpoint <inode> <offset>: atomically write the cursor to the current inode +
# offset. Called by main ONLY after a batch is durably delivered-or-spooled -
# never before parsing - so a page that could be neither delivered nor stored
# leaves the cursor put and the next run retries the same rows. The 9>&- keeps the
# lock fd out of the mv child (see the fd-hygiene note in main).
_checkpoint() { printf '%s %s\n' "$1" "$2" >"$STATE.tmp" && mv -f "$STATE.tmp" "$STATE" 9>&-; }

main() {
  # Take the single-instance lock BEFORE reading the cursor and hold it through
  # route -> send_alert -> checkpoint, so overlapping WatchPaths invocations
  # serialize: exactly one run delivers a batch and advances the cursor; a
  # contended run is a clean no-op. fd HYGIENE: the lock lives on fd 9 (held by
  # this process); EVERY external command spawned below closes it with 9>&- so a
  # forked child - especially a backgrounded notifier from send_alert - can never
  # inherit fd 9 and keep the lock held after this run exits (the latent bug from
  # the allowlist writer's lock).
  _take_single_instance_lock || exit 0

  mkdir -p "$(dirname "$STATE")"
  [[ -f $LOG ]] || exit 0

  # Portable size + inode (wc -c / ls -i work on macOS and Linux; BSD stat -f does
  # not). The inode lets us notice a rotated/recreated log at the same path.
  local size inode
  size="$(wc -c <"$LOG" 9>&-)"
  size=${size//[[:space:]]/}
  # shellcheck disable=SC2012  # $LOG is a fixed, controlled path - ls -i is safe and portable
  inode="$({ ls -i "$LOG" | awk '{print $1}'; } 9>&-)"

  # The cursor holds "<inode> <offset>". A missing or malformed cursor is an
  # ALERTING FAILURE, not a silent seek-to-EOF: deleting the cursor must not be a
  # way to suppress a queued batch. Capture-then-validate (not branch-on-read): a
  # state file missing its trailing newline makes `read` return non-zero even
  # though it populated the vars, so keying the re-seed on read's status would skip
  # a whole batch.
  local prev_inode="" prev_offset="" cursor_reset=0
  if [[ -f $STATE ]]; then read -r prev_inode prev_offset <"$STATE" || true; fi
  if ! [[ $prev_inode =~ ^[0-9]+$ && $prev_offset =~ ^[0-9]+$ ]]; then
    # Replay the WHOLE current log from byte 0 so a page-worthy finding is never
    # silently dropped by a lost cursor - a bounded recent tail skipped anything
    # before it. This is bounded: osquery caps results.log at 10 MB (logger_rotate),
    # and render_page consolidates the batch into ONE capped page (8 blocks + an
    # "N more" marker), so a full replay is one page, not a per-finding storm.
    # Identical repeated resets share the occurrence id inode:0:size, so the store
    # (request_id UNIQUE) and the Hermes gateway dedup them. Warn LOUDLY below.
    cursor_reset=1
    prev_inode="$inode"
    prev_offset=0
  fi

  # New inode (rotation/recreation) or a shrink (truncation) -> read from byte 0 so
  # nothing is skipped or replayed from the old file.
  if [[ $inode != "$prev_inode" || $size -lt $prev_offset ]]; then
    prev_offset=0
  fi

  # Nothing new since the cursor: exit without touching the cursor.
  [[ $size -eq $prev_offset ]] && exit 0

  # Warn LOUDLY that the cursor was reset (a real sound -> a page that reaches the
  # operator, not a muted note). The full replay below re-surfaces every finding in
  # the current log on its own; this is the meta-signal that the alerter's own state
  # was disturbed. Best-effort (|| true) so it never blocks the batch; the batch page
  # gates the checkpoint below.
  if [[ $cursor_reset -eq 1 ]]; then
    send_alert CRIT "🔴 **osquery cursor reset**" \
      "**The osquery alerter cursor was missing or corrupt - monitoring state was disturbed.**"$'\n'"- The current log was replayed in full, so findings are re-surfaced below, not lost."$'\n'"- If you did not clear ~/.local/state, something else reset it - **investigate now**." \
      "Sosumi" "cursor-reset:$inode:$size" 9>&- || true
  fi

  # Read only the new bytes since the cursor. Bound the read to the snapshot window
  # (head -c) so rows appended after we captured $size are not consumed early and
  # re-fired next time. A sentinel byte (x) is appended then stripped: a command
  # substitution strips trailing newlines, and we must know EXACTLY where the last
  # complete record ends. The `; printf x` runs after the pipeline regardless of a
  # head SIGPIPE, so no `|| true` is needed.
  local span snapshot=""
  span=$((size - prev_offset))
  if [[ $span -gt 0 ]]; then
    snapshot="$(
      {
        tail -c "+$((prev_offset + 1))" "$LOG" 2>/dev/null | head -c "$span" 2>/dev/null
        printf x
      } 9>&-
    )"
    snapshot="${snapshot%x}"
  fi

  # Advance only through COMPLETE records. osquery writes a row then its newline; a
  # torn trailing line (mid-write, no newline yet) must be RETAINED, not skipped, so
  # the next run re-reads it once complete. complete_records is the snapshot through
  # its last newline; the trailing bytes after it are a partial line, neither fed to
  # the pipeline nor checkpointed past. This also stops a complete-JSON-without-a-
  # newline from being processed early and then double-processed once its newline
  # lands. complete_bytes is BYTE-exact (LC_ALL=C wc -c) since the cursor is a byte
  # offset.
  local complete_records="" complete_bytes=0
  if [[ $snapshot == *$'\n'* ]]; then
    complete_records="${snapshot%$'\n'*}"$'\n'
    complete_bytes="$({ printf '%s' "$complete_records" | LC_ALL=C wc -c; } 9>&-)"
    complete_bytes="${complete_bytes//[[:space:]]/}"
  fi
  local checkpoint_offset=$((prev_offset + complete_bytes))

  # The pipeline: normalize the complete records into findings, route each to
  # page/digest/log-only (digest and log-only are handled in-stage), render the CRIT
  # page-candidates into the #priority body. A malformed row yields nothing for that
  # row (normalize's per-line try/fromjson), so a garbage batch produces an empty
  # page and never aborts.
  local render pcount pbody
  render="$({ printf '%s' "$complete_records" | normalize_findings | route_findings | render_page; } 9>&-)"
  pcount="$(jq -r '.pcount // 0' 9>&- <<<"$render" 2>/dev/null || printf '0')"
  [[ $pcount =~ ^[0-9]+$ ]] || pcount=0

  # Checkpoint ONLY after the batch is durably handled, and only through the last
  # COMPLETE record (checkpoint_offset), never past a torn trailing line. deliver_ok
  # starts true (a batch with no page consumed only digest/log-only rows, already
  # handled); a page that hard-fails to deliver AND spool sets it false, so the
  # cursor stays put and the next run retries the SAME rows (at-least-once). The
  # occurrence id (inode + this batch's complete-record byte range) makes the page's
  # request_id occurrence-unique yet stable across a retry of this same batch.
  local deliver_ok=1
  if [[ $pcount -gt 0 ]]; then
    pbody="$(jq -r '.pbody' 9>&- <<<"$render")"
    local title="🔴 **CRITICAL**"
    [[ $pcount -gt 1 ]] && title="🔴 **CRITICAL** · $pcount"
    if ! send_alert CRIT "$title" "$pbody" "Sosumi" "$inode:$prev_offset:$checkpoint_offset" 9>&-; then
      deliver_ok=0
    fi
  fi

  # Exit 0 even on a delivery hard-failure: the batch was processed and the failure
  # is already surfaced (send_alert's loud local alert + delivery log). A nonzero
  # exit would false-trip the uptime watchdog's crash-loop check. The cursor simply
  # stays put for a retry.
  [[ $deliver_ok -eq 1 ]] && _checkpoint "$inode" "$checkpoint_offset"
  exit 0
}

main "$@"
