#!/usr/bin/env bash
#
# The file-integrity mechanism has three layers that must cover the IDENTICAL file
# set, or it breaks in one of two silent ways:
#
#   WATCH    (.chezmoitemplates/osquery/osquery.conf file_paths)      what osquery reports
#   TRACKED  (results-alerter/pipeline-verdict.sh _pipeline_is_tracked) what the alerter judges
#   MANIFEST (.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh) what can be vouched for
#
# The manifest has a SECOND consumer, the periodic audit (pipeline-audit.sh), which
# parses the file directly instead of going through the verdict. It is driven here
# against the real generated manifest for the same reason: a format change that only
# the hand-built fixtures in its own suite kept up with would leave it refusing every
# real manifest.
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
# The MANAGED-BIN arm is pinned the same way, with one structural difference worth
# stating plainly. ~/.local/bin is watched WHOLE, because osquery watches
# directories, while only the chezmoi-managed files in it are manifested. TRACKED
# is therefore derived from the manifest rather than from a second path filter, so
# tracked and manifested are identical by construction and the agreement that has
# to be checked here is the containment one: every manifested bin path must fall
# under a watched root, and an unmanaged neighbor must be watched but untracked.
#
# It also pins the end-to-end agreement between the real generated manifest and the
# real verdict (unchanged is SILENT, a one-byte tamper PAGES, and a chmod on
# otherwise unchanged content PAGES) and the bounded apply-race settle window.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$REPO_ROOT/.chezmoiscripts/run_after_05-osquery-known-good-manifests.sh"
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
# A managed ~/.local/bin tool, so the runner's managed-bin arm has something to
# manifest (it refuses to install an EMPTY manifest) and so the agreement checks
# below can drive that arm too.
manifest_fixture_add_bin_script update-skills.sh 'echo update-skills'
# The page-launchd allowlist, so the manifested-implies-tracked sweep below covers
# the one manifested file that lives outside both the pipeline home and bin.
manifest_fixture_add_config private_page-launchd-allowlist.txt \
  '{"label":"com.seed","path":"~/x.plist","program":"~/x","sha256":""}'
manifest_fixture_apply
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero"

script_target="$MF_HOME/.local/libexec/osquery/digest.sh"
bin_target="$MF_HOME/.local/bin/update-skills.sh"
# An UNMANAGED neighbor on disk only, the way mise and herdr sit in the real
# ~/.local/bin: watched (the whole directory is), but nothing can vouch for it.
bin_unmanaged="$MF_HOME/.local/bin/mise"
printf 'unmanaged self-updating binary\n' >"$bin_unmanaged"

# shellcheck source=/dev/null
source "$VERDICT"
export OSQUERY_PIPELINE_MANIFEST="$MF_MANIFEST" OSQUERY_PIPELINE_REHASH_DELAY=0
export OSQUERY_MANAGED_BIN_MANIFEST="$MF_BIN_MANIFEST"
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
    # Subshell-local by design; run_audit below scopes HOME the same way.
    # shellcheck disable=SC2030
    export HOME="$MF_HOME" OSQUERY_PIPELINE_SETTLE_SECONDS="$settle"
    pipeline_verdict "$target" "$hash_value" "$verb"
  )
}

# tracked_rc <target> -- the real _pipeline_is_tracked, under the fixture HOME.
tracked_rc() {
  local rc=0
  HOME="$MF_HOME" _pipeline_is_tracked "$1" || rc=$?
  printf '%s' "$rc"
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

# --- THE PERIODIC AUDIT READS THE SAME GENERATED MANIFEST --------------------
# The audit is the manifest's OTHER consumer, and it parses the file itself rather
# than going through the verdict. Its own suite builds fixture manifests by hand, so
# it stayed green through a producer format change that left it reporting
# "malformed" against every real manifest: a loud break, but a permanent one. This
# drives the REAL scan over the REAL generated manifest, which is the only check
# that fails when producer and consumer drift apart.
AUDIT="$REPO_ROOT/dot_local/libexec/osquery/executable_pipeline-audit.sh"
[[ -f $AUDIT ]] || fail "missing the periodic audit: $AUDIT"

# run_audit -- one scan, in a subshell, for the same reason run_verdict uses one.
# Prints the scan return code on the first line, then the scan's stdout.
run_audit() {
  (
    # HOME is meant to be subshell-local here, exactly as it is in run_verdict:
    # that scoping is the isolation, not an accident of it.
    # shellcheck disable=SC2030,SC2031
    export HOME="$MF_HOME"
    # shellcheck source=/dev/null
    source "$AUDIT"
    local rc=0 out
    out="$(pipeline_audit_scan)" || rc=$?
    printf '%s\n%s' "$rc" "$out"
  )
}

# A plain refute helper. `! grep` inside a test body is a silent no-op under set -e,
# so absence is asserted with an explicit case instead.
refute_line() { # <haystack> <needle> <label>
  case "$1" in
    *"$2"*) fail "$3" ;;
  esac
}

audit_out="$(run_audit)"
audit_rc="${audit_out%%$'\n'*}"
audit_body="${audit_out#*$'\n'}"
[[ $audit_rc == 0 ]] ||
  fail "the audit could not complete against the generated manifest (reason token: $audit_body)"
refute_line "$audit_body" "$script_target" \
  "the audit reported a divergence for an untampered manifested script: $audit_body"

# ...and it actually LOOKS at the file, rather than passing everything by default.
printf 'echo audit tamper\n' >>"$script_target"
audit_out="$(run_audit)"
audit_rc="${audit_out%%$'\n'*}"
audit_body="${audit_out#*$'\n'}"
[[ $audit_rc == 0 ]] ||
  fail "the audit could not complete over a tampered tree (reason token: $audit_body)"
case "$audit_body" in
  *"content $script_target"*) ;;
  *) fail "the audit did not report the tampered script as a content divergence: $audit_body" ;;
esac
manifest_fixture_apply # restore

# ...and the PAGE-LAUNCHD ALLOWLIST is audited on the same tick, because it rides in
# the pipeline manifest and the audit is a driver over every manifested path. That
# matters beyond tidiness: the event-time verdict is path-based, so an attacker who
# edits the allowlist through a hard link outside ~/.config/osquery fires no event on
# the watched path and layer 1 never runs. This is the layer that still finds it,
# within two ticks, and it reports content and mode as SEPARATE kinds, so an edit
# that later widens the file's permissions is a new fingerprint rather than a
# condition already reported.
allowlist_audit_target="$MF_HOME/.config/osquery/page-launchd-allowlist.txt"
[[ -r $allowlist_audit_target ]] || fail "the fixture did not deploy an allowlist to audit"
printf '{"label":"com.evil","path":"~/e.plist","program":"~/e","sha256":""}\n' >>"$allowlist_audit_target"
chmod 644 "$allowlist_audit_target"
audit_out="$(run_audit)"
audit_rc="${audit_out%%$'\n'*}"
audit_body="${audit_out#*$'\n'}"
[[ $audit_rc == 0 ]] ||
  fail "the audit could not complete over a tampered allowlist (reason token: $audit_body)"
case "$audit_body" in
  *"content $allowlist_audit_target"*) ;;
  *) fail "the periodic audit does not cover the page-launchd allowlist: $audit_body" ;;
esac
case "$audit_body" in
  *"mode $allowlist_audit_target"*) ;;
  *) fail "the audit did not report the widened allowlist mode as its own divergence kind: $audit_body" ;;
esac
manifest_fixture_apply # restore

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

# Every path EITHER manifest holds must be one the verdict tracks (no manifested-
# but-unchecked file, in either trust domain).
for agreement_manifest in "$MF_MANIFEST" "$MF_BIN_MANIFEST"; do
  while read -r _ _ _ manifested_path; do
    [[ -n $manifested_path ]] || continue
    [[ "$(tracked_rc "$manifested_path")" -eq 0 ]] ||
      fail "manifested but NOT tracked (never checked): $manifested_path"
  done <"$agreement_manifest"
done

# --- THE THREE-WAY AGREEMENT for the MANAGED-BIN arm --------------------------
# ~/.local/bin is watched WHOLE (osquery watches directories) while only the
# chezmoi-managed files in it are manifested, so the property to pin is
# containment, in both directions:
#
#   WATCH covers MANIFEST   - a manifested path nothing watches is never checked
#                             on the event path at all.
#   TRACKED == MANIFEST     - an unmanaged neighbor is watched but untracked, or it
#                             would page on every third-party self-update.
bin_watch_roots="$(jq -r '.file_paths.managed_bin // [] | .[]' <<<"$conf_json")"
[[ -n $bin_watch_roots ]] ||
  fail "the config declares no managed_bin watch root; the managed ~/.local/bin scripts generate no events at all"

saw_bin_root=0
while IFS= read -r root; do
  [[ -n $root ]] || continue
  dir="${root%/%%}" # strip the osquery recursive-watch suffix
  # The rendered config names the RENDER home; the fixture manifest names the
  # fixture home. Compare the HOME-relative suffix, which is what has to agree.
  [[ ${dir#"$render_home"/} == ".local/bin" ]] ||
    fail "the managed_bin watch root is $dir, not ~/.local/bin; the manifest covers ~/.local/bin only"
  saw_bin_root=1
done <<<"$bin_watch_roots"
[[ $saw_bin_root -eq 1 ]] ||
  fail "no managed_bin watch root resolved; this test's containment check went blind"

# WATCH covers MANIFEST: every manifested bin path is under the watched directory.
while read -r _ _ _ manifested_path; do
  [[ -n $manifested_path ]] || continue
  [[ $manifested_path == "$MF_HOME"/.local/bin/* ]] ||
    fail "the managed-bin manifest holds $manifested_path, which the managed_bin watch root does not cover (a blind spot: never checked on the event path)"
done <"$MF_BIN_MANIFEST"

# TRACKED == MANIFEST: the manifested tool is tracked, the unmanaged neighbor in
# the same watched directory is NOT. Getting this backwards is the two silent
# failure modes: a shim that pages on every self-update, or a managed script whose
# tamper is judged as somebody else's business.
[[ -n "$(bin_manifest_hash_of "$bin_target")" ]] ||
  fail "the managed-bin manifest does not cover the managed tool under ~/.local/bin"
[[ "$(tracked_rc "$bin_target")" -eq 0 ]] ||
  fail "the managed bin tool is manifested but NOT tracked (a blind spot: never checked)"
[[ -z "$(bin_manifest_hash_of "$bin_unmanaged")" ]] ||
  fail "an UNMANAGED ~/.local/bin neighbor was signed into the managed-bin manifest"
[[ "$(tracked_rc "$bin_unmanaged")" -ne 0 ]] ||
  fail "an UNMANAGED ~/.local/bin neighbor is TRACKED but can never be manifested (it would page on every self-update)"

# ...and neither manifest may claim the other's paths, or one list could vouch for
# a file the other is responsible for.
[[ -z "$(manifest_hash_of "$bin_target")" ]] ||
  fail "a ~/.local/bin tool is in the osquery PIPELINE manifest (the two trust domains must stay disjoint)"
[[ -z "$(bin_manifest_hash_of "$script_target")" ]] ||
  fail "an osquery pipeline file is in the MANAGED-BIN manifest (the two trust domains must stay disjoint)"

# The bin arm binds all four columns too, from the same generator.
[[ "$(bin_manifest_mode_of "$bin_target")" == 0755 ]] ||
  fail "an executable_ managed bin tool must be manifested 0755, got '$(bin_manifest_mode_of "$bin_target")'"
[[ "$(bin_manifest_uid_of "$bin_target")" == "$(id -u)" ]] ||
  fail "the managed-bin owner column is not the uid the apply runs as"

# End to end, through the real verdict: unchanged is SILENT, a one-byte tamper
# PAGES, a chmod on unchanged content PAGES, and the unmanaged neighbor stays
# SILENT throughout.
expect_verdict 1 "an unchanged managed bin tool is SILENT" "$bin_target" "$(hash_of "$bin_target")" UPDATED
expect_verdict 1 "an unmanaged ~/.local/bin neighbor is SILENT" "$bin_unmanaged" "$(hash_of "$bin_unmanaged")" UPDATED
printf 'echo tampered\n' >>"$bin_target"
expect_verdict 0 "a one-byte tamper of a managed bin tool PAGES" "$bin_target" "$(hash_of "$bin_target")" UPDATED
manifest_fixture_apply # restore
chmod g+w "$bin_target"
expect_verdict 0 "a chmod g+w on a managed bin tool PAGES" "$bin_target" "$(hash_of "$bin_target")" ATTRIBUTES_MODIFIED
chmod 755 "$bin_target"
expect_verdict 1 "restoring the intended mode is SILENT again" "$bin_target" "$(hash_of "$bin_target")" ATTRIBUTES_MODIFIED

# --- the producer and the consumer name the SAME default manifest paths -------
# A security-critical file whose writer and reader agree only by copy-paste is one
# rename away from a monitor that watches nothing. Both defaults are pinned.
for manifest_literal in pipeline-known-good managed-bin-known-good; do
  runner_literal="$(grep -o "/var/osquery/$manifest_literal\.sha256" "$RUNNER" | head -1)"
  verdict_literal="$(grep -o "/var/osquery/$manifest_literal\.sha256" "$VERDICT" | head -1)"
  [[ -n $runner_literal && $runner_literal == "$verdict_literal" ]] ||
    fail "producer ($runner_literal) and consumer ($verdict_literal) must name the same default /var/osquery/$manifest_literal.sha256"
done

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

# --- an ATTRIBUTE-only apply can settle too ----------------------------------
# A chmod moves a file's inode CHANGE time, not its modification time. Keying the
# settle guard on mtime therefore made an attribute-only apply skip the window
# outright: the file lands, the manifest has not been reinstalled yet, and the
# alerter judges that finding exactly once, so the false CRIT is never
# reconsidered. Now that mode is part of the tuple, that race is reachable
# whenever a source attribute changes without the bytes changing.
#
# The fixture back-dates the target's mtime and leaves its ctime at now, which is
# the shape a chmod produces. The filesystem state is set up BEFORE the call, so
# run_verdict's subshell sees it: a subshell inherits the filesystem and isolates
# only shell state, which is exactly the split this case needs.
chmod_settle_target="$MF_HOME/.local/libexec/osquery/chmod-settle.sh"
printf 'echo chmod settle\n' >"$chmod_settle_target"
chmod 755 "$chmod_settle_target"
touch -t 200001010000 "$chmod_settle_target"
chmod_settle_hash="$(hash_of "$chmod_settle_target")"
printf 'deadbeef 0755 %s /nowhere\n' "$(id -u)" >"$MF_MANIFEST"
touch -t 200001010000 "$MF_MANIFEST"
(
  sleep 1
  printf '%s 0755 %s %s\n' "$chmod_settle_hash" "$(id -u)" "$chmod_settle_target" >"$MF_MANIFEST"
) &
chmod_settle_pid=$!
got=0
run_verdict "$chmod_settle_target" "$chmod_settle_hash" ATTRIBUTES_MODIFIED 5 || got=$?
wait "$chmod_settle_pid"
[[ $got -eq 1 ]] ||
  fail "an attribute-only change whose manifest lands inside the settle window must resolve to SILENT (got rc $got)"

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

# --- THE ALLOWLIST BINDING SURVIVES THE SAME APPLY RACE ----------------------
# allowlist_verdict refuses to suppress unless the manifest vouches for the
# allowlist it just read, so it inherits the apply race the settle window exists
# for: the new allowlist lands, the manifest is reinstalled a moment later, and a
# persistence finding judged in between would otherwise be told the allowlist
# cannot be trusted and would page for a known-good own agent. The alerter judges
# each finding exactly once, so that false CRIT would never be reconsidered.
#
# The binding reuses _pipeline_tuple_settles rather than waiting its own way, and
# this is what pins that: the same back-dated-manifest fixture that resolves for a
# pipeline script has to resolve for the allowlist.
ALLOWLIST_VERDICT="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/allowlist-verdict.sh"
[[ -f $ALLOWLIST_VERDICT ]] || fail "missing the allowlist verdict: $ALLOWLIST_VERDICT"
# The settle sections above deliberately leave $MF_MANIFEST holding a back-dated
# stub, so regenerate it here rather than reading columns out of that wreckage.
manifest_fixture_apply
manifest_fixture_run_runner "$RUNNER" || fail "the runner exited non-zero before the allowlist race case"
allowlist_target="$MF_HOME/.config/osquery/page-launchd-allowlist.txt"
[[ -r $allowlist_target ]] || fail "the fixture apply did not deploy the allowlist to $allowlist_target"

# run_allowlist_verdict <settle-seconds> -- one verdict for the fixture's com.seed
# tuple, in a SUBSHELL for the reason run_verdict uses one: the settle budget is
# one per alerter run, and a case that spends it must not answer for the next.
run_allowlist_verdict() {
  (
    # shellcheck disable=SC2030,SC2031
    export HOME="$MF_HOME" OSQUERY_PIPELINE_SETTLE_SECONDS="$1"
    export OSQUERY_LAUNCHD_ALLOWLIST="$allowlist_target"
    # shellcheck source=/dev/null
    source "$ALLOWLIST_VERDICT"
    allowlist_verdict com.seed "$MF_HOME/x.plist" "$MF_HOME/x"
  )
}

allowlist_hash="$(hash_of "$allowlist_target")"
allowlist_mode="$(manifest_mode_of "$allowlist_target")"
allowlist_uid="$(manifest_uid_of "$allowlist_target")"
[[ $allowlist_mode == 0600 ]] ||
  fail "the generated manifest records the allowlist at '$allowlist_mode', expected 0600 (its private_ prefix)"

# A manifest that PREDATES the allowlist and does not yet name it: the verdict
# waits, and suppresses once the regeneration lands inside the window.
printf 'deadbeef 0755 %s /nowhere\n' "$(id -u)" >"$MF_MANIFEST"
touch -t 200001010000 "$MF_MANIFEST"
(
  sleep 1
  printf '%s %s %s %s\n' "$allowlist_hash" "$allowlist_mode" "$allowlist_uid" "$allowlist_target" >"$MF_MANIFEST"
) &
allowlist_settle_pid=$!
got=0
run_allowlist_verdict 4 || got=$?
wait "$allowlist_settle_pid"
[[ $got -eq 0 ]] ||
  fail "a legitimate apply must NOT false-page: the allowlist binding has to settle when the manifest lands (got rc $got)"

# ...and the wait is bounded the same way: a binding that never arrives refuses to
# suppress, so the finding pages rather than trusting an unaccountable allowlist.
printf 'deadbeef 0755 %s /nowhere\n' "$(id -u)" >"$MF_MANIFEST"
touch -t 200001010000 "$MF_MANIFEST"
start=$(date +%s)
got=0
run_allowlist_verdict 2 || got=$?
elapsed=$(($(date +%s) - start))
[[ $got -eq 1 ]] ||
  fail "an allowlist the manifest never vouches for must not suppress (got rc $got)"
((elapsed <= 6)) || fail "the allowlist binding wait is not bounded (${elapsed}s for a 2s window)"

if [[ $fails -gt 0 ]]; then
  printf '%d check(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'osquery-pipeline-manifest-agreement: OK (both generated manifests and the real verdict agree, including a chmod on unchanged content; the periodic audit parses the same generated manifests and reports a real tamper; watch/tracked/manifest cover the identical set across BOTH launch agent roots; a /Library twin is untracked; the managed-bin arm is contained by its watch root, tracks exactly what it manifests, stays disjoint from the pipeline manifest, and pages a tamper and a chmod while an unmanaged neighbor is silent; producer and consumer name both default paths; the settle window resolves a live regeneration for a content AND an attribute-only change, stays bounded, and spends ONE budget per alerter run across many misses; the allowlist binding settles through the same apply race and stays bounded; the periodic audit covers the allowlist and reports its content and mode divergences as separate kinds)\n'
