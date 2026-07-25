#!/usr/bin/env bash
#
# The pipeline-integrity mechanism has three layers that must cover the IDENTICAL
# file set, or it breaks in one of two silent ways:
#
#   WATCH    (.chezmoitemplates/osquery/osquery.conf file_paths)      what osquery reports
#   TRACKED  (results-alerter/pipeline-verdict.sh _pipeline_is_tracked) what the alerter judges
#   MANIFEST (.chezmoiscripts/run_after_05-osquery-pipeline-manifest.sh) what can be vouched for
#
# A watched-and-tracked file the manifest can never contain pages FOREVER; a
# manifested file nothing watches is never checked at all. This test drives all
# three against the same fixture and pins their agreement, including the launch
# agent roots: the watch covers /Library/LaunchAgents and /Library/LaunchDaemons
# as well as ~/Library/LaunchAgents, but the manifest only ever covers the user
# agents chezmoi manages, so a com.webdavis.osquery-*.plist under /Library must NOT
# be tracked (it belongs to the persistence detector's default-deny instead). An
# earlier revision matched that basename anywhere and had exactly this divergence.
#
# It also pins the end-to-end agreement between the real generated manifest and the
# real verdict (unchanged is SILENT, a one-byte tamper PAGES, and a chmod on
# otherwise unchanged content PAGES) and the bounded apply-race settle window.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/.chezmoiscripts/run_after_05-osquery-pipeline-manifest.sh"
VERDICT="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"
CONF="$REPO_ROOT/.chezmoitemplates/osquery/osquery.conf"
# shellcheck source=../fixtures/osquery-manifest-lib.bash
source "$REPO_ROOT/test/fixtures/osquery-manifest-lib.bash"

fails=0
fail() {
  printf 'osquery-pipeline-manifest-agreement: FAIL -- %s\n' "$*" >&2
  fails=$((fails + 1))
}

for f in "$RUNNER" "$VERDICT" "$CONF"; do
  [[ -f $f ]] || {
    printf 'osquery-pipeline-manifest-agreement: FAIL -- missing %s\n' "$f" >&2
    exit 1
  }
done
for tool in chezmoi shasum jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'osquery-pipeline-manifest-agreement: SKIP -- %s is required\n' "$tool"
    exit 0
  }
done
[[ "$(uname)" == Darwin ]] || {
  printf 'osquery-pipeline-manifest-agreement: SKIP -- the runner is darwin-gated\n'
  exit 0
}

manifest_fixture_setup
trap manifest_fixture_teardown EXIT

manifest_fixture_add_script digest.sh 'echo digest'
manifest_fixture_add_script results-alerter/normalize.sh 'true'
manifest_fixture_add_plist com.webdavis.osquery-digest '<plist>{{ .chezmoi.os }}</plist>'
manifest_fixture_apply
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero"

script_target="$MF_HOME/.local/libexec/osquery/digest.sh"

# shellcheck source=/dev/null
source "$VERDICT"
export OSQUERY_PIPELINE_MANIFEST="$MF_MANIFEST" OSQUERY_PIPELINE_REHASH_DELAY=0
export OSQUERY_PIPELINE_SETTLE_SECONDS=0

hash_of() { shasum -a 256 "$1" | awk '{print $1}'; }

# run_verdict <target> <hash> <verb> [settle-seconds] -- invoke the real verdict for
# ONE case, in a SUBSHELL. Every call site below goes through here; nothing in this
# file calls pipeline_verdict directly.
#
# Why the subshell, and why this is not a workaround for a production bug. The settle
# budget is deliberately ONE per alerter INVOCATION, not one per finding: findings are
# judged sequentially while the alerter holds its single-instance lock, so a per-finding
# wait would let anyone who plants N files stall the pipeline for N times the bound and
# delay unrelated security findings. The alerter sources this helper once per run, so in
# production every invocation genuinely starts with an empty _pipeline_settle_deadline.
#
# This file drives many cases inside ONE shell, which production never does. Without
# isolation, a case that opens the window (a tuple miss on a target NEWER than the
# manifest) leaves the deadline SPENT in the shell, and every later case inherits it and
# answers immediately: the 4-second settle case below then returns PAGE where it must
# return SILENT. Whether the earlier case opens the window at all depends on whole-second
# stat mtime granularity, so the leak surfaced as an intermittent failure under load.
#
# A subshell per case gives each one its own copy of that global, which is exactly what
# re-sourcing gives a real alerter run. It is structural rather than a reset someone has
# to remember, so a case added to this file later cannot silently inherit a spent budget.
run_verdict() {
  local target="$1" hash_value="$2" verb="$3" settle="${4:-$OSQUERY_PIPELINE_SETTLE_SECONDS}"
  (
    export HOME="$MF_HOME" OSQUERY_PIPELINE_SETTLE_SECONDS="$settle"
    pipeline_verdict "$target" "$hash_value" "$verb"
  )
}

# expect_verdict <expected-rc> <label> <target> <hash> <verb>
expect_verdict() {
  local want="$1" label="$2" got=0
  run_verdict "$3" "$4" "$5" || got=$?
  [[ $got == "$want" ]] || fail "$label: expected rc $want, got $got"
}

# --- the generated manifest and the real verdict agree -----------------------
expect_verdict 1 "an unchanged pipeline script is SILENT" "$script_target" "$(hash_of "$script_target")" UPDATED
printf 'echo tampered\n' >>"$script_target"
expect_verdict 0 "a one-byte tamper PAGES" "$script_target" "$(hash_of "$script_target")" UPDATED
manifest_fixture_apply # restore
expect_verdict 1 "the restored script is SILENT again" "$script_target" "$(hash_of "$script_target")" UPDATED

# A chmod PAGES end to end, against the REAL generated manifest. osquery reports
# this as ATTRIBUTES_MODIFIED carrying the file's UNCHANGED digest, which is why
# the event hash below is the same one that was just SILENT: only the mode moved.
chmod g+w "$script_target"
expect_verdict 0 "a chmod g+w on a manifested script PAGES" "$script_target" "$(hash_of "$script_target")" ATTRIBUTES_MODIFIED
chmod 755 "$script_target"
expect_verdict 1 "restoring the intended mode is SILENT again" "$script_target" "$(hash_of "$script_target")" ATTRIBUTES_MODIFIED

# --- THE THREE-WAY AGREEMENT, across BOTH launch agent watch roots ------------
# Every launch-agent path the WATCH reports is classified, and the TRACKED verdict
# must say yes exactly when the MANIFEST can contain it.
render_home="$MF_ROOT/render-home"
mkdir -p "$render_home"
conf_json="$(HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty <"$CONF")" ||
  fail "osquery.conf failed to render"
watch_roots="$(jq -r '(.file_paths.launch_agents // []) + (.file_paths.launch_daemons // []) | .[]' <<<"$conf_json")"
[[ -n $watch_roots ]] || fail "the config declares no launch agent/daemon watch roots"

saw_home_root=0
while IFS= read -r root; do
  [[ -n $root ]] || continue
  dir="${root%/%%}"                                  # strip the osquery recursive-watch suffix
  candidate="$dir/com.webdavis.osquery-digest.plist" # one of OUR agent names
  tracked=0
  HOME="$MF_HOME" _pipeline_is_tracked "$candidate" || tracked=$?
  # The manifest can only ever contain a path chezmoi manages, i.e. under $HOME.
  case "$dir" in
    "$render_home"/Library/LaunchAgents)
      saw_home_root=1
      # The $HOME root: manifested (the runner covers it) so it MUST be tracked.
      home_candidate="$MF_HOME/Library/LaunchAgents/com.webdavis.osquery-digest.plist"
      [[ -n "$(manifest_hash_of "$home_candidate")" ]] ||
        fail "the manifest does not cover our own agent under the home LaunchAgents dir"
      home_tracked=0
      HOME="$MF_HOME" _pipeline_is_tracked "$home_candidate" || home_tracked=$?
      [[ $home_tracked -eq 0 ]] ||
        fail "our own agent under the home LaunchAgents dir is manifested but NOT tracked (a blind spot: never checked)"
      ;;
    *)
      # A system root: the manifest can never contain it, so it must NOT be tracked.
      [[ $tracked -ne 0 ]] ||
        fail "$candidate is TRACKED but the manifest can never cover it (it would page forever)"
      ;;
  esac
done <<<"$watch_roots"
[[ $saw_home_root -eq 1 ]] ||
  fail "the watch no longer covers ~/Library/LaunchAgents; this test's agreement check went blind"

# Every path the manifest holds must be one the verdict tracks (no manifested-but-
# unchecked file).
while read -r _ _ _ manifested_path; do
  [[ -n $manifested_path ]] || continue
  t=0
  HOME="$MF_HOME" _pipeline_is_tracked "$manifested_path" || t=$?
  [[ $t -eq 0 ]] || fail "manifested but NOT tracked (never checked): $manifested_path"
done <"$MF_MANIFEST"

# --- the bounded apply-race settle window ------------------------------------
# The alerter judges a finding exactly once, so a change seen before the manifest
# is reinstalled must not page a false CRIT that is never reconsidered.
settle_target="$script_target"
settle_hash="$(hash_of "$settle_target")"
settle_mode="$(manifest_mode_of "$settle_target")"
settle_uid="$(manifest_uid_of "$settle_target")"
[[ -n $settle_mode && -n $settle_uid ]] ||
  fail "the settle fixture could not read the generated mode/owner columns"
# A manifest that PREDATES the target and lacks the tuple: the verdict waits, and
# goes SILENT when the regeneration lands inside the window.
printf 'deadbeef 0755 %s /nowhere\n' "$(id -u)" >"$MF_MANIFEST"
touch -t 200001010000 "$MF_MANIFEST"
(
  sleep 1
  printf '%s %s %s %s\n' "$settle_hash" "$settle_mode" "$settle_uid" "$settle_target" >"$MF_MANIFEST"
) &
settle_pid=$!
got=0
run_verdict "$settle_target" "$settle_hash" UPDATED 4 || got=$?
wait "$settle_pid"
[[ $got -eq 1 ]] ||
  fail "a manifest that lands during the settle window must resolve to SILENT (got rc $got)"

# ...but the wait is BOUNDED: a tuple that never arrives still PAGES.
printf 'deadbeef 0755 %s /nowhere\n' "$(id -u)" >"$MF_MANIFEST"
touch -t 200001010000 "$MF_MANIFEST"
start=$(date +%s)
got=0
run_verdict "$settle_target" "$settle_hash" UPDATED 2 || got=$?
elapsed=$(($(date +%s) - start))
[[ $got -eq 0 ]] || fail "a tuple that never arrives must still PAGE (got rc $got)"
((elapsed <= 6)) || fail "the settle wait is not bounded (${elapsed}s for a 2s window)"

# --- the settle budget is per ALERTER RUN, not per finding -------------------
# route_findings judges findings sequentially while the alerter holds its
# single-instance lock, and a contended WatchPaths invocation exits without
# processing. A per-tuple wait would therefore let anyone who creates N files
# under the tracked home stall the whole pipeline for N x the bound, delaying
# UNRELATED security findings. The budget is one shared deadline per invocation:
# the first miss opens it, and once it is spent every later miss answers at once.
#
# This case runs its misses inside ONE `bash -c`, deliberately NOT through
# run_verdict: it is the case that asserts the budget IS shared, so its misses must
# land in a single invocation. run_verdict isolates CASES from each other; this
# asserts what happens WITHIN one case. The two are the same rule from both sides.
miss_manifest="$MF_ROOT/miss.sha256"
printf 'deadbeef 0755 %s /nowhere\n' "$(id -u)" >"$miss_manifest"
touch -t 200001010000 "$miss_manifest"
# The targets must EXIST and post-date the manifest, or the mtime guard short
# circuits and nothing settles (the shape a real apply produces).
misses=10
for i in $(seq 1 "$misses"); do
  printf 'echo miss\n' >"$MF_HOME/.local/libexec/osquery/miss-$i.sh"
done
start=$(date +%s)
HOME="$MF_HOME" OSQUERY_PIPELINE_MANIFEST="$miss_manifest" \
  OSQUERY_PIPELINE_REHASH_DELAY=0 OSQUERY_PIPELINE_SETTLE_SECONDS=3 \
  bash -c '
    source "$1"
    for i in $(seq 1 "$2"); do
      pipeline_verdict "$HOME/.local/libexec/osquery/miss-$i.sh" \
        "3333333333333333333333333333333333333333333333333333333333333333" UPDATED || true
    done
  ' _ "$VERDICT" "$misses" >/dev/null 2>&1
elapsed=$(($(date +%s) - start))
((elapsed <= 8)) ||
  fail "$misses misses took ${elapsed}s: the settle budget is per finding, not per alerter run (a stall vector)"

if [[ $fails -gt 0 ]]; then
  printf '%d check(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'osquery-pipeline-manifest-agreement: OK (generated manifest and real verdict agree, including a chmod on unchanged content; watch/tracked/manifest cover the identical set across BOTH launch agent roots; a /Library twin is untracked; the settle window resolves a live regeneration, stays bounded, and spends ONE budget per alerter run across many misses)\n'
