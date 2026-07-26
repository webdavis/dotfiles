#!/usr/bin/env bash
#
# pipeline-audit.sh, sourced helper, not run directly. The PERIODIC MANIFEST AUDIT:
# it reads every root-owned known-good manifest and reports each path whose CURRENT
# content, mode or owner disagrees with the tuple recorded for it. Sourced by the
# uptime watchdog, which runs it once per 15-minute tick.
#
# BOTH manifests are audited on the same tick: the osquery pipeline's own
# (pipeline-known-good.sha256) and the chezmoi-managed scripts under ~/.local/bin
# (managed-bin-known-good.sha256). The blind spot below is a property of path-based
# event GENERATION, not of any one directory, so it applies to the managed bin
# scripts exactly as it does to the pipeline home - and those scripts run
# unattended from LaunchAgents and shell hooks, which is precisely the case where
# nobody is present to notice a missing event. All three columns are compared for
# both, so a chmod or chown on a managed operator script is a divergence here under
# its own kind, not a content-only blind spot. The manifests stay separate files
# (see the runner), but a monitor is only as good as its least-covered input, so
# the audit reads them all and a refusal on either one refuses the whole tick.
#
# WHY A SCHEDULED AUDIT AND NOT ANOTHER EVENT CHECK. Until this existed, the
# manifest was enforced only through osquery file_events, and osquery watches
# PATHS. An attacker who hard-links a manifested script to a writable path outside
# the pipeline home and then overwrites the outside alias mutates the SAME INODE:
# the filesystem event names the attacker's path, nothing fires for the watched
# path, no verdict runs, and the tampered script executes with nothing paged. A
# chmod or chown through that same alias does it without changing a byte. A symlink
# referent gives the same blind spot. That is a gap in EVENT GENERATION, not in the
# verdict, so no amount of judging events can close it. Re-reading the files on a
# schedule can, because it never depends on an event having fired.
#
# Usage (from a sourcing script):
#   source "$HOME/.local/libexec/osquery/pipeline-audit.sh"
#   findings="$(pipeline_audit_scan)" || reason="$findings"
#
# The manifest path constant and the root-ownership check are REUSED from the
# verdict helper rather than copied: the producer/consumer agreement on a
# security-critical file is already pinned by a test, and a third copy of either
# would be a third thing to drift.
#
# Sourced CONDITIONALLY, and its absence is reported by the scan rather than
# thrown. Every caller runs under `set -euo pipefail`, so an unconditional source
# of a file someone had deleted would abort the caller mid-run: the watchdog would
# die in the middle of its tick and page nothing at all, which is precisely the
# silent-monitor failure this audit exists to prevent. A missing dependency has to
# be loud, and the only way to be loud is to survive long enough to say so.
_pipeline_audit_verdict_helper="$HOME/.local/libexec/osquery/results-alerter/pipeline-verdict.sh"
if [[ -r $_pipeline_audit_verdict_helper ]]; then
  # shellcheck source=/dev/null
  source "$_pipeline_audit_verdict_helper"
fi

# COST, measured rather than assumed. The pipeline manifest lists 25 files totalling
# about 230 KiB, and a full scan of them takes 0.6 to 0.8s wall clock on this host:
# a size stat, a mode read, an owner read (~4ms each) and one shasum (~28ms) fork per
# file, with the hashing itself lost in the noise. shasum is a Perl script, so its
# startup, not the SHA-256, is nearly all of that. A single batched shasum over every
# path would cut it to ~0.05s, and is deliberately not done: it would have to be
# parsed back by path (an output line is absent for a file that vanished mid-scan,
# and some implementations escape unusual filenames), which trades an
# obviously-correct loop for a false-page vector. At one scan per 15-minute tick this
# is a 0.09% duty cycle, and the audit runs LAST, so it delays none of the other
# probes. The managed-bin manifest adds roughly 19 small scripts, so a tick reading
# both costs about 1.2s, still under a 0.14% duty cycle.
#
# Comparing the two ATTRIBUTE columns is what the mode and owner reads cost, measured
# side by side over a 25-file fixture on this host: 0.70 to 0.73s, against 0.46 to
# 0.48s for the same scan comparing content alone. Each read is a fork, and on BSD
# each pays a failed GNU-stat attempt before the BSD form answers. Folding all three
# values into one stat call would recover most of that quarter-second and is not
# done: the mode reader exists because BSD `stat -f '%Lp'` prints only the low NINE
# permission bits, so a hand-rolled stat here would read a setuid bit back as an
# ordinary mode and compare EQUAL. A quarter-second per 15-minute tick is not worth
# re-introducing that.
#
# Bounds, so one tick cannot become a long-running process. Every one of them
# fails toward a page, never toward a quiet skip:
#   MAX_ENTRIES  - a manifest longer than this is refused whole (a partial audit
#                  that reported "clean" would be a lie about the unread tail).
#                  Applied PER MANIFEST, which is what the overlong token means.
#                  The real manifests list roughly 25 and 19 files.
#   MAX_BYTES    - a manifested file larger than this is NOT hashed; hashing is
#                  the only work an attacker can inflate, and a pipeline script
#                  that suddenly weighs megabytes is itself a divergence.
#   BUDGET       - a wall-clock ceiling for the whole scan, SHARED across the
#                  manifests: it bounds the tick, not each list, so adding a
#                  manifest can never extend how long one tick may run.
# Each is validated as a plain decimal and clamped on BOTH sides, so a hostile or
# fat-fingered environment cannot turn a bound into unbounded work (or into
# nonsense arithmetic).
OSQUERY_PIPELINE_AUDIT_MAX_ENTRIES="${OSQUERY_PIPELINE_AUDIT_MAX_ENTRIES:-500}"
OSQUERY_PIPELINE_AUDIT_MAX_BYTES="${OSQUERY_PIPELINE_AUDIT_MAX_BYTES:-8388608}"
OSQUERY_PIPELINE_AUDIT_BUDGET_SECONDS="${OSQUERY_PIPELINE_AUDIT_BUDGET_SECONDS:-60}"

# _pipeline_audit_clamp <value> <min> <max> <default>: print <value> when it is a
# plain decimal inside [min, max], else <default>. The regex bounds the DIGIT
# COUNT before the value ever reaches arithmetic, so an over-range value cannot
# overflow, and a leading zero cannot be read as octal.
_pipeline_audit_clamp() {
  local value="$1" low="$2" high="$3" fallback="$4"
  [[ $value =~ ^(0|[1-9][0-9]{0,9})$ ]] || {
    printf '%s' "$fallback"
    return 0
  }
  if ((value < low)) || ((value > high)); then
    printf '%s' "$fallback"
    return 0
  fi
  printf '%s' "$value"
}

# _pipeline_audit_now: current epoch seconds, without a fork where bash 5 provides
# it (the same idiom the verdict helper uses).
_pipeline_audit_now() {
  printf '%s' "${EPOCHSECONDS:-$(date +%s)}"
}

# _pipeline_audit_size <path>: byte size, or empty when it cannot be read. GNU stat
# first (the nix shell), BSD stat as the fallback (the portable order used
# throughout this feature-set).
_pipeline_audit_size() {
  stat -c '%s' "$1" 2>/dev/null || stat -f '%z' "$1" 2>/dev/null
}

# pipeline_audit_scan: audit every manifested path against its recorded tuple.
#
#   return 0 - the scan COMPLETED. Each divergence is one stdout line,
#              "<kind> <path>", in manifest order; no output means the deployed
#              tree matches the manifest exactly. One line PER DIVERGING COLUMN, so
#              a path that drifted on two of them is reported twice, under two
#              kinds. Kinds:
#                content     the bytes on disk differ from the recorded hash
#                mode        the permission bits differ from the recorded mode
#                owner       the owning uid differs from the recorded one
#                missing     nothing exists at the manifested path
#                irregular   a symlink or a non-regular file stands there
#                oversize    too large to hash within the tick's bound
#                unreadable  present and regular, but its size, mode, owner or
#                            hash could not be read
#   return 1 - the scan could NOT be completed, and stdout is a single reason
#              TOKEN: missing (absent, unreadable, or empty manifest),
#              unavailable (the verdict helper it reuses is not installed),
#              untrustworthy (not root-owned, or group/world-writable),
#              malformed (a line that is not
#              "<sha256> <mode> <uid> <absolute-path>"),
#              overlong (more entries than the audit will examine), budget (the
#              wall-clock ceiling was reached first).
#
# The completion/return split is the fail-safe hinge. "No output" alone would read
# identically for "nothing diverged" and "the scan never got started", and a
# monitor that goes quiet when its own input breaks is the failure mode this whole
# subsystem exists to avoid. Every refusal is the caller's cue to page.
#
# SCOPE, recorded honestly. This audits MANIFEST -> DISK, across ALL THREE bound
# columns: content, mode and owner. What that does and does not reach:
#   - A file planted under the pipeline home that the manifest does not list is not
#     found here; that direction is the alerter's, which pages any tracked path
#     whose tuple it cannot confirm.
#   - Comparing mode and owner here is what covers ATTRIBUTE DRIFT THAT FIRES NO
#     EVENT: a chmod or chown applied through a hard-link alias outside the pipeline
#     home changes the shared inode while the event names the outside path, so the
#     event-time verdict never judges it and no byte of content moves for a
#     content-only comparison to notice. It is now a divergence here, within one
#     tick of the change.
#   - GROUP OWNERSHIP is still not compared, because the manifest does not bind it.
#     See the coverage map in results-alerter/pipeline-verdict.sh for why, and for
#     what remains uncovered across both layers.
#   - A path is judged against exactly ONE manifest, the one that lists it. The two
#     lists are disjoint by construction (the runner's arms select disjoint sets),
#     so nothing is read twice and neither can vouch for the other's files.
pipeline_audit_scan() {
  # The reused seam has to actually be here. Checked by NAME rather than assumed,
  # so a partial deploy reports a broken audit instead of an all-clear.
  if ! declare -F _pipeline_manifest_is_trustworthy >/dev/null 2>&1; then
    printf 'unavailable\n'
    return 1
  fi
  local manifest="${OSQUERY_PIPELINE_MANIFEST:-${PIPELINE_MANIFEST:-}}"
  local max_entries max_bytes budget deadline manifest
  max_entries="$(_pipeline_audit_clamp "$OSQUERY_PIPELINE_AUDIT_MAX_ENTRIES" 1 100000 500)"
  max_bytes="$(_pipeline_audit_clamp "$OSQUERY_PIPELINE_AUDIT_MAX_BYTES" 1 1073741824 8388608)"
  # A zero budget is a legitimate (if drastic) setting: it refuses immediately and
  # therefore pages, which is the safe direction, and it makes the exhausted-budget
  # path testable without a sleep.
  budget="$(_pipeline_audit_clamp "$OSQUERY_PIPELINE_AUDIT_BUDGET_SECONDS" 0 300 60)"
  # ONE deadline for the whole tick, computed before the first manifest is opened.
  deadline=$(($(_pipeline_audit_now) + budget))

  # Every known-good manifest, in audit order. A REFUSAL on any of them refuses the
  # whole scan immediately and drops the findings gathered so far: the caller pages
  # on a refusal anyway, and reporting a partial list beside a refusal token would
  # invite reading it as the complete picture. Findings from a COMPLETED manifest
  # accumulate, so a divergence in one is never hidden by the other being clean.
  local manifests=(
    "${OSQUERY_PIPELINE_MANIFEST:-${PIPELINE_MANIFEST:-}}"
    "${OSQUERY_MANAGED_BIN_MANIFEST:-${MANAGED_BIN_MANIFEST:-}}"
  )
  for manifest in "${manifests[@]}"; do
    _pipeline_audit_scan_manifest "$manifest" "$deadline" "$max_entries" "$max_bytes" || return 1
  done
  return 0
}

# _pipeline_audit_scan_manifest <manifest> <deadline> <max-entries> <max-bytes>:
# audit ONE manifest. Same contract as pipeline_audit_scan for a single list:
# return 0 having printed a "<kind> <path>" line per diverging COLUMN, or return 1
# having printed a single refusal token.
_pipeline_audit_scan_manifest() {
  local manifest="$1" deadline="$2" max_entries="$3" max_bytes="$4"

  # An unusable manifest is refused before anything is judged: without a trustworthy
  # list there is no known-good to compare against, and "found nothing" would be a
  # false all-clear. -s rejects an empty manifest, which a truncated write leaves.
  [[ -r $manifest && -s $manifest ]] || {
    printf 'missing\n'
    return 1
  }
  _pipeline_manifest_is_trustworthy "$manifest" || {
    printf 'untrustworthy\n'
    return 1
  }

  # The manifest is "<sha256> <mode> <uid> <path>", single-space separated with the
  # PATH LAST. The line is matched WHOLE rather than split into fields: a path may
  # contain spaces, and word-splitting would silently truncate it into a path that
  # exists nowhere (which would then report as a bogus divergence forever). The path
  # must be absolute, because the audit resolves nothing itself and a relative path
  # would be read against whatever directory launchd happened to start the caller
  # in.
  #
  # All four columns are matched, and all four are compared below. Requiring the
  # attribute columns to be present and well-formed is what keeps this fail-closed:
  # a manifest in the older content-only format does not match, so it reports
  # malformed and the caller pages, rather than being read as if the missing columns
  # did not matter. The pattern is also what BOUNDS the two attribute columns, so
  # the comparisons below are string equalities over already-constrained values.
  local line_pattern='^([0-9a-fA-F]{64}) ([0-7]{4}) ([0-9]{1,10}) (/.+)$'
  local entries=0 line want_hash want_mode want_uid target
  local size disk_hash disk_mode disk_uid

  # `|| [[ -n $line ]]` so a final line with no trailing newline is still examined
  # (the same idiom the verdict helper reads the manifest with).
  while IFS= read -r line || [[ -n $line ]]; do
    entries=$((entries + 1))
    if ((entries > max_entries)); then
      printf 'overlong\n'
      return 1
    fi
    if (($(_pipeline_audit_now) >= deadline)); then
      printf 'budget\n'
      return 1
    fi
    if [[ ! $line =~ $line_pattern ]]; then
      printf 'malformed\n'
      return 1
    fi
    # Lower-cased on both sides: shasum and osquery both emit lowercase, so this is
    # documented defense in depth against a future producer, and it costs nothing.
    #
    # All four columns are captured HERE, in one go. BASH_REMATCH is global and is
    # overwritten by the next [[ =~ ]] anywhere in this loop, so reading a column
    # later would read whatever pattern matched most recently.
    want_hash="${BASH_REMATCH[1],,}"
    want_mode="${BASH_REMATCH[2]}"
    want_uid="${BASH_REMATCH[3]}"
    target="${BASH_REMATCH[4]}"
    # A manifested path must hold a REGULAR FILE, and links are never followed. A
    # symlink standing where a pipeline script belongs would otherwise be hashed
    # THROUGH to content the manifest vouches for, while the bytes that actually
    # execute live somewhere the manifest does not cover and nothing watches: the
    # same blind spot in a second shape.
    if [[ -L $target ]]; then
      printf 'irregular %s\n' "$target"
    elif [[ ! -e $target ]]; then
      printf 'missing %s\n' "$target"
    elif [[ ! -f $target ]]; then
      printf 'irregular %s\n' "$target"
    else
      # The three columns are read BEFORE anything is judged, so an attribute that
      # cannot be read reports unreadable ONCE for the path rather than once per
      # failing reader. The mode and owner come from the verdict helper's readers,
      # never from a stat here: BSD `stat -f '%Lp'` prints only the low NINE
      # permission bits, so a setuid, setgid or sticky bit added to a pipeline
      # script would read back as an ordinary mode and compare equal. The helpers
      # ask each platform for a field that carries all twelve.
      size="$(_pipeline_audit_size "$target")"
      disk_mode="$(_pipeline_file_mode "$target")" || disk_mode=""
      disk_uid="$(_pipeline_file_uid "$target")" || disk_uid=""
      if [[ ! $size =~ ^(0|[1-9][0-9]{0,18})$ ]] ||
        [[ ! $disk_mode =~ ^[0-7]{4}$ ]] || [[ ! $disk_uid =~ ^[0-9]{1,10}$ ]]; then
        printf 'unreadable %s\n' "$target"
      else
        if ((size > max_bytes)); then
          printf 'oversize %s\n' "$target"
        else
          # The hash is cut with parameter expansion rather than piped through awk:
          # the audit runs this once per manifested file, and a saved fork per file is
          # most of the tick's cost. shasum prints "<hash>  <path>", so everything from
          # the first space on is dropped.
          disk_hash="$(shasum -a 256 -- "$target" 2>/dev/null)"
          disk_hash="${disk_hash%% *}"
          disk_hash="${disk_hash,,}"
          if [[ ! $disk_hash =~ ^[0-9a-f]{64}$ ]]; then
            printf 'unreadable %s\n' "$target"
          elif [[ $disk_hash != "$want_hash" ]]; then
            printf 'content %s\n' "$target"
          fi
        fi
        # Mode and owner are compared for every readable regular file, INCLUDING one
        # too large to hash: the size bound caps hashing, and an attacker who grows a
        # manifested file must not buy silence on its permissions with it. Both are
        # STRING equalities against columns the line pattern already constrained
        # (four octal digits, up to ten decimal digits), so neither side reaches
        # arithmetic and a manifest value that is merely unusual cannot be read as
        # octal or overflow. A divergence per COLUMN, each with its own kind: the
        # watchdog dedupes on a fingerprint of this report, and folding both into one
        # per-path line would make an escalation from mode drift to content tamper
        # look like the condition already reported.
        [[ $disk_mode == "$want_mode" ]] || printf 'mode %s\n' "$target"
        [[ $disk_uid == "$want_uid" ]] || printf 'owner %s\n' "$target"
      fi
    fi
  done <"$manifest"
  return 0
}
