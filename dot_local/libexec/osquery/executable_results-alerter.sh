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

# _checkpoint <inode> <offset>: atomically write the cursor to the current inode +
# offset. Called by main ONLY after a batch is durably delivered-or-spooled -
# never before parsing - so a page that could be neither delivered nor stored
# leaves the cursor put and the next run retries the same rows.
_checkpoint() { printf '%s %s\n' "$1" "$2" >"$STATE.tmp" && mv -f "$STATE.tmp" "$STATE"; }

main() {
  mkdir -p "$(dirname "$STATE")"
  [[ -f $LOG ]] || exit 0

  # Portable size + inode (wc -c / ls -i work on macOS and Linux; BSD stat -f does
  # not). The inode lets us notice a rotated/recreated log at the same path.
  local size inode
  size="$(wc -c <"$LOG")"
  size=${size//[[:space:]]/}
  # shellcheck disable=SC2012  # $LOG is a fixed, controlled path - ls -i is safe and portable
  inode="$(ls -i "$LOG" | awk '{print $1}')"

  # The cursor holds "<inode> <offset>". A missing or malformed cursor is an
  # ALERTING FAILURE, not a silent seek-to-EOF: deleting the cursor must not be a
  # way to suppress a queued batch. Capture-then-validate (not branch-on-read): a
  # state file missing its trailing newline makes `read` return non-zero even
  # though it populated the vars, so keying the re-seed on read's status would skip
  # a whole batch.
  local prev_inode="" prev_offset="" cursor_reset=0
  if [[ -f $STATE ]]; then read -r prev_inode prev_offset <"$STATE" || true; fi
  if ! [[ $prev_inode =~ ^[0-9]+$ && $prev_offset =~ ^[0-9]+$ ]]; then
    # Reprocess a BOUNDED recent tail (avoids a multi-MB replay; the page cap bounds
    # the blast radius) and warn LOUDLY below.
    cursor_reset=1
    prev_inode="$inode"
    local reset_tail="${OSQUERY_RESULTS_RESET_TAIL_BYTES:-262144}"
    if [[ $size -gt $reset_tail ]]; then prev_offset=$((size - reset_tail)); else prev_offset=0; fi
  fi

  # New inode (rotation/recreation) or a shrink (truncation) -> read from byte 0 so
  # nothing is skipped or replayed from the old file.
  if [[ $inode != "$prev_inode" || $size -lt $prev_offset ]]; then
    prev_offset=0
  fi

  # Nothing new since the cursor: exit without touching the cursor.
  [[ $size -eq $prev_offset ]] && exit 0

  # Warn LOUDLY that the cursor was reset (a real sound -> a page that reaches the
  # operator, not a muted note). The reprocessed tail surfaces any queued unsafe row
  # on its own; this is the meta-signal that the alerter's own state was disturbed.
  # Best-effort (|| true) so it never blocks the batch; the batch page gates the
  # checkpoint below.
  if [[ $cursor_reset -eq 1 ]]; then
    send_alert CRIT "🔴 **osquery cursor reset**" \
      "**The osquery alerter cursor was missing or corrupt - monitoring state was disturbed.**"$'\n'"- Recent results were reprocessed from a bounded tail; a queued batch may have been briefly skipped before this run."$'\n'"- If you did not clear ~/.local/state, something else reset it - **investigate now**." \
      "Sosumi" "cursor-reset:$inode:$size" || true
  fi

  # Read only the new bytes since the cursor. Bound the read to the snapshot window
  # (head -c) so rows appended after we captured $size are not consumed early and
  # re-fired next time; || true absorbs head's SIGPIPE. Clamp defensively: an
  # inode-reusing rotation in the window could otherwise hand head -c a non-positive
  # count.
  local span new_lines=""
  span=$((size - prev_offset))
  if [[ $span -gt 0 ]]; then
    new_lines="$(tail -c "+$((prev_offset + 1))" "$LOG" | head -c "$span" || true)"
  fi

  # The pipeline: normalize the raw rows into findings, route each to page/digest/
  # log-only (digest and log-only are handled in-stage), render the CRIT
  # page-candidates into the #priority body. A malformed row yields nothing for that
  # row (normalize's per-line try/fromjson), so a garbage batch produces an empty
  # page and never aborts.
  local render pcount pbody
  render="$(printf '%s\n' "$new_lines" | normalize_findings | route_findings | render_page)"
  pcount="$(jq -r '.pcount // 0' <<<"$render" 2>/dev/null || printf '0')"
  [[ $pcount =~ ^[0-9]+$ ]] || pcount=0

  # Checkpoint ONLY after the batch is durably handled. deliver_ok starts true (a
  # batch with no page consumed only digest/log-only rows, already handled); a page
  # that hard-fails to deliver AND spool sets it false, so the cursor stays put and
  # the next run retries the SAME rows (at-least-once). The occurrence id (inode +
  # this batch's byte range) makes the page's request_id occurrence-unique yet
  # stable across a retry of this same batch.
  local deliver_ok=1
  if [[ $pcount -gt 0 ]]; then
    pbody="$(jq -r '.pbody' <<<"$render")"
    local title="🔴 **CRITICAL**"
    [[ $pcount -gt 1 ]] && title="🔴 **CRITICAL** · $pcount"
    if ! send_alert CRIT "$title" "$pbody" "Sosumi" "$inode:$prev_offset:$size"; then
      deliver_ok=0
    fi
  fi

  # Exit 0 even on a delivery hard-failure: the batch was processed and the failure
  # is already surfaced (send_alert's loud local alert + delivery log). A nonzero
  # exit would false-trip the uptime watchdog's crash-loop check. The cursor simply
  # stays put for a retry.
  [[ $deliver_ok -eq 1 ]] && _checkpoint "$inode" "$size"
  exit 0
}

main "$@"
