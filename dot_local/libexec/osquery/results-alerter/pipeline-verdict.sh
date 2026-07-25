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
# Blind spot, recorded honestly: the manifest binds CONTENT only. An
# ATTRIBUTES_MODIFIED event (for example `chmod g+w` on a pipeline script) carries
# the unchanged hash, matches its tuple, and stays silent. A mode/owner column is a
# follow-up, not part of this change.
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

# _pipeline_mtime <path>: epoch mtime, or empty when it cannot be read. GNU stat
# first (the nix shell), BSD stat as the fallback (the portable order used
# elsewhere in this feature-set).
_pipeline_mtime() {
  stat -c '%Y' "$1" 2>/dev/null || stat -f '%m' "$1" 2>/dev/null
}

# _pipeline_manifest_is_trustworthy <path>: 0 when the manifest may be trusted to
# SUPPRESS a page. The whole design rests on the manifest being root-owned and not
# group/world-writable (an attacker who could write it could self-whitelist a file
# they just tampered), so verify that rather than assume it: a permissions drift
# then degrades LOUDLY (everything pages) instead of silently (anything can be
# blessed).
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

# _pipeline_tuple_settles <path> <hash>: the tuple check, plus a bounded wait for
# an in-flight manifest regeneration. It waits only when the manifest EXISTS but is
# OLDER than the file that changed, which is exactly the apply-race shape (the file
# landed, the manifest has not been reinstalled yet). A missing manifest pages
# immediately, and a manifest newer than the target is already the final word.
_pipeline_tuple_settles() {
  local target="$1" hash_value="$2"
  _pipeline_manifest_has_tuple "$target" "$hash_value" && return 0
  local manifest="${OSQUERY_PIPELINE_MANIFEST:-$PIPELINE_MANIFEST}"
  [[ -r $manifest ]] || return 1
  local manifest_mtime target_mtime waited=0
  manifest_mtime=$(_pipeline_mtime "$manifest") || true
  target_mtime=$(_pipeline_mtime "$target") || true
  [[ -n $manifest_mtime && -n $target_mtime ]] || return 1
  ((manifest_mtime < target_mtime)) || return 1
  while ((waited < OSQUERY_PIPELINE_SETTLE_SECONDS)); do
    sleep 1
    waited=$((waited + 1))
    _pipeline_manifest_has_tuple "$target" "$hash_value" && return 0
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
pipeline_verdict() {
  local target="$1" hash_value="$2" verb="$3" disk_hash
  # Not pipeline infrastructure -> a neighbor in the watched dir, log-only.
  _pipeline_is_tracked "$target" || return 1
  # A destructive removal of a tracked file has no bytes to confirm -> always page.
  [[ $verb == DELETED ]] && return 0
  # A non-empty EVENT hash (CREATED/UPDATED carry one): validate the exact
  # (path, hash) tuple directly. No manifest -> not-found -> page (fail-safe).
  if [[ -n $hash_value ]]; then
    _pipeline_tuple_settles "$target" "$hash_value" && return 1
    return 0
  fi
  # Empty event hash: the live atomic-rename shape (chezmoi writes via rename, and
  # osquery does not content-hash a rename). Debounce briefly - the rename may
  # still be settling - then re-hash the on-disk target and check its (path, hash)
  # tuple. A known-good deployed file matches the same-apply manifest -> silent; a
  # mismatch, a missing file, or no manifest -> page.
  sleep "${OSQUERY_PIPELINE_REHASH_DELAY:-0.3}"
  disk_hash=$(shasum -a 256 "$target" 2>/dev/null | awk '{print $1}')
  [[ -n $disk_hash ]] && _pipeline_tuple_settles "$target" "$disk_hash" && return 1
  return 0
}
