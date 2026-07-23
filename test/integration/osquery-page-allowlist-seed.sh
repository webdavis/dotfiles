#!/usr/bin/env bash
#
# The launchd page-allowlist ships as the private NDJSON tuple file
# dot_config/osquery/private_page-launchd-allowlist.txt (the private_ prefix makes
# chezmoi deploy it at 0600), and the old flat bare-label list is gone. Pins:
#   - the tuple file exists and its source basename carries the private_ prefix
#     (chezmoi sets the 0600 mode from that prefix);
#   - every non-comment line is valid NDJSON carrying exactly the four tuple fields
#     (label, path, program, sha256);
#   - a home-rooted path/program is stored as ~/ , never an absolute /Users/ path,
#     so the file stays user-agnostic;
#   - the file contains no em-dash or en-dash;
#   - the old flat dot_config/osquery/launch-allowlist.txt is absent from the tree.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1 && pwd)"

new_file="$REPO_ROOT/dot_config/osquery/private_page-launchd-allowlist.txt"
old_file="$REPO_ROOT/dot_config/osquery/launch-allowlist.txt"

if ! command -v jq >/dev/null 2>&1; then
  printf 'SKIP: jq not found (run inside the nix dev shell)\n'
  exit 0
fi

fails=0
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  fails=$((fails + 1))
}

entry_count=0

# The tuple file ships, at the private_-prefixed source path (chezmoi -> 0600 target).
if [[ ! -f $new_file ]]; then
  fail "the tuple file is missing: $new_file"
else
  case "$(basename "$new_file")" in
    private_*) : ;;
    *) fail "the source basename must carry the private_ prefix (0600 deploy): $(basename "$new_file")" ;;
  esac

  # No em-dash (U+2014) or en-dash (U+2013) anywhere in the file.
  if LC_ALL=C grep -qE $'\xe2\x80\x94|\xe2\x80\x93' "$new_file"; then
    fail "the tuple file contains an em-dash or en-dash"
  fi

  # No absolute home leak: a home-rooted path must be stored as ~/ , never /Users/.
  if grep -q '/Users/' "$new_file"; then
    fail "the tuple file leaks an absolute /Users/ path (store home as ~/)"
  fi

  # Every non-comment, non-blank line is valid NDJSON carrying exactly the four fields.
  while IFS= read -r line || [[ -n $line ]]; do
    case "$line" in
      '' | '#'*) continue ;;
    esac
    entry_count=$((entry_count + 1))
    if ! jq -e 'type == "object"' <<<"$line" >/dev/null 2>&1; then
      fail "not valid NDJSON: $line"
      continue
    fi
    if ! jq -e '(keys_unsorted | sort) == ["label","path","program","sha256"]' <<<"$line" >/dev/null 2>&1; then
      fail "a tuple must carry exactly label,path,program,sha256: $line"
    fi
  done <"$new_file"

  # The seed migrates this host's own-agents (results-alerter,
  # firewall-gatekeeper-monitor, uptime-watchdog, alert-drainer, heartbeat) to tuples,
  # so none false-pages the alerter's persistence_launchd detector (which
  # default-denies an unallowlisted user LaunchAgent) when its plist first appears.
  if [[ $entry_count -ne 5 ]]; then
    fail "expected 5 seeded tuples (the host's own-agents), got $entry_count"
  fi

  # The alert-drainer is one own-agent of the class: a real own-agent, so it does not
  # false-page when the alerter goes live at the D1 cutover.
  if ! grep -qF '"label":"com.webdavis.osquery-alert-drainer"' "$new_file"; then
    fail "the alert-drainer own-agent tuple is missing from the seed"
  fi

  # The heartbeat is an own-agent added after the cutover: without its own tuple its
  # plist would self-page the alerter (default-deny), so its tuple is seeded here too.
  if ! grep -qF '"label":"com.webdavis.osquery-heartbeat"' "$new_file"; then
    fail "the heartbeat own-agent tuple is missing from the seed"
  fi
fi

# The old flat bare-label allowlist is gone.
if [[ -e $old_file ]]; then
  fail "the old flat allowlist must be removed from the tree: $old_file"
fi

if [[ $fails -gt 0 ]]; then
  printf '%d check(s) failed\n' "$fails" >&2
  exit 1
fi

printf 'osquery-page-allowlist-seed: OK (%d tuple(s), private_ 0600, no /Users/ leak, no dash, old flat file gone)\n' "$entry_count"
