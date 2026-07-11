#!/usr/bin/env bash
# update-skills-lock-symlink.sh proves the npx CLI reads and writes the npx
# lock THROUGH the ~/.agents/.skill-lock.json symlink into the live generation
# (Wave 3a fix4 lock-symlink test). POSIX open() follows the symlink, so a CLI
# that opens ~/.agents/.skill-lock.json transparently reads and updates the
# generation's real lock. Encoded here with the npx stub.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

# Resolve the generation-exchange tool the way the updater does at run time
# (gmv on a Homebrew host, plain mv in the Nix devshell), never a hardcoded
# host path.
# shellcheck source=test/fixtures/exchange-tool.lib.sh
source "$REPO_ROOT/test/fixtures/exchange-tool.lib.sh"
GMV_BIN="$(resolve_exchange_tool)" ||
  fail "no GNU coreutils mv with a working --exchange on PATH (need gmv or mv)"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{"npxTracked":{"alpha":{"repo":"x/a"}},"clawhubTracked":{}}
EOF

export UPDATE_SKILLS_GMV="$GMV_BIN"
export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck disable=SC1090
source "$SCRIPT"

# A published generation with the lock symlink planted.
mkdir -p "$SKILLS_CURRENT/skills/alpha"
printf -- '---\nname: alpha\n---\n' >"$SKILLS_CURRENT/skills/alpha/SKILL.md"
printf '{"initial":true}\n' >"$SKILLS_CURRENT/.skill-lock.json"
__gen_write_meta "$SKILLS_CURRENT" "live-1-1"
__gen_plant_store_link alpha
__gen_plant_lock_link
[[ -L $SKILL_LOCK_LINK ]] || fail "precondition: the lock link was not planted as a symlink"

# A CLI reads the lock through the symlink.
read_via_link="$(cat "$SKILL_LOCK_LINK")"
[[ $read_via_link == '{"initial":true}' ]] ||
  fail "reading through the lock symlink did not return the generation's lock content: $read_via_link"

# A CLI (npx stub) opens ~/.agents/.skill-lock.json for WRITE and updates it; the
# write must land in the generation's real lock, not clobber the symlink.
stub="$tmp/stub"
mkdir -p "$stub"
cat >"$stub/npx" <<STUB
#!/usr/bin/env bash
set -euo pipefail
# Simulate the CLI persisting an updated lock through the well-known path.
printf '{"updatedByCli":true}\n' >"\$HOME/.agents/.skill-lock.json"
echo "stub npx: lock rewritten"
STUB
chmod +x "$stub/npx"
PATH="$stub:$PATH" npx skills update >/dev/null 2>&1 || fail "stub npx run failed"

# The symlink itself must still be a symlink (not clobbered into a file).
[[ -L $SKILL_LOCK_LINK ]] ||
  fail "the write clobbered the lock symlink into a regular file (open should follow it)"
# The write landed in the generation's real lock.
[[ "$(cat "$SKILLS_CURRENT/.skill-lock.json")" == '{"updatedByCli":true}' ]] ||
  fail "the CLI write did not land in the generation's real lock through the symlink"
# Reading through the link reflects the update.
[[ "$(cat "$SKILL_LOCK_LINK")" == '{"updatedByCli":true}' ]] ||
  fail "reading through the symlink does not reflect the CLI update"

echo "update-skills-lock-symlink: OK"
