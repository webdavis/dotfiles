#!/usr/bin/env bash
#
# update-agent-plugins-scope.sh: the version snapshot helper reads USER scope
# only. `claude plugin list --json` can report a plugin at more than one scope
# (user, project, local), and the weekly job manages the USER installation. Left
# unscoped, a plugin present only at project scope was mistaken for the user copy,
# and a plugin with a user AND a project row produced duplicate snapshot rows that
# inflated the change count. This pins the scope filter directly on the helper.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/bin/executable_update-agent-plugins.sh"

fail() {
  printf 'update-agent-plugins-scope: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $HELPER ]] || fail "helper not found: $HELPER"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.local/bin"

set --
# shellcheck source=/dev/null
UPDATE_AGENT_PLUGINS_LIB_ONLY=1 source "$HELPER"
command -v __agent_plugins_versions >/dev/null 2>&1 ||
  fail "LIB_ONLY source did not define __agent_plugins_versions"

lock="$tmp/lock.json"
cat >"$lock" <<'EOF'
{"plugins":{"a@m":{"identityLane":"versioned"},"b@m":{"identityLane":"versioned"}}}
EOF

# a@m: a user row AND a project row (different versions); b@m: project-only.
inv="$tmp/inv.json"
cat >"$inv" <<'EOF'
[
  {"id":"a@m","version":"1.0.0","scope":"user"},
  {"id":"a@m","version":"9.9.9","scope":"project"},
  {"id":"b@m","version":"2.0.0","scope":"project"}
]
EOF

out="$(__agent_plugins_versions "$inv" "$lock")"

[[ "$(printf '%s\n' "$out" | grep -c .)" -eq 1 ]] ||
  fail "expected exactly one user-scope row, got: [$out]"
grep -qxF "$(printf 'a@m\t1.0.0')" <<<"$out" ||
  fail "the user-scope a@m row is missing or wrong: [$out]"
if grep -qF '9.9.9' <<<"$out"; then
  fail "the project-scope duplicate row leaked into the snapshot, which would inflate the change count: [$out]"
fi
if grep -qF 'b@m' <<<"$out"; then
  fail "a project-only plugin appeared in the user-scope snapshot: [$out]"
fi

# The undetermined-identity collector reads the same scope, so a project-only
# plugin with no version is not flagged as a user-scope identity problem.
uinv="$tmp/uinv.json"
cat >"$uinv" <<'EOF'
[
  {"id":"a@m","version":"1.0.0","scope":"user"},
  {"id":"b@m","version":"unknown","scope":"project"}
]
EOF
uout="$(__agent_plugins_versions_unknown "$uinv" "$lock")"
if grep -qF 'b@m' <<<"$uout"; then
  fail "a project-scope plugin with no version was flagged as a user-scope identity problem: [$uout]"
fi

printf 'update-agent-plugins-scope: OK (the version snapshot and the undetermined-identity collector read user scope only: a project-scope duplicate row is dropped and a project-only plugin is excluded)\n'
