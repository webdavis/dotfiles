#!/usr/bin/env bash
# update-skills-refusal-tempfile.sh (fix-A F9): the roster snapshot mktemp's a
# run-private copy and an EXIT trap removes it, but the zero-tracked refusal
# `exit 1` fired BEFORE that trap was installed, leaking one mktemp per refused
# run. The fix installs the cleanup trap the moment the snapshot succeeds, so
# every later refusal exit cleans up. Assert a refused run leaves no
# update-skills-roster.* temp behind.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# shellcheck source=test/fixtures/exchange-tool.lib.sh
source "$REPO_ROOT/test/fixtures/exchange-tool.lib.sh"
GMV_BIN="$(resolve_exchange_tool)" ||
  fail "no GNU coreutils mv with a working --exchange on PATH (need gmv or mv)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
export UPDATE_SKILLS_GMV="$GMV_BIN"
# A private, empty TMPDIR so the only files that appear are the updater's.
TMPDIR="$tmp/tmpdir"
export TMPDIR
mkdir -p "$TMPDIR" "$HOME/.agents/skills"

stub="$tmp/stub"
mkdir -p "$stub"
printf '#!/usr/bin/env bash\nexit 0\n' >"$stub/alerter"
chmod +x "$stub/alerter"
export PATH="$stub:$PATH"

# A VALID roster whose tracked union is empty: the snapshot SUCCEEDS (creating
# the run-private temp), then the zero-union guard refuses, the exact path that
# leaked before the fix.
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {},
  "hermesProfiles": {},
  "hermesRegistry": {},
  "npxTracked": {},
  "clawhubTracked": {},
  "forks": {}
}
EOF

count_snapshots() {
  local n=0 f
  for f in "$TMPDIR"/update-skills-roster.*; do
    [[ -e $f ]] && n=$((n + 1))
  done
  printf '%s' "$n"
}

set +e
out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)"
rc=$?
set -e
[[ $rc -ne 0 ]] || fail "the zero-union run did not refuse (exit 0): $out"
grep -qi 'REQUIRED-FAILURE' <<<"$out" || fail "the run did not record a required failure: $out"

leaked="$(count_snapshots)"
[[ $leaked -eq 0 ]] ||
  fail "the refused run leaked $leaked roster snapshot temp file(s) in $TMPDIR"

echo "update-skills-refusal-tempfile: OK"
