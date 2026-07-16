#!/usr/bin/env bash
#
# Guard: the osquery feature-set lives under ~/.local/libexec/osquery/ and every
# live consumer references the new location. Asserts only the NEW expected state
# (positive assertions; no scans for the old path). Fail CLOSED: any git failure
# fails the test, never reads as a pass.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

fail=0

# 1. The libexec home holds the five scripts (prefix dropped). A git ls-files
# failure fails the test.
for f in alert-dispatch enrich-finding firewall-gatekeeper-monitor results-alerter uptime-watchdog; do
  path="dot_local/libexec/osquery/executable_${f}.sh"
  if ! listed="$(git ls-files "$path")"; then
    printf 'FAIL: git ls-files failed while checking %s\n' "$path" >&2
    fail=1
  elif [[ -z $listed ]]; then
    printf 'FAIL: missing %s\n' "$path" >&2
    fail=1
  fi
done

# 2. Positive reference assertions: every consumer cites its new path.
# git grep -qF branched on its exact status: 0 = found (pass), 1 = missing
# (fail with diagnostics), anything else = fail with the git error.
assert_contains() {
  # $1 = fixed string that must appear, $2 = tracked file it must appear in.
  local needle="$1" file="$2" status=0
  git grep -qF "$needle" -- "$file" || status=$?
  case $status in
    0) ;;
    1)
      printf 'FAIL: %s does not reference %s\n' "$file" "$needle" >&2
      fail=1
      ;;
    *)
      printf 'FAIL: git grep failed (status %d) while checking %s\n' "$status" "$file" >&2
      fail=1
      ;;
  esac
}

# The three launchd plists' ProgramArguments point at the libexec home.
for name in firewall-gatekeeper-monitor results-alerter uptime-watchdog; do
  assert_contains ".local/libexec/osquery/${name}.sh" \
    "Library/LaunchAgents/com.webdavis.osquery-${name}.plist.tmpl"
done

# The three consumers source the dispatch library from the libexec home.
# The needles below are literal source lines; $HOME must NOT expand here.
for name in firewall-gatekeeper-monitor results-alerter uptime-watchdog; do
  # shellcheck disable=SC2016
  assert_contains 'source "$HOME/.local/libexec/osquery/alert-dispatch.sh"' \
    "dot_local/libexec/osquery/executable_${name}.sh"
done

# results-alerter invokes the enricher from the libexec home.
# shellcheck disable=SC2016
assert_contains 'ENRICH_SCRIPT="$HOME/.local/libexec/osquery/enrich-finding.sh"' \
  "dot_local/libexec/osquery/executable_results-alerter.sh"

if [[ $fail -ne 0 ]]; then
  printf 'osquery-feature-set-location: FAIL\n' >&2
  exit 1
fi
printf 'osquery-feature-set-location: OK\n'
