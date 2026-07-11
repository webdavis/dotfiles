#!/usr/bin/env bash
# update-skills-profile-universe.sh, two convergence-scope guarantees:
#
#   Item 6 (ownership): a link is updater-owned ONLY when its target resolves to
#   THIS user's store followed by a single skill basename. A foreign symlink
#   whose target merely CONTAINS ".agents/skills/" under some other root must
#   survive convergence untouched (the old substring match would unlink it).
#
#   Item 7 (profile universe): convergence walks every EXISTING hermes skills
#   dir (default and each profile), not just the profiles the lock still maps.
#   So a profile whose LAST mapped skill is de-mapped is still walked and its
#   stale updater-owned links are reaped, while foreign files there survive.
#
# The real script runs unmodified in a sandbox: a FULL run (offline stubs) so
# destructive convergence happens, FORCE to bypass the idle-gate.
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
SPEC="$HOME/.hermes/profiles/spec/skills"
mkdir -p "$STORE" "$HOME/.claude/skills" "$HERMES" "$SPEC"

for s in alpha beta; do
  mkdir -p "$STORE/$s"
  printf -- '---\nname: %s\ndescription: fixture\n---\n' "$s" >"$STORE/$s/SKILL.md"
done

write_lock() { # $1 = default mapping for alpha, $2 = spec mapping for beta (json arrays)
  cat >"$HOME/.agents/custom-skill-lock.json" <<EOF
{
  "version": 2,
  "tiers": {"alpha": "core", "beta": "core"},
  "hermesProfiles": {"alpha": $1, "beta": $2},
  "hermesRegistry": {},
  "npxTracked": {},
  "clawhubTracked": {},
  "forks": {}
}
EOF
}

stub="$tmp/stub"
mkdir -p "$stub"
printf '#!/usr/bin/env bash\necho stub\n' >"$stub/npx"
printf '#!/usr/bin/env bash\necho stub\n' >"$stub/hermes"
chmod +x "$stub"/*
export PATH="$stub:$PATH"

# Foreign links whose targets CONTAIN .agents/skills/ but under foreign roots,
# plus a plain foreign file. None is updater-owned; all must survive.
plant_foreign() {
  local dir="$1"
  ln -sfn "/tmp/foreign-root/.agents/skills/evil" "$dir/foreign-abs"
  ln -sfn "/Users/someone-else/.agents/skills/evil" "$dir/foreign-user"
  printf 'keep me\n' >"$dir/notes.txt"
}

# ── Part 1: item 6, default is WALKED (alpha mapped), and its foreign links
#    survive; a stale owned link in spec is reaped. ─────────────────────────
write_lock '["default"]' '["spec"]'
ln -sfn "../../.agents/skills/alpha" "$HERMES/alpha" # owned, desired
plant_foreign "$HERMES"
ln -sfn "../../../../.agents/skills/beta" "$SPEC/beta"  # owned, desired
ln -sfn "../../../../.agents/skills/gone" "$SPEC/stale" # owned, stale
plant_foreign "$SPEC"

UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >"$tmp/pu1.log" 2>&1 || fail "run 1 exited non-zero: $(cat "$tmp/pu1.log")"

for d in "$HERMES" "$SPEC"; do
  [[ -L "$d/foreign-abs" && "$(readlink "$d/foreign-abs")" == "/tmp/foreign-root/.agents/skills/evil" ]] ||
    fail "a foreign /tmp .agents/skills link in $d was unlinked (ownership false positive)"
  [[ -L "$d/foreign-user" && "$(readlink "$d/foreign-user")" == "/Users/someone-else/.agents/skills/evil" ]] ||
    fail "a foreign /Users .agents/skills link in $d was unlinked (ownership false positive)"
  [[ -f "$d/notes.txt" ]] || fail "a foreign plain file in $d was removed"
done
[[ -L "$HERMES/alpha" ]] || fail "the desired default link 'alpha' was removed"
[[ -L "$SPEC/beta" ]] || fail "the desired spec link 'beta' was removed"
[[ ! -e "$SPEC/stale" ]] || fail "a stale owned link in spec was not reaped"

# ── Part 2: item 7, de-map the sole skill of BOTH default and spec; each
#    profile is still walked (its dir exists) and its owned links are reaped,
#    foreign entries still survive. ────────────────────────────────────────
write_lock '[]' '[]'
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >"$tmp/pu2.log" 2>&1 || fail "run 2 exited non-zero: $(cat "$tmp/pu2.log")"

[[ ! -e "$HERMES/alpha" ]] ||
  fail "de-mapped default profile was not walked: owned link 'alpha' survived"
[[ ! -e "$SPEC/beta" ]] ||
  fail "de-mapped spec profile was not walked: owned link 'beta' survived"
for d in "$HERMES" "$SPEC"; do
  [[ -L "$d/foreign-abs" ]] || fail "a foreign link in $d was reaped during de-map convergence"
  [[ -L "$d/foreign-user" ]] || fail "a foreign user link in $d was reaped during de-map convergence"
  [[ -f "$d/notes.txt" ]] || fail "a foreign file in $d was removed during de-map convergence"
done

echo "update-skills-profile-universe: OK"
