#!/usr/bin/env bash
#
# pipeline-verdict.sh - a sourced helper for results-alerter.sh. Functions only,
# no main. It answers one question for a file change under the watched pipeline
# directories: is this a tamper to PAGE, a known-good apply to stay SILENT, or an
# untracked neighbor to log only?
#
# TWO known-good manifests, each root-owned and regenerated on every apply from
# chezmoi's managed intent. One whitespace-separated tuple per line, PATH LAST so a
# path containing spaces is still read whole:
#
#   <sha256> <mode> <uid> <path>
#
#   pipeline-known-good.sha256     the alerter's own scripts and plists. The
#                                  monitor's body: everything under the pipeline
#                                  home is tracked, so a planted file pages.
#   managed-bin-known-good.sha256  the chezmoi-managed scripts under ~/.local/bin.
#                                  Not pipeline files, but update-skills.sh,
#                                  homebrew-weekly-upgrade.sh and the claude-*
#                                  hooks run unattended from LaunchAgents and shell
#                                  hooks, so a tamper there executes on a timer.
#                                  Only manifested paths are tracked, because the
#                                  same directory holds self-updating third-party
#                                  shims that chezmoi cannot vouch for.
#
# The manifests are separate files on purpose: the pipeline manifest's single
# responsibility is the pipeline's own integrity, and neither list can vouch for
# the other's paths, because _pipeline_manifest_for picks exactly one per target.
#
# A change is legitimate only when the file's CURRENT content hash, mode AND owner
# all equal the columns bound to its exact path, so a deployed known-good file
# matches and stays silent while a tampered one does not. Binding mode and owner is
# what makes a `chmod g+w` or a chown page: osquery reports those as
# ATTRIBUTES_MODIFIED events carrying the file's unchanged digest, which a
# content-only manifest matched and suppressed.
#
# Fail-safe (criterion 6): a missing, unreadable, or untrustworthy manifest makes
# _pipeline_manifest_has_tuple return not-found, so a tracked change that cannot be
# confirmed legitimate PAGES. A tracked change is never silently suppressed without
# a manifest tuple to justify it, and a missing/empty/mismatched hash pages too. On
# the bin arm the same rule reaches one step further back: a broken managed-bin
# manifest makes every bin path TRACKED (see _managed_bin_is_tracked), so a monitor
# whose known-good list failed gets louder rather than un-watching the directory.
#
# COVERAGE MAP. Two layers enforce the manifest. They compare the SAME three
# columns and differ in what TRIGGERS them, so for any given tamper this says which
# one catches it, and how fast.
#
#   LAYER 1, this file, at EVENT time. Judges a filesystem event against the full
#   tuple, so it catches CONTENT, PERMISSION and OWNERSHIP drift, but only on a
#   change that produces an event on a watched path. It is the fast answer:
#   whatever fires an event is judged within seconds.
#
#   LAYER 2, the PERIODIC MANIFEST AUDIT in ../pipeline-audit.sh, which the uptime
#   watchdog runs every 15 minutes. Re-reads every manifested path on a schedule and
#   compares all three columns itself, so it catches drift that produces NO EVENT AT
#   ALL and that layer 1 therefore never sees. The two shapes that matters for are
#   hard links and symlink referents: the watch is path-based, so an attacker who
#   hard-links a manifested script to a writable path outside the pipeline home acts
#   on the SAME INODE through the outside alias, the event names that path, and
#   nothing fires for the watched one. That covers both rewriting the file and
#   merely chmod-ing or chown-ing it, which moves no bytes at all. It is a property
#   of path-based file-integrity monitoring, not of the judgment below. Slower by
#   design: a divergence must repeat on two consecutive ticks before it pages (so an
#   in-flight apply cannot false-page), which puts detection at 15 to 30 minutes.
#   The symlink and regular-file checks below remain the immediate answer on the
#   event path.
#
# So: anything that fires an event on a watched path is judged in seconds by layer
# 1, on all three columns; anything that fires no event is found within two ticks by
# layer 2, on the same three columns. Both layers cover BOTH manifests: the managed
# ~/.local/bin scripts are judged on the event path here and re-read on the same
# audit tick, because nothing about either argument is specific to the pipeline
# home.
#
# WHAT NEITHER LAYER COVERS, recorded honestly:
#
#   - GROUP OWNERSHIP. Content, mode and owner are bound; the owning GROUP is not.
#     chezmoi has no group intent to derive one from, and a chgrp alone cannot make
#     a file writable that the bound mode does not already grant group write to (a
#     bound 0755 or 0644 grants none), so the case a group column would add on its
#     own is chgrp plus chmod, which the mode column already pages.
#   - RE-TAMPERING WITHIN THE SAME SET OF DIVERGENCES. Layer 2 dedupes on a
#     fingerprint of WHICH paths disagree and HOW (the divergence KIND per path), not
#     of the bytes or the mode they now hold. So a path already reported as a content
#     divergence can be rewritten again, with different content, without paging a
#     second time, and a mode already reported as drifted can drift further. What
#     DOES page again is any change to the kind set: a file reported for its mode and
#     then rewritten adds a content divergence, which is a new fingerprint and a
#     fresh confirmation. Deliberate (it is what stops one persistent divergence
#     paging every 15 minutes forever), and worth knowing.
#   - NEW FILES IN ~/.local/bin. Nothing here pages for a file that simply appears
#     in ~/.local/bin, because the tracked set is the manifest and an unmanaged
#     neighbor is exactly what a third-party install looks like. Planting a script
#     there does not by itself get it executed; what does is tampering with the
#     managed scripts the LaunchAgents already invoke, and that is what the bin arm
#     catches. The pipeline home has the opposite rule, because anything appearing
#     inside the monitor's own body is a tamper.
#   - SOURCE COMPROMISE. The manifest is generated from chezmoi's source state,
#     which is user-writable, so tampering with a managed file's SOURCE and letting
#     a legitimate apply deploy it is signed as known-good by BOTH layers. See the
#     runner's docblock: this buys post-deployment integrity, not source integrity.
#
# Return-code contract (from c69baab _pipeline_verdict):
#   0 = PAGE   (tamper / cannot confirm legit / no manifest / delete)
#   1 = SILENT (an untracked neighbor, or an exact (path, sha256, mode, uid)
#               manifest match)

# Keep these defaults in sync with the manifest paths in
# .chezmoiscripts/run_after_05-osquery-known-good-manifests.sh (the producer). Tests
# pin the literals equal, because the producer and the consumer of a
# security-critical file must not agree only by copy-paste.
#
# A target is judged against exactly one of them, chosen by _pipeline_manifest_for.
PIPELINE_MANIFEST="${OSQUERY_PIPELINE_MANIFEST:-/var/osquery/pipeline-known-good.sha256}"
MANAGED_BIN_MANIFEST="${OSQUERY_MANAGED_BIN_MANIFEST:-/var/osquery/managed-bin-known-good.sha256}"

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

# _pipeline_change_time <path>: epoch inode CHANGE time, or empty when it cannot be
# read. Same portable order as _pipeline_mtime.
#
# This, not mtime, is what "when did this file last change" means once mode and
# owner are part of the tuple: a chmod or a chown moves ctime and leaves mtime
# alone, while a content write moves both. Asking for mtime would make an
# attribute-only apply look older than the manifest and skip the settle window
# entirely, which the alerter cannot recover from - it judges each finding exactly
# once, so the false CRIT would never be reconsidered.
_pipeline_change_time() {
  stat -c '%Z' "$1" 2>/dev/null || stat -f '%c' "$1" 2>/dev/null
}

# _pipeline_file_mode <path>: the file's permission bits as EXACTLY four octal
# digits (0755, or 4755 for a setuid file), non-zero and empty output when they
# cannot be read.
#
# The two platforms are deliberately asked for DIFFERENT fields. GNU %a already
# prints all twelve permission bits. BSD has no equivalent: %Lp prints only the
# low NINE, so a setuid, setgid or sticky bit set on a pipeline script would read
# back as an ordinary mode. %p prints the full mode including the file type
# (100755), so the low four octal digits are taken from whichever form answered
# and both platforms yield the same string for the same file.
#
# The value is range-bound by a regex BEFORE it is sliced, so a stat that printed
# something unexpected fails the read instead of producing a plausible-looking
# mode.
_pipeline_file_mode() {
  local raw
  raw=$(stat -c '%a' "$1" 2>/dev/null || stat -f '%p' "$1" 2>/dev/null) || return 1
  [[ $raw =~ ^[0-7]{1,7}$ ]] || return 1
  raw="000$raw"
  printf '%s' "${raw: -4}"
}

# _pipeline_file_uid <path>: the file's owner uid in decimal, non-zero and empty
# output when it cannot be read. Validated as digits so a caller never compares
# against a stat error string.
_pipeline_file_uid() {
  local raw
  raw=$(stat -c '%u' "$1" 2>/dev/null || stat -f '%u' "$1" 2>/dev/null) || return 1
  [[ $raw =~ ^[0-9]{1,10}$ ]] || return 1
  printf '%s' "$raw"
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
  [[ -n ${OSQUERY_PIPELINE_MANIFEST:-} && $1 == "${OSQUERY_PIPELINE_MANIFEST}" ]] && return 0
  [[ -n ${OSQUERY_MANAGED_BIN_MANIFEST:-} && $1 == "${OSQUERY_MANAGED_BIN_MANIFEST}" ]] && return 0
  local owner mode
  owner=$(_pipeline_file_uid "$1") || true
  mode=$(_pipeline_file_mode "$1") || true
  [[ $owner == 0 ]] || return 1
  # Refuse a group- or world-writable manifest. The mode is OCTAL, so both operands
  # are read base 8; an unreadable mode defaults to 7777 and is refused.
  ((8#${mode:-7777} & 8#22)) && return 1
  return 0
}

# _pipeline_manifest_has_tuple <path> <hash> <mode> <uid>: 0 when the manifest
# holds a line binding exactly this content hash, mode and owner to exactly this
# path, else 1. Legitimacy is the EXACT tuple, not any one column:
#
#   - binding the hash to ITS path defeats a swap-in-place (a valid hash lifted
#     onto a different tracked path);
#   - binding MODE and OWNER alongside the content defeats an attribute-only
#     change. osquery reports a chmod or a chown as an ATTRIBUTES_MODIFIED event
#     carrying the file's UNCHANGED digest, so a content-only manifest matched it
#     and stayed silent. Making a pipeline script group-writable now, to modify it
#     later from a less privileged context, was therefore invisible.
#
# Manifest line shape, four whitespace-separated fields with the PATH LAST:
#
#   <sha256> <mode> <uid> <path>
#
# Path last is load-bearing: `read -r` assigns the remainder of the line to the
# final variable, so a path containing spaces is still read whole. It also makes a
# SHORT line inert rather than dangerous - a two-column line (the shape this
# manifest had before mode and owner were bound) leaves uid and path empty, and an
# empty path can never equal a real target, so the line vouches for nothing and
# the change pages. The non-empty guard below states that explicitly rather than
# leaving it to fall out of the comparison.
#
# Hashes are compared case-insensitively via bash case-folding (no forks): shasum
# and osquery both emit lowercase, so this is documented defense-in-depth against a
# future producer, kept because it now costs nothing. Mode and uid are compared
# verbatim: the producer writes mode as exactly four octal digits and uid in
# decimal, and _pipeline_file_mode / _pipeline_file_uid normalize the observed
# values to the same form. A missing, unreadable, or untrustworthy manifest
# returns 1 - the fail-safe hinge.
# _pipeline_manifest_for <target>: print the manifest path that governs <target>.
# One target is judged against exactly one manifest, so the two lists can never
# vouch for each other's files: a tuple lifted out of the bin manifest cannot bless
# a pipeline path, and the reverse holds too.
#
# Read through the environment overrides at CALL time rather than through the
# constants alone: tests source this helper first and set the override afterwards,
# which is the seam that keeps every fixture out of /var.
_pipeline_manifest_for() {
  case "$1" in
    "$HOME"/.local/bin/*) printf '%s' "${OSQUERY_MANAGED_BIN_MANIFEST:-$MANAGED_BIN_MANIFEST}" ;;
    *) printf '%s' "${OSQUERY_PIPELINE_MANIFEST:-$PIPELINE_MANIFEST}" ;;
  esac
}

_pipeline_manifest_has_tuple() {
  local manifest
  manifest="$(_pipeline_manifest_for "$1")"
  [[ -r $manifest ]] || return 1
  _pipeline_manifest_is_trustworthy "$manifest" || return 1
  local want_path="$1" want_hash="${2,,}" want_mode="$3" want_uid="$4" h m u p
  # An observed column that could not be read must never be matched against, or an
  # equally empty manifest column would vouch for a file nothing was learned about.
  [[ -n $want_hash && -n $want_mode && -n $want_uid && -n $want_path ]] || return 1
  # `|| [[ -n $h ]]` so a final line with no trailing newline is still examined.
  while read -r h m u p || [[ -n $h ]]; do
    [[ -n $h && -n $m && -n $u && -n $p ]] || continue
    [[ ${h,,} == "$want_hash" && $m == "$want_mode" &&
      $u == "$want_uid" && $p == "$want_path" ]] && return 0
  done <"$manifest"
  return 1
}

# _pipeline_deployed_state_is_known_good <path>: read the file's content hash, mode
# and owner AS THEY ARE NOW and require that state to be the manifest's tuple for
# this exact path. Re-reading here (rather than trusting what osquery recorded when
# the event fired) is what closes the swap-after-the-event race: an attacker could
# otherwise let a known-good event be recorded, replace the file, run it, and
# restore it before the next collection, and the stale digest would have vouched
# for bytes that were not on disk when the decision was made. The same argument
# applies to the attributes, which is why they are stat-ed here and not taken from
# the event. A file whose hash, mode or owner cannot be read returns 1, so it
# pages.
_pipeline_deployed_state_is_known_good() {
  local target="$1" disk_hash disk_mode disk_uid
  disk_hash=$(shasum -a 256 "$target" 2>/dev/null | awk '{print $1}')
  [[ -n $disk_hash ]] || return 1
  disk_mode=$(_pipeline_file_mode "$target") || return 1
  disk_uid=$(_pipeline_file_uid "$target") || return 1
  _pipeline_manifest_has_tuple "$target" "$disk_hash" "$disk_mode" "$disk_uid"
}

# _pipeline_tuple_settles <path>: the current-state check, plus a bounded wait for
# an in-flight manifest regeneration. It waits only when the manifest EXISTS but
# PREDATES the change to the file, which is exactly the apply-race shape (the file
# landed, the manifest has not been reinstalled yet). A missing manifest pages
# immediately, and a manifest newer than the change is already the final word. The
# state is re-read on every retry, so a file still settling is judged on what it
# finally holds, not on a mid-write snapshot.
#
# The target side is its CHANGE time, not its modification time, because a chmod or
# a chown moves only the former and both are now part of the tuple. The manifest
# side stays mtime: the runner installs a fresh file, so its mtime is the moment
# that manifest became current.
_pipeline_tuple_settles() {
  local target="$1"
  _pipeline_deployed_state_is_known_good "$target" && return 0
  local manifest
  manifest="$(_pipeline_manifest_for "$target")"
  [[ -r $manifest ]] || return 1
  local manifest_mtime target_ctime
  manifest_mtime=$(_pipeline_mtime "$manifest") || true
  target_ctime=$(_pipeline_change_time "$target") || true
  [[ -n $manifest_mtime && -n $target_ctime ]] || return 1
  ((manifest_mtime < target_ctime)) || return 1
  # Open the shared window on the first miss of this invocation; a later miss
  # inherits whatever is left of it, and answers at once when it is spent.
  [[ -n $_pipeline_settle_deadline ]] ||
    _pipeline_settle_deadline=$(($(_pipeline_now) + OSQUERY_PIPELINE_SETTLE_SECONDS))
  while (($(_pipeline_now) < _pipeline_settle_deadline)); do
    sleep 1
    _pipeline_deployed_state_is_known_good "$target" && return 0
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
# ~/.local/bin is tracked on a DIFFERENT rule: see _managed_bin_is_tracked below.
_pipeline_is_tracked() {
  local target="$1"
  case "$target" in
    "$HOME"/.local/libexec/osquery/*) return 0 ;;
    "$HOME"/Library/LaunchAgents/com.webdavis.osquery-*.plist) return 0 ;;
    "$HOME"/.local/bin/*) _managed_bin_is_tracked "$target" ;;
    *) return 1 ;;
  esac
}

# _managed_bin_is_tracked <target>: 0 when a ~/.local/bin path is ours to judge.
#
# MANIFEST-DRIVEN, not directory-driven, and that is the whole reason covering this
# directory is affordable. ~/.local/bin holds the chezmoi-managed operator scripts
# (update-skills.sh, homebrew-weekly-upgrade.sh, the claude-* hooks, the Relay
# tools) side by side with third-party shims that chezmoi does not manage and that
# rewrite themselves on their own schedule: herdr, mise, bob, hermes, yt-dlp, and a
# pile of symlinks into pipx and uv tool directories. Tracking the whole directory
# the way the pipeline home is tracked would page on every one of those
# self-updates, which is the churn objection that kept this directory unwatched.
# Taking the tracked set FROM the manifest instead makes tracked and manifested
# identical by construction, so the two cannot drift apart the way a second
# hand-maintained filter would.
#
# THE FAIL-SAFE HINGE. A manifest that is missing, unreadable, empty or
# untrustworthy tracks EVERYTHING under ~/.local/bin rather than nothing. The
# tempting reading of "not in the manifest" is "not ours", but applied to a broken
# manifest that reads as "un-watch the directory", which is a monitor going quiet
# exactly when its own inputs failed. Loud is the only safe direction: every bin
# event then pages, the tuple check that follows cannot confirm any of them either,
# and the periodic audit refuses on the same manifest within the tick. The noise is
# the alarm.
#
# The membership scan reads the four-column line with the PATH LAST, so a path
# holding spaces is taken whole by the final field. A SHORT line leaves that field
# empty, and an empty path can never equal a real target, so a malformed line
# vouches for nothing here either.
_managed_bin_is_tracked() {
  local manifest manifest_path
  manifest="${OSQUERY_MANAGED_BIN_MANIFEST:-$MANAGED_BIN_MANIFEST}"
  [[ -r $manifest && -s $manifest ]] || return 0
  _pipeline_manifest_is_trustworthy "$manifest" || return 0
  # `|| [[ -n $manifest_path ]]` so a final line with no trailing newline is still
  # examined (the same idiom the tuple check reads the manifest with).
  while read -r _ _ _ manifest_path || [[ -n $manifest_path ]]; do
    [[ -n $manifest_path && $manifest_path == "$1" ]] && return 0
  done <"$manifest"
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
