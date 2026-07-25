#!/usr/bin/env bash
#
# route_findings (results-alerter/route.sh) ends by stamping every page-candidate
# with .sev = "CRIT" in one jq pass and writing it to stdout. That emit is the
# LAST point at which a confirmed page can be lost: downstream, render_page counts
# what it receives, and the entry checkpoints the cursor past the batch once the
# page is delivered or spooled.
#
# This suite pins that the emit's status REACHES the caller. A jq that dies there
# must not leave route_findings reporting a clean "nothing to page" - the entry
# would then advance the cursor past rows whose pages were never written, and
# those findings are gone for good.
#
# The gate pass runs WITHOUT `set -e` on purpose. Under the entry's options an
# errexit abort masks the question, but a library must not owe its correctness to
# the caller's shell options - this suite and the other route suites source
# route.sh exactly this way, so a swallowed status would green them too.
#
# Unit test: one page-tier finding, with jq shimmed to fail only the emit call.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTE="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/route.sh"

fail() {
  printf 'osquery-route-emit-failure: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $ROUTE ]] || fail "missing helper: $ROUTE"

REAL_JQ="$(command -v jq)" || fail "jq is not on PATH"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin"

# A jq shim that fails ONLY the page emit, and only when armed. Every other call
# (the per-field extractions, route_severity's classifier) passes through to the
# real jq, and a disarmed shim passes the emit through too, so the same PATH
# serves both the failure case and its control. The emit is identified by its
# program text, which no other call in route.sh uses.
cat >"$work/bin/jq" <<'SHIM'
#!/usr/bin/env bash
if [[ ${JQ_SHIM_FAIL_EMIT:-0} == 1 ]]; then
  for arg in "$@"; do
    if [[ $arg == '.sev = "CRIT"' ]]; then
      printf 'jq shim: simulated page-emit failure\n' >&2
      exit 5
    fi
  done
fi
exec "$REAL_JQ" "$@"
SHIM
chmod +x "$work/bin/jq"

# run_gate <arm-shim: 0|1> <finding-json> -> route_findings' exit status on
# stdout, its own stdout in $work/out and stderr in $work/err. No `set -e` in the
# gate shell: see the docblock.
run_gate() { # <arm-shim> <finding-json>
  local arm="$1" finding="$2" status=0
  printf '%s\n' "$finding" |
    REAL_JQ="$REAL_JQ" PATH="$work/bin:$PATH" JQ_SHIM_FAIL_EMIT="$arm" \
      OSQUERY_ENRICH_SCRIPT="$work/no-enricher.sh" \
      bash -c '
      set -o pipefail
      source "$1"
      digest_append() { :; }
      route_findings
    ' _ "$ROUTE" >"$work/out" 2>"$work/err" || status=$?
  printf '%s' "$status"
}

# A page-tier finding: new_admin_user is CRIT with no gate arm, so it reaches the
# emit as a page-candidate and nothing else decides its fate.
page_finding='{"q":"new_admin_user","act":"added","cols":{"username":"adminTAG_A","uid":"501"},"ep":""}'

# -- The emit fails: the status must reach the caller. --
failed_status="$(run_gate 1 "$page_finding")"
[[ $failed_status -ne 0 ]] ||
  fail "a failed page emit must not report success; route_findings returned 0 with stdout: $(cat "$work/out")"

# -- Control 1: the same finding with the shim disarmed still pages and exits 0,
# -- so the fix cannot be a blanket nonzero return.
healthy_status="$(run_gate 0 "$page_finding")"
[[ $healthy_status -eq 0 ]] ||
  fail "a healthy page emit must exit 0, exited $healthy_status ($(cat "$work/err"))"
grep -qF TAG_A "$work/out" ||
  fail "a healthy page emit must write the page-candidate, got: $(cat "$work/out")"

# -- Control 2: a batch with NO page-candidates never reaches the emit, so an
# -- armed shim is inert and the pass must still report success and write nothing.
# -- homebrew_packages is INFO drift, log-only.
logonly_status="$(run_gate 1 '{"q":"homebrew_packages","act":"added","cols":{"name":"pkgTAG_B"},"ep":""}')"
[[ $logonly_status -eq 0 ]] ||
  fail "a batch with no page-candidates must exit 0, exited $logonly_status ($(cat "$work/err"))"
if [[ -s $work/out ]]; then
  fail "a batch with no page-candidates must emit nothing, got: $(cat "$work/out")"
fi

printf 'osquery-route-emit-failure: OK (a failed page emit reports nonzero; a healthy emit pages and exits 0; nothing to page exits 0)\n'
