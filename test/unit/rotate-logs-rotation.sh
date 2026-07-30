#!/usr/bin/env bash
#
# The core behaviour: an oversized log is archived and then truncated IN PLACE,
# preserving its inode.
#
# Inode preservation is the whole reason this repo does not use newsyslog(8).
# Every log under ~/.local/log is a launchd StandardOutPath redirect, which means
# launchd opened the file and handed the descriptor to the daemon. Renaming the
# file (newsyslog's model) leaves the daemon writing to the renamed inode
# forever; truncating in place keeps the descriptor valid so the daemon's next
# write lands in the fresh file. This test pins that property directly.
#
# ROTATE_LOGS_ROOT is the seam: every path here is inside a mktemp sandbox, so
# no test ever reads or writes the real ~/.local/log.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROTATE_LOGS="$REPO_ROOT/dot_local/bin/executable_rotate-logs.sh"

failures=0
fail() {
  printf 'rotate-logs-rotation: FAIL -- %s\n' "$*" >&2
  failures=$((failures + 1))
}

[[ -f $ROTATE_LOGS ]] || {
  printf 'rotate-logs-rotation: FAIL -- missing script: %s\n' "$ROTATE_LOGS" >&2
  exit 1
}

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT
log_root="$sandbox/log"
mkdir -p "$log_root"

# stat(1) differs between GNU and BSD, and the nix dev shell can put GNU
# coreutils first on PATH even on macOS, where the BSD `-f` flag then means
# "filesystem status" and succeeds with the wrong output. GNU form first,
# BSD form as the fallback -- the order the repo guard enforces.
file_size() { stat -c '%s' "$1" 2>/dev/null || stat -f '%z' "$1"; }
file_inode() { stat -c '%i' "$1" 2>/dev/null || stat -f '%i' "$1"; }
file_mode() { stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"; }

THRESHOLD=1024

seed_file() { # <path> <bytes>
  local path="$1" bytes="$2"
  mkdir -p "$(dirname "$path")"
  : >"$path"
  [[ $bytes -gt 0 ]] && /usr/bin/head -c "$bytes" /dev/zero | /usr/bin/tr '\0' 'x' >"$path"
  return 0
}

big="$log_root/big-daemon.log"
small="$log_root/small-daemon.log"
nested="$log_root/nested/service.log"
seed_file "$big" 4096
seed_file "$small" 100
seed_file "$nested" 2048

big_inode_before="$(file_inode "$big")"
small_inode_before="$(file_inode "$small")"

ROTATE_LOGS_ROOT="$log_root" \
  ROTATE_LOGS_AT_BYTES="$THRESHOLD" \
  ROTATE_LOGS_ARCHIVES_KEPT=3 \
  bash "$ROTATE_LOGS" >"$sandbox/report.txt" 2>&1 ||
  fail "rotation pass exited non-zero: $(cat "$sandbox/report.txt")"

# --- the oversized log is truncated, not renamed ---------------------------
[[ -f $big ]] || fail "oversized log disappeared (it must be truncated, never renamed away)"
big_size_after="$(file_size "$big")"
[[ $big_size_after -eq 0 ]] ||
  fail "oversized log was not truncated: size is $big_size_after, want 0"

big_inode_after="$(file_inode "$big")"
[[ $big_inode_before == "$big_inode_after" ]] ||
  fail "inode changed ($big_inode_before -> $big_inode_after): a held descriptor would be orphaned"

# --- its content survived in a compressed archive --------------------------
archive="$big.1.gz"
[[ -f $archive ]] || fail "no archive at $archive; rotated content must be retained"
if [[ -f $archive ]]; then
  recovered="$(/usr/bin/gzip -dc "$archive" | /usr/bin/wc -c | /usr/bin/tr -d ' ')"
  [[ $recovered -eq 4096 ]] ||
    fail "archive holds $recovered bytes, want the full 4096 that were rotated out"
fi

# --- a log under the threshold is left completely alone --------------------
small_size_after="$(file_size "$small")"
[[ $small_size_after -eq 100 ]] ||
  fail "under-threshold log was modified: size is $small_size_after, want 100"
[[ $small_inode_before == "$(file_inode "$small")" ]] ||
  fail "under-threshold log inode changed; it must not be touched at all"
if [[ -e $small.1.gz ]]; then
  fail "under-threshold log was archived; it must not be"
fi

# --- the scan reaches nested directories -----------------------------------
nested_size_after="$(file_size "$nested")"
[[ $nested_size_after -eq 0 ]] ||
  fail "nested log was not rotated (size $nested_size_after); the scan must recurse"
[[ -f $nested.1.gz ]] || fail "no archive for the nested log"

# --- archives are owner-only ------------------------------------------------
if [[ -f $archive ]]; then
  archive_mode="$(file_mode "$archive")"
  [[ $archive_mode == "600" ]] ||
    fail "archive mode is $archive_mode, want 600 (log content stays owner-only)"
fi

if [[ $failures -gt 0 ]]; then
  printf 'rotate-logs-rotation: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi

printf 'rotate-logs-rotation: OK (truncate-in-place, inode preserved, content archived)\n'
