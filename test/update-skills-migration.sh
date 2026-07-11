#!/usr/bin/env bash
# update-skills-migration.sh proves the flat-store -> generation migration
# (Wave 3a fix4 brief "Migration"). A machine with the old flat store
# (~/.agents/skills/<name> real dirs + a real ~/.agents/.skill-lock.json) is
# migrated: every tracked store entry becomes a stable symlink into a real
# .skills-current generation whose content is a clone of the legacy dirs, and
# the lock becomes a symlink. Asserted:
#   1. .skills-current is built from the legacy real dirs (content preserved).
#   2. Each tracked store entry is now a symlink into the generation.
#   3. Vendored real dirs and app-owned symlinks (NOT tracked) are untouched.
#   4. The .skill-lock.json symlink resolves to the original lock content.
#   5. Per-entry atomicity: every store name resolves (never dangling) and no
#      *.migrating.* / *.garbage.* leftovers survive.
#   6. Idempotence: a second migrate run is a no-op (content byte-identical, no
#      dangling links, no new generations).
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
GMV_BIN="${UPDATE_SKILLS_GMV:-/opt/homebrew/bin/gmv}"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.agents/skills"
# alpha/beta npx-tracked, claw clawhub-tracked; vendored + app not tracked.
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "npxTracked": {"alpha": {"repo": "x/a"}, "beta": {"repo": "x/b"}},
  "clawhubTracked": {"claw": {"slug": "@o/claw", "registry": "https://c.example"}}
}
EOF

export UPDATE_SKILLS_GMV="$GMV_BIN"
export UPDATE_SKILLS_LIB_ONLY=1
# shellcheck disable=SC1090
source "$SCRIPT"

# Build the flat legacy store.
seed_skill() {
  local name="$1"
  mkdir -p "$STORE/$name"
  printf -- '---\nname: %s\n---\n# %s\n' "$name" "$name" >"$STORE/$name/SKILL.md"
  printf 'LEGACY-%s' "$name" >"$STORE/$name/content.txt"
}
seed_skill alpha
seed_skill beta
seed_skill claw
# A vendored real dir NOT in the tracked tables.
mkdir -p "$STORE/vendored"
printf 'VENDORED' >"$STORE/vendored/content.txt"
# An app-owned symlink NOT in the tracked tables.
mkdir -p "$tmp/app/appskill"
printf 'APP' >"$tmp/app/appskill/content.txt"
ln -s "$tmp/app/appskill" "$STORE/appskill"
# The legacy real npx lock.
printf '{"legacy":true}\n' >"$SKILL_LOCK_LINK"

[[ ! -e $SKILLS_CURRENT ]] || fail "precondition: .skills-current should not exist yet"
__gen_migration_needed || fail "migration should be reported as needed on a flat store"

# Migrate.
__gen_migrate || fail "migration returned non-zero"

# 1) .skills-current built from the legacy dirs, content preserved.
__gen_is_complete "$SKILLS_CURRENT" || fail "migration did not produce a complete .skills-current"
for name in alpha beta claw; do
  [[ -f "$SKILLS_CURRENT/skills/$name/content.txt" ]] ||
    fail "generation is missing $name content"
  [[ "$(cat "$SKILLS_CURRENT/skills/$name/content.txt")" == "LEGACY-$name" ]] ||
    fail "generation $name content was not the legacy clone"
done

# 2) Each tracked store entry is now a correct symlink into the generation.
for name in alpha beta claw; do
  [[ -L "$STORE/$name" ]] || fail "store/$name is not a symlink after migration"
  [[ "$(readlink "$STORE/$name")" == "../.skills-current/skills/$name" ]] ||
    fail "store/$name points at the wrong target: $(readlink "$STORE/$name")"
  [[ "$(cat "$STORE/$name/content.txt")" == "LEGACY-$name" ]] ||
    fail "store/$name does not resolve to the legacy content"
done

# 3) Vendored real dir and app-owned symlink untouched.
[[ -d "$STORE/vendored" && ! -L "$STORE/vendored" ]] ||
  fail "vendored real dir was converted (must stay outside the generation)"
[[ "$(cat "$STORE/vendored/content.txt")" == "VENDORED" ]] || fail "vendored content changed"
[[ -L "$STORE/appskill" ]] || fail "app-owned symlink was replaced"
[[ "$(readlink "$STORE/appskill")" == "$tmp/app/appskill" ]] ||
  fail "app-owned symlink target changed"

# 4) The lock is now a symlink resolving to the original content.
[[ -L $SKILL_LOCK_LINK ]] || fail ".skill-lock.json is not a symlink after migration"
[[ "$(readlink "$SKILL_LOCK_LINK")" == ".skills-current/.skill-lock.json" ]] ||
  fail ".skill-lock.json symlink points at the wrong target"
grep -q '"legacy":true' "$SKILL_LOCK_LINK" ||
  fail "the lock symlink does not resolve to the original legacy lock content"

# 5) Per-entry atomicity: every store name resolves; no in-flight leftovers.
for entry in "$STORE"/*; do
  name="${entry##*/}"
  [[ -e $entry ]] || fail "store/$name is dangling after migration (per-entry atomicity broken)"
done
shopt -s nullglob
leftovers=("$STORE"/.*.migrating.* "$STORE"/*.garbage.* "$AGENTS"/.*.migrating.*)
shopt -u nullglob
[[ ${#leftovers[@]} -eq 0 ]] ||
  fail "migration left in-flight leftovers: ${leftovers[*]}"

# 6) Idempotence.
before="$(find "$STORE" "$SKILLS_CURRENT" -exec stat -f '%N %m %Z' {} \; | sort)"
__gen_migration_needed && fail "migration still reported as needed after a successful migrate"
__gen_migrate || fail "second migrate run returned non-zero"
after="$(find "$STORE" "$SKILLS_CURRENT" -exec stat -f '%N %m %Z' {} \; | sort)"
[[ $before == "$after" ]] || fail "second migrate run mutated the tree (not idempotent)"
for name in alpha beta claw; do
  [[ "$(readlink "$STORE/$name")" == "../.skills-current/skills/$name" ]] ||
    fail "store/$name drifted after the idempotent second run"
done

echo "update-skills-migration: OK"
