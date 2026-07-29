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
PIPELINE_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"

fail() {
  printf 'osquery-allowlist-verdict: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $HELPER ]] || fail "missing helper: $HELPER"
[[ -f $PIPELINE_HELPER ]] || fail "missing helper: $PIPELINE_HELPER"
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
  printf '{"label":"com.seedtampered","path":"~/Library/LaunchAgents/com.seedtampered.plist","program":"~/bin/seed","sha256":""}\n'
  printf '{"label":"com.seedlink","path":"~/Library/LaunchAgents/com.seedlink.plist","program":"~/bin/seed","sha256":""}\n'
  printf '{"label":"com.degraded","path":"","program":"","sha256":""}\n'
} >"$allowlist"
# The allowlist deploys 0600 (chezmoi's private_ prefix).
chmod 600 "$allowlist"

# THE MANIFEST BINDING (D-prime). The allowlist decides whether an unknown user
# LaunchAgent pages, so an allowlist the root-owned pipeline-integrity manifest
# cannot vouch for may not suppress ANYTHING. This is the fixture for the bound
# (post-apply) state: a manifest tuple naming the allowlist's current content, its
# 0600 mode and its owner.
bound_manifest="$work/pipeline-known-good.sha256"
# The seed plist is a REAL file here, and the manifest vouches for it alongside the
# allowlist. An empty-sha256 entry carries no pin of its own, so the manifest is the
# only thing that can say anything about the bytes at that path; a fixture where the
# manifest does not cover the plist cannot tell a vouched plist from a rewritten one.
printf 'SEED PLIST CONTENT\n' >"$home/Library/LaunchAgents/com.seed.plist"
write_bound_manifest() {
  {
    printf '%s 0600 %s %s\n' \
      "$(shasum -a 256 "$allowlist" | awk '{print $1}')" "$(id -u)" "$allowlist"
    printf '%s 0644 %s %s\n' \
      "$(shasum -a 256 "$home/Library/LaunchAgents/com.seed.plist" | awk '{print $1}')" \
      "$(id -u)" "$home/Library/LaunchAgents/com.seed.plist"
  } >"$bound_manifest"
}
write_bound_manifest
chmod 0644 "$home/Library/LaunchAgents/com.seed.plist"

# The tampered case. Its plist is recorded in the manifest as it was, then REWRITTEN.
# The allowlist file is not touched, so the D-prime binding still passes; only the
# plist's own bytes have moved out from under the manifest.
printf 'SEEDTAMPERED ORIGINAL\n' >"$home/Library/LaunchAgents/com.seedtampered.plist"
chmod 0644 "$home/Library/LaunchAgents/com.seedtampered.plist"
printf '%s 0644 %s %s\n' \
  "$(shasum -a 256 "$home/Library/LaunchAgents/com.seedtampered.plist" | awk '{print $1}')" \
  "$(id -u)" "$home/Library/LaunchAgents/com.seedtampered.plist" >>"$bound_manifest"
printf 'SEEDTAMPERED REWRITTEN BY AN ATTACKER\n' >"$home/Library/LaunchAgents/com.seedtampered.plist"

# The symlink case. com.seedlink's plist path holds a SYMLINK to an attacker-owned
# copy of the manifested bytes, outside the watched tree, where nothing pages when
# it is rewritten afterwards. The manifest tuple is written FROM THE LINK with the
# production readers, so every column matches exactly what the verdict reads back:
# shasum hashes THROUGH the link to the referent's pristine content, and the
# mode/uid readers lstat the link itself. Nothing distinguishes this from the bound
# com.seed case except the file kind, which is the point: only a refusal to judge a
# non-regular file can tell them apart.
seedlink="$home/Library/LaunchAgents/com.seedlink.plist"
seedlink_referent="$work/attacker-copy.plist"
printf 'SEEDLINK PLIST CONTENT\n' >"$seedlink_referent"
chmod 0644 "$seedlink_referent"
ln -s "$seedlink_referent" "$seedlink"
printf '%s %s %s %s\n' \
  "$(shasum -a 256 "$seedlink" | awk '{print $1}')" \
  "$(bash -c 'source "$1"; _pipeline_file_mode "$2"' _ "$PIPELINE_HELPER" "$seedlink")" \
  "$(bash -c 'source "$1"; _pipeline_file_uid "$2"' _ "$PIPELINE_HELPER" "$seedlink")" \
  "$seedlink" >>"$bound_manifest"
absent_manifest="$work/no-such-manifest.sha256"

# Each case: <expected-rc> <TAB> <manifest> <TAB> <allowlist-file> <TAB> <label>
# <TAB> <path> <TAB> <program> <TAB> <behavior label>. Every case runs in ONE
# sourcing subshell (below) instead of a subshell per case, so the suite stays under
# the fast unit bar while the live on-disk re-hash is still exercised for real.
#
# The manifest column is what pins D-prime: a suppress verdict is only reachable
# when the root-owned manifest vouches for the allowlist the verdict just read.
missing="$work/does-not-exist.txt"
cases=(
  # (a) full tuple match, on-disk hash matches the pin -> suppress.
  $'0\t'"$bound_manifest"$'\t'"$allowlist"$'\tcom.full\t'"$home/Library/LaunchAgents/com.full.plist"$'\t'"$home/bin/full"$'\tfull tuple match (label+path+program+matching on-disk hash) suppresses'
  # (b) same label, different program -> reused-label page (diverges before the hash check).
  $'2\t'"$bound_manifest"$'\t'"$allowlist"$'\tcom.full\t'"$home/Library/LaunchAgents/com.full.plist"$'\t'"$home/bin/EVIL"$'\tan allowlisted label with a different program pages (reused label)'
  # (b2) same label, same program, different path -> reused-label page.
  $'2\t'"$bound_manifest"$'\t'"$allowlist"$'\tcom.full\t'"$home/Library/LaunchAgents/EVIL.plist"$'\t'"$home/bin/full"$'\tan allowlisted label with a different path pages (reused label)'
  # (c) same label/path/program but the on-disk plist no longer matches the pin -> page.
  $'2\t'"$bound_manifest"$'\t'"$allowlist"$'\tcom.hashpin\t'"$home/Library/LaunchAgents/com.hashpin.plist"$'\t'"$home/bin/hp"$'\ta pinned-hash entry whose on-disk plist was rewritten pages (hash mismatch)'
  # (d) unknown label -> not allowlisted.
  $'1\t'"$bound_manifest"$'\t'"$allowlist"$'\tcom.unknown\t'"$home/Library/LaunchAgents/com.unknown.plist"$'\t'"$home/bin/unknown"$'\tan unknown label is not allowlisted'
  # (e) empty-sha256 seed entry, on-disk plist STILL MATCHES the manifest -> suppress.
  #     The entry carries no pin, so the manifest is what vouches for the bytes.
  $'0\t'"$bound_manifest"$'\t'"$allowlist"$'\tcom.seed\t'"$home/Library/LaunchAgents/com.seed.plist"$'\t'"$home/bin/seed"$'\tan empty-sha256 seed entry suppresses on label+path+program, skipping the hash dimension'
  # (e2) THE HOLE. Same empty-sha256 entry, but the plist at that path has been
  #      REWRITTEN since the manifest recorded it. The allowlist file itself is
  #      untouched, so the D-prime binding is satisfied and cannot help here.
  #      Carrying no pin must mean "the manifest vouches for this", not "trust
  #      whatever is at this path", or a rewritten own-agent plist is silenced.
  $'1\t'"$bound_manifest"$'\t'"$allowlist"$'\tcom.seedtampered\t'"$home/Library/LaunchAgents/com.seedtampered.plist"$'\t'"$home/bin/seed"$'\tan empty-sha256 entry whose on-disk plist no longer matches the manifest cannot vouch'
  # (e3) THE SAME HOLE IN A SECOND SHAPE. The unpinned entry plist path holds a
  #      SYMLINK to an attacker-owned copy of the manifested bytes. shasum hashes
  #      THROUGH the link and the mode/uid readers lstat a link whose tuple the
  #      fixture recorded verbatim, so content, mode and owner all read back
  #      matching; only refusing to judge a non-regular file tells the link from
  #      the file, and the referent stays rewritable where nothing watches it.
  $'1\t'"$bound_manifest"$'\t'"$allowlist"$'\tcom.seedlink\t'"$home/Library/LaunchAgents/com.seedlink.plist"$'\t'"$home/bin/seed"$'\ta symlink standing at an unpinned entry plist path is never vouched, even with the manifested bytes at its referent'
  # (f) degraded label-only entry (no path/program) cannot vouch -> not allowlisted (fail-safe).
  $'1\t'"$bound_manifest"$'\t'"$allowlist"$'\tcom.degraded\t'"$home/Library/LaunchAgents/com.degraded.plist"$'\t'"$home/bin/degraded"$'\ta degraded label-only entry cannot vouch and does not suppress (fail-safe)'
  # (g) missing allowlist file -> not allowlisted, cleanly, no error.
  $'1\t'"$bound_manifest"$'\t'"$missing"$'\tcom.full\t'"$home/Library/LaunchAgents/com.full.plist"$'\t'"$home/bin/full"$'\ta missing allowlist file yields not-allowlisted for everything, no error'
  # -- D-PRIME: an allowlist the manifest cannot vouch for suppresses NOTHING. --
  # (h) NO MANIFEST. Detection after the fact is nearly worthless for a component
  #     whose whole job is to suppress, so an unbindable allowlist fails toward
  #     paging instead of quietly vouching for its entries.
  $'1\t'"$absent_manifest"$'\t'"$allowlist"$'\tcom.full\t'"$home/Library/LaunchAgents/com.full.plist"$'\t'"$home/bin/full"$'\ta full tuple match with NO manifest does not suppress (an unbound allowlist vouches for nothing)'
  $'1\t'"$absent_manifest"$'\t'"$allowlist"$'\tcom.seed\t'"$home/Library/LaunchAgents/com.seed.plist"$'\t'"$home/bin/seed"$'\tan own-agent seed entry with NO manifest does not suppress'
  # (i) A reused label still pages with no manifest: the gate is on the SUPPRESS
  #     path only, so the louder verdict is never softened into a quieter one.
  $'2\t'"$absent_manifest"$'\t'"$allowlist"$'\tcom.full\t'"$home/Library/LaunchAgents/com.full.plist"$'\t'"$home/bin/EVIL"$'\ta reused label still pages when the manifest is absent (the gate only blocks suppression)'
)

# Split into parallel arrays and feed the (manifest, file, label, path, program)
# tuples through ONE sourcing subshell that prints a return code per line, in order.
expected=()
labels=()
feed=""
for row in "${cases[@]}"; do
  IFS=$'\t' read -r rc case_manifest file label path program label_text <<<"$row"
  expected+=("$rc")
  labels+=("$label_text")
  feed+="$case_manifest"$'\t'"$file"$'\t'"$label"$'\t'"$path"$'\t'"$program"$'\n'
done

# Both helpers are sourced, in the order results-alerter.sh sources them, because
# the manifest binding is a real dependency of the verdict and not a test fixture.
# The settle window is zeroed so a miss answers immediately: the window's own
# behavior belongs to the manifest-agreement suite, and a unit test must not sleep.
got=()
mapfile -t got < <(
  printf '%s' "$feed" | HOME="$home" OSQUERY_PIPELINE_SETTLE_SECONDS=0 bash -c '
    source "$1"
    source "$2"
    while IFS="$(printf "\t")" read -r case_manifest file label path program; do
      OSQUERY_LAUNCHD_ALLOWLIST="$file"
      OSQUERY_PIPELINE_MANIFEST="$case_manifest"
      rc=0
      allowlist_verdict "$label" "$path" "$program" || rc=$?
      printf "%s\n" "$rc"
    done
  ' _ "$HELPER" "$PIPELINE_HELPER"
)

[[ ${#got[@]} -eq ${#expected[@]} ]] ||
  fail "the verdict driver emitted ${#got[@]} results for ${#expected[@]} cases (one per case expected)"

for i in "${!expected[@]}"; do
  [[ ${got[i]} == "${expected[i]}" ]] ||
    fail "${labels[i]}: expected return ${expected[i]}, got ${got[i]}"
done

# --- THE ATTACK THIS EXISTS TO STOP ------------------------------------------
# A process running as the operator appends a tuple naming its own LaunchAgent,
# with the empty sha256 that skips the hash dimension, and installs that agent.
# Before D-prime the verdict returned suppress, the persistence finding was
# dropped, and the edit itself only reached the next day's digest.
#
# Appending changes the allowlist's bytes, so it no longer matches the tuple the
# root-owned manifest holds for it, and an allowlist nothing can vouch for
# suppresses NOTHING: not the attacker's freshly added entry, and not the
# legitimate entry that was suppressing a moment ago. The manifest is NOT
# regenerated here, which is the whole point: regenerating it is an apply, and an
# apply is the operator's authority, not the attacker's.
verdict_with() { # <label> <path> <program> -> prints the return code
  local rc=0
  HOME="$home" OSQUERY_PIPELINE_SETTLE_SECONDS=0 \
    OSQUERY_LAUNCHD_ALLOWLIST="$allowlist" OSQUERY_PIPELINE_MANIFEST="$bound_manifest" \
    bash -c 'source "$1"; source "$2"; allowlist_verdict "$3" "$4" "$5"' \
    _ "$HELPER" "$PIPELINE_HELPER" "$1" "$2" "$3" || rc=$?
  printf '%s' "$rc"
}

# Baseline: bound allowlist, the legitimate entry suppresses.
[[ "$(verdict_with com.full "$home/Library/LaunchAgents/com.full.plist" "$home/bin/full")" == 0 ]] ||
  fail "the bound-allowlist baseline does not suppress, so the attack pin below would pass vacuously"

printf '{"label":"com.evil","path":"~/Library/LaunchAgents/com.evil.plist","program":"~/bin/evil","sha256":""}\n' \
  >>"$allowlist"

[[ "$(verdict_with com.evil "$home/Library/LaunchAgents/com.evil.plist" "$home/bin/evil")" == 1 ]] ||
  fail "SECURITY: a tuple appended to the allowlist out of band SUPPRESSED its own agent (D-prime is not enforcing)"
[[ "$(verdict_with com.full "$home/Library/LaunchAgents/com.full.plist" "$home/bin/full")" == 1 ]] ||
  fail "SECURITY: a tampered allowlist still vouched for its pre-existing entries"

# ...and a legitimate apply restores suppression: the operator's edit regenerates
# the manifest in the same flow, so the file is bound again and nothing false-pages.
#
# The entry is REWRITTEN WITH A PIN here, because that is the only form the writer
# can produce: `allowlist.sh -a` refuses to write an unpinned tuple, so a
# third-party agent adopted through it always carries a captured hash. An unpinned
# tuple is reserved for the own-agent seeds, whose plists chezmoi manages and the
# manifest therefore covers; com.evil's plist is neither. Modelling the adopted
# entry as unpinned would be modelling something the writer cannot emit.
printf 'EVIL PLIST CONTENT\n' >"$home/Library/LaunchAgents/com.evil.plist"
evil_hash="$(shasum -a 256 "$home/Library/LaunchAgents/com.evil.plist" | awk '{print $1}')"
grep -vF '"label":"com.evil"' "$allowlist" >"$allowlist.tmp" && mv "$allowlist.tmp" "$allowlist"
# mv brings the temp file's mode with it; the manifest tuple pins 0600, and mode is
# part of what it vouches for, so restore it or the file reads as tampered.
chmod 0600 "$allowlist"
printf '{"label":"com.evil","path":"~/Library/LaunchAgents/com.evil.plist","program":"~/bin/evil","sha256":"%s"}\n' \
  "$evil_hash" >>"$allowlist"
write_bound_manifest
[[ "$(verdict_with com.full "$home/Library/LaunchAgents/com.full.plist" "$home/bin/full")" == 0 ]] ||
  fail "a legitimate apply (allowlist edited, manifest regenerated) must restore suppression, not false-page"
[[ "$(verdict_with com.evil "$home/Library/LaunchAgents/com.evil.plist" "$home/bin/evil")" == 0 ]] ||
  fail "an entry seeded through the writer (source edit + apply + manifest refresh) must suppress"

# --- A PARTIAL INSTALL FAILS TOWARD PAGING -----------------------------------
# results-alerter.sh sources both helpers, so a missing pipeline verdict aborts the
# alerter outright and this state is not reachable in production. It is checked by
# NAME anyway, for the reason the periodic audit checks its own reused seam: a
# monitor that goes quiet when its own dependency is absent is the failure mode
# this whole subsystem exists to avoid.
rc=0
HOME="$home" OSQUERY_LAUNCHD_ALLOWLIST="$allowlist" OSQUERY_PIPELINE_MANIFEST="$bound_manifest" \
  bash -c 'source "$1"; allowlist_verdict "$2" "$3" "$4"' \
  _ "$HELPER" com.full "$home/Library/LaunchAgents/com.full.plist" "$home/bin/full" || rc=$?
[[ $rc == 1 ]] ||
  fail "SECURITY: with the pipeline verdict helper absent the allowlist still suppressed (expected 1, got $rc)"

printf 'osquery-allowlist-verdict: OK (full-tuple suppress, reused-label page on path/program/hash divergence, empty-sha256 seed suppress, unknown/degraded/missing not-allowlisted; D-prime: an allowlist the manifest cannot vouch for suppresses nothing, an out-of-band appended tuple never silences its own agent, a legitimate apply restores suppression, and a missing verdict helper fails toward paging)\n'
