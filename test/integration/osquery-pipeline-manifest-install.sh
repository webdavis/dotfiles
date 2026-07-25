#!/usr/bin/env bash
#
# The pipeline-manifest runner installs the manifest root-owned, but only when the
# content actually changed, and it must run on the apply path this repository
# mandates for agents: `chezmoi apply --exclude=templates`.
#
# That last point is why the runner is a PLAIN script and not a .tmpl. Template
# scripts are skipped by --exclude=templates while the pipeline's plain
# executable_*.sh files are still applied, so a templated runner would leave every
# agent apply updating the watched files without refreshing the manifest - each
# changed pipeline file would then page a false CRIT until someone ran a full
# interactive apply. Pinned end to end below: the runner is dropped into a fixture
# chezmoi source and must actually execute under --exclude=templates.
#
# Also pinned: root:wheel 0644, the diff-guard (no privileged write when nothing
# changed), re-install after a real change, refusal to install an empty manifest,
# a generation failure leaving the previous manifest intact, and that the producer
# and the consumer name the same default manifest path.
#
# `sudo` is PATH-shadowed by a stub that records argv and emulates the copy, so no
# real privilege is used and nothing is written under /var.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/.chezmoiscripts/run_after_05-osquery-pipeline-manifest.sh"
VERDICT="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"
# shellcheck source=../fixtures/osquery-manifest-lib.bash
source "$REPO_ROOT/test/fixtures/osquery-manifest-lib.bash"

fails=0
fail() {
  printf 'osquery-pipeline-manifest-install: FAIL -- %s\n' "$*" >&2
  fails=$((fails + 1))
}

# --- the runner is not a template, and is executable -------------------------
# (asserted first: this is the property that makes it run at all)
[[ -f $RUNNER ]] || {
  printf 'osquery-pipeline-manifest-install: FAIL -- missing runner: %s\n' "$RUNNER" >&2
  exit 1
}
case "$RUNNER" in
  *.tmpl) fail "the runner must NOT be a template: --exclude=templates would skip it on every agent apply" ;;
esac
[[ -x $RUNNER ]] || fail "the runner must be executable"
grep -q 'uname' "$RUNNER" ||
  fail "a plain runner must gate darwin at RUNTIME (no Go-template guard is available)"

# --- the producer and the consumer agree on the manifest path ----------------
runner_path="$(grep -o '/var/osquery/[a-z-]*\.sha256' "$RUNNER" | head -1)"
verdict_path="$(grep -o '/var/osquery/[a-z-]*\.sha256' "$VERDICT" | head -1)"
[[ -n $runner_path && $runner_path == "$verdict_path" ]] ||
  fail "producer ($runner_path) and consumer ($verdict_path) must name the same default manifest path"

for tool in chezmoi shasum; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'osquery-pipeline-manifest-install: SKIP -- %s is required\n' "$tool"
    exit 0
  }
done
[[ "$(uname)" == Darwin ]] || {
  printf 'osquery-pipeline-manifest-install: SKIP -- the runner is darwin-gated\n'
  exit 0
}

manifest_fixture_setup
trap manifest_fixture_teardown EXIT

manifest_fixture_add_script digest.sh 'echo digest'
manifest_fixture_add_plist com.webdavis.osquery-digest '<plist>{{ .chezmoi.os }}</plist>'
manifest_fixture_apply

# --- 1. first run: a privileged install, root:wheel 0644 ---------------------
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero on the first run"
manifest_fixture_installed || fail "the first run did not install the manifest"
grep -qF -- 'install -o root -g wheel -m 0644' "$MF_SUDO_LOG" ||
  fail "the install is not root:wheel 0644 (sudo argv: $(cat "$MF_SUDO_LOG"))"
[[ -s $MF_MANIFEST ]] || fail "the first run did not create the manifest"

# --- 2. nothing changed: the diff-guard skips the privileged write ------------
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero on an unchanged run"
manifest_fixture_installed &&
  fail "an unchanged tree still triggered a privileged write (sudo argv: $(cat "$MF_SUDO_LOG"))"

# --- 3. a real change re-installs -------------------------------------------
previous="$(cat "$MF_MANIFEST")"
manifest_fixture_add_script digest.sh 'echo digest v2'
manifest_fixture_apply
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero after a pipeline change"
manifest_fixture_installed || fail "a changed pipeline file did not trigger a re-install"
[[ "$(cat "$MF_MANIFEST")" != "$previous" ]] || fail "the manifest was not refreshed after a change"

# --- 4. a generation failure leaves the previous manifest in force -----------
# An unreadable SOURCE file makes `chezmoi cat` fail; under set -o pipefail the
# runner must abort rather than install a partial manifest. Without this pin a
# future `|| true` would silently truncate the manifest and bless nothing.
good="$(cat "$MF_MANIFEST")"
chmod 000 "$MF_SRC/dot_local/libexec/osquery/executable_digest.sh"
status=0
manifest_fixture_run_runner "$RUNNER" >/dev/null 2>&1 || status=$?
chmod 644 "$MF_SRC/dot_local/libexec/osquery/executable_digest.sh"
[[ $status -ne 0 ]] || fail "an unreadable managed file must abort the runner, not install a partial manifest"
manifest_fixture_installed &&
  fail "a generation failure must not perform a privileged write (sudo argv: $(cat "$MF_SUDO_LOG"))"
[[ "$(cat "$MF_MANIFEST")" == "$good" ]] ||
  fail "a generation failure must leave the previous manifest untouched"

# --- 5. an empty result is refused, never installed over a good manifest -----
# Pointing the home at a tree with no managed pipeline files resolves zero paths.
empty_home="$MF_ROOT/empty-home"
mkdir -p "$empty_home"
status=0
: >"$MF_SUDO_LOG"
env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$MF_HOME" PATH="$MF_ROOT/bin:$PATH" \
  CHEZMOI_SOURCE_DIR="$MF_SRC" CHEZMOI_HOME_DIR="$empty_home" \
  OSQUERY_PIPELINE_MANIFEST="$MF_MANIFEST" SUDO_LOG="$MF_SUDO_LOG" \
  bash "$RUNNER" >/dev/null 2>&1 || status=$?
[[ $status -ne 0 ]] || fail "an empty manifest must be refused with a non-zero exit"
manifest_fixture_installed && fail "an empty manifest must never be installed"
[[ "$(cat "$MF_MANIFEST")" == "$good" ]] || fail "an empty result must leave the good manifest in place"

# --- 6. END TO END: the runner actually executes under --exclude=templates ----
# The whole point of it not being a .tmpl. Drop it into the fixture source and run
# the mandated agent apply; the manifest must be produced by chezmoi itself.
mkdir -p "$MF_SRC/.chezmoiscripts"
cp "$RUNNER" "$MF_SRC/.chezmoiscripts/run_after_05-osquery-pipeline-manifest.sh"
chmod +x "$MF_SRC/.chezmoiscripts/run_after_05-osquery-pipeline-manifest.sh"
rm -f "$MF_MANIFEST"
: >"$MF_SUDO_LOG"
env -u XDG_CONFIG_HOME -u XDG_DATA_HOME HOME="$MF_HOME" CI=1 PATH="$MF_ROOT/bin:$PATH" \
  OSQUERY_PIPELINE_MANIFEST="$MF_MANIFEST" SUDO_LOG="$MF_SUDO_LOG" \
  chezmoi --config "$MF_HOME/.config/chezmoi/chezmoi.toml" --source "$MF_SRC" \
  --destination "$MF_HOME" apply --exclude=templates --force >/dev/null 2>&1 || true
[[ -s $MF_MANIFEST ]] ||
  fail "the runner did NOT run under 'chezmoi apply --exclude=templates' (the mandated agent apply path)"

if [[ $fails -gt 0 ]]; then
  printf '%d check(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'osquery-pipeline-manifest-install: OK (plain script, runs under --exclude=templates; root:wheel 0644; diff-guard skips an unchanged run; re-installs on change; a generation failure or an empty result never overwrites a good manifest; producer and consumer name one path)\n'
