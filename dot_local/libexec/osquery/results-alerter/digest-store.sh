#!/usr/bin/env bash
#
# digest-store.sh - a sourced helper for results-alerter.sh. Functions only, no
# main. It owns the digest tier's write side: suspicious-but-ambiguous findings
# that do not page accumulate here as NDJSON, one line each, for a later daily
# grouped summary. The read side is a separate slice; this helper only appends.
#
# Best-effort by design: failing to record a digest line must never abort
# detection, so every step is guarded and the function always returns success.
#
# Privacy posture (the F5-A lesson - secret digests must not spread to readable
# files): the line carries only DERIVED triage fields (timestamp, detector,
# category, identity, action, summary). It never copies the whole columns object,
# so a raw sha256 or a secret column never reaches the spool. Full filesystem
# paths ARE stored - the digest is a private single-user triage view where the
# full path disambiguates (which .env?) - so the spool must not be world-readable:
# dir 700, file 600, the way the page spool is.

OSQUERY_DIGEST_STORE_DEFAULT="$HOME/.local/state/osquery-digest-spool/digest.ndjson"

# digest_append <finding-json>: append one NDJSON digest line for the finding.
digest_append() {
  local finding="$1"
  local store="${OSQUERY_DIGEST_STORE:-$OSQUERY_DIGEST_STORE_DEFAULT}"
  local dir
  dir="$(dirname "$store")"
  mkdir -p "$dir" 2>/dev/null || true
  # Set the dir to 700 BEFORE any file exists, so even the brief window before the
  # file's own 600 mode is applied is covered by an unreadable parent directory.
  chmod 700 "$dir" 2>/dev/null || true
  jq -c --arg timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{
      timestamp: $timestamp,
      detector: .q,
      category: (.cols.category // ""),
      identity: (if .q == "listening_ports_non_loopback"
                 then ((.cols.name // .cols.path // "?") + " " + (.cols.address // "?") + ":" + (.cols.port // "?"))
                 else (.cols.label // .cols.identifier // .cols.target_path // .cols.path // .cols.username // "?") end),
      action: .act,
      summary: (.q + " " + ((.cols.label // .cols.identifier // .cols.target_path // .cols.path // .cols.username) // "?"))
    }' <<<"$finding" >>"$store" 2>/dev/null || true
  chmod 600 "$store" 2>/dev/null || true
}
