#!/usr/bin/env bash
# update-skills-dryrun-additive.sh, two convergence guarantees the audit found
# violated:
#   * --dry-run makes ZERO filesystem writes. The old convergence still ran
#     mkdir/ln/rm under --dry-run, so a "preview" mutated live links. A dry run
#     must leave link state byte-identical.
#   * --install-only is truly ADDITIVE. It may create a missing store install
#     and create an absent symlink, but it must NEVER replace a wrong-target
#     link or remove a stale one. Destructive reconciliation belongs only to the
#     full weekly path behind the idle gate. A wrong-target link under
#     --install-only is left untouched and a loud warning is logged.
#
# The real script runs unmodified in a sandbox. Fixture store = three real skill
# dirs; the Claude fan-out desired set is the full store roster. Pre-existing
# Claude drift covers all three convergence actions:
#   keep, correct link            → kept
#   wrongt, wrong-target owned link → full run replaces; dry/install-only leave
#   miss, absent                  → created
#   ghost, stale owned link        → full run removes; dry/install-only leave
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
CLAUDE="$HOME/.claude/skills"
mkdir -p "$STORE" "$CLAUDE" "$HOME/.hermes/skills"

# keep/wrongt/miss are vendored Claude fan-out fixtures; anchor is an
# npx-tracked skill so the roster's tracked union is non-empty (the zero-union
# gate refuses any full/install-only run otherwise). anchor is not asserted on.
for s in keep wrongt miss anchor; do
  mkdir -p "$STORE/$s"
  printf -- '---\nname: %s\ndescription: fixture\n---\n' "$s" >"$STORE/$s/SKILL.md"
done
printf '{"skills":{"anchor":{}}}\n' >"$HOME/.agents/.skill-lock.json"

# npx stub: the anchor's generation build never calls the real CLI.
stub="$tmp/stub"
mkdir -p "$stub"
printf '#!/usr/bin/env bash\necho stub\n' >"$stub/npx"
chmod +x "$stub/npx"
export PATH="$stub:$PATH"

cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 2,
  "tiers": {"keep": "core", "wrongt": "core", "miss": "core", "anchor": "core"},
  "hermesProfiles": {"keep": [], "wrongt": [], "miss": []},
  "hermesRegistry": {},
  "npxTracked": {"anchor": {"repo": "fixture/pack"}},
  "clawhubTracked": {},
  "forks": {}
}
EOF

# Pre-existing Claude drift.
ln -s "../../.agents/skills/keep" "$CLAUDE/keep"
ln -s "../../.agents/skills/WRONG" "$CLAUDE/wrongt" # owned, wrong target
ln -s "../../.agents/skills/ghost" "$CLAUDE/ghost"  # owned, stale (not in store)

snapshot() {
  local d="$1" p
  {
    for p in "$d"/*; do
      [[ -e $p || -L $p ]] || continue
      if [[ -L $p ]]; then
        printf '%s -> %s\n' "${p##*/}" "$(readlink "$p")"
      else
        printf '%s [real]\n' "${p##*/}"
      fi
    done
  } | sort
}

# ── --dry-run: byte-identical link state, but it must REPORT every action. ───
before="$(snapshot "$CLAUDE")"
dry_out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --dry-run 2>&1)" || fail "--dry-run exited non-zero: $dry_out"
after="$(snapshot "$CLAUDE")"
[[ $before == "$after" ]] ||
  fail "--dry-run mutated Claude link state:
--- before ---
$before
--- after ---
$after"
printf '%s\n' "$dry_out" | grep -qi 'would create.*miss' ||
  fail "--dry-run did not report the missing link it would create: $dry_out"
printf '%s\n' "$dry_out" | grep -qi 'would replace.*wrongt' ||
  fail "--dry-run did not report the wrong-target link it would replace: $dry_out"
printf '%s\n' "$dry_out" | grep -qi 'would remove.*ghost' ||
  fail "--dry-run did not report the stale link it would remove: $dry_out"

# ── --install-only (additive): create missing, LEAVE wrong-target (+ warn),
#    LEAVE stale. ──────────────────────────────────────────────────────────
io_out="$(UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" --install-only 2>&1)" || fail "--install-only exited non-zero: $io_out"
# created the missing link
[[ -L "$CLAUDE/miss" && "$(readlink "$CLAUDE/miss")" == "../../.agents/skills/miss" ]] ||
  fail "--install-only did not create the absent link 'miss'"
# LEFT the wrong-target link untouched
[[ -L "$CLAUDE/wrongt" && "$(readlink "$CLAUDE/wrongt")" == "../../.agents/skills/WRONG" ]] ||
  fail "--install-only replaced a wrong-target link (must be additive-only): $(readlink "$CLAUDE/wrongt" 2>/dev/null)"
# and logged a loud warning about it
printf '%s\n' "$io_out" | grep -i 'warn' | grep -q 'wrongt' ||
  fail "--install-only did not log a loud warning about the wrong-target link it left: $io_out"
# LEFT the stale link (no removal under additive mode)
[[ -L "$CLAUDE/ghost" ]] ||
  fail "--install-only removed a stale link (must be additive-only, never remove)"

# ── full run (FORCE, no --install-only): destructive reconciliation runs. ────
UPDATE_SKILLS_FORCE=1 bash "$SCRIPT" >/dev/null 2>&1 || fail "full run exited non-zero"
[[ -L "$CLAUDE/wrongt" && "$(readlink "$CLAUDE/wrongt")" == "../../.agents/skills/wrongt" ]] ||
  fail "full run did not repair the wrong-target link"
[[ ! -e "$CLAUDE/ghost" && ! -L "$CLAUDE/ghost" ]] ||
  fail "full run did not remove the stale link"

echo "update-skills-dryrun-additive: OK"
