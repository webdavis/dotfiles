#!/usr/bin/env bash
#
# osquery-digest.sh - the daily digest builder. Drains the digest spool (NDJSON,
# written by the alerter's _digest_append) into ONE grouped, silent #priority
# message, then rotates the live store to .last. Empty-suppressed: an absent or
# whitespace-only store produces zero output. Cadence is owned by the launchd
# StartCalendarInterval agent, NOT this script - there is no internal time gate,
# so the script builds (or stays silent) the same way whenever it is invoked.
set -euo pipefail

# shellcheck source=/dev/null
source "$HOME/.local/bin/osquery-alert-dispatch.sh"

store="${OSQUERY_DIGEST_STORE:-$HOME/.local/state/osquery-digest-spool/digest.ndjson}"

# Guard 1: nothing to summarize → silent.
[ -s "$store" ] || exit 0

# Atomically rotate the live store aside so findings appended by the alerter while
# we build are not lost - they accumulate into a fresh store for the next run.
work_file="$store.$(date -u +%s).build"
mv -f "$store" "$work_file" 2>/dev/null || exit 0

# If any step below aborts before the send, restore the batch so the next daily
# run retries it - a silently-dropped digest is invisible to this user.
trap 'mv -f "$work_file" "$store" 2>/dev/null || true' ERR

# Guard 2: a whitespace-only or zero-byte store has no real records → silent.
grep -q '[^[:space:]]' "$work_file" 2>/dev/null || {
  rm -f "$work_file"
  exit 0
}

item_count=$(grep -c . "$work_file" 2>/dev/null || echo 0)

# Group findings by detector: one header per detector with a count, up to ten
# bullets, then a "+K more" roll-up for the rest. Cap the whole body well under
# Discord's 2000-character message limit.
# Parse per line and skip any unparseable (torn/partial) line instead of slurping
# the whole file as one document: a single malformed line - an interrupted
# _digest_append - must not abort the run and lose the day's digest. Mirrors the
# alerter's own resilient results.log reader.
body=$(jq -rRs '
  # One rendered block per detector group: header + up to ten bullets + a roll-up.
  def render_group:
    "**\(.[0].detector)** (\(length))",
    (.[0:10][] | "- \(.identity) - \(.summary)"),
    (if length > 10 then "… +\(length - 10) more" else empty end),
    "";
  split("\n")
  | map(select(length > 0) | (try fromjson catch empty))
  | group_by(.detector) as $groups
  # Cap the NUMBER of groups and emit a marker for the rest, so a busy day cannot
  # drop whole trailing groups to a silent mid-line head -c cut (the content still
  # survives in results.log/.last). head -c below stays as a hard backstop.
  | ($groups[0:12][] | render_group),
    (if ($groups | length) > 12
     then "… and \(($groups | length) - 12) more detector group(s) - see results.log"
     else empty end)
' "$work_file" 2>/dev/null | head -c 1800) || true

# Empty-suppression, second gate: if EVERY spooled line was torn/unparseable the body
# renders empty even though Guard 2 (non-whitespace bytes) and item_count saw the torn
# lines. Do NOT POST a misleading silent "N item(s)" with an empty body; rotate the
# unrecoverable batch to .last for forensics and stay silent.
if ! printf '%s' "$body" | grep -q '[^[:space:]]'; then
  mv -f "$work_file" "$store.last" 2>/dev/null || rm -f "$work_file"
  chmod 600 "$store.last" 2>/dev/null || true
  exit 0
fi

title="🗒️ osquery daily digest · $(date -u +%Y-%m-%d) · ${item_count} item(s)"

# CRIT selects the #priority channel (the dispatcher's only route); the empty sound makes
# it silent AND threads tier=muted into the POST so the Hermes adapter suppresses the ping
# (R2-11) - a digest must never notify like a page. `|| true`: send_alert now returns nonzero
# on a HARD delivery failure (R2-6); the digest is fire-and-forget (a lost daily digest is
# low-stakes and send_alert already fired a loud local alert), so keep it off set -e's abort
# path and still rotate the batch to .last below. The launchd agent runs this once daily.
send_alert CRIT "$title" "$body" "" || true

# Keep the rendered batch as .last for forensics; the live store is already gone. The
# .last persists indefinitely and holds full paths, so keep it 600 (mv preserves mode,
# but be defensive in case the source store was written before the hardening landed).
mv -f "$work_file" "$store.last" 2>/dev/null || rm -f "$work_file"
chmod 600 "$store.last" 2>/dev/null || true
