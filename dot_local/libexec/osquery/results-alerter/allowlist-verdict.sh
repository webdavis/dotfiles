#!/usr/bin/env bash
#
# allowlist-verdict.sh - a sourced helper for results-alerter.sh. Functions only,
# no main. It answers one question for a user LaunchAgent persistence finding: is
# this a known-good item (suppress), a reused allowlisted label pointing at a
# different plist identity (page), or simply not allowlisted?
#
# The allowlist is the launchd page-allowlist: OSQUERY_LAUNCHD_ALLOWLIST (the
# unified env var name, matching the slice-5 writer osquery-allowlist.sh), an
# NDJSON file of {label, path, program, sha256} tuples, one per line, default
# ~/.config/osquery/page-launchd-allowlist.txt. Paths/programs are stored
# home-relative (~/) so the committed seed file stays user-agnostic; the verdict
# expands ~ to $HOME before comparing to the finding's absolute path/program.
#
# The finding supplies (label, path, program). The plist sha256 is NOT an
# argument and is NOT read from the osquery row or from enrichment: when a stored
# tuple pins a hash, the verdict re-hashes the ON-DISK plist at the finding's path
# with shasum at decision time. That binds the allowlist entry to the plist's
# current bytes, defeating a same-label/same-path/same-program rewrite.

# _allowlist_verdict_expand_home: expand a leading ~/ in a stored value to $HOME/.
# Namespaced so it does not collide with the other sourced helpers.
_allowlist_verdict_expand_home() { printf '%s' "${1//\~\//$HOME/}"; }

# allowlist_verdict <label> <path> <program>:
#   0 = suppress (full tuple match; an empty stored sha256 skips only the hash
#       dimension, the own-agent seed entries)
#   2 = reused label -> page (the label is allowlisted but path/program diverges,
#       or the pinned hash no longer matches the on-disk plist)
#   1 = not allowlisted (no label match, a degraded label-only entry that cannot
#       vouch, or a missing/unreadable allowlist file)
allowlist_verdict() {
  local want_label="$1" want_path="$2" want_program="$3"
  local file="${OSQUERY_LAUNCHD_ALLOWLIST:-$HOME/.config/osquery/page-launchd-allowlist.txt}"
  local match jpath jprog jhash disk_hash
  [[ -r $file ]] || return 1
  # Pull the FIRST tuple whose label matches, in one pass. `-R` reads each line as
  # a raw string and `fromjson?` parses it, the `?` dropping any line that is not
  # JSON (comments, blanks) instead of aborting - so one jq handles the whole file.
  # The tuple's path/program/sha256 come back tab-separated (empty for absent).
  match=$(jq -rR --arg want "$want_label" \
    'fromjson? | select(.label == $want) | [.path // "", .program // "", .sha256 // ""] | @tsv' \
    "$file" 2>/dev/null | head -n1)
  # No line matched the label. (A degraded label-only entry, below, also returns 1,
  # so no-match and cannot-vouch are the same not-allowlisted outcome.)
  [[ -n $match ]] || return 1
  IFS=$'\t' read -r jpath jprog jhash <<<"$match"
  jpath=$(_allowlist_verdict_expand_home "$jpath")
  jprog=$(_allowlist_verdict_expand_home "$jprog")
  # A degraded label-only entry (no captured identity) cannot vouch for a program.
  # Do NOT suppress on the bare label (that is the R2-1 bug); fail safe as absent.
  [[ -n $jpath && -n $jprog ]] || return 1
  # The tuple must match on path AND program; any divergence is a reused label.
  [[ $want_path == "$jpath" && $want_program == "$jprog" ]] || return 2
  # When the entry pins the plist hash, the on-disk plist must still match it
  # (defeats a same-path/same-program plist rewrite). An empty pin skips this
  # dimension, which is how the own-agent seed entries are stored.
  if [[ -n $jhash ]]; then
    disk_hash=$(shasum -a 256 "$want_path" 2>/dev/null | awk '{print $1}')
    [[ $disk_hash == "$jhash" ]] || return 2
  fi
  return 0 # full tuple match -> known-good, suppress
}
