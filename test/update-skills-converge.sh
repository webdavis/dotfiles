#!/usr/bin/env bash
# update-skills-converge.sh, the symlink fan-out must CONVERGE each managed dir
# to the lock's desired set, not just add missing links. The additive
# `[[ -e ]] || ln -s` left stale links behind, never fixed a wrong target, and
# crashed on a DANGLING link (`[[ -e ]]` is false for it, so `ln -s` then failed
# on the existing name). The audit found 29 store links vs 13 declared in the
# hermes default profile.
#
# Desired set (from the lock): Claude = the full store roster; each hermes
# profile = exactly its hermesProfiles entries, minus the catalog-collision
# names (humanizer, hyperframes) which hermes serves from its own catalog and
# which must NEVER be symlinked from the store. Convergence per managed dir:
#   * create a missing desired link;
#   * REPLACE an updater-owned link whose target differs (wrong-target, incl.
#     dangling);
#   * REMOVE an updater-owned link no longer desired (stale);
#   * NEVER touch a real directory (hub-owned registry dir, catalog), a
#     non-store symlink, or anything in a profile the lock does not map.
# "updater-owned" = a symlink whose literal target points under ~/.agents/skills
# (works for dangling links too, the string still points there).
#
# The real script runs unmodified in a sandbox: FORCE bypasses the idle-gate and
# the weekly stamp, offline stubs neutralize the network passes, and the FULL run
# exercises destructive convergence (replace/remove), which the additive
# --install-only bootstrap deliberately never does.
set -euo pipefail

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_COMMON_DIR

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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
CLAUDE="$HOME/.claude/skills"
HERMES="$HOME/.hermes/skills"
mkdir -p "$STORE" "$CLAUDE" "$HERMES"

# Fixture store: six real skill dirs.
for s in keeper mover revived demoted humanizer dualname; do
  mkdir -p "$STORE/$s"
  printf -- '---\nname: %s\ndescription: fixture\n---\n' "$s" >"$STORE/$s/SKILL.md"
done

# Fixture lock. humanizer is a catalog-collision name mapped to default ON
# PURPOSE, convergence must still refuse to create it hermes-side and must
# remove a stale one. dualname is hermes-OWNED (hermesProfiles [] + a
# hermesRegistry entry): hermes keeps a real hub dir of that name, untouchable.
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {
    "keeper": "core", "mover": "core", "revived": "core",
    "demoted": "core", "humanizer": "core", "dualname": "on-demand"
  },
  "hermesProfiles": {
    "keeper": ["default"],
    "mover": ["default"],
    "revived": ["default"],
    "demoted": [],
    "humanizer": ["default"],
    "dualname": []
  },
  "hermesRegistry": {
    "dualname": {"profiles": ["default"], "source": "clawhub", "identifier": "clawhub/dualname", "lockKey": "dualname"}
  },
  "npxTracked": {},
  "clawhubTracked": {},
  "forks": {}
}
EOF

# ── Pre-existing drift ─────────────────────────────────────────────────────
# Claude: one correct link (kept), one stale updater-owned link to a skill that
# left the store (removed). Every other store skill is missing (created).
ln -s "../../.agents/skills/keeper" "$CLAUDE/keeper"
ln -s "../../.agents/skills/gone" "$CLAUDE/gone" # stale: gone not in store

# Hermes default drift:
#   keeper, absent            → created (missing)
#   mover, wrong target      → replaced
#   revived, DANGLING target   → replaced (the old ln -s crashed here)
#   demoted, correct target but hermesProfiles [] → removed (stale)
#   humanizer, collision name    → removed, never re-created
#   dualname, REAL hub dir      → untouched
#   external, non-store symlink  → untouched
#   hermes-superpowers, real dir → untouched
ln -s "../../.agents/skills/WRONGTARGET" "$HERMES/mover"
ln -s "../../.agents/skills/revived-old" "$HERMES/revived" # dangling (revived-old absent)
ln -s "../../.agents/skills/demoted" "$HERMES/demoted"
ln -s "../../.agents/skills/humanizer" "$HERMES/humanizer"
mkdir -p "$HERMES/dualname"
printf -- '---\nname: dualname\ndescription: hub-owned\n---\n' >"$HERMES/dualname/SKILL.md"
ln -s "/tmp/external-target" "$HERMES/external"
mkdir -p "$HERMES/hermes-superpowers"
printf 'mirror\n' >"$HERMES/hermes-superpowers/marker"

# Destructive reconciliation (replace wrong-target links, remove stale ones)
# runs only on the FULL weekly path, never under the additive --install-only
# bootstrap, so this test exercises a FULL run. Offline stubs stand in for the
# network passes a full run would otherwise make (npx update; the hermes
# registry phase for the dualname hub entry). FORCE bypasses the idle-gate and
# the weekly stamp, so the second (idempotence) run reconverges instead of
# early-exiting on the stamp.
stub_dir="$tmp/stubs"
mkdir -p "$stub_dir"
printf '#!/usr/bin/env bash\necho stub\n' >"$stub_dir/npx"
printf '#!/usr/bin/env bash\necho stub\n' >"$stub_dir/hermes"
chmod +x "$stub_dir"/*
export PATH="$stub_dir:$PATH"

# ── RED gate: capture whether the current script even survives the dangling
#    link. It is informational; the assertions below are the contract.
run() { UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" 2>&1; }
output="$(run)" || fail "update-skills full run exited non-zero (a dangling link must not crash the fan-out): $output"

# ── Claude convergence ─────────────────────────────────────────────────────
for s in keeper mover revived demoted humanizer dualname; do
  [[ -L "$CLAUDE/$s" ]] || fail "Claude link missing for store skill: $s"
  [[ "$(readlink "$CLAUDE/$s")" == "../../.agents/skills/$s" ]] ||
    fail "Claude link for $s has wrong target: $(readlink "$CLAUDE/$s")"
done
[[ ! -e "$CLAUDE/gone" && ! -L "$CLAUDE/gone" ]] ||
  fail "stale updater-owned Claude link 'gone' was not removed"

# ── Hermes default convergence ─────────────────────────────────────────────
# created (was missing)
[[ -L "$HERMES/keeper" && "$(readlink "$HERMES/keeper")" == "../../.agents/skills/keeper" ]] ||
  fail "hermes 'keeper' link was not created with the right target"
# replaced (wrong target)
[[ -L "$HERMES/mover" && "$(readlink "$HERMES/mover")" == "../../.agents/skills/mover" ]] ||
  fail "hermes 'mover' wrong-target link was not replaced: $(readlink "$HERMES/mover" 2>/dev/null)"
# replaced (dangling) and now resolves
[[ -L "$HERMES/revived" && "$(readlink "$HERMES/revived")" == "../../.agents/skills/revived" ]] ||
  fail "hermes 'revived' dangling link was not replaced: $(readlink "$HERMES/revived" 2>/dev/null)"
[[ -e "$HERMES/revived/SKILL.md" ]] || fail "hermes 'revived' link does not resolve after convergence"
# removed (stale updater-owned, no longer desired)
[[ ! -e "$HERMES/demoted" && ! -L "$HERMES/demoted" ]] ||
  fail "stale updater-owned hermes link 'demoted' (hermesProfiles []) was not removed"
# removed (collision name), never re-created
[[ ! -e "$HERMES/humanizer" && ! -L "$HERMES/humanizer" ]] ||
  fail "collision-name hermes link 'humanizer' was not removed / was re-created"
# untouched: hub-owned real dir
[[ -d "$HERMES/dualname" && ! -L "$HERMES/dualname" ]] ||
  fail "hub-owned real dir 'dualname' was altered by convergence"
[[ -e "$HERMES/dualname/SKILL.md" ]] || fail "hub-owned 'dualname' content was disturbed"
# untouched: non-store symlink
[[ -L "$HERMES/external" && "$(readlink "$HERMES/external")" == "/tmp/external-target" ]] ||
  fail "non-store symlink 'external' was altered by convergence"
# untouched: real mirror dir
[[ -d "$HERMES/hermes-superpowers" && -e "$HERMES/hermes-superpowers/marker" ]] ||
  fail "the hermes-superpowers mirror dir was disturbed by convergence"

# ── Idempotence: a second run changes nothing and stays quiet about convergence.
second="$(run)" || fail "second --install-only run exited non-zero: $second"
if printf '%s\n' "$second" | grep -qiE 'converge: (created|replaced|removed)'; then
  fail "a no-op convergence run still logged create/replace/remove actions: $second"
fi

echo "update-skills-converge: OK"
