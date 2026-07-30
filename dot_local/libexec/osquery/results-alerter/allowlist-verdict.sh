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

# _allowlist_path_is_manifest_vouched <path>: 0 when the root-owned
# pipeline-integrity manifest vouches for the file at THIS path as it stands right
# now (content, mode and owner), else 1.
#
# Asked about TWO different paths, for two different reasons: the allowlist file
# itself (is this list trustworthy at all), and an individual entry's plist when
# that entry carries no pin of its own (does the manifest vouch for these bytes).
# Same question, same comparison, so it is one function rather than two.
#
# WHY THE VERDICT ASKS THIS AT ALL. The allowlist is deployed user-writable, and
# under default-deny an entry in it SUPPRESSES a persistence page. A process
# running as the operator could therefore append a tuple naming its own
# LaunchAgent and silence the page for it. Manifesting the file makes that edit
# page, but a page after the fact is nearly worthless for a component whose whole
# job is to suppress: by the time the alert lands the attacker has already had the
# silent window they wanted. Refusing to honor an allowlist nothing can vouch for
# is what turns detection into prevention - the appended tuple simply does not
# work, and the agent it was meant to hide pages on its own merits.
#
# The check is reused, never reimplemented: _pipeline_tuple_settles is the same
# comparison the file_events verdict makes, including the bounded wait for an
# in-flight manifest regeneration. That wait is what keeps a legitimate apply from
# false-paging in the window between the new allowlist landing and the manifest
# being reinstalled a moment later. The refusal to judge anything but a regular
# file rides along too, because it lives in _pipeline_deployed_state_is_known_good,
# the state reader that comparison re-runs on every retry - not in this caller. A
# symlink standing at the asked-about path is therefore never hashed through to a
# referent's pristine bytes, here or in any other consumer.
#
# Checked by NAME rather than assumed. results-alerter.sh sources both helpers, so
# a missing pipeline verdict aborts the alerter and this branch is unreachable in
# production; a partial install must still fail toward paging rather than toward a
# quiet suppression nobody can account for.
_allowlist_path_is_manifest_vouched() {
  declare -F _pipeline_tuple_settles >/dev/null 2>&1 || return 1
  _pipeline_tuple_settles "$1"
}

# allowlist_verdict <label> <path> <program>:
#   0 = suppress (full tuple match, FROM AN ALLOWLIST THE PIPELINE-INTEGRITY
#       MANIFEST VOUCHES FOR; an empty stored sha256, the own-agent seed
#       entries, carries no per-entry hash evidence, so the manifest must
#       vouch for the plist bytes instead before it can suppress anything)
#   2 = reused label -> page (the label is allowlisted but path/program diverges,
#       or the pinned hash no longer matches the on-disk plist)
#   1 = not allowlisted (no label match, a degraded label-only entry that cannot
#       vouch, a missing/unreadable allowlist file, or an allowlist the manifest
#       cannot vouch for - see _allowlist_path_is_manifest_vouched)
allowlist_verdict() {
  local want_label="$1" want_path="$2" want_program="$3"
  local file="${OSQUERY_LAUNCHD_ALLOWLIST:-$HOME/.config/osquery/page-launchd-allowlist.txt}"
  local match_json jpath jprog jhash disk_hash
  [[ -r $file ]] || return 1
  # Pull the FIRST tuple whose label matches, as a JSON OBJECT, in one pass. `-R`
  # reads each line as a raw string and `fromjson?` parses it, the `?` dropping any
  # line that is not JSON (comments, blanks) instead of aborting - so one jq handles
  # the whole file. The tuple fields are then extracted per-field from that object
  # (below), never through an in-band delimiter: a value can hold any byte, and
  # per-field extraction keeps each opaque so a crafted stored value cannot shift
  # the path/program/sha256 boundaries.
  match_json=$(jq -Rc --arg want "$want_label" \
    'fromjson? | select(.label == $want)' \
    "$file" 2>/dev/null | head -n1)
  # No line matched the label. (A degraded label-only entry, below, also returns 1,
  # so no-match and cannot-vouch are the same not-allowlisted outcome.)
  [[ -n $match_json ]] || return 1
  jpath=$(_allowlist_verdict_expand_home "$(jq -r '.path // ""' <<<"$match_json")")
  jprog=$(_allowlist_verdict_expand_home "$(jq -r '.program // ""' <<<"$match_json")")
  jhash=$(jq -r '.sha256 // ""' <<<"$match_json")
  # A degraded label-only entry (no captured identity) cannot vouch for a program.
  # Do NOT suppress on the bare label (that is the R2-1 bug); fail safe as absent.
  [[ -n $jpath && -n $jprog ]] || return 1
  # The tuple must match on path AND program; any divergence is a reused label.
  [[ $want_path == "$jpath" && $want_program == "$jprog" ]] || return 2
  # The bytes at the path must be accounted for, by one of two authorities.
  #
  # A PINNED entry carries its own: the on-disk plist must still hash to the pin,
  # which defeats a same-path/same-program rewrite. That is how third-party agents
  # captured by `allowlist.sh -a` are held.
  #
  # An UNPINNED entry is an own-agent seed. It has no pin because chezmoi rewrites
  # those plists on every apply, so a pin recorded once is wrong by the next one.
  # What vouches for them instead is the root-owned manifest, which chezmoi
  # refreshes in the same breath. Until now nothing ASKED it: an empty pin simply
  # skipped the dimension, so "no pin because the manifest covers this" and "no pin
  # at all" were the same thing, and a rewritten own-agent plist was suppressed.
  #
  # Note this is deliberately not a presence check. The plist is listed in the
  # manifest whether or not it has since been edited, so membership would vouch for
  # exactly the bytes an attacker just replaced. The comparison is on current
  # content, mode and owner.
  if [[ -n $jhash ]]; then
    disk_hash=$(shasum -a 256 "$want_path" 2>/dev/null | awk '{print $1}')
    [[ $disk_hash == "$jhash" ]] || return 2
  else
    # 1, not 2. An unpinned entry the manifest cannot vouch for is a DEGRADED
    # entry: it carries no evidence of its own and its backing authority has
    # nothing to say, so it cannot vouch for this finding at all. That is the same
    # answer the label-only entry above gets, and it keeps 2 meaning what it means
    # everywhere else, that a specific identity DIVERGED.
    _allowlist_path_is_manifest_vouched "$want_path" || return 1
  fi
  # LAST GATE, and only on the suppress path. Everything above has agreed this
  # entry vouches for the finding; what is left is whether the FILE that entry came
  # from is itself accounted for. An allowlist the root-owned manifest cannot
  # confirm is treated as not-allowlisted (1), so the finding pages.
  #
  # Placed here rather than at the top of the function on purpose. It is the only
  # outcome the binding can change: a reused label (2) and a miss (1) both page
  # already, and gating them too would spend a hash and a stat on every finding to
  # reach an answer that was never in doubt.
  _allowlist_path_is_manifest_vouched "$file" || return 1
  return 0 # full tuple match, from an allowlist the manifest vouches for -> suppress
}
