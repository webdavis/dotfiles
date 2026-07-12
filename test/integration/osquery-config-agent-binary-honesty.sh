#!/usr/bin/env bash
# osquery-config-agent-binary-honesty.sh (R2-12). The agent_binary_changed detector is blind to
# CONTENT changes of oversized/unsigned binaries: codex (~260 MB) exceeds osquery read_max so its
# sha256 is always empty, and paseo is unsigned so it has no cdhash signal - its only signal is the
# coarse (size, inode, mtime) tuple, which an in-place edit can preserve. The tier is (and must
# stay) LOG-ONLY, and its description must be HONEST about this limitation and point at the robust
# follow-up, making no false promise of content-change detection.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1 && pwd)"
cd "$REPO_ROOT" || exit 1

pack=".chezmoitemplates/osquery/packs/agent-attack-surface.conf"
desc="$(jq -r '.queries.agent_binary_changed.description' "$pack")"
[[ -n $desc && $desc != null ]] || {
  printf 'FAIL: agent_binary_changed description missing\n' >&2
  exit 1
}

fails=0
must() {
  if ! grep -qiE "$1" <<<"$desc"; then
    printf 'FAIL: description must mention /%s/\n' "$1" >&2
    fails=$((fails + 1))
  fi
}
must_not() {
  if grep -qiE "$1" <<<"$desc"; then
    printf 'FAIL: description must NOT claim /%s/ (false promise)\n' "$1" >&2
    fails=$((fails + 1))
  fi
}

# Honest about the blind spot + the follow-up.
must 'read_max|oversized'                          # names why the content hash is unavailable
must 'does NOT detect|no false promise|undetected' # states the limitation plainly
must 'unsigned'                                    # names the unsigned-binary gap (paseo)
must 'follow-up|task #23|#23'                      # points at the robust fix

# No FALSE promise of content-change detection.
must_not 'content hash of (codex|paseo)'
must_not 'detects content changes'

# The alerter keeps this LOG-ONLY: the gate arm must continue (never page/digest it).
alerter="dot_local/bin/executable_osquery-results-alerter.sh"
if ! grep -qE 'agent_binary_changed\) continue' "$alerter"; then
  printf 'FAIL: agent_binary_changed must stay log-only (its gate arm should continue)\n' >&2
  fails=$((fails + 1))
fi

if ((fails > 0)); then
  printf '%d agent_binary_changed honesty assertion(s) failed\n' "$fails" >&2
  exit 1
fi
printf 'PASS: agent_binary_changed is honest about its oversized/unsigned blind spot, references the follow-up, stays log-only\n'
