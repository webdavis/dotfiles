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

# Rewrite the allowlist, preserving comment/blank lines and dropping any tuple for <label>.
# Reads $ALLOWLIST, writes the filtered result to stdout (a no-op if the file is absent).
_without_label() {
  local drop="$1" line
  [[ -f $ALLOWLIST ]] || return 0
  while IFS= read -r line || [[ -n $line ]]; do
    case "$line" in
      '' | '#'*)
        printf '%s\n' "$line"
        continue
        ;;
    esac
    [[ "$(entry_label "$line")" == "$drop" ]] && continue
    printf '%s\n' "$line"
  done <"$ALLOWLIST"
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
  # updates its identity and never duplicates it.
  mkdir -p "$(dirname "$ALLOWLIST" 9>&-)" 9>&-
  touch "$ALLOWLIST" 9>&-
  local temp
  temp=$(mktemp 9>&-)
  _without_label "$label" >"$temp"
  jq -cn --arg label "$label" --arg path "$rel_path" --arg program "$rel_prog" --arg sha256 "$sha" \
    '{label:$label, path:$path, program:$program, sha256:$sha256}' 9>&- >>"$temp"
  mv -f "$temp" "$ALLOWLIST" 9>&-
  printf 'allowed: %s -> %s\n' "$label" "$abs_prog"
}

deny_label() {
  local label="$1"
  if ! is_valid_label "$label"; then
    printf 'refused (invalid or system label): %s\n' "$label" >&2
    exit 1
  fi
  # Removing a label that was never allowed is a clean no-op: exit 0, file untouched,
  # a note on stdout (nothing on stderr), so a caller can deny unconditionally.
  if [[ ! -f $ALLOWLIST ]] || ! grep -qF "\"label\":\"$label\"" "$ALLOWLIST" 9>&- 2>/dev/null; then
    printf 'not present: %s\n' "$label"
    return 0
  fi
  local temp
  temp=$(mktemp 9>&-)
  _without_label "$label" >"$temp"
  mv -f "$temp" "$ALLOWLIST" 9>&-
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
