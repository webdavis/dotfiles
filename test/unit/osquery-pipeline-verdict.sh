#!/usr/bin/env bash
#
# pipeline_verdict (results-alerter/pipeline-verdict.sh) decides whether a file
# change under the watched pipeline directories is a tamper to PAGE, a known-good
# apply to stay SILENT, or an untracked neighbor to log only. It checks the
# change against the pipeline-integrity manifest, a root-owned sha256 list of the
# alerter's own scripts/plists.
#
# Return-code contract (from c69baab _pipeline_verdict), inverted vs the
# allowlist verdict on purpose:
#   0 = PAGE   - a tracked file changed and we cannot confirm it legitimate
#                (tamper, a delete, an empty/mismatched hash, or NO manifest).
#   1 = SILENT - an untracked neighbor in a watched dir, OR a tracked change whose
#                exact (path, sha256) tuple is present in the manifest.
#
# Criterion 6, the headline this behavior pins: the integrity manifest slice is
# LAST in the stack and does not exist yet, so with NO manifest present a change
# to a tracked pipeline file PAGES. That is the conservative fail-open direction -
# a pipeline-script change is never silently suppressed without a manifest to
# justify it.
#
# Dual tuple-prefix: the tracked pipeline scripts now live under BOTH
# ~/.local/libexec/osquery/ (the relocated alerter scripts, osquery- prefix
# dropped) and ~/.local/bin/ (root-of-trust operator tools); our own LaunchAgents
# are matched by the com.webdavis.osquery-*.plist basename. c69baab only knew
# ~/.local/bin/osquery-*.sh; the identification is rebased onto the new layout.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"

fail() {
  printf 'osquery-pipeline-verdict: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $HELPER ]] || fail "missing helper: $HELPER"
command -v shasum >/dev/null 2>&1 || fail "shasum is required for this test"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
home="$work/home"
mkdir -p "$home/.local/libexec/osquery/results-alerter" "$home/.local/bin" "$home/Library/LaunchAgents"

# An on-disk tracked file for the atomic-rename (empty-hash) rehash path.
libexec_script="$home/.local/libexec/osquery/results-alerter.sh"
bin_script="$home/.local/bin/relay.sh"
printf 'echo libexec\n' >"$libexec_script"
printf 'echo bin\n' >"$bin_script"

# Synthetic manifest hashes (the non-empty-hash path compares the EVENT hash to
# the manifest, no disk read, so these need not match any real file).
hash_libexec="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
hash_bin="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
hash_wrong="0000000000000000000000000000000000000000000000000000000000000000"

# A stubbed manifest binding each hash to ITS path (shasum format: "<hash>  <path>").
manifest="$work/pipeline-known-good.sha256"
{
  printf '%s  %s\n' "$hash_libexec" "$libexec_script"
  printf '%s  %s\n' "$hash_bin" "$bin_script"
} >"$manifest"
absent_manifest="$work/no-such-manifest.sha256"

# Each case: <expected-rc> TAB <manifest> TAB <target> TAB <hash> TAB <verb> TAB <label>.
# An empty manifest field means "no manifest" (points at a nonexistent path).
cases=(
  # -- Fail-open headline: NO manifest, a tracked change under either prefix PAGES --
  $'0\t'"$absent_manifest"$'\t'"$libexec_script"$'\t'"$hash_libexec"$'\tUPDATED\ttracked libexec script, no manifest -> PAGE (fail-open, criterion 6)'
  $'0\t'"$absent_manifest"$'\t'"$bin_script"$'\t'"$hash_bin"$'\tUPDATED\ttracked ~/.local/bin script, no manifest -> PAGE (fail-open, second prefix)'
  # -- An untracked neighbor in a watched dir is SILENT --
  $'1\t'"$absent_manifest"$'\t'"$home/Library/LaunchAgents/com.apple.something.plist"$'\t'"$hash_libexec"$'\tUPDATED\tan untracked neighbor plist -> SILENT (not pipeline infrastructure)'
  # -- Our own osquery LaunchAgent (basename branch), no manifest -> PAGE --
  $'0\t'"$absent_manifest"$'\t'"$home/Library/LaunchAgents/com.webdavis.osquery-uptime-watchdog.plist"$'\t'"$hash_libexec"$'\tUPDATED\tour own osquery LaunchAgent, no manifest -> PAGE'
  # -- A DELETE of a tracked file always PAGES, even with a manifest present --
  $'0\t'"$manifest"$'\t'"$libexec_script"$'\t\tDELETED\ta delete of a tracked file -> PAGE (destructive, manifest cannot vouch)'
  # -- Empty event hash (atomic-rename shape): debounce, rehash disk; no manifest -> PAGE --
  $'0\t'"$absent_manifest"$'\t'"$libexec_script"$'\t\tMOVED_TO\tatomic-rename empty-hash event, no manifest -> PAGE after rehash'
  # -- Manifest present: exact (path, hash) tuple known-good -> SILENT --
  $'1\t'"$manifest"$'\t'"$libexec_script"$'\t'"$hash_libexec"$'\tUPDATED\ttracked change whose exact (path,hash) tuple is in the manifest -> SILENT'
  # -- Manifest present: hash mismatch on a tracked path -> PAGE (tamper) --
  $'0\t'"$manifest"$'\t'"$libexec_script"$'\t'"$hash_wrong"$'\tUPDATED\ta tracked path with a hash absent from the manifest -> PAGE (tamper)'
  # -- Manifest present: a valid hash lifted onto a DIFFERENT tracked path -> PAGE --
  $'0\t'"$manifest"$'\t'"$libexec_script"$'\t'"$hash_bin"$'\tUPDATED\tswap-in-place (a real hash bound to another path) -> PAGE (tuple binding)'
)

expected=()
labels=()
feed=""
for row in "${cases[@]}"; do
  IFS=$'\t' read -r rc manifest_path target hash verb label_text <<<"$row"
  expected+=("$rc")
  labels+=("$label_text")
  feed+="$manifest_path"$'\t'"$target"$'\t'"$hash"$'\t'"$verb"$'\n'
done

# One sourcing subshell drives every case; OSQUERY_PIPELINE_REHASH_DELAY=0 keeps
# the atomic-rename debounce from adding real wall time to the unit test.
got=()
mapfile -t got < <(
  printf '%s' "$feed" | HOME="$home" OSQUERY_PIPELINE_REHASH_DELAY=0 bash -c '
    source "$1"
    while IFS="$(printf "\t")" read -r manifest target hash verb; do
      OSQUERY_PIPELINE_MANIFEST="$manifest"
      rc=0
      pipeline_verdict "$target" "$hash" "$verb" || rc=$?
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

printf 'osquery-pipeline-verdict: OK (fail-open PAGE without a manifest under both prefixes; neighbor SILENT; delete PAGES; manifest tuple match SILENT, mismatch/swap-in-place PAGE)\n'
