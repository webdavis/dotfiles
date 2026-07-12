#!/usr/bin/env bash
# osquery-agent-binary-signal.sh (FX10). The agent_binary_changed query used the hash
# table, but codex is a ~260MB binary that exceeds osquery's read_max, so its sha256
# comes back EMPTY — a hash-based change detector that can never fire for the very
# binary most worth watching. The replacement must emit a NONEMPTY change signal for
# BOTH codex and paseo using a size-agnostic mechanism. This runs the rendered query
# live and asserts each binary present yields a row with at least one nonempty non-path
# field (the file size/inode/mtime tuple, plus cdhash/team_identifier when signed).
#
# Live + environment-bound (needs osqueryi and the real binaries) -> e2e camp; it SKIPs
# where they are absent (e.g. CI), so it is a real check on the dev host and a no-op in CI.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." || exit 1 && pwd)"
cd "$REPO_ROOT" || exit 1

OSQUERYI="${OSQUERYI:-$(command -v osqueryi || true)}"
[[ -n $OSQUERYI ]] || {
  printf 'SKIP: osqueryi not found\n'
  exit 0
}
command -v chezmoi >/dev/null 2>&1 || {
  printf 'SKIP: chezmoi not found (run inside the nix dev shell)\n'
  exit 0
}

render_home="$(mktemp -d)"
trap 'rm -rf "$render_home"' EXIT
query="$(HOME="$render_home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <.chezmoitemplates/osquery/packs/agent-attack-surface.conf | jq -r '.queries.agent_binary_changed.query')"
[[ -n $query && $query != null ]] || {
  printf 'FAIL: agent_binary_changed query not found\n' >&2
  exit 1
}

rows="$("$OSQUERYI" --json "$query" 2>/dev/null || echo '[]')"

checked=0
fails=0
for path in /opt/homebrew/bin/codex /opt/homebrew/bin/paseo; do
  [[ -e $path ]] || {
    printf 'SKIP: %s not present on this host\n' "$path"
    continue
  }
  checked=$((checked + 1))
  # A nonempty signal = a row for this path where at least one non-path field is nonempty.
  signal="$(jq -r --arg p "$path" '
    [ .[] | select(.path == $p) | to_entries[] | select(.key != "path" and (.value | tostring | length > 0)) ] | length
  ' <<<"$rows" 2>/dev/null || echo 0)"
  if [[ ${signal:-0} -gt 0 ]]; then
    printf 'PASS: %s yields a nonempty change signal (%s field(s))\n' "$path" "$signal"
  else
    printf 'FAIL: %s produced NO nonempty signal — a swap would be undetectable\n' "$path" >&2
    fails=$((fails + 1))
  fi
done

[[ $checked -eq 0 ]] && {
  printf 'SKIP: neither codex nor paseo present\n'
  exit 0
}
[[ $fails -eq 0 ]] || exit 1
printf 'PASS: agent_binary_changed emits a size-agnostic nonempty signal for every present binary\n'
