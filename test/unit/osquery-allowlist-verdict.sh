#!/usr/bin/env bash
#
# allowlist_verdict (results-alerter/allowlist-verdict.sh) decides whether a
# user LaunchAgent persistence finding is a known-good item to suppress, a
# reused-label attack to page, or simply not allowlisted. It reads the launchd
# page-allowlist (OSQUERY_LAUNCHD_ALLOWLIST, the NDJSON tuple file the slice-5
# writer curates) and matches the finding's identity as a FULL tuple.
#
# The identity the finding supplies is (label, path, program); the plist sha256
# is NOT one of the arguments and is NOT read from the osquery row (the
# persistence_launchd row carries no sha256 column) nor from enrichment - when a
# stored tuple PINS a hash, the verdict re-hashes the ON-DISK plist at the
# finding's path with shasum at decision time and compares. That defeats a
# same-label/same-path/same-program plist rewrite.
#
# Return-code contract (from c69baab _allowlist_verdict):
#   0 = suppress   - full tuple match (label+path+program, and the on-disk hash
#                    matches the pin, or the pin is empty so the hash dimension is
#                    skipped: the own-agent seed entries).
#   2 = reused-label / page - the label is allowlisted but the identity diverges
#                    (path/program differs, or the pinned hash no longer matches).
#                    This is the R2-1 property: a reused allowlisted label pointing
#                    at a different plist identity is never silently suppressed.
#   1 = not allowlisted - no label match, a degraded label-only entry that cannot
#                    vouch, or a missing/empty allowlist file.
#
# Unit test: a fixture tuple file + fixture on-disk plists under a temp HOME, so
# the live re-hash is exercised for real.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/allowlist-verdict.sh"

fail() {
  printf 'osquery-allowlist-verdict: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $HELPER ]] || fail "missing helper: $HELPER"
command -v shasum >/dev/null 2>&1 || fail "shasum is required for this test"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

home="$work/home"
mkdir -p "$home/Library/LaunchAgents" "$home/bin" "$home/.config/osquery"

# Fixture plists on disk. com.full's stored pin will match this content; com.hashpin's
# stored pin will deliberately NOT match, to exercise the on-disk re-hash mismatch.
printf 'FULL PLIST CONTENT\n' >"$home/Library/LaunchAgents/com.full.plist"
printf 'HASHPIN PLIST CONTENT ON DISK\n' >"$home/Library/LaunchAgents/com.hashpin.plist"
full_hash="$(shasum -a 256 "$home/Library/LaunchAgents/com.full.plist" | awk '{print $1}')"
wrong_hash="0000000000000000000000000000000000000000000000000000000000000000"

# The allowlist tuple file. Paths/programs are stored home-relative (~/) exactly
# as the committed, user-agnostic seed file does; the verdict expands ~ to $HOME.
# Includes a comment and a blank line to pin robustness, a full pinned entry, a
# wrong-hash entry, an empty-sha256 own-agent seed entry, and a degraded
# label-only entry (no path/program).
allowlist="$home/.config/osquery/page-launchd-allowlist.txt"
{
  printf '# curated by osquery-allowlist.sh\n'
  printf '\n'
  printf '{"label":"com.full","path":"~/Library/LaunchAgents/com.full.plist","program":"~/bin/full","sha256":"%s"}\n' "$full_hash"
  printf '{"label":"com.hashpin","path":"~/Library/LaunchAgents/com.hashpin.plist","program":"~/bin/hp","sha256":"%s"}\n' "$wrong_hash"
  printf '{"label":"com.seed","path":"~/Library/LaunchAgents/com.seed.plist","program":"~/bin/seed","sha256":""}\n'
  printf '{"label":"com.degraded","path":"","program":"","sha256":""}\n'
} >"$allowlist"

# Each case: <expected-rc> <TAB> <allowlist-file> <TAB> <label> <TAB> <path>
# <TAB> <program> <TAB> <behavior label>. Every case runs in ONE sourcing subshell
# (below) instead of a subshell per case, so the suite stays under the fast unit
# bar while the live on-disk re-hash is still exercised for real.
missing="$work/does-not-exist.txt"
cases=(
  # (a) full tuple match, on-disk hash matches the pin -> suppress.
  $'0\t'"$allowlist"$'\tcom.full\t'"$home/Library/LaunchAgents/com.full.plist"$'\t'"$home/bin/full"$'\tfull tuple match (label+path+program+matching on-disk hash) suppresses'
  # (b) same label, different program -> reused-label page (diverges before the hash check).
  $'2\t'"$allowlist"$'\tcom.full\t'"$home/Library/LaunchAgents/com.full.plist"$'\t'"$home/bin/EVIL"$'\tan allowlisted label with a different program pages (reused label)'
  # (b2) same label, same program, different path -> reused-label page.
  $'2\t'"$allowlist"$'\tcom.full\t'"$home/Library/LaunchAgents/EVIL.plist"$'\t'"$home/bin/full"$'\tan allowlisted label with a different path pages (reused label)'
  # (c) same label/path/program but the on-disk plist no longer matches the pin -> page.
  $'2\t'"$allowlist"$'\tcom.hashpin\t'"$home/Library/LaunchAgents/com.hashpin.plist"$'\t'"$home/bin/hp"$'\ta pinned-hash entry whose on-disk plist was rewritten pages (hash mismatch)'
  # (d) unknown label -> not allowlisted.
  $'1\t'"$allowlist"$'\tcom.unknown\t'"$home/Library/LaunchAgents/com.unknown.plist"$'\t'"$home/bin/unknown"$'\tan unknown label is not allowlisted'
  # (e) empty-sha256 seed entry -> suppress on label+path+program, skipping the hash dimension
  #     (the seed plist need not even exist).
  $'0\t'"$allowlist"$'\tcom.seed\t'"$home/Library/LaunchAgents/com.seed.plist"$'\t'"$home/bin/seed"$'\tan empty-sha256 seed entry suppresses on label+path+program, skipping the hash dimension'
  # (f) degraded label-only entry (no path/program) cannot vouch -> not allowlisted (fail-safe).
  $'1\t'"$allowlist"$'\tcom.degraded\t'"$home/Library/LaunchAgents/com.degraded.plist"$'\t'"$home/bin/degraded"$'\ta degraded label-only entry cannot vouch and does not suppress (fail-safe)'
  # (g) missing allowlist file -> not allowlisted, cleanly, no error.
  $'1\t'"$missing"$'\tcom.full\t'"$home/Library/LaunchAgents/com.full.plist"$'\t'"$home/bin/full"$'\ta missing allowlist file yields not-allowlisted for everything, no error'
)

# Split into parallel arrays and feed the (file, label, path, program) tuples
# through ONE sourcing subshell that prints a return code per line, in order.
expected=()
labels=()
feed=""
for row in "${cases[@]}"; do
  IFS=$'\t' read -r rc file label path program label_text <<<"$row"
  expected+=("$rc")
  labels+=("$label_text")
  feed+="$file"$'\t'"$label"$'\t'"$path"$'\t'"$program"$'\n'
done

got=()
mapfile -t got < <(
  printf '%s' "$feed" | HOME="$home" bash -c '
    source "$1"
    while IFS="$(printf "\t")" read -r file label path program; do
      OSQUERY_LAUNCHD_ALLOWLIST="$file"
      rc=0
      allowlist_verdict "$label" "$path" "$program" || rc=$?
      printf "%s\n" "$rc"
    done
  ' _ "$HELPER"
)

[[ ${#got[@]} -eq ${#expected[@]} ]] ||
  fail "the verdict driver emitted ${#got[@]} results for ${#expected[@]} cases (one per case expected)"

for i in "${!expected[@]}"; do
  [[ ${got[i]} == "${expected[i]}" ]] ||
    fail "${labels[i]}: expected return ${expected[i]}, got ${got[i]}"
done

printf 'osquery-allowlist-verdict: OK (full-tuple suppress, reused-label page on path/program/hash divergence, empty-sha256 seed suppress, unknown/degraded/missing not-allowlisted)\n'
