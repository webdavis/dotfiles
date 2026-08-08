#!/usr/bin/env bash
#
# What the rotator must REFUSE to touch, and why each refusal is the safe
# direction rather than a convenience:
#
#   symlink          osquery publishes ~/.local/log/osquery/osqueryd.INFO as a
#                    symlink to its current generation. Truncating through it
#                    would blank a file osquery owns and still holds open.
#   not writable     the osqueryd.results/snapshots logs are root-owned. A user
#                    -scope pass cannot truncate them and must not pretend to;
#                    it reports them instead of failing the whole run.
#   existing archive rotating a .N.gz would re-compress our own output forever.
#
# A skip must be LOUD. Silently passing over a file that is growing is the
# fail-open direction, so each skipped path is named in the report.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROTATE_LOGS="$REPO_ROOT/dot_local/libexec/executable_compress-and-truncate-local-logs.sh"

failures=0
fail() {
  printf 'rotate-logs-skips-unmanageable: FAIL -- %s\n' "$*" >&2
  failures=$((failures + 1))
}

[[ -f $ROTATE_LOGS ]] || {
  printf 'rotate-logs-skips: FAIL -- missing script: %s\n' "$ROTATE_LOGS" >&2
  exit 1
}

sandbox="$(mktemp -d)"
trap 'chmod -R u+w "$sandbox" 2>/dev/null; rm -rf "$sandbox"' EXIT
log_root="$sandbox/log"
mkdir -p "$log_root"

# stat(1) differs between GNU and BSD, and a gnubin-fronted PATH can put
# GNU coreutils first even on macOS, where the BSD `-f` flag then means
# "filesystem status" and succeeds with the wrong output. GNU form first,
# BSD form as the fallback -- the order the repo guard enforces.
file_size() { stat -c '%s' "$1" 2>/dev/null || stat -f '%z' "$1"; }
file_inode() { stat -c '%i' "$1" 2>/dev/null || stat -f '%i' "$1"; }

big_bytes() { /usr/bin/head -c 4096 /dev/zero | /usr/bin/tr '\0' 'z'; }

# A real oversized target, so we can tell "the pass ran" from "the pass did nothing".
control="$log_root/control.log"
big_bytes >"$control"

# 1. A symlink pointing at an oversized file outside the managed set.
target="$sandbox/outside-target.log"
big_bytes >"$target"
target_inode="$(file_inode "$target")"
ln -s "$target" "$log_root/current.log"

# 2. An oversized file we cannot write.
unwritable="$log_root/root-owned.log"
big_bytes >"$unwritable"
chmod 000 "$unwritable"

# 3. An oversized file that is already one of our archives.
archive="$log_root/already.log.1.gz"
big_bytes >"$archive"
archive_inode="$(file_inode "$archive")"

# 4. A SMALL unwritable file. Reporting it would be noise: it is not growing,
#    so nothing about it needs a human. This machine carries roughly fifty
#    root-owned osquery files of a few hundred bytes each, and naming them every
#    hour would bury the one line that matters under a page of ones that do not.
small_unwritable="$log_root/tiny-root-owned.log"
printf 'small\n' >"$small_unwritable"
chmod 000 "$small_unwritable"

report="$sandbox/report.txt"
ROTATE_LOGS_ROOT="$log_root" \
  ROTATE_LOGS_AT_BYTES=1024 \
  ROTATE_LOGS_ARCHIVES_KEPT=3 \
  bash "$ROTATE_LOGS" >"$report" 2>&1 ||
  fail "rotation pass exited non-zero: $(cat "$report")"

# The control proves the pass actually did work; without it every assertion
# below would pass just as well against a script that scanned nothing.
[[ "$(file_size "$control")" -eq 0 ]] ||
  fail "control log was not rotated; the pass did not do any work, so the skips below prove nothing"

# --- symlink: neither the link nor its target is touched -------------------
[[ -L $log_root/current.log ]] || fail "the symlink itself was replaced or removed"
[[ "$(file_size "$target")" -eq 4096 ]] ||
  fail "truncated through a symlink: target is now $(file_size "$target") bytes"
[[ "$(file_inode "$target")" == "$target_inode" ]] || fail "symlink target inode changed"
if [[ -e $target.1.gz || -e $log_root/current.log.1.gz ]]; then
  fail "a symlink was archived"
fi

# --- unwritable: left intact and named in the report -----------------------
chmod u+r "$unwritable" 2>/dev/null || true
[[ "$(file_size "$unwritable")" -eq 4096 ]] ||
  fail "an unwritable log was modified"
/usr/bin/grep -q 'root-owned.log' "$report" ||
  fail "an OVERSIZED unwritable log was skipped SILENTLY; that is the fail-open case and must be reported"

# --- but a small unmanageable file is NOT reported --------------------------
chmod u+r "$small_unwritable" 2>/dev/null || true
if /usr/bin/grep -q 'tiny-root-owned.log' "$report"; then
  fail "a small unwritable file was reported; only an over-threshold skip is worth a line"
fi
[[ "$(file_size "$small_unwritable")" -eq 6 ]] ||
  fail "a small unwritable file was modified"

# --- existing archive: not re-rotated --------------------------------------
[[ "$(file_inode "$archive")" == "$archive_inode" ]] ||
  fail "an existing archive was rotated"
if [[ -e $log_root/already.log.1.gz.1.gz ]]; then
  fail "an archive was re-archived into a second generation"
fi
# Silently, too. An archive is ours and already bounded by the retention window,
# so an oversized one is normal and must not be dressed up as a problem the
# operator needs to look at.
if /usr/bin/grep -q 'already.log.1.gz' "$report"; then
  fail "an oversized archive was reported as unmanageable; archives are bounded by retention and must stay silent"
fi

# --- is_rotatable_file, called directly -------------------------------------
# The end-to-end assertions above pass for the symlink case even if the -L guard
# is deleted, because `find -type f` already declines to emit symlinks. That
# makes the scan the only thing under test and leaves the predicate's own
# contract unpinned. These call the predicate directly, so each refusal is
# attributed to the layer that actually implements it.
# shellcheck source=/dev/null
source "$ROTATE_LOGS"

predicate_dir="$sandbox/predicate"
mkdir -p "$predicate_dir"
plain="$predicate_dir/plain.log"
: >"$plain"

link_to_plain="$predicate_dir/link.log"
ln -s "$plain" "$link_to_plain"

if is_rotatable_file "$link_to_plain"; then
  fail "is_rotatable_file accepts a symlink; only find's -type f would be stopping it"
fi
if ! is_rotatable_file "$plain"; then
  fail "is_rotatable_file rejects an ordinary writable log; the refusals above prove nothing"
fi
if is_rotatable_file "$predicate_dir"; then
  fail "is_rotatable_file accepts a directory"
fi
if is_rotatable_file "$predicate_dir/does-not-exist.log"; then
  fail "is_rotatable_file accepts a path that does not exist"
fi
archive_shaped="$predicate_dir/plain.log.2.gz"
: >"$archive_shaped"
if is_rotatable_file "$archive_shaped"; then
  fail "is_rotatable_file accepts one of our own archives"
fi
not_writable="$predicate_dir/locked.log"
: >"$not_writable"
chmod 000 "$not_writable"
if is_rotatable_file "$not_writable"; then
  fail "is_rotatable_file accepts a file it cannot write"
fi
chmod u+rw "$not_writable" 2>/dev/null || true

if [[ $failures -gt 0 ]]; then
  printf 'rotate-logs-skips-unmanageable: %d assertion(s) failed\n' "$failures" >&2
  exit 1
fi

printf 'rotate-logs-skips-unmanageable: OK (symlink, unwritable, archive all refused and reported)\n'
