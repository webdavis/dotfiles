#!/usr/bin/env bash
#
# pipeline-verdict.sh - a sourced helper for results-alerter.sh. Functions only,
# no main. It answers one question for a file change under the watched pipeline
# directories: is this a tamper to PAGE, a known-good apply to stay SILENT, or an
# untracked neighbor to log only?
#
# The pipeline-integrity manifest is a root-owned, source-derived sha256 list of
# the alerter's own scripts and plists (shasum format: "<sha256>  <path>"). A
# change is legitimate only when its EXACT (path, sha256) tuple is in the
# manifest; a legit chezmoi apply regenerates the manifest in the same apply, so
# a deployed known-good file matches and stays silent.
#
# Fail-open (criterion 6): the manifest slice is LAST in the stack and does not
# exist yet. With no manifest present, _pipeline_manifest_has_tuple returns
# not-found, so a tracked change cannot be confirmed legitimate and PAGES. That
# is the conservative direction - a pipeline-script change is never silently
# suppressed without a manifest to justify it, and a missing/empty/mismatched
# hash pages too. Over-paging until the manifest lands is the accepted tradeoff.
#
# Return-code contract (from c69baab _pipeline_verdict):
#   0 = PAGE   (tamper / cannot confirm legit / no manifest / delete)
#   1 = SILENT (an untracked neighbor, or an exact (path, sha256) manifest match)

PIPELINE_MANIFEST="${OSQUERY_PIPELINE_MANIFEST:-/var/osquery/pipeline-known-good.sha256}"

# _pipeline_manifest_has_tuple <path> <hash>: 0 when the manifest holds a line
# binding exactly this hash to exactly this path, else 1. Legitimacy is the EXACT
# (path, sha256) tuple, not the hash alone: binding the hash to ITS path defeats a
# swap-in-place (a valid hash lifted onto a different tracked path). Hashes are
# compared case-insensitively (shasum and osquery emit lowercase, but normalize
# defensively). A missing/unreadable manifest returns 1 - the fail-open hinge.
_pipeline_manifest_has_tuple() {
  local manifest="${OSQUERY_PIPELINE_MANIFEST:-$PIPELINE_MANIFEST}"
  [[ -r $manifest ]] || return 1
  local want_path="$1" want_hash h p
  want_hash=$(printf '%s' "$2" | tr '[:upper:]' '[:lower:]')
  while read -r h p; do
    h=$(printf '%s' "$h" | tr '[:upper:]' '[:lower:]')
    [[ $h == "$want_hash" && $p == "$want_path" ]] && return 0
  done <"$manifest"
  return 1
}

# _pipeline_is_tracked <target>: 0 when the path is pipeline infrastructure. The
# watches fire for every file in a watched dir, so the tracked set is filtered
# here: a script under either watched script dir (~/.local/libexec/osquery or
# ~/.local/bin), or one of our own osquery LaunchAgents by basename. Anything else
# in a watched dir is a neighbor. Dual-prefix: c69baab only knew the flat
# ~/.local/bin/osquery-*.sh layout; the scripts now live under libexec/osquery too
# (with the osquery- prefix dropped), and ~/.local/bin is a root-of-trust dir.
_pipeline_is_tracked() {
  local target="$1" base="${1##*/}"
  case "$target" in
    "$HOME"/.local/libexec/osquery/* | "$HOME"/.local/bin/*) return 0 ;;
  esac
  case "$base" in
    com.webdavis.osquery-*.plist) return 0 ;;
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
  # (path, hash) tuple directly. No manifest -> not-found -> page (fail-open).
  if [[ -n $hash_value ]]; then
    _pipeline_manifest_has_tuple "$target" "$hash_value" && return 1
    return 0
  fi
  # Empty event hash: the live atomic-rename shape (chezmoi writes via rename, and
  # osquery does not content-hash a rename). Debounce briefly - the rename may
  # still be settling - then re-hash the on-disk target and check its (path, hash)
  # tuple. A known-good deployed file matches the same-apply manifest -> silent; a
  # mismatch, a missing file, or (the current state) no manifest -> page.
  sleep "${OSQUERY_PIPELINE_REHASH_DELAY:-0.3}"
  disk_hash=$(shasum -a 256 "$target" 2>/dev/null | awk '{print $1}')
  [[ -n $disk_hash ]] && _pipeline_manifest_has_tuple "$target" "$disk_hash" && return 1
  return 0
}
