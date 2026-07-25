#!/usr/bin/env bash
#
# pipeline-audit.sh, sourced helper, not run directly. The PERIODIC CONTENT AUDIT
# of the osquery pipeline: it reads the root-owned pipeline-integrity manifest,
# hashes every path the manifest lists, and reports each path whose CURRENT state
# disagrees with its recorded tuple. Sourced by the uptime watchdog, which runs it
# once per 15-minute tick.
#
# WHY A SCHEDULED AUDIT AND NOT ANOTHER EVENT CHECK. Until this existed, the
# manifest was enforced only through osquery file_events, and osquery watches
# PATHS. An attacker who hard-links a manifested script to a writable path outside
# the pipeline home and then overwrites the outside alias mutates the SAME INODE:
# the filesystem event names the attacker's path, nothing fires for the watched
# path, no verdict runs, and the tampered script executes with nothing paged. A
# symlink referent gives the same blind spot. That is a gap in EVENT GENERATION,
# not in the verdict, so no amount of judging events can close it. Comparing bytes
# on a schedule can, because it never depends on an event having fired.
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

# COST, measured rather than assumed. The real manifest lists 25 files totalling
# about 224 KiB, and a full scan of them takes ~0.75s wall clock on this host: one
# stat (~4ms) and one shasum (~28ms) fork per file, with the hashing itself lost in
# the noise. shasum is a Perl script, so its startup, not the SHA-256, is nearly all
# of that. A single batched shasum over every path would cut it to ~0.05s, and is
# deliberately not done: it would have to be parsed back by path (an output line is
# absent for a file that vanished mid-scan, and some implementations escape unusual
# filenames), which trades an obviously-correct loop for a false-page vector. At one
# scan per 15-minute tick this is a 0.08% duty cycle, and the audit runs LAST, so it
# delays none of the other probes.
#
# Bounds, so one tick cannot become a long-running process. Every one of them
# fails toward a page, never toward a quiet skip:
#   MAX_ENTRIES  - a manifest longer than this is refused whole (a partial audit
#                  that reported "clean" would be a lie about the unread tail).
#                  The real manifest lists roughly 25 files.
#   MAX_BYTES    - a manifested file larger than this is NOT hashed; hashing is
#                  the only work an attacker can inflate, and a pipeline script
#                  that suddenly weighs megabytes is itself a divergence.
#   BUDGET       - a wall-clock ceiling for the whole scan.
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
#              tree matches the manifest exactly. Kinds:
#                content     the bytes on disk differ from the recorded hash
#                missing     nothing exists at the manifested path
#                irregular   a symlink or a non-regular file stands there
#                oversize    too large to hash within the tick's bound
#                unreadable  present and regular, but its size or hash failed
#   return 1 - the scan could NOT be completed, and stdout is a single reason
#              TOKEN: missing (absent, unreadable, or empty manifest),
#              unavailable (the verdict helper it reuses is not installed),
#              untrustworthy (not root-owned, or group/world-writable),
#              malformed (a line that is not "<sha256>  <absolute-path>"),
#              overlong (more entries than the audit will examine), budget (the
#              wall-clock ceiling was reached first).
#
# The completion/return split is the fail-safe hinge. "No output" alone would read
# identically for "nothing diverged" and "the scan never got started", and a
# monitor that goes quiet when its own input breaks is the failure mode this whole
# subsystem exists to avoid. Every refusal is the caller's cue to page.
#
# SCOPE, recorded honestly: this audits MANIFEST -> DISK. A file planted under the
# pipeline home that the manifest does not list is not found here; that direction
# is the alerter's, which pages any tracked path whose tuple it cannot confirm.
pipeline_audit_scan() {
  # The reused seam has to actually be here. Checked by NAME rather than assumed,
  # so a partial deploy reports a broken audit instead of an all-clear.
  if ! declare -F _pipeline_manifest_is_trustworthy >/dev/null 2>&1; then
    printf 'unavailable\n'
    return 1
  fi
  local manifest="${OSQUERY_PIPELINE_MANIFEST:-${PIPELINE_MANIFEST:-}}"
  local max_entries max_bytes budget
  max_entries="$(_pipeline_audit_clamp "$OSQUERY_PIPELINE_AUDIT_MAX_ENTRIES" 1 100000 500)"
  max_bytes="$(_pipeline_audit_clamp "$OSQUERY_PIPELINE_AUDIT_MAX_BYTES" 1 1073741824 8388608)"
  # A zero budget is a legitimate (if drastic) setting: it refuses immediately and
  # therefore pages, which is the safe direction, and it makes the exhausted-budget
  # path testable without a sleep.
  budget="$(_pipeline_audit_clamp "$OSQUERY_PIPELINE_AUDIT_BUDGET_SECONDS" 0 300 60)"

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

  # The manifest is shasum format, "<sha256>  <path>" with exactly two spaces. The
  # line is matched WHOLE rather than split into fields: a path may contain spaces,
  # and word-splitting would silently truncate it into a path that exists nowhere
  # (which would then report as a bogus divergence forever). The path must be
  # absolute, because the audit resolves nothing itself and a relative path would be
  # read against whatever directory launchd happened to start the caller in.
  local line_pattern='^([0-9a-fA-F]{64}) {2}(/.+)$'
  local deadline entries=0 line want_hash target size disk_hash
  deadline=$(($(_pipeline_audit_now) + budget))

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
    want_hash="${BASH_REMATCH[1],,}"
    target="${BASH_REMATCH[2]}"
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
      size="$(_pipeline_audit_size "$target")"
      if [[ ! $size =~ ^(0|[1-9][0-9]{0,18})$ ]]; then
        printf 'unreadable %s\n' "$target"
      elif ((size > max_bytes)); then
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
    fi
  done <"$manifest"
  return 0
}
