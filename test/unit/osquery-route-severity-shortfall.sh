#!/usr/bin/env bash
#
# route_findings (results-alerter/route.sh) computes the batch of base severities
# by piping every finding through route_severity ONCE, then reads back one
# severity per finding by index. This suite pins what must happen when that batch
# comes back SHORT: route_severity (or a jq inside it) dies partway, so there are
# fewer severities than findings.
#
# The invariant: a severity that is missing must NEVER turn a finding into a
# silent all-clear. The unclassified finding resolves to CRIT - it PAGES, the
# fail-safe direction - and the degradation is announced on stderr (the alerter's
# launchd log). Over-paging is recoverable; a dropped security finding is not.
#
# Production faithfulness: the entry (results-alerter.sh) runs `set -euo
# pipefail` and sources these helpers into one process, so the gate pass here
# runs under exactly those options. That matters in both directions - under
# `set -u` a missing index is a fatal unbound-variable abort that loses the WHOLE
# batch, and without it the missing findings vanish silently - and neither is
# acceptable, so route_findings must not rely on the caller's shell options to
# notice the shortfall.
#
# Unit test: fixture normalized findings, each tagged with a unique token, run
# through ONE gate pass with route_severity doubled. A token in stdout => paged.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTE="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/route.sh"
ALLOWLIST_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/allowlist-verdict.sh"
PIPELINE_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/pipeline-verdict.sh"

fail() {
  printf 'osquery-route-severity-shortfall: FAIL -- %s\n' "$*" >&2
  exit 1
}

# A plain refute helper, NOT a bare `! grep`: an inverted pipeline returns 0 to
# `set -e`, so `! grep -q ...` can never fail a test. This one calls fail itself.
refute_contains() { # <haystack> <needle> <message>
  if grep -qF -- "$2" <<<"$1"; then fail "$3"; fi
}

assert_contains() { # <haystack> <needle> <message>
  grep -qF -- "$2" <<<"$1" || fail "$3"
}

[[ -f $ROUTE ]] || fail "missing helper: $ROUTE"
[[ -f $ALLOWLIST_HELPER ]] || fail "missing helper: $ALLOWLIST_HELPER"
[[ -f $PIPELINE_HELPER ]] || fail "missing helper: $PIPELINE_HELPER"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Real normalize-shaped findings. TAG_A is classified by the truncated batch;
# TAG_B and TAG_C are the ones whose severity never arrives. Both are page-tier
# detectors whose gate arm does NOT set the severity itself (filevault_off has no
# arm at all; suid_bin_unexpected relies on the base CRIT), so each one's outcome
# is decided purely by the severity read - exactly the findings a shortfall would
# lose. TAG_D is an INFO drift row, the control: it must stay log-only under a
# HEALTHY batch, so a "page everything, always" implementation cannot pass.
findings=(
  '{"q":"new_admin_user","act":"added","cols":{"username":"adminTAG_A","uid":"501"},"ep":""}'
  '{"q":"filevault_off","act":"added","cols":{"note":"TAG_B"},"ep":""}'
  '{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/suidTAG_C"},"ep":""}'
  '{"q":"homebrew_packages","act":"added","cols":{"name":"pkgTAG_D"},"ep":""}'
)

# run_gate <severity-double-body> -> stdout on fd 1, stderr captured to $work/err,
# route_findings' exit status echoed on the last line of $work/status.
#
# The gate pass runs under `set -euo pipefail` (the entry's options) in its own
# bash so the doubles cannot leak into this test's shell. digest_append is doubled
# to a no-op: no fixture here is digest-tier, and the spool is another suite's
# subject. The enricher is pointed at a path that does not exist, so enrichment is
# skipped and the severity read is the only variable.
run_gate() { # <severity-double-body>
  local double_body="$1"
  local status=0
  printf '%s\n' "${findings[@]}" |
    OSQUERY_ENRICH_SCRIPT="$work/no-enricher.sh" \
      OSQUERY_LAUNCHD_ALLOWLIST="$work/no-allowlist.txt" \
      OSQUERY_PIPELINE_MANIFEST="$work/no-manifest.sha256" \
      SEVERITY_DOUBLE="$double_body" \
      bash -c '
      set -euo pipefail
      source "$1"
      source "$2"
      source "$3"
      digest_append() { :; }
      eval "$SEVERITY_DOUBLE"
      route_findings
    ' _ "$ROUTE" "$ALLOWLIST_HELPER" "$PIPELINE_HELPER" 2>"$work/err" || status=$?
  printf '%s' "$status" >"$work/status"
}

# -- The shortfall: route_severity classifies the first finding, then dies (the
# -- jq-killed-partway shape). Three severities are owed; one arrives.
short_out="$(run_gate 'route_severity() { printf "CRIT\n"; return 5; }')"
short_status="$(cat "$work/status")"
short_err="$(cat "$work/err")"

assert_contains "$short_out" TAG_A \
  "the classified finding must still page (status=$short_status, out=$short_out, err=$short_err)"
assert_contains "$short_out" TAG_B \
  "filevault_off lost its severity and was DROPPED instead of paging (status=$short_status, out=$short_out, err=$short_err)"
assert_contains "$short_out" TAG_C \
  "suid_bin_unexpected lost its severity and was DROPPED instead of paging (status=$short_status, out=$short_out, err=$short_err)"

# The pass must COMPLETE, not abort: aborting mid-batch loses the findings it had
# already classified, and a deterministic failure would then wedge every later
# batch behind an un-advancing cursor.
[[ $short_status -eq 0 ]] ||
  fail "route_findings must complete the batch on a severity shortfall, exited $short_status (err=$short_err)"

# The degradation must be HUMAN-VISIBLE, not inferred from an over-page.
[[ -n $short_err ]] ||
  fail "a severity shortfall must announce itself on stderr; nothing was written"
assert_contains "$short_err" severity \
  "the stderr diagnostic must name the severity shortfall (got: $short_err)"
# The double exits 5. Reporting that code proves route_severity's status is
# CAPTURED rather than thrown away by a process substitution, which is what let a
# failed batch look like a complete one in the first place.
assert_contains "$short_err" "route_severity exit 5" \
  "the diagnostic must report route_severity's real exit status (got: $short_err)"

# -- The control: with a healthy route_severity the same batch routes normally,
# -- so the fail-safe fallback cannot be a blanket page.
healthy_out="$(run_gate ':')"
healthy_status="$(cat "$work/status")"

[[ $healthy_status -eq 0 ]] || fail "the healthy pass must exit 0, exited $healthy_status"
assert_contains "$healthy_out" TAG_A "new_admin_user must page on a healthy batch"
assert_contains "$healthy_out" TAG_B "filevault_off added must page on a healthy batch"
assert_contains "$healthy_out" TAG_C "suid_bin_unexpected added must page on a healthy batch"
refute_contains "$healthy_out" TAG_D \
  "homebrew_packages is INFO drift and must stay log-only on a healthy batch"
[[ -z "$(cat "$work/err")" ]] ||
  fail "a healthy batch must not warn: $(cat "$work/err")"

printf 'osquery-route-severity-shortfall: OK (a short severity batch pages the unclassified findings, completes, and warns; a healthy batch is unchanged)\n'
