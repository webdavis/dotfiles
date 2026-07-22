#!/usr/bin/env bash
#
# The route gate's persistence_launchd arm, wired to allowlist_verdict
# (B11). This pins the DEFAULT-DENY security property (operator ruling
# 2026-07-22): a new user LaunchAgent PAGES unless it exactly matches an
# allowlisted known-good tuple. This is a deliberate hardening over the reverted
# #52 (c69baab), which DIGESTED unknown user LaunchAgents.
#
# Verdict -> outcome:
#   0 (full-tuple match, known-good) -> SUPPRESS (not paged; log-only).
#   2 (reused allowlisted label, identity diverges) -> PAGE.
#   1 (unknown, not allowlisted) -> PAGE (the default-deny change from c69baab).
#   /System/Library/* -> skipped (not paged); */LaunchDaemons/* -> PAGE by path.
#
# Unit test: a fixture allowlist tuple file + fixture on-disk plists under a temp
# HOME (so the pinned-hash dimension is exercised for real), the gate driven with
# allowlist_verdict sourced. Classification is by a unique TAGnn in each finding's
# columns (the paged finding is emitted whole on stdout; a suppressed one is not,
# and none of these should digest).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUTE="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/route.sh"
ALLOWLIST_HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/allowlist-verdict.sh"

fail() {
  printf 'osquery-route-persistence-allowlist: FAIL -- %s\n' "$*" >&2
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

# The known-good agent's on-disk plist, hashed into the allowlist tuple.
printf 'KNOWN GOOD PLIST\n' >"$home/Library/LaunchAgents/com.known.plist"
known_hash="$(shasum -a 256 "$home/Library/LaunchAgents/com.known.plist" | awk '{print $1}')"

# The allowlist tuple file: one known-good agent, stored home-relative (~/).
allowlist="$home/.config/osquery/page-launchd-allowlist.txt"
printf '{"label":"com.known","path":"~/Library/LaunchAgents/com.known.plist","program":"~/bin/known","sha256":"%s"}\n' \
  "$known_hash" >"$allowlist"

# Fixture persistence_launchd findings. Each carries a unique tag in .cols.tag,
# which allowlist_verdict ignores (it reads only label/path/program).
findings=(
  # full-tuple match -> SUPPRESS (not paged)
  "{\"q\":\"persistence_launchd\",\"act\":\"added\",\"cols\":{\"label\":\"com.known\",\"path\":\"$home/Library/LaunchAgents/com.known.plist\",\"program\":\"$home/bin/known\",\"tag\":\"TAG01\"},\"ep\":\"\"}"
  # same label, different program -> reused label -> PAGE
  "{\"q\":\"persistence_launchd\",\"act\":\"added\",\"cols\":{\"label\":\"com.known\",\"path\":\"$home/Library/LaunchAgents/com.known.plist\",\"program\":\"$home/bin/EVIL\",\"tag\":\"TAG02\"},\"ep\":\"\"}"
  # unknown label -> PAGE (default-deny)
  "{\"q\":\"persistence_launchd\",\"act\":\"added\",\"cols\":{\"label\":\"com.unknown\",\"path\":\"$home/Library/LaunchAgents/com.unknown.plist\",\"program\":\"$home/bin/unknown\",\"tag\":\"TAG03\"},\"ep\":\"\"}"
  # a LaunchDaemon path -> PAGE by path (allowlist not consulted)
  '{"q":"persistence_launchd","act":"added","cols":{"label":"com.daemon","path":"/Library/LaunchDaemons/com.daemon.plist","program":"/usr/bin/daemon","tag":"TAG04"},"ep":""}'
  # an Apple /System path -> skipped (not paged)
  '{"q":"persistence_launchd","act":"added","cols":{"label":"com.apple.x","path":"/System/Library/LaunchAgents/com.apple.x.plist","program":"/usr/bin/x","tag":"TAG05"},"ep":""}'
)

page_out="$(printf '%s\n' "${findings[@]}" |
  HOME="$home" OSQUERY_LAUNCHD_ALLOWLIST="$allowlist" DIGEST_SPY="$spy" bash -c '
    source "$1"
    source "$2"
    digest_append() { printf "%s\n" "$1" >>"$DIGEST_SPY"; }
    route_findings
  ' _ "$ROUTE" "$ALLOWLIST_HELPER")"

in_page() { grep -qF "$1" <<<"$page_out"; }

# full-tuple match -> suppressed
in_page TAG01 && fail "TAG01 (full-tuple match) must be SUPPRESSED, but it paged"
# reused label -> page
in_page TAG02 || fail "TAG02 (reused label, different program) must PAGE"
# unknown -> page (default-deny)
in_page TAG03 || fail "TAG03 (unknown user LaunchAgent) must PAGE under default-deny (operator ruling)"
# LaunchDaemon -> page by path
in_page TAG04 || fail "TAG04 (LaunchDaemon path) must PAGE by path"
# /System -> skipped
in_page TAG05 && fail "TAG05 (/System Apple agent) must be skipped, but it paged"

# Under default-deny nothing here digests (the digest-unknowns path is gone).
[[ ! -s $spy ]] || fail "no persistence_launchd finding should digest under default-deny; spy got: $(cat "$spy")"

# Every paged line is a CRIT finding.
[[ "$(jq -s 'all(.[]; .sev == "CRIT")' <<<"$page_out")" == true ]] ||
  fail "every paged persistence finding must carry .sev == CRIT"

printf 'osquery-route-persistence-allowlist: OK (full-tuple suppress; reused-label page; UNKNOWN pages [default-deny]; LaunchDaemon pages by path; /System skipped; none digest)\n'
