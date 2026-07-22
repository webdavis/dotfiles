#!/usr/bin/env bash
#
# The route gate's signing enrichment (B13). For a finding with an inspectable
# path and a CRIT/NOTICE base tier, the gate calls the enricher on the path,
# attaches its .signing fact for render-page, and promotes NOTICE -> CRIT when the
# signing verdict is UNTRUSTED (enricher exit 10).
#
# THE headline invariant: enrichment runs BEFORE the allowlist suppression, so an
# untrusted program behind a FULLY allowlisted launchd label still PAGES - a
# promoted CRIT is never suppressed by the allowlist. The tuple's plist-hash
# dimension catches a tampered plist; the signing verdict catches an untrusted
# program the plist launches; both are separate defenses and both must page.
#
# Unit test: a stubbed enricher (untrusted when the path contains UNTRUSTED, else
# trusted), a fixture allowlist with a full-tuple match, findings under a temp HOME.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTE="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/route.sh"
ALLOWLIST_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/allowlist-verdict.sh"

fail() {
  printf 'osquery-route-enrichment: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $ROUTE ]] || fail "missing helper: $ROUTE"
[[ -f $ALLOWLIST_HELPER ]] || fail "missing helper: $ALLOWLIST_HELPER"
command -v shasum >/dev/null 2>&1 || fail "shasum is required for this test"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
home="$work/home"
spy="$work/digest-spy.ndjson"
mkdir -p "$home/Library/LaunchAgents" "$home/bin" "$home/.config/osquery"

# The stub enricher: UNTRUSTED (exit 10) when the inspected path contains
# UNTRUSTED, else a trusted authority (exit 0). Deterministic, no real codesign.
enricher="$work/enrich-stub.sh"
cat >"$enricher" <<'STUB'
#!/usr/bin/env bash
case "$1" in
  *UNTRUSTED*) printf 'UNSIGNED'; exit 10 ;;
  *) printf 'signed: Apple'; exit 0 ;;
esac
STUB
chmod +x "$enricher"

# Two allowlisted known-good agents (full-tuple match). One's plist path carries
# UNTRUSTED (its program's signature is bad); the other is clean.
mk_plist() {
  printf 'PLIST %s\n' "$1" >"$1"
  shasum -a 256 "$1" | awk '{print $1}'
}
untrusted_plist="$home/Library/LaunchAgents/com.evilUNTRUSTED.plist"
trusted_plist="$home/Library/LaunchAgents/com.good.plist"
untrusted_hash="$(mk_plist "$untrusted_plist")"
trusted_hash="$(mk_plist "$trusted_plist")"

allowlist="$home/.config/osquery/page-launchd-allowlist.txt"
{
  printf '{"label":"com.evil","path":"~/Library/LaunchAgents/com.evilUNTRUSTED.plist","program":"~/bin/evil","sha256":"%s"}\n' "$untrusted_hash"
  printf '{"label":"com.good","path":"~/Library/LaunchAgents/com.good.plist","program":"~/bin/good","sha256":"%s"}\n' "$trusted_hash"
} >"$allowlist"

findings=(
  # HEADLINE: fully allowlisted (label+path+program+hash all match) BUT the program
  # is untrusted (ep path has UNTRUSTED) -> enrichment promotes NOTICE->CRIT ->
  # PAGES, beating the allowlist suppression. tag A.
  "{\"q\":\"persistence_launchd\",\"act\":\"added\",\"cols\":{\"label\":\"com.evil\",\"path\":\"$untrusted_plist\",\"program\":\"$home/bin/evil\",\"tag\":\"TAGA\"},\"ep\":\"$untrusted_plist\"}"
  # Fully allowlisted AND trusted -> stays NOTICE -> SUPPRESSED (not paged). tag B.
  "{\"q\":\"persistence_launchd\",\"act\":\"added\",\"cols\":{\"label\":\"com.good\",\"path\":\"$trusted_plist\",\"program\":\"$home/bin/good\",\"tag\":\"TAGB\"},\"ep\":\"$trusted_plist\"}"
  # suid_bin_unexpected (base CRIT already) with an untrusted binary -> PAGES and
  # carries .signing (enrichment attaches the fact even when no promotion is needed). tag C.
  '{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/suidUNTRUSTED","username":"root","tag":"TAGC"},"ep":"/tmp/suidUNTRUSTED"}'
  # suid with a trusted binary -> PAGES and carries the trusted authority in .signing. tag D.
  '{"q":"suid_bin_unexpected","act":"added","cols":{"path":"/tmp/suidOK","username":"root","tag":"TAGD"},"ep":"/tmp/suidOK"}'
)

page_out="$(printf '%s\n' "${findings[@]}" |
  HOME="$home" OSQUERY_LAUNCHD_ALLOWLIST="$allowlist" OSQUERY_ENRICH_SCRIPT="$enricher" DIGEST_SPY="$spy" bash -c '
    source "$1"
    source "$2"
    digest_append() { printf "%s\n" "$1" >>"$DIGEST_SPY"; }
    route_findings
  ' _ "$ROUTE" "$ALLOWLIST_HELPER")"

# signing_of <tag> -> the .signing value of the paged finding with that tag ("" if absent).
signing_of() { jq -rs --arg t "$1" 'map(select(.cols.tag == $t)) | (.[0].signing // "")' <<<"$page_out"; }
in_page() { grep -qF "$1" <<<"$page_out"; }

# HEADLINE: allowlisted + untrusted -> PAGES (promoted CRIT beats allowlist).
in_page TAGA || fail "HEADLINE: an untrusted program behind a fully allowlisted label MUST page (promoted CRIT beats suppression)"
[[ "$(signing_of TAGA)" == "UNSIGNED" ]] || fail "the promoted finding must carry .signing=UNSIGNED for render, got '$(signing_of TAGA)'"

# allowlisted + trusted -> suppressed.
in_page TAGB && fail "an allowlisted AND trusted agent must be SUPPRESSED (stays NOTICE), but it paged"

# suid untrusted -> pages with .signing (already CRIT, no promotion needed).
in_page TAGC || fail "a CRIT suid finding must still page"
[[ "$(signing_of TAGC)" == "UNSIGNED" ]] || fail "the CRIT finding must carry the untrusted .signing, got '$(signing_of TAGC)'"

# suid trusted -> pages with the trusted authority attached.
in_page TAGD || fail "a CRIT suid finding must page regardless of signature"
[[ "$(signing_of TAGD)" == "signed: Apple" ]] || fail "a trusted CRIT finding must carry .signing='signed: Apple', got '$(signing_of TAGD)'"

# The suppressed (trusted allowlisted) finding is log-only, not digested.
[[ ! -s $spy ]] || fail "the suppressed finding is log-only, not digested; spy got: $(cat "$spy")"

printf 'osquery-route-enrichment: OK (untrusted-behind-allowlisted PAGES [promoted CRIT beats suppression]; trusted allowlisted suppressed; .signing attached on CRIT findings, untrusted and trusted)\n'
