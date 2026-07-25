#!/usr/bin/env bash
#
# pipeline-verdict.sh - a sourced helper for results-alerter.sh. Functions only,
# no main. It answers one question for a file change under the watched pipeline
# directories: is this a tamper to PAGE, a known-good apply to stay SILENT, or an
# untracked neighbor to log only?
#
# The pipeline-integrity manifest is a root-owned sha256 list of the alerter's own
# scripts and plists (shasum format: "<sha256>  <path>"), regenerated on every
# apply from chezmoi's managed intent. A change is legitimate only when its EXACT
# (path, sha256) tuple is in the manifest, so a deployed known-good file matches
# and stays silent while a tampered one does not.
#
# Fail-safe (criterion 6): a missing, unreadable, or untrustworthy manifest makes
# _pipeline_manifest_has_tuple return not-found, so a tracked change that cannot be
# confirmed legitimate PAGES. A pipeline-script change is never silently suppressed
# without a manifest tuple to justify it, and a missing/empty/mismatched hash pages
# too.
#
# Blind spots, recorded honestly. This check judges EVENTS about the deployed tree,
# so it is bounded by what generates events and by what the manifest binds:
#
#   - CONTENT ONLY. An ATTRIBUTES_MODIFIED event (for example `chmod g+w` on a
#     pipeline script) carries unchanged content, matches its tuple, and stays
#     silent. A mode/owner column is a follow-up.
#   - HARD LINKS / EVENT GENERATION. The watch is path-based, so an attacker who
#     hard-links a manifested script to a writable path outside the pipeline home
#     can overwrite the SAME INODE through the outside alias: the filesystem event
#     names that path, nothing fires for the watched one, and no verdict runs at
#     all. This is a property of path-based file-integrity monitoring, not of the
#     judgment below (before the manifest existed, no event meant no page either).
#     Closing it needs a PERIODIC CONTENT AUDIT that hashes every manifested path on
#     a schedule and compares, which is a scheduled feature in its own right; the
#     symlink and regular-file checks below are the cheap partial. Filed as a
#     follow-up (the uptime watchdog is its natural home: it already runs every
#     15 minutes).
#   - SOURCE COMPROMISE. The manifest is generated from chezmoi's source state,
#     which is user-writable, so tampering with a managed file's SOURCE and letting
#     a legitimate apply deploy it is signed as known-good. See the runner's
#     docblock: this layer buys post-deployment integrity, not source integrity.
#
# Return-code contract (from c69baab _pipeline_verdict):
#   0 = PAGE   (tamper / cannot confirm legit / no manifest / delete)
#   1 = SILENT (an untracked neighbor, or an exact (path, sha256) manifest match)

# Keep this default in sync with the manifest path in
# .chezmoiscripts/run_after_05-osquery-pipeline-manifest.sh (the producer). A test
# pins the two literals equal, because the producer and the consumer of a
# security-critical file must not agree only by copy-paste.
PIPELINE_MANIFEST="${OSQUERY_PIPELINE_MANIFEST:-/var/osquery/pipeline-known-good.sha256}"

# How long to let an in-flight manifest regeneration settle before paging on a
# tuple miss (seconds). The alerter is WatchPaths-triggered and judges a finding
# exactly once, so a change observed in the window between the file landing and the
# manifest being reinstalled would page a false CRIT that is never reconsidered.
# Bounded and small: a real tamper is delayed by at most this, and still pages.
OSQUERY_PIPELINE_SETTLE_SECONDS="${OSQUERY_PIPELINE_SETTLE_SECONDS:-5}"

# The settle budget is spent ONCE PER ALERTER INVOCATION, not once per finding.
# route_findings judges findings sequentially while the alerter holds its
# single-instance lock, and a contended WatchPaths invocation exits without
# processing, so a per-finding wait would let anyone who creates N files under the
# tracked home stall the pipeline for N times the bound and delay UNRELATED
# security findings. The first miss opens the window; once it has elapsed every
# later miss is answered immediately. The alerter sources this file per run, so the
# deadline starts empty each invocation.
_pipeline_settle_deadline=""

# _pipeline_now: current epoch seconds, without a fork where bash 5 provides it.
_pipeline_now() {
  printf '%s' "${EPOCHSECONDS:-$(date +%s)}"
}

# _pipeline_mtime <path>: epoch mtime, or empty when it cannot be read. GNU stat
# first (the nix shell), BSD stat as the fallback (the portable order used
# elsewhere in this feature-set).
_pipeline_mtime() {
  stat -c '%Y' "$1" 2>/dev/null || stat -f '%m' "$1" 2>/dev/null
}

# _pipeline_manifest_is_trustworthy <path>: 0 when the manifest may be trusted to
# SUPPRESS a page. Whoever can write the manifest can self-whitelist a file they
# just tampered, so root ownership and a not-group/world-writable mode are VERIFIED
# here rather than assumed: a permissions drift then degrades LOUDLY (everything
# pages) instead of silently (anything can be blessed).
#
# What that ownership actually buys, stated without overclaiming. It stops a process
# that stays at the user's privilege level, which cannot rewrite a root-owned 0644
# file, and it turns a drifted mode into loud pages. It is NOT a boundary against a
# determined user-level attacker on a host with passwordless sudo, which this one
# is: a process running as the operator can escalate with no prompt and rewrite the
# manifest at will, whitelisting whatever it just tampered. Requiring a password
# would be the only thing that changed that, and every unattended chezmoi script
# depends on the current configuration, so this is a recorded limit of the layer,
# not a defect to fix here. The honest summary is a raised bar plus a loud failure
# mode, not an integrity guarantee that survives a user-level attacker.
#
# Test seam: the check applies only at the PRODUCTION DEFAULT path. Tests point
# OSQUERY_PIPELINE_MANIFEST at a fixture manifest they own, which is by definition
# neither root-owned nor at the protected path, so an explicit override skips the
# check. Production never sets that variable.
_pipeline_manifest_is_trustworthy() {
  [[ -n ${OSQUERY_PIPELINE_MANIFEST:-} ]] && return 0
  local owner mode
  owner=$(stat -c '%u' "$1" 2>/dev/null || stat -f '%u' "$1" 2>/dev/null) || true
  mode=$(stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1" 2>/dev/null) || true
  [[ $owner == 0 ]] || return 1
  # Refuse a group- or world-writable manifest. The mode is OCTAL, so both operands
  # are read base 8; an unreadable mode defaults to 777 and is refused.
  ((8#${mode:-777} & 8#22)) && return 1
  return 0
}

# _pipeline_manifest_has_tuple <path> <hash>: 0 when the manifest holds a line
# binding exactly this hash to exactly this path, else 1. Legitimacy is the EXACT
# (path, sha256) tuple, not the hash alone: binding the hash to ITS path defeats a
# swap-in-place (a valid hash lifted onto a different tracked path). Hashes are
# compared case-insensitively via bash case-folding (no forks): shasum and osquery
# both emit lowercase, so this is documented defense-in-depth against a future
# producer, kept because it now costs nothing. A missing, unreadable, or
# untrustworthy manifest returns 1 - the fail-safe hinge.
_pipeline_manifest_has_tuple() {
  local manifest="${OSQUERY_PIPELINE_MANIFEST:-$PIPELINE_MANIFEST}"
  [[ -r $manifest ]] || return 1
  _pipeline_manifest_is_trustworthy "$manifest" || return 1
  local want_path="$1" want_hash="${2,,}" h p
  # `|| [[ -n $h ]]` so a final line with no trailing newline is still examined.
  while read -r h p || [[ -n $h ]]; do
    [[ ${h,,} == "$want_hash" && $p == "$want_path" ]] && return 0
  done <"$manifest"
  return 1
}

# _pipeline_content_is_known_good <path>: hash the file AS IT IS NOW and require
# that current content to be the manifest's tuple for this exact path. Re-reading
# here (rather than trusting the digest osquery recorded when the event fired) is
# what closes the swap-after-the-event race: an attacker could otherwise let a
# known-good event be recorded, replace the file, run it, and restore it before the
# next collection, and the stale digest would have vouched for bytes that were not
# on disk when the decision was made. A file that cannot be hashed returns 1, so it
# pages.
_pipeline_content_is_known_good() {
  local target="$1" disk_hash
  disk_hash=$(shasum -a 256 "$target" 2>/dev/null | awk '{print $1}')
  [[ -n $disk_hash ]] || return 1
  _pipeline_manifest_has_tuple "$target" "$disk_hash"
}

# _pipeline_tuple_settles <path>: the current-content check, plus a bounded wait for
# an in-flight manifest regeneration. It waits only when the manifest EXISTS but is
# OLDER than the file that changed, which is exactly the apply-race shape (the file
# landed, the manifest has not been reinstalled yet). A missing manifest pages
# immediately, and a manifest newer than the target is already the final word. The
# content is re-read on every retry, so a file still settling is judged on what it
# finally holds, not on a mid-write snapshot.
_pipeline_tuple_settles() {
  local target="$1"
  _pipeline_content_is_known_good "$target" && return 0
  local manifest="${OSQUERY_PIPELINE_MANIFEST:-$PIPELINE_MANIFEST}"
  [[ -r $manifest ]] || return 1
  local manifest_mtime target_mtime
  manifest_mtime=$(_pipeline_mtime "$manifest") || true
  target_mtime=$(_pipeline_mtime "$target") || true
  [[ -n $manifest_mtime && -n $target_mtime ]] || return 1
  ((manifest_mtime < target_mtime)) || return 1
  # Open the shared window on the first miss of this invocation; a later miss
  # inherits whatever is left of it, and answers at once when it is spent.
  [[ -n $_pipeline_settle_deadline ]] ||
    _pipeline_settle_deadline=$(($(_pipeline_now) + OSQUERY_PIPELINE_SETTLE_SECONDS))
  while (($(_pipeline_now) < _pipeline_settle_deadline)); do
    sleep 1
    _pipeline_content_is_known_good "$target" && return 0
  done
  return 1
}

# _pipeline_is_tracked <target>: 0 when the path is pipeline infrastructure. The
# watches fire for every file in a watched dir, so the tracked set is filtered
# here: a file under the dedicated pipeline home (~/.local/libexec/osquery, where
# the whole osquery delivery path lives), or one of OUR OWN osquery LaunchAgents.
#
# The plist arm is anchored to $HOME/Library/LaunchAgents, not matched by basename
# anywhere: the watch also covers /Library/LaunchAgents and /Library/LaunchDaemons,
# and the manifest only ever covers the user agents chezmoi manages, so a bare
# basename match would track a com.webdavis.osquery-*.plist under /Library that the
# manifest can never contain - a watched-but-unmanifested file that pages forever.
# A rogue /Library plist instead falls through to the persistence detector, which
# default-denies it. This keeps the tracked set, the manifest's coverage, and the
# osquery.conf watch on the identical file set.
#
# ~/.local/bin is NOT tracked: those operator tools are the Relay/shell-notifier
# subsystem's, not osquery pipeline files, and the manifest never covers them, so a
# bin edit is a silent neighbor here, never a pipeline tamper.
_pipeline_is_tracked() {
  local target="$1"
  case "$target" in
    "$HOME"/.local/libexec/osquery/*) return 0 ;;
    "$HOME"/Library/LaunchAgents/com.webdavis.osquery-*.plist) return 0 ;;
  esac
  return 1
}

# pipeline_verdict <target> <event_hash> <verb>: 0 = page, 1 = silent.
#
# The event digest is NOT a trust input. It is used only to recognize the
# atomic-rename shape (osquery does not content-hash a rename, so that event
# arrives with an empty hash and the write may still be settling). Every
# suppression decision is made against the file's CURRENT content.
pipeline_verdict() {
  local target="$1" hash_value="$2" verb="$3"
  # Not pipeline infrastructure -> a neighbor in the watched dir, log-only.
  _pipeline_is_tracked "$target" || return 1
  # A destructive removal of a tracked file has no bytes to confirm -> always page.
  [[ $verb == DELETED ]] && return 0
  # A manifested path must hold a REGULAR FILE, and links are never followed: a
  # symlink standing where a pipeline script belongs would otherwise be hashed
  # through to content the manifest vouches for while the executed bytes live
  # somewhere the manifest does not cover.
  [[ -L $target ]] && return 0
  [[ -f $target ]] || return 0
  # The atomic-rename shape: give the rename a moment to land before hashing. Only
  # on that shape, so a normal event adds no latency.
  [[ -n $hash_value ]] || sleep "${OSQUERY_PIPELINE_REHASH_DELAY:-0.3}"
  _pipeline_tuple_settles "$target" && return 1
  return 0
}
