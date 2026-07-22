#!/usr/bin/env bash
#
# The route gate's file_events pipeline arm, wired to pipeline_verdict (B12). A
# pipeline_integrity / launch_agents / launch_daemons file event consults the
# verdict instead of paging unconditionally:
#   pipeline_verdict 0 (page: tamper / cannot confirm / no manifest) -> sev=CRIT
#   pipeline_verdict 1 (silent: untracked neighbor, or an exact manifest match) -> continue
#
# The manifest slice (15) does not exist yet, so a tracked change fails open to a
# PAGE (criterion 6). This test pins both halves: the fail-open page (no manifest)
# AND that the verdict is genuinely consulted - an untracked neighbor stays silent,
# and a stubbed exact (path, sha256) manifest match suppresses the page.
#
# Unit test: fixture file_events findings under a temp HOME (so the tracked-path
# prefixes resolve), two gate passes (no manifest, then a stubbed manifest).
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

# A file event: q=file_events_recent, cols.category the watch category, target_path
# the file, sha256 the event hash, action the FSEvents verb.
fe() { # <category> <target_path> <sha256> <verb>
  printf '{"q":"file_events_recent","act":"added","cols":{"category":"%s","target_path":"%s","sha256":"%s","action":"%s"},"ep":""}\n' \
    "$1" "$2" "$3" "$4"
}

# run_gate <manifest> <finding...> -> page NDJSON on stdout (digests go to the spy).
run_gate() {
  local manifest="$1"
  shift
  printf '%s\n' "$@" |
    HOME="$home" OSQUERY_PIPELINE_MANIFEST="$manifest" OSQUERY_PIPELINE_REHASH_DELAY=0 \
      OSQUERY_LAUNCHD_ALLOWLIST="$work/no-allowlist.txt" DIGEST_SPY="$spy" bash -c '
        source "$1"
        source "$2"
        source "$3"
        digest_append() { printf "%s\n" "$1" >>"$DIGEST_SPY"; }
        route_findings
      ' _ "$ROUTE" "$PIPELINE_HELPER" "$ALLOWLIST_HELPER"
}

# ---- Pass A: NO manifest. Tracked changes fail open to a page; an untracked
#      neighbor is silent (proving the verdict is consulted, not page-always). ----
page_a="$(run_gate "$absent_manifest" \
  "$(fe pipeline_integrity "$home/.local/libexec/osquery/results-alerterTAG01.sh" abc UPDATED)" \
  "$(fe pipeline_integrity "$home/.local/bin/relayTAG02.sh" abc UPDATED)" \
  "$(fe launch_agents "$home/Library/LaunchAgents/com.webdavis.osquery-uptimeTAG03.plist" abc UPDATED)" \
  "$(fe launch_agents "$home/Library/LaunchAgents/com.apple.somethingTAG04.plist" abc UPDATED)")"

in_a() { grep -qF "$1" <<<"$page_a"; }
in_a TAG01 || fail "a ~/.local/libexec/osquery script change must PAGE (fail-open, no manifest)"
in_a TAG02 || fail "a ~/.local/bin script change must PAGE (fail-open, second prefix)"
in_a TAG03 || fail "our own osquery LaunchAgent change must PAGE (fail-open)"
in_a TAG04 && fail "an untracked neighbor plist must be SILENT (the verdict is consulted, not page-always)"

# ---- Pass B: a stubbed manifest with an exact (path, sha256) match. That event
#      is confirmed known-good and stays silent; a DELETE still pages. ----
known_target="$home/.local/libexec/osquery/knownTAG05.sh"
known_hash="1111111111111111111111111111111111111111111111111111111111111111"
manifest="$work/pipeline-known-good.sha256"
printf '%s  %s\n' "$known_hash" "$known_target" >"$manifest"

page_b="$(run_gate "$manifest" \
  "$(fe pipeline_integrity "$known_target" "$known_hash" UPDATED)" \
  "$(fe pipeline_integrity "$home/.local/libexec/osquery/results-alerterTAG06.sh" "" DELETED)")"

in_b() { grep -qF "$1" <<<"$page_b"; }
in_b TAG05 && fail "an exact (path, sha256) manifest match must be SILENT (the verdict consults the manifest)"
in_b TAG06 || fail "a DELETE of a tracked pipeline file must PAGE even with a manifest present"

# The pipeline arm never digests; the spy must be empty.
[[ ! -s $spy ]] || fail "a pipeline file event must never digest; spy got: $(cat "$spy")"

# Every paged line is a CRIT finding.
for out in "$page_a" "$page_b"; do
  [[ -z $out ]] && continue
  [[ "$(jq -s 'all(.[]; .sev == "CRIT")' <<<"$out")" == true ]] ||
    fail "every paged pipeline finding must carry .sev == CRIT"
done

printf 'osquery-route-pipeline: OK (fail-open PAGE under both prefixes + own plist; neighbor SILENT; manifest exact match SILENT; delete PAGES; none digest)\n'
