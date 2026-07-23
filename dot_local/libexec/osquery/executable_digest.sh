#!/usr/bin/env bash
#
# digest.sh - the daily osquery digest builder. Drains the digest spool (NDJSON,
# written by the alerter's digest_append) into ONE grouped, silent, non-paging
# message, then rotates the live store aside for forensics. Empty-suppressed: an
# absent, zero-byte, or whitespace-only store produces no message and no error.
#
# Cadence is owned by the daily launchd agent, NOT this script: there is no
# internal time gate, so a manual invocation builds (or stays silent) exactly the
# way the scheduled one does. That keeps the "when" in one declarative place (the
# LaunchAgent's StartCalendarInterval) and makes the builder trivially testable.
set -euo pipefail

# The shared dispatch library, from the libexec home (the same deployed path the
# other consumers source; the literal string lets the relocation guard assert it).
# send_alert is the write-ahead-durable sender the CRIT page path also uses, so
# the digest inherits that durability without its own delivery machinery.
# shellcheck source=/dev/null
source "$HOME/.local/libexec/osquery/alert-dispatch.sh"

# The digest spool the alerter's digest_append accumulates into, one NDJSON line
# per non-paging finding. Same default path and OSQUERY_DIGEST_STORE override as
# the write side (results-alerter/digest-store.sh), so reader and writer agree on
# the file without a shared constant that could drift between them.
OSQUERY_DIGEST_STORE_DEFAULT="$HOME/.local/state/osquery-digest-spool/digest.ndjson"

# rotated_work_file <store> - the unique work-file path this run claims its batch
# into. Derived from the store path plus a UTC unix timestamp and a .build suffix:
# unique-per-run so a stale work file from a crashed run is never silently reused,
# and .build names the in-flight batch for forensics.
rotated_work_file() { printf '%s.%s.build' "$1" "$(date -u +%s)"; }

# restore_batch <work_file> <store> - put the claimed batch back as the live store so the next
# daily run retries it. It APPENDS rather than mv -f overwrites: the alerter can append a new
# finding to the fresh store DURING the build, and an overwrite would destroy that concurrent
# append, whereas an append cannot clobber it (order within a grouped digest is irrelevant). The
# work file is removed only if the append succeeded, so a failed append leaves the batch to retry.
# This is the ERR trap's action for a build failure before the send AND the hard-send-failure
# branch: a silently dropped digest is invisible to this single user.
restore_batch() {
  if cat -- "$1" >>"$2" 2>/dev/null; then rm -f -- "$1" || true; fi
}

# rotate_to_last <work_file> <store> - preserve the built batch as $store.last for
# forensics, regardless of send outcome; the live store is already fresh (rotated at
# the start), so a re-run is silent. The .last holds full filesystem paths and
# persists indefinitely, so keep it 600 (defensive: mv preserves mode, but a store
# written before the 600 hardening might not have carried it).
rotate_to_last() {
  # A failed same-dir rename DELETES the work file (unlike restore_batch's mv || true,
  # which keeps the batch to retry): a rename failure within one directory implies a
  # broken filesystem, and the findings also live in results.log, so a lost .last is
  # low-stakes forensic loss, not a lost digest.
  mv -f "$1" "$2.last" 2>/dev/null || rm -f "$1"
  chmod 600 "$2.last" 2>/dev/null || true
}

# digest_title <item_count> - the one-line digest header: a notebook glyph, the UTC
# date, and the item count. Sent as the CRIT title of the silent (muted) message.
digest_title() { printf '🗒️ osquery daily digest · %s · %s item(s)' "$(date -u +%Y-%m-%d)" "$1"; }

# render_digest_body <work_file> - build and print the grouped, capped,
# Discord-safe digest body from the rotated batch. Single responsibility: produce
# the body string, never send. Findings group by detector; each group renders a
# header with its true count, up to DIGEST_MAX_BULLETS_PER_GROUP bullets, then a
# "+K more" roll-up. At most DIGEST_MAX_GROUPS groups render (the rest collapse to
# an "and K more" marker), and a codepoint-wise cap inside jq truncates the whole
# body at DIGEST_MAX_BODY_CHARS with a marker (no `head -c` pipe, so no silent
# byte-cut and no SIGPIPE), well under Discord's 2000-char limit. Each field is
# truncated at DIGEST_MAX_FIELD_CHARS so one giant value cannot fill the body cap
# and crowd every other detector out. The four caps are env-overridable named
# knobs, not magic numbers, and the group cap keeps a busy day from losing whole
# trailing groups to a silent mid-line cut.
#
# Injection safety: .identity and .summary originate from findings with
# attacker-influenceable columns, so every rendered field is WRAPPED in an inline
# code span and flows through sanitize first (strip backticks so the value cannot
# close that span, squash CR/newline/tab to spaces, per-field cap), exactly the
# treatment the alerter's render-page gives these same fields. A crafted newline can
# never forge an extra markdown line or block, and a crafted mention (@everyone) or
# link renders as literal inline-code text, never a live Discord mention or link.
render_digest_body() {
  local work_file="$1"
  local max_bullets="${DIGEST_MAX_BULLETS_PER_GROUP:-10}"
  local max_groups="${DIGEST_MAX_GROUPS:-12}"
  local max_body_chars="${DIGEST_MAX_BODY_CHARS:-1800}"
  local max_field="${DIGEST_MAX_FIELD_CHARS:-240}"
  jq -rRs \
    --argjson max_bullets "$max_bullets" \
    --argjson max_groups "$max_groups" \
    --argjson max_field "$max_field" \
    --argjson max_body_chars "$max_body_chars" '
    # The single sanitize chokepoint every attacker-influenceable field passes
    # through before it is wrapped in an inline-code span below: strip backticks (so
    # a crafted value cannot close its wrapping span and escape to live markdown),
    # squash CR, newline, and tab to a space (a newline would forge an extra line),
    # and truncate at $max_field so one crafted or huge value cannot fill the body
    # cap and crowd every other detector out. Data, never structure.
    def sanitize:
      gsub("`"; "") | gsub("[\r\n\t]"; " ")
      | if length > $max_field then .[0:$max_field] + "…(truncated)" else . end;
    # One block per detector group: header with the true count, up to $max_bullets
    # bullets, then a "+K more" roll-up for the overflow, then a blank separator.
    # Each attacker-influenceable field is WRAPPED in backticks (an inline-code
    # span), exactly as the alerter render-page does, so a crafted mention or link
    # renders as literal inert text, never a live Discord @everyone or clickable link.
    # A null, missing, or numeric field is coerced first (via // and tostring), so a
    # valid-JSON wrong-shape line cannot abort the render with a non-string gsub error.
    def render_group:
      "**\(.[0].detector // "?" | tostring)** (\(length))",
      (.[0:$max_bullets][] | "- `\(.identity // "?" | tostring | sanitize)` - `\(.summary // "?" | tostring | sanitize)`"),
      (if length > $max_bullets then "… +\(length - $max_bullets) more" else empty end),
      "";
    # Parse per line and DROP a torn or malformed line (try/catch), never
    # slurp-and-abort: one interrupted digest_append must not fail the whole run
    # and drop the whole day of findings. Mirrors the resilient results.log reader.
    split("\n")
    | map(select(length > 0) | (try fromjson catch empty))
    | group_by(.detector) as $groups
    # Cap the NUMBER of groups and mark the overflow, so a busy day cannot drop whole
    # trailing groups to a silent mid-line cut (the dropped content still lives in the
    # spool/.last). Collect the whole stream into one string, then codepoint-slice it
    # to $max_body_chars with a marker: the cap lives INSIDE jq (no `| head -c`), so it
    # truncates honestly (no lost tail markers, no mid-multibyte cut) and there is no
    # pipe to block on a huge body and take SIGPIPE.
    | [ ($groups[0:$max_groups][] | render_group),
        (if ($groups | length) > $max_groups
         then "… and \(($groups | length) - $max_groups) more detector group(s) - see results.log"
         else empty end) ]
    | join("\n")
    | if length > $max_body_chars then .[0:$max_body_chars] + "\n… (truncated)" else . end
  ' "$work_file"
}

main() {
  local store work_file body item_count title
  store="${OSQUERY_DIGEST_STORE:-$OSQUERY_DIGEST_STORE_DEFAULT}"

  # Empty-suppression, first gate: an absent or zero-byte store has nothing to
  # summarize, so stay silent. -s is false for both a missing and an empty file.
  [[ -s $store ]] || return 0

  # Atomically claim the batch: move the live store aside so findings the alerter
  # appends WHILE we build land in a fresh $store for the next run instead of being
  # consumed (then rotated away) by this one. A failed mv leaves the store
  # untouched, so nothing is lost; stay silent and let the next run retry.
  work_file="$(rotated_work_file "$store")"
  mv -f "$store" "$work_file" 2>/dev/null || return 0

  # From here until the send, any build failure must restore the batch rather than
  # drop the day's digest. The send itself is fire-and-forget (a lost daily digest
  # is low-stakes and send_alert is write-ahead durable on its own), so the send
  # behavior clears this trap; it guards the BUILD only.
  trap 'restore_batch "$work_file" "$store"' ERR

  # Empty-suppression, second gate, now on the CLAIMED batch: a whitespace-only or
  # zero-byte batch has no real records, so discard it and stay silent. Reading the
  # work file (not the live store) both guards the exact batch this run claimed and
  # clears accumulated blank lines from the live store on every run.
  grep -q '[^[:space:]]' "$work_file" 2>/dev/null || {
    rm -f "$work_file"
    return 0
  }

  # Build the grouped, capped, sanitized body from the claimed batch. Under the ERR
  # trap: a render failure here restores the batch for the next run (pre-send).
  body="$(render_digest_body "$work_file")"

  # Empty-body second gate: if every line was torn/dropped the body renders empty
  # even though Guard 2 (non-whitespace bytes) passed. Do NOT send a misleading silent
  # "N item(s)" with an empty body; preserve the unrecoverable batch to .last and stay
  # silent (re-rendering it would only render empty again).
  if ! printf '%s' "$body" | grep -q '[^[:space:]]'; then
    rotate_to_last "$work_file" "$store"
    return 0
  fi

  # The item count for the title, from a torn-safe line count (grep -c, never a JSON
  # parse), so a torn line never breaks it. || guards the (here impossible) empty case.
  item_count="$(grep -c . "$work_file" 2>/dev/null)" || item_count=0
  title="$(digest_title "$item_count")"

  # Clear the pre-send restore trap: item_count and title above were the last trap-guarded build
  # steps, and the send outcome is handled EXPLICITLY below, not by the trap.
  trap - ERR

  # CRIT selects the #priority route; the EMPTY sound makes it locally silent AND threads tier=muted
  # into the POST so the Hermes adapter suppresses the ping (a digest must never notify like a page).
  # No occurrence id is threaded, so send_alert derives a per-send-unique request id (distinct days
  # never collide); the dedup is the atomic CLAIM at the start (which already emptied the live store),
  # not this rotate. send_alert returns nonzero ONLY when its write-ahead persist FAILED (the page was
  # neither delivered nor stored, "the caller must not advance its cursor past it"), so on failure
  # RESTORE the batch for the next run; restoring cannot double-store because nothing was stored. On
  # success (any STORED outcome: delivered / stored-nosecret / stored-delivery-pending) durability is
  # delegated to send_alert's own store + drainer, so rotate the batch to .last for forensics.
  if send_alert CRIT "$title" "$body" ""; then
    rotate_to_last "$work_file" "$store"
  else
    restore_batch "$work_file" "$store"
  fi
}

# Run only when executed, not when sourced: a test sources this file to exercise
# an individual step (e.g. to force a build failure and assert the ERR-trap
# restore) without launching the whole flow.
if [[ ${BASH_SOURCE[0]} == "${0}" ]]; then
  main "$@"
fi
