#!/usr/bin/env bash
#
# Operator ruling 2026-07-22: a newly-loaded UNTRUSTED kernel or system extension
# pages; a signed one stays at its base tier. Enrichment already promotes a NOTICE
# finding with an inspectable path to CRIT on an untrusted signing verdict; the
# kernel_extensions_new and system_extensions_new gate arms must HONOR that
# promotion instead of discarding it.
#
#   kernel_extensions_new: untrusted (promoted CRIT) -> PAGE; signed -> log-only.
#   system_extensions_new: untrusted (promoted CRIT) -> PAGE; signed -> digest.
#
# Regression guards (scope is kext/sysext ONLY): es_launchd_writes and
# persistence_startup_items_crontab stay log-only REGARDLESS of the promotion.
#
# Unit test: stubbed enricher (untrusted when the path contains UNTRUSTED), findings
# tagged so a token in stdout = paged, in the digest spy = digested, in neither =
# log-only.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTE="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/route.sh"

fail() {
  printf 'osquery-route-kext-sysext: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $ROUTE ]] || fail "missing helper: $ROUTE"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
spy="$work/digest-spy.ndjson"

enricher="$work/enrich-stub.sh"
cat >"$enricher" <<'STUB'
#!/usr/bin/env bash
case "$1" in
  *UNTRUSTED*) printf 'UNSIGNED'; exit 10 ;;
  *) printf 'signed: Apple'; exit 0 ;;
esac
STUB
chmod +x "$enricher"

# Each finding carries a unique TAG in a natural field; ep drives the enricher.
findings=(
  # kernel_extensions_new: untrusted -> PAGE, trusted -> log-only
  '{"q":"kernel_extensions_new","act":"added","cols":{"name":"com.evilTAG_KEXT_U","path":"/x/UNTRUSTED.kext"},"ep":"/x/UNTRUSTED.kext"}'
  '{"q":"kernel_extensions_new","act":"added","cols":{"name":"com.goodTAG_KEXT_T","path":"/x/good.kext"},"ep":"/x/good.kext"}'
  # system_extensions_new: untrusted -> PAGE, trusted -> DIGEST
  '{"q":"system_extensions_new","act":"added","cols":{"identifier":"com.evilTAG_SYSEXT_U","bundle_path":"/x/UNTRUSTED.app"},"ep":"/x/UNTRUSTED.app"}'
  '{"q":"system_extensions_new","act":"added","cols":{"identifier":"com.goodTAG_SYSEXT_T","bundle_path":"/x/good.app"},"ep":"/x/good.app"}'
  # regression guards: still log-only even when the enricher would promote them
  '{"q":"es_launchd_writes","act":"added","cols":{"path":"/x/UNTRUSTED_esTAG_ES_U"},"ep":"/x/UNTRUSTED_esTAG_ES_U"}'
  '{"q":"persistence_startup_items_crontab","act":"added","cols":{"path":"/x/UNTRUSTED_cronTAG_CRON_U"},"ep":"/x/UNTRUSTED_cronTAG_CRON_U"}'
)

page_out="$(printf '%s\n' "${findings[@]}" |
  OSQUERY_ENRICH_SCRIPT="$enricher" DIGEST_SPY="$spy" bash -c '
    source "$1"
    digest_append() { printf "%s\n" "$1" >>"$DIGEST_SPY"; }
    route_findings
  ' _ "$ROUTE")"

classify() {
  local token="$1" want="$2" in_page=no in_spool=no
  grep -qF "$token" <<<"$page_out" && in_page=yes
  [[ -f $spy ]] && grep -qF "$token" "$spy" && in_spool=yes
  case "$want" in
    page) [[ $in_page == yes && $in_spool == no ]] || fail "$token expected PAGE, got page=$in_page digest=$in_spool" ;;
    digest) [[ $in_spool == yes && $in_page == no ]] || fail "$token expected DIGEST, got page=$in_page digest=$in_spool" ;;
    logonly) [[ $in_page == no && $in_spool == no ]] || fail "$token expected LOG-ONLY, got page=$in_page digest=$in_spool" ;;
  esac
}

classify TAG_KEXT_U page     # untrusted kernel extension pages (operator ruling)
classify TAG_KEXT_T logonly  # signed kernel extension stays at base tier (log-only)
classify TAG_SYSEXT_U page   # untrusted system extension pages
classify TAG_SYSEXT_T digest # signed system extension stays at base tier (digest)
classify TAG_ES_U logonly    # es_launchd_writes stays log-only despite the promotion
classify TAG_CRON_U logonly  # persistence_startup_items_crontab stays log-only despite the promotion

# The paged findings are emitted as CRIT.
[[ "$(jq -s 'all(.[]; .sev == "CRIT")' <<<"$page_out")" == true ]] ||
  fail "every paged kext/sysext finding must carry .sev == CRIT"

printf 'osquery-route-kext-sysext: OK (untrusted kext/sysext page; signed kext log-only, signed sysext digest; es_launchd_writes + crontab stay log-only despite the promotion)\n'
