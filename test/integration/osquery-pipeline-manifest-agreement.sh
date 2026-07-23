#!/usr/bin/env bash
#
# End-to-end agreement between the two halves of the pipeline-integrity mechanism:
# the manifest that generate-pipeline-manifest.sh produces must be exactly what the
# shipped pipeline-verdict.sh (the alerter's consumer) needs to vouch for a
# known-good file and to page on a tamper. Both are REAL here - a real generated
# manifest, the real verdict, real shasum - so this proves they agree, not that a
# stub matches a stub.
#
# Integration test: build a fixture pipeline tree, generate the manifest from it,
# then drive the real pipeline_verdict against that manifest:
#   - an unchanged deployed file (event hash = its real sha256) stays SILENT;
#   - a one-byte mutation (the new hash absent from the manifest) PAGES;
#   - the atomic-rename shape (empty event hash, verdict rehashes on disk) is SILENT
#     before the mutation and PAGES after it;
#   - a known-good own-agent plist stays SILENT.
# The SILENT->PAGE flip on a single byte is the mutation-verify that the manifest
# actually binds each file's real content.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GENERATOR="$REPO_ROOT/dot_local/libexec/osquery/executable_generate-pipeline-manifest.sh"
VERDICT="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"

fails=0
fail() {
  printf 'osquery-pipeline-manifest-agreement: FAIL -- %s\n' "$*" >&2
  fails=$((fails + 1))
}

for f in "$GENERATOR" "$VERDICT"; do
  [[ -f $f ]] || {
    printf 'osquery-pipeline-manifest-agreement: FAIL -- missing %s\n' "$f" >&2
    exit 1
  }
done
command -v shasum >/dev/null 2>&1 || {
  printf 'osquery-pipeline-manifest-agreement: SKIP -- shasum is required\n'
  exit 0
}

home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT
libexec="$home/.local/libexec/osquery"
agents="$home/Library/LaunchAgents"
mkdir -p "$libexec/results-alerter" "$agents"

script="$libexec/digest.sh"
helper="$libexec/results-alerter/normalize.sh"
plist="$agents/com.webdavis.osquery-digest.plist"
printf 'echo digest\n' >"$script"
printf 'true\n' >"$helper"
printf '<plist>digest</plist>\n' >"$plist"

manifest="$home/pipeline-known-good.sha256"
HOME="$home" "$GENERATOR" >"$manifest" || fail "the generator exited non-zero"

# The verdict reads the manifest from OSQUERY_PIPELINE_MANIFEST and the rehash
# debounce from OSQUERY_PIPELINE_REHASH_DELAY; zero the debounce so the empty-hash
# path adds no wall time. HOME must match the fixture so _pipeline_is_tracked
# resolves the libexec prefix.
# shellcheck source=/dev/null
source "$VERDICT"
export OSQUERY_PIPELINE_MANIFEST="$manifest" OSQUERY_PIPELINE_REHASH_DELAY=0

hash_of() { shasum -a 256 "$1" | awk '{print $1}'; }

# expect_verdict <expected-rc> <label> <target> <hash> <verb>
expect_verdict() {
  local want="$1" label="$2" target="$3" hash="$4" verb="$5" got=0
  HOME="$home" pipeline_verdict "$target" "$hash" "$verb" || got=$?
  [[ $got == "$want" ]] || fail "$label: expected rc $want, got $got"
}

# Known-good, exact event hash: the generated manifest vouches for it -> SILENT.
expect_verdict 1 "unchanged script (event hash) -> SILENT" "$script" "$(hash_of "$script")" UPDATED
expect_verdict 1 "unchanged helper (event hash) -> SILENT" "$helper" "$(hash_of "$helper")" UPDATED
expect_verdict 1 "unchanged own-agent plist (event hash) -> SILENT" "$plist" "$(hash_of "$plist")" UPDATED

# Atomic-rename shape (empty event hash): the verdict rehashes the on-disk file and
# still finds the manifest tuple -> SILENT.
expect_verdict 1 "unchanged script (empty hash, disk rehash) -> SILENT" "$script" "" MOVED_TO

# Mutation-verify: change one byte. The new content's hash is not in the manifest,
# so the SAME file now PAGES on both the event-hash and the disk-rehash paths.
printf 'echo tampered\n' >>"$script"
expect_verdict 0 "tampered script (new event hash) -> PAGE" "$script" "$(hash_of "$script")" UPDATED
expect_verdict 0 "tampered script (empty hash, disk rehash) -> PAGE" "$script" "" MOVED_TO

if [[ $fails -gt 0 ]]; then
  printf '%d check(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'osquery-pipeline-manifest-agreement: OK (the generated manifest makes the real verdict SILENT on unchanged files and PAGE on a one-byte tamper, via both the event-hash and disk-rehash paths)\n'
