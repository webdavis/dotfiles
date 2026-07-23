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

# restore_batch <work_file> <store> - put the claimed batch back as the live store
# so the next daily run retries it. This is the ERR trap's action for a build
# failure BEFORE the send: a silently dropped digest is invisible to this single
# user, so a failed build must leave the findings for another run, not lose them.
restore_batch() { mv -f "$1" "$2" 2>/dev/null || true; }

# render_digest_body <work_file> - build and print the grouped, capped,
# Discord-safe digest body from the rotated batch. Single responsibility: produce
# the body string, never send. Findings group by detector; each group renders a
# header with its true count, up to DIGEST_MAX_BULLETS_PER_GROUP bullets, then a
# "+K more" roll-up. At most DIGEST_MAX_GROUPS groups render (the rest collapse to
# an "and K more" marker), and a hard head -c backstop caps the whole body at
# DIGEST_MAX_BODY_CHARS, well under Discord's 2000-char limit. Each field is
# truncated at DIGEST_MAX_FIELD_CHARS so one giant value cannot fill the body cap
# and crowd every other detector out. The four caps are env-overridable named
# knobs, not magic numbers, and the group cap keeps a busy day from losing whole
# trailing groups to a silent mid-line cut.
#
# Injection safety: .identity and .summary originate from findings with
# attacker-influenceable columns, so every rendered field flows through sanitize
# (strip backticks, squash CR/newline/tab to spaces), the same chokepoint the
# alerter's render-page uses. A crafted newline or backtick therefore stays inert
# inside its own bullet and can never forge an extra markdown line or block.
render_digest_body() {
  local work_file="$1"
  local max_bullets="${DIGEST_MAX_BULLETS_PER_GROUP:-10}"
  local max_groups="${DIGEST_MAX_GROUPS:-12}"
  local max_body_chars="${DIGEST_MAX_BODY_CHARS:-1800}"
  local max_field="${DIGEST_MAX_FIELD_CHARS:-240}"
  jq -rRs \
    --argjson max_bullets "$max_bullets" \
    --argjson max_groups "$max_groups" \
    --argjson max_field "$max_field" '
    # The single sanitize chokepoint every attacker-influenceable field passes
    # through: strip backticks (they open an inline-code span), squash CR, newline,
    # and tab to a space (a newline would break the value out of its bullet into a
    # forged line), and truncate at $max_field so one crafted or huge value cannot
    # fill the body cap and crowd every other detector out. Data, never structure.
    def sanitize:
      gsub("`"; "") | gsub("[\r\n\t]"; " ")
      | if length > $max_field then .[0:$max_field] + "…(truncated)" else . end;
    # One block per detector group: header with the true count, up to $max_bullets
    # bullets, then a "+K more" roll-up for the overflow, then a blank separator.
    def render_group:
      "**\(.[0].detector)** (\(length))",
      (.[0:$max_bullets][] | "- \(.identity | sanitize) - \(.summary | sanitize)"),
      (if length > $max_bullets then "… +\(length - $max_bullets) more" else empty end),
      "";
    split("\n")
    | map(select(length > 0) | fromjson)
    | group_by(.detector) as $groups
    # Cap the NUMBER of groups and mark the overflow, so a busy day cannot drop
    # whole trailing groups to a silent mid-line head -c cut (the dropped content
    # still lives in the spool/.last). head -c below is the final hard backstop.
    | ($groups[0:$max_groups][] | render_group),
      (if ($groups | length) > $max_groups
       then "… and \(($groups | length) - $max_groups) more detector group(s) - see results.log"
       else empty end)
  ' "$work_file" | head -c "$max_body_chars"
}

main() {
  local store work_file
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

  render_digest_body "$work_file"
}

# Run only when executed, not when sourced: a test sources this file to exercise
# an individual step (e.g. to force a build failure and assert the ERR-trap
# restore) without launching the whole flow.
if [[ ${BASH_SOURCE[0]} == "${0}" ]]; then
  main "$@"
fi
