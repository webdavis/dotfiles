#!/usr/bin/env bash
#
# allowlist.sh - the ONE writer for the launchd page-allowlist: the *user* LaunchAgents
# whose new persistence digests instead of paging. Every caller (manual curation now, the
# tap-button bot and /osquery skill later) goes through this single security boundary, so
# all validation lives here.
#
#   allowlist.sh -a <label>   # allow: capture <label>'s identity and add/refresh it
#   allowlist.sh -d <label>   # deny: remove the entry for <label>
#   allowlist.sh -l           # list the current allowlist
#
# -a and -d curate the CHEZMOI SOURCE and then deploy: edit the source, apply that one
# target, refresh the pipeline-integrity manifest. See publish_allowlist for why all
# three steps are required and what each failure leaves behind. -l reads the DEPLOYED
# file, because that is the one the alerter actually consults.
#
# R2-1: an entry is a TUPLE, not a bare label. Suppressing on the label alone let an attacker
# reuse an allowlisted label but point the plist at a malicious program and be silently
# suppressed. `-a` captures the label's KNOWN-GOOD identity (canonical plist path + program +
# plist sha256) from the SAME launchd table a persistence_launchd finding comes from, so the
# alerter suppresses ONLY a full-tuple match and PAGES a reused label. One NDJSON tuple per
# line: {"label","path","program","sha256"}; a leading $HOME is stored as ~/ (user-agnostic).
#
# System daemons (/Library/LaunchDaemons) page by path in the alerter's gate regardless of this
# file, so Apple/system labels are refused here (allowlisting them would be a false suppression).
set -euo pipefail

ALLOWLIST="${OSQUERY_LAUNCHD_ALLOWLIST:-$HOME/.config/osquery/page-launchd-allowlist.txt}"
OSQUERYI="${OSQUERYI:-$(command -v osqueryi || echo /usr/local/bin/osqueryi)}"
CHEZMOI="${CHEZMOI:-$(command -v chezmoi || echo chezmoi)}"

# The manifest runner, which is a chezmoi SOURCE script and is therefore never
# deployed into $HOME; it is located inside the source tree. Overridable for tests.
MANIFEST_RUNNER_REL=".chezmoiscripts/run_after_05-osquery-known-good-manifests.sh"

usage() {
  printf 'usage: %s -a <label> | -d <label> | -l\n' "${0##*/}" >&2
  exit 2
}

# Serialize every mutating run (-a and -d) around its whole read -> capture -> rewrite ->
# publish critical section, so a slow -a (capture in flight) can never publish after a
# completed -d and silently restore the denied tuple (lost update). House kernel-lock
# pattern (mirrors the alert drainer's take_single_instance_lock): /usr/bin/lockf on a
# held fd. Unlike the drainer this BLOCKS until the lock frees - curation must serialize,
# not skip - and the lock releases when the process exits (fd 9 closes). A genuine
# lock-setup error fails CLOSED (per the DR-B ruling). The ONE exception is a host with
# no lockf at all (any non-darwin box, e.g. Linux CI): there is no kernel lock to take,
# so the write proceeds unlocked by design, matching the drainer.
#
# fd-inheritance discipline: `exec 9>>` leaves fd 9 inheritable, so EVERY external command
# spawned while the lock is held closes it with `9>&-`. Otherwise a child (osqueryi/jq/
# shasum/awk/grep/mkdir/dirname/touch/mktemp/mv) inherits the lock fd; if it outlives the
# writer it keeps the kernel lock held and every later -a/-d blocks forever. `9>&-` is
# added ONLY to forked externals - never to a function call or builtin running in this
# shell, which would close fd 9 in the writer itself and release the lock early.
take_allowlist_write_lock() {
  local lockf_bin="${OSQUERY_ALLOWLIST_LOCKF_BIN:-/usr/bin/lockf}"
  # No lockf available: the documented non-darwin fallback. Proceed unlocked.
  [[ -x $lockf_bin ]] || return 0
  # From here the lock is REQUIRED. Any failure to set it up fails CLOSED. The brace
  # group scopes the stderr silence to the exec itself; a bare `exec 9>>f 2>/dev/null`
  # (no command word) would redirect the WHOLE script's stderr to /dev/null for good,
  # eating every later refusal/failure message.
  mkdir -p "$(dirname "$ALLOWLIST")" 2>/dev/null || return 1
  { exec 9>>"${ALLOWLIST}.lock"; } 2>/dev/null || return 1
  "$lockf_bin" -s 9
}

# The JSON label of an allowlist line, or empty for a comment/blank/non-JSON line.
# 9>&- so this jq (spawned under the write lock) never inherits the lock fd - see the
# fd-inheritance note on take_allowlist_write_lock.
entry_label() { jq -r '.label // empty' 9>&- <<<"$1" 2>/dev/null || true; }

# Rewrite an allowlist file, preserving comment/blank lines and dropping any tuple for
# <label>. Reads <file>, writes the filtered result to stdout (a no-op if it is absent).
_without_label() {
  local drop="$1" file="$2" line
  [[ -f $file ]] || return 0
  while IFS= read -r line || [[ -n $line ]]; do
    case "$line" in
      '' | '#'*)
        printf '%s\n' "$line"
        continue
        ;;
    esac
    [[ "$(entry_label "$line")" == "$drop" ]] && continue
    printf '%s\n' "$line"
  done <"$file"
}

# THE DEPLOY PATH, and why curation goes through chezmoi rather than writing the
# deployed file.
#
# The allowlist is a chezmoi-managed plain file, so an apply rewrites it from source
# every time (verified empirically). A tuple written straight to
# ~/.config/osquery/page-launchd-allowlist.txt therefore survives only until the next
# apply and then vanishes, taking its suppression with it and leaving the operator
# with an agent that pages again for no visible reason. The file is also covered by
# the pipeline-integrity manifest now, and allowlist_verdict refuses to honor an
# allowlist the manifest cannot vouch for, so an out-of-band write suppresses nothing
# even before the next apply erases it.
#
# So a seed is a SOURCE change: edit the source, apply that one target, refresh the
# manifest. That order is the one a real apply uses (files land, then the run_after
# runner signs them), so the brief disagreement in between is exactly the shape
# _pipeline_tuple_settles already waits out.
#
# The runner is invoked DIRECTLY rather than left to the apply, because a targeted
# `chezmoi apply <one-file>` does NOT run run_after scripts (verified empirically:
# a full apply fires the runner, a single-target apply does not). Without this call
# the deployed allowlist would sit ahead of the manifest, and the verdict would then
# refuse to honor it - the seed would appear to do nothing.

# allowlist_source_path: the chezmoi SOURCE file backing the deployed allowlist.
# Fails closed: a target chezmoi does not manage, or an unavailable chezmoi, must
# stop the run rather than silently fall back to writing the deployed file.
allowlist_source_path() { "$CHEZMOI" source-path "$ALLOWLIST" 9>&-; }

# manifest_runner_path: the known-good-manifests runner inside the chezmoi source
# tree. It refreshes BOTH arms (pipeline and managed-bin) from one managed listing;
# the allowlist rides in the pipeline arm, which is the one that has to be current
# before the deployed allowlist can be honored again.
manifest_runner_path() {
  if [[ -n ${OSQUERY_PIPELINE_MANIFEST_RUNNER:-} ]]; then
    printf '%s' "$OSQUERY_PIPELINE_MANIFEST_RUNNER"
    return 0
  fi
  local source_dir
  source_dir="$("$CHEZMOI" source-path 9>&-)" || return 1
  printf '%s/%s' "$source_dir" "$MANIFEST_RUNNER_REL"
}

# publish_allowlist <staged-file> <source-file>: install the staged content as the
# new source, deploy it, and refresh the manifest. All-or-nothing as far as it can
# be, and loud where it cannot.
#
# A FAILED APPLY is fully recoverable, so it is rolled back: the source returns to
# its previous bytes and nothing was deployed, which is indistinguishable from the
# command never having run.
#
# A FAILED MANIFEST REFRESH is not recoverable that way, and it is the one outcome
# that matters most to say out loud. The source and the deployed file are updated
# but the manifest still describes the old bytes, so until it is refreshed the
# deployed allowlist is unbound and the verdict refuses to honor it: every user
# LaunchAgent pages. That fails in the SAFE direction, which is why it is reported
# rather than papered over, and the exit status is non-zero so no caller can mistake
# it for a completed seed. The runner's own stderr (sudo's, usually) is deliberately
# NOT redirected: it is the only thing that says why.
publish_allowlist() {
  local staged="$1" source_file="$2" backup runner
  backup="$(mktemp 9>&-)" || {
    printf 'refused: could not stage a rollback copy of the allowlist source\n' >&2
    return 1
  }
  cp "$source_file" "$backup" 9>&- 2>/dev/null || : >"$backup"
  if ! mv -f "$staged" "$source_file" 9>&-; then
    printf 'refused: could not write the allowlist source at %s\n' "$source_file" >&2
    rm -f "$backup" 9>&-
    return 1
  fi
  if ! "$CHEZMOI" apply --force "$ALLOWLIST" 9>&-; then
    cp "$backup" "$source_file" 9>&-
    rm -f "$backup" 9>&-
    printf 'FAILED: chezmoi apply of %s did not succeed. The allowlist source has been rolled back and nothing was deployed.\n' \
      "$ALLOWLIST" >&2
    return 1
  fi
  rm -f "$backup" 9>&-
  if ! runner="$(manifest_runner_path)" || [[ ! -f $runner ]]; then
    printf 'FAILED: the allowlist was deployed but the known-good-manifests runner could not be located (%s). The manifest is now STALE: until it is refreshed the deployed allowlist is unbound and every user LaunchAgent will page. Run a full chezmoi apply to refresh it.\n' \
      "${runner:-unresolved}" >&2
    return 1
  fi
  # No CHEZMOI_SOURCE_DIR is set: the runner treats an absent one as a direct
  # invocation and resolves the configured source itself, which is the same source
  # the apply above just used.
  if ! bash "$runner" 9>&-; then
    printf 'FAILED: the allowlist was deployed but refreshing the known-good manifests failed (%s). The manifest is now STALE: until it is refreshed the deployed allowlist is unbound and every user LaunchAgent will page. Re-run this command, or run a full chezmoi apply.\n' \
      "$runner" >&2
    return 1
  fi
  return 0
}

# A real launchd label starts alphanumeric, then allows . _ @ - (so
# homebrew.mxcl.postgresql@17 passes) and nothing else - no wildcards, paths,
# spaces, or empties. Apple/system labels are refused outright.
is_valid_label() {
  [[ $1 =~ ^[A-Za-z0-9][A-Za-z0-9._@-]+$ ]] || return 1
  # Refuse Apple/system labels case-insensitively (and the dotless prefix), so a
  # COM.APPLE.* variant can't slip past and falsely suppress a system-daemon page.
  local lower="${1,,}"
  [[ $lower == com.apple.* || $lower == com.apple ]] && return 1
  return 0
}

allow_label() {
  local label="$1"
  if ! is_valid_label "$label"; then
    printf 'refused (invalid or system label): %s\n' "$label" >&2
    exit 1
  fi
  # Capture the label's known-good identity from the SAME launchd table the finding comes from,
  # so a future persistence_launchd row matches the stored tuple exactly.
  local row abs_path abs_prog sha rel_path rel_prog
  row=$("$OSQUERYI" --json \
    "SELECT path, COALESCE(NULLIF(program,''), program_arguments) AS program FROM launchd WHERE label = '$label';" \
    9>&- 2>/dev/null | jq -c '.[0] // empty' 9>&- 2>/dev/null) || row=""
  abs_path=$(jq -r '.path // ""' 9>&- <<<"$row" 2>/dev/null || true)
  abs_prog=$(jq -r '.program // ""' 9>&- <<<"$row" 2>/dev/null || true)
  # A live capture MUST yield a full, sha256-pinned identity or nothing is written. An
  # empty sha256 is RESERVED for the operator-curated own-agent entries in the seed file
  # (their plists change with the dotfiles and are verified by the pipeline-integrity
  # manifest); it is never writer-produced, so a hash-capture failure fails CLOSED
  # rather than storing an unpinned tuple a later plist swap at the same path/program
  # could hide behind.
  if [[ -z $abs_path || -z $abs_prog ]]; then
    printf 'refused: %s has no loaded LaunchAgent to capture an identity from; load it and re-run\n' "$label" >&2
    exit 1
  fi
  sha=""
  if [[ -f $abs_path ]]; then
    sha=$(shasum -a 256 "$abs_path" 9>&- 2>/dev/null | awk '{print $1}' 9>&-) || sha=""
  fi
  if ! [[ $sha =~ ^[0-9a-f]{64}$ ]]; then
    printf 'refused: sha256 hash capture failed for %s; not writing an unpinned tuple\n' "$abs_path" >&2
    exit 1
  fi
  # Relativize a leading $HOME to ~/ (keeps the file user-agnostic; the alerter re-expands it).
  rel_path="${abs_path/#"$HOME"\//\~/}"
  rel_prog="${abs_prog//"$HOME"\//\~/}"
  # Refresh in place: drop any existing tuple for this label (preserving every other line
  # and all comments/blanks), then append the freshly captured tuple, so re-adding a label
  # updates its identity and never duplicates it. An unchanged identity therefore
  # reproduces the source byte for byte, which makes a repeated -a a true no-op: the
  # apply writes identical bytes and the manifest runner compares equal and installs
  # nothing.
  local source_file
  if ! source_file="$(allowlist_source_path)" || [[ -z $source_file ]]; then
    printf 'refused: could not resolve the chezmoi source for %s; the allowlist is chezmoi-managed and must be curated through its source\n' \
      "$ALLOWLIST" >&2
    exit 1
  fi
  local temp
  temp=$(mktemp 9>&-)
  _without_label "$label" "$source_file" >"$temp"
  jq -cn --arg label "$label" --arg path "$rel_path" --arg program "$rel_prog" --arg sha256 "$sha" \
    '{label:$label, path:$path, program:$program, sha256:$sha256}' 9>&- >>"$temp"
  publish_allowlist "$temp" "$source_file" || exit 1
  printf 'allowed: %s -> %s\n' "$label" "$abs_prog"
}

deny_label() {
  local label="$1"
  if ! is_valid_label "$label"; then
    printf 'refused (invalid or system label): %s\n' "$label" >&2
    exit 1
  fi
  local source_file
  if ! source_file="$(allowlist_source_path)" || [[ -z $source_file ]]; then
    printf 'refused: could not resolve the chezmoi source for %s; the allowlist is chezmoi-managed and must be curated through its source\n' \
      "$ALLOWLIST" >&2
    exit 1
  fi
  # Removing a label that was never allowed is a clean no-op: exit 0, nothing
  # deployed, no manifest refresh, a note on stdout (nothing on stderr), so a caller
  # can deny unconditionally. The SOURCE is what is consulted, because the source is
  # the authority an apply deploys from.
  if [[ ! -f $source_file ]] || ! grep -qF "\"label\":\"$label\"" "$source_file" 9>&- 2>/dev/null; then
    printf 'not present: %s\n' "$label"
    return 0
  fi
  local temp
  temp=$(mktemp 9>&-)
  _without_label "$label" "$source_file" >"$temp"
  publish_allowlist "$temp" "$source_file" || exit 1
  printf 'denied: %s\n' "$label"
}

# Print the current allowlist entries (one NDJSON tuple per line) verbatim to stdout,
# skipping comment/blank lines. An empty or absent allowlist prints nothing.
list_entries() {
  [[ -s $ALLOWLIST ]] || return 0
  local line
  while IFS= read -r line || [[ -n $line ]]; do
    case "$line" in
      '' | '#'*) continue ;;
    esac
    printf '%s\n' "$line"
  done <"$ALLOWLIST"
}

action=""
label=""
while getopts ':a:d:l' option; do
  case "$option" in
    a)
      action="allow"
      label="$OPTARG"
      ;;
    d)
      action="deny"
      label="$OPTARG"
      ;;
    l) action="list" ;;
    :)
      printf 'option -%s requires a label\n' "$OPTARG" >&2
      usage
      ;;
    *) usage ;;
  esac
done

case "$action" in
  allow | deny)
    # The lock covers the verb's entire read-modify-write, capture included.
    if ! take_allowlist_write_lock; then
      printf 'failed to set up the allowlist write lock (%s.lock)\n' "$ALLOWLIST" >&2
      exit 1
    fi
    ;;
esac

case "$action" in
  allow) allow_label "$label" ;;
  deny) deny_label "$label" ;;
  list) list_entries ;;
  *) usage ;;
esac
