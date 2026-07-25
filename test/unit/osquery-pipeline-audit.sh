#!/usr/bin/env bash
#
# pipeline_audit_scan (dot_local/libexec/osquery/pipeline-audit.sh) is the
# PERIODIC CONTENT AUDIT seam: it reads the root-owned pipeline-integrity
# manifest, hashes every path the manifest lists, and reports each path whose
# CURRENT state disagrees with its recorded tuple.
#
# Why it exists. The manifest was enforced only by osquery file_events, and
# osquery watches PATHS. An attacker who hard-links a manifested script to a
# writable path outside the pipeline home and overwrites the alias mutates the
# SAME INODE while the filesystem event names the outside path: no event ever
# reaches the watched path, no verdict runs, and the tampered script executes with
# nothing paged. This audit is the answer, because it compares bytes on a
# schedule and never depends on an event having fired.
#
# Contract:
#   return 0 = the scan COMPLETED. Every divergence is one stdout line,
#              "<kind> <path>"; no output means the deployed tree matches.
#   return 1 = the scan could NOT be completed, and stdout is a single reason
#              TOKEN from a fixed vocabulary (missing, untrustworthy, malformed,
#              overlong, budget). A caller that cannot verify must page: this is
#              a monitor, so a broken input is a loud condition, never a quiet
#              all-clear.
#
# The completion/return split is the fail-safe hinge: "no output" alone would
# read identically for "nothing diverged" and "the scan died before it started".
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
AUDIT_HELPER="$REPO_ROOT/dot_local/libexec/osquery/executable_pipeline-audit.sh"
VERDICT_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"

fails=0
fail() {
  printf 'osquery-pipeline-audit: FAIL -- %s\n' "$*" >&2
  fails=$((fails + 1))
}

[[ -f $AUDIT_HELPER ]] || {
  printf 'osquery-pipeline-audit: FAIL -- missing helper: %s\n' "$AUDIT_HELPER" >&2
  exit 1
}
[[ -f $VERDICT_HELPER ]] || {
  printf 'osquery-pipeline-audit: FAIL -- missing helper: %s\n' "$VERDICT_HELPER" >&2
  exit 1
}
command -v shasum >/dev/null 2>&1 || {
  printf 'osquery-pipeline-audit: FAIL -- shasum is required for this test\n' >&2
  exit 1
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
home="$work/home"
pipeline="$home/.local/libexec/osquery"
mkdir -p "$pipeline/results-alerter"

# The helpers are installed at the SAME relative paths the deploy uses, because the
# audit sources the verdict helper from the pipeline home (it reuses the manifest
# constant and the root-ownership check rather than keeping a second copy).
cp "$AUDIT_HELPER" "$pipeline/pipeline-audit.sh"
cp "$VERDICT_HELPER" "$pipeline/results-alerter/pipeline-verdict.sh"

sha_of() { shasum -a 256 -- "$1" | awk '{print $1}'; }

# The manifested fixture set: an ordinary script, a second script whose name holds
# a SPACE (the manifest binds one path per line, so a space in a path must not be
# read as a field separator), and a plist.
good_script="$pipeline/digest.sh"
spaced_script="$pipeline/two words.sh"
plist="$home/Library/LaunchAgents/com.webdavis.osquery-digest.plist"
mkdir -p "$home/Library/LaunchAgents"
printf 'echo digest\n' >"$good_script"
printf 'echo spaced\n' >"$spaced_script"
printf '<plist/>\n' >"$plist"

manifest="$work/pipeline-known-good.sha256"
write_manifest() {
  {
    printf '%s  %s\n' "$(sha_of "$good_script")" "$good_script"
    printf '%s  %s\n' "$(sha_of "$spaced_script")" "$spaced_script"
    printf '%s  %s\n' "$(sha_of "$plist")" "$plist"
  } >"$manifest"
}
write_manifest

# run_scan [VAR=value ...] -- run the audit in a fresh shell against the fixture
# manifest. SCAN_RC is the return code, SCAN_OUT the stdout.
SCAN_RC=0
SCAN_OUT=""
run_scan() {
  SCAN_RC=0
  # The child script is single-quoted on purpose: $1 must be expanded by the CHILD
  # shell, not by this one. `env` hides the bash -c from shellcheck's nested-script
  # analysis, hence the explicit directive.
  # shellcheck disable=SC2016
  SCAN_OUT="$(env HOME="$home" OSQUERY_PIPELINE_MANIFEST="$manifest" "$@" \
    bash -c 'source "$1"
      pipeline_audit_scan' _ "$pipeline/pipeline-audit.sh")" || SCAN_RC=$?
}

# expect_rc <label> <want> / expect_out <label> <want-exact-stdout>
expect_rc() {
  [[ $SCAN_RC -eq $2 ]] || fail "$1: expected return $2, got $SCAN_RC (stdout: $SCAN_OUT)"
}
expect_out() {
  [[ $SCAN_OUT == "$2" ]] || fail "$1: expected stdout [$2], got [$SCAN_OUT]"
}

# --- a deployed tree that matches the manifest is clean -----------------------
run_scan
expect_rc "an untampered tree completes" 0
expect_out "an untampered tree reports no divergence" ""

# --- the headline: content that diverges is reported, with NO event involved ---
printf 'echo tampered\n' >>"$good_script"
run_scan
expect_rc "a tampered file still completes the scan" 0
expect_out "tampered content is reported" "content $good_script"
printf 'echo digest\n' >"$good_script" # restore

# --- a path whose file is GONE ------------------------------------------------
mv "$plist" "$work/plist.stashed"
run_scan
expect_rc "a missing manifested path still completes the scan" 0
expect_out "a missing manifested path is reported" "missing $plist"
mv "$work/plist.stashed" "$plist"

# --- a symlink standing where a manifested regular file belongs ---------------
# The referent holds the EXACT bytes the manifest vouches for, so an audit that
# followed the link would call this clean. It must not: the manifest binds a
# regular file at that path, and the executed bytes now live somewhere the
# manifest does not cover and nothing watches.
mv "$good_script" "$work/relocated-payload.sh"
ln -s "$work/relocated-payload.sh" "$good_script"
run_scan
expect_rc "a symlinked manifested path still completes the scan" 0
expect_out "a symlink at a manifested path is reported even when its referent matches" \
  "irregular $good_script"
rm -f "$good_script"
mv "$work/relocated-payload.sh" "$good_script"

# --- a non-regular file (a directory) standing at a manifested path -----------
rm -f "$good_script"
mkdir -p "$good_script"
run_scan
expect_rc "a non-regular manifested path still completes the scan" 0
expect_out "a directory at a manifested path is reported" "irregular $good_script"
rmdir "$good_script"
printf 'echo digest\n' >"$good_script"

# --- a path with a SPACE is one manifest entry, not two fields ----------------
printf 'echo tampered\n' >>"$spaced_script"
run_scan
expect_rc "a spaced path completes the scan" 0
expect_out "a manifested path containing a space is read whole and judged" \
  "content $spaced_script"
printf 'echo spaced\n' >"$spaced_script" # restore
run_scan
expect_out "a restored spaced path is clean (the space is not a false divergence)" ""

# --- every divergence in one pass, in manifest order --------------------------
printf 'echo tampered\n' >>"$good_script"
mv "$plist" "$work/plist.stashed"
run_scan
expect_rc "a multi-divergence scan completes" 0
expect_out "every diverging path is reported, not just the first" \
  "content $good_script
missing $plist"
printf 'echo digest\n' >"$good_script"
mv "$work/plist.stashed" "$plist"

# --- the scan is BOUNDED, and every bound fails toward a page -----------------
# A manifested file larger than the per-file cap is not hashed at all (hashing an
# attacker-grown file is the only unbounded work in the tick) and is itself a
# divergence: our pipeline files are a few kilobytes.
run_scan OSQUERY_PIPELINE_AUDIT_MAX_BYTES=1
expect_rc "an over-cap file does not abort the scan" 0
[[ $SCAN_OUT == *"oversize $good_script"* ]] ||
  fail "an over-cap manifested file must be reported, got [$SCAN_OUT]"

run_scan OSQUERY_PIPELINE_AUDIT_MAX_ENTRIES=2
expect_rc "a manifest with more entries than the audit will examine refuses" 1
expect_out "an over-long manifest reports the overlong token" "overlong"

run_scan OSQUERY_PIPELINE_AUDIT_BUDGET_SECONDS=0
expect_rc "an exhausted time budget refuses" 1
expect_out "an exhausted time budget reports the budget token" "budget"

# --- an unusable manifest is a LOUD failure, never a quiet all-clear ----------
run_scan OSQUERY_PIPELINE_MANIFEST="$work/no-such-manifest.sha256"
expect_rc "an absent manifest refuses" 1
expect_out "an absent manifest reports the missing token" "missing"

empty_manifest="$work/empty.sha256"
: >"$empty_manifest"
run_scan OSQUERY_PIPELINE_MANIFEST="$empty_manifest"
expect_rc "an empty manifest refuses" 1
expect_out "an empty manifest reports the missing token" "missing"

bad_manifest="$work/malformed.sha256"
printf 'not-a-hash  /some/path\n' >"$bad_manifest"
run_scan OSQUERY_PIPELINE_MANIFEST="$bad_manifest"
expect_rc "a malformed manifest line refuses" 1
expect_out "a malformed manifest reports the malformed token" "malformed"

# A manifest whose paths are RELATIVE is malformed too: the audit resolves nothing
# itself, so a relative path would be read against whatever directory launchd
# happened to start the watchdog in.
rel_manifest="$work/relative.sha256"
printf '%s  digest.sh\n' "$(sha_of "$good_script")" >"$rel_manifest"
run_scan OSQUERY_PIPELINE_MANIFEST="$rel_manifest"
expect_rc "a relative manifested path refuses" 1
expect_out "a relative manifested path reports the malformed token" "malformed"

# --- the manifest must be root-owned before its verdict means anything --------
# Driven with the OSQUERY_PIPELINE_MANIFEST override UNSET, because that override
# is the test seam that skips the ownership check for fixture manifests (the same
# seam the verdict helper uses). A manifest anyone can write could vouch for bytes
# an attacker just planted, so an untrusted manifest is refused rather than obeyed.
trust_rc=0
# shellcheck disable=SC2016 # $1/$2 are expanded by the child shell, as above
trust_out="$(env HOME="$home" bash -c '
  source "$1"
  unset OSQUERY_PIPELINE_MANIFEST
  PIPELINE_MANIFEST="$2"
  pipeline_audit_scan' _ "$pipeline/pipeline-audit.sh" "$manifest")" || trust_rc=$?
[[ $trust_rc -eq 1 ]] ||
  fail "a manifest that is not root-owned must be refused, got return $trust_rc"
[[ $trust_out == "untrustworthy" ]] ||
  fail "a non-root-owned manifest must report the untrustworthy token, got [$trust_out]"

if [[ $fails -gt 0 ]]; then
  printf '%d check(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'osquery-pipeline-audit: OK (a matching tree is clean; tampered content, a missing path, a symlink whose referent matches, a directory, and an over-cap file are each reported without any file event; spaced paths are read whole; an absent, empty, malformed, relative-path, over-long, or non-root-owned manifest and an exhausted budget all refuse LOUDLY)\n'
