#!/usr/bin/env bash
# update-skills-symlink-safety.sh, profile discovery must NOT delete through a
# foreign directory symlink (Wave 3a item 8). The audit found that -d follows a
# symlinked profile (or its skills child), and ownership is decided from the
# literal relative link text, so a profiles/<name> symlink pointing outside
# ~/.hermes let convergence remove links in a foreign location THROUGH it. A
# managed hermes dir reached through a directory symlink must be skipped, so
# nothing is created or removed in the foreign target.
#
# The real script runs unmodified in a sandbox: a FULL run (offline stubs) so
# destructive convergence would happen, FORCE to bypass the idle-gate.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
STORE="$HOME/.agents/skills"
HERMES="$HOME/.hermes/skills"
PROFILES="$HOME/.hermes/profiles"
mkdir -p "$STORE" "$HOME/.claude/skills" "$HERMES" "$PROFILES"

mkdir -p "$STORE/alpha"
printf -- '---\nname: alpha\ndescription: fixture\n---\n' >"$STORE/alpha/SKILL.md"

# A FOREIGN store, outside ~/.hermes, with an owned-SHAPED stale link inside a
# skills dir. If convergence walks THROUGH a symlinked profile into here, it
# would unlink this via its literal relative target.
OUTSIDE="$tmp/outside"
mkdir -p "$OUTSIDE/skills"
ln -s "../../../../.agents/skills/gone" "$OUTSIDE/skills/victim" # owned-shaped, stale
printf 'precious\n' >"$OUTSIDE/skills/keepme.txt"

# profiles/spec is a SYMLINK pointing outside .hermes; its skills child resolves
# to the foreign dir above.
ln -s "$OUTSIDE" "$PROFILES/spec"

# Also test the skills-CHILD-is-a-symlink shape: a real profile dir whose skills
# child is a symlink into a second foreign store.
OUTSIDE2="$tmp/outside2"
mkdir -p "$OUTSIDE2/skills"
ln -s "../../../../.agents/skills/gone2" "$OUTSIDE2/skills/victim2"
mkdir -p "$PROFILES/spec2"
ln -s "$OUTSIDE2/skills" "$PROFILES/spec2/skills"

# Lock maps alpha to the default profile only; spec/spec2 are unmapped. Their
# skills dirs still EXIST on disk (through the symlinks), so the profile-universe
# walk would reach them, but the symlink guard must skip them.
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"alpha": "core"},
  "hermesProfiles": {"alpha": ["default"]},
  "hermesRegistry": {},
  "npxTracked": {},
  "clawhubTracked": {},
  "superpowersRouting": {},
  "forks": {}
}
EOF

stub="$tmp/stub"
mkdir -p "$stub"
printf '#!/usr/bin/env bash\necho stub\n' >"$stub/npx"
printf '#!/usr/bin/env bash\necho stub\n' >"$stub/hermes"
chmod +x "$stub"/*
export PATH="$stub:$PATH"

out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1)" || fail "full run exited non-zero: $out"

# The foreign links reached through the symlinked profile dirs must survive.
[[ -L "$OUTSIDE/skills/victim" ]] ||
  fail "an owned-shaped link was deleted THROUGH a symlinked profile dir (profiles/spec): $out"
[[ -f "$OUTSIDE/skills/keepme.txt" ]] ||
  fail "a foreign file was removed through a symlinked profile dir"
[[ -L "$OUTSIDE2/skills/victim2" ]] ||
  fail "an owned-shaped link was deleted THROUGH a symlinked skills child (profiles/spec2/skills): $out"

# The symlinked profile dirs must be reported as skipped.
grep -qi 'symlink' <<<"$out" ||
  fail "the run did not warn about skipping a symlinked profile/skills dir: $out"

# Sanity: the legitimate default-profile link is still created.
[[ -L "$HERMES/alpha" && "$(readlink "$HERMES/alpha")" == "../../.agents/skills/alpha" ]] ||
  fail "the legitimate default-profile link was not created"

echo "update-skills-symlink-safety: OK"
