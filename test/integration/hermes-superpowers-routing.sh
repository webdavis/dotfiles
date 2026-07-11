#!/usr/bin/env bash
# hermes-superpowers-routing.sh — proves assert-hermes-superpowers-routing.sh, offline.
#
# The live ~/.hermes/skills/hermes-superpowers mirror is hand-patched so skills
# with hermes-native adaptations are referenced by their adaptation names, not
# superpowers:<name>. Any re-mirror stomps those patches; the transform script
# re-applies them from the lock's superpowersRouting table. The real script
# runs unmodified in a sandbox: a scratch HOME whose fixture tree is modeled on
# the REAL upstream reference shapes found in recon of the live mirror —
# colon-form refs (prose, bold related-skills lists, graphviz nodes), the
# slash-commands dispatcher with skill_view(name="...") literals and the
# {skill_name} placeholder, and frontmatter name: lines. Assertions:
#   1. Every mapped superpowers:<base> reference is rewritten to the mapped
#      adaptation name — including a pair whose adaptation name differs from
#      the bare base name, so the script provably reads the VALUE from data
#      (all five production pairs are identity-after-prefix-strip).
#   2. Non-mapped references survive byte-identical: other superpowers:* skills
#      and the superpowers:code-reviewer AGENT type (not a skill).
#   3. Frontmatter name: lines are never touched.
#   4. Idempotence: a second run leaves the tree byte-identical.
#   5. --check exits non-zero on a stale tree and names the stale file, exits 0
#      on a clean tree, and never writes.
#   6. --dry-run reports would-be rewrites and never writes.
#   7. The slash-commands dispatcher ends up carrying every adaptation-map line,
#      its mapped skill_view literals rewritten, its non-mapped literals and its
#      superpowers-{skill-name} fallback placeholder untouched.
#   8. Non-markdown files (find-polluter.sh in the real tree) are never touched.
#   9. An absent mirror dir is a graceful skip: exit 0 (fresh machine before
#      hermes setup).
#  10. The weekly wiring: an update-skills.sh run heals a stomped tree through
#      its routing re-assert pass and logs the drift loudly — deleting the
#      assert_superpowers_routing call cannot go unnoticed.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT="$REPO_ROOT/dot_local/bin/executable_assert-hermes-superpowers-routing.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f $SCRIPT ]] || fail "transform script missing: $SCRIPT"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# The script derives its lock and tree from $HOME, so swapping this one
# variable redirects its entire blast radius into the sandbox.
HOME="$tmp/home"
export HOME
TREE="$HOME/.hermes/skills/hermes-superpowers"
mkdir -p "$HOME/.agents" "$TREE"

# The lock: the five production pairs plus one pair whose adaptation name is
# NOT the bare base name — the canary that catches a transform which strips
# the superpowers- prefix instead of reading the mapped value. `anchor` is an
# npx-tracked skill kept healthy in the store below, so the roster's tracked
# union is non-empty (update-skills' zero-union gate refuses any mutation run
# otherwise); it is incidental scaffolding for step 10, never asserted on.
cat >"$HOME/.agents/custom-skill-lock.json" <<'EOF'
{
  "version": 1,
  "npxTracked": {"anchor": {"repo": "fixture/pack"}},
  "clawhubTracked": {},
  "superpowersRouting": {
    "slashCommandsSkill": "superpowers-slash-commands",
    "map": {
      "superpowers-mock-flow": "mock-flow-hermes",
      "superpowers-requesting-code-review": "requesting-code-review",
      "superpowers-subagent-driven-development": "subagent-driven-development",
      "superpowers-systematic-debugging": "systematic-debugging",
      "superpowers-test-driven-development": "test-driven-development",
      "superpowers-writing-plans": "writing-plans"
    }
  }
}
EOF
# anchor: present and healthy in the store (a real dir with a SKILL.md on a
# machine with no live generation), so step 10's --install-only finds no roster
# work and never invokes the real npx CLI.
mkdir -p "$HOME/.agents/skills/anchor"
printf -- '---\nname: anchor\ndescription: fixture\n---\n' >"$HOME/.agents/skills/anchor/SKILL.md"

# ---- fixture tree: upstream shapes, per recon of the live patched mirror ----

mkdir -p "$TREE/superpowers-writing-plans" "$TREE/superpowers-subagent-driven-development" \
  "$TREE/superpowers-systematic-debugging" "$TREE/superpowers-requesting-code-review" \
  "$TREE/superpowers-writing-skills" "$TREE/superpowers-brainstorming" \
  "$TREE/superpowers-slash-commands"

cat >"$TREE/superpowers-writing-plans/SKILL.md" <<'EOF'
---
name: superpowers-writing-plans
description: Use when you have a spec or requirements for a multi-step task, before touching code
---

# Writing Plans

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.

## Execution Handoff

- **REQUIRED SUB-SKILL:** Use superpowers:executing-plans
EOF

cat >"$TREE/superpowers-subagent-driven-development/SKILL.md" <<'EOF'
---
name: superpowers-subagent-driven-development
description: Use when executing implementation plans with independent tasks in the current session
---

# Subagent-Driven Development

```dot
digraph workflow {
    "Dispatch final code reviewer" -> "Use superpowers:finishing-a-development-branch";
}
```

## Related Skills

- **superpowers:using-git-worktrees** - REQUIRED: Set up isolated workspace before starting
- **superpowers:writing-plans** - Creates the plan this skill executes
- **superpowers:requesting-code-review** - Review dimensions for reviewer subagents
- **superpowers:test-driven-development** - Subagents follow TDD for each code-producing task
- **superpowers:finishing-a-development-branch** - Complete development after all tasks
- **superpowers:executing-plans** - Use for parallel session instead of same-session execution
EOF

cat >"$TREE/superpowers-systematic-debugging/SKILL.md" <<'EOF'
---
name: superpowers-systematic-debugging
description: Use when encountering any bug, test failure, or unexpected behavior, before proposing fixes
---

# Systematic Debugging

   - Use superpowers:test-driven-development for writing proper failing tests

## Related Skills

- **superpowers:verification-before-completion** - Verify fix worked before claiming success
EOF

cat >"$TREE/superpowers-requesting-code-review/SKILL.md" <<'EOF'
---
name: superpowers-requesting-code-review
description: Use when completing tasks, implementing major features, or before merging
---

# Requesting Code Review

Dispatch superpowers:code-reviewer subagent to catch issues before they cascade.

Use Task tool with superpowers:code-reviewer type, fill template at `code-reviewer.md`
EOF

# Support file beyond SKILL.md — the exact stale line found live in recon.
cat >"$TREE/superpowers-writing-skills/testing-skills-with-subagents.md" <<'EOF'
# Testing Skills With Subagents

**REQUIRED BACKGROUND:** You MUST understand superpowers:test-driven-development before using this skill. That skill defines the fundamental RED-GREEN-REFACTOR cycle.
EOF

# Non-markdown file: same mapped reference, must never be rewritten.
cat >"$TREE/superpowers-systematic-debugging/find-polluter.sh" <<'EOF'
#!/usr/bin/env bash
# See superpowers:test-driven-development for the RED-GREEN-REFACTOR cycle.
echo polluter
EOF

cat >"$TREE/superpowers-brainstorming/SKILL.md" <<'EOF'
---
name: superpowers-brainstorming
description: You MUST use this before any creative work
---

# Brainstorming

**The terminal state is invoking superpowers:writing-plans.**

Use superpowers:mock-flow for the mock step.
EOF

cat >"$TREE/superpowers-slash-commands/SKILL.md" <<'EOF'
---
name: superpowers-slash-commands
description: "Slash command dispatcher for Superpowers skills. Detects /superpowers-skill-name syntax and loads the appropriate skill."
---

# Superpowers Slash Commands

## Available Commands

- `/superpowers-brainstorming` - Turn ideas into fully formed designs
- `/superpowers-writing-plans` - Write detailed implementation plans
- `/superpowers-test-driven-development` - RED-GREEN-REFACTOR discipline

## Technical Implementation

1. Extract skill name from the command (e.g., `/superpowers-brainstorming` → `brainstorming`)
2. Construct full skill name: `superpowers-{skill_name}`
3. Load the skill with `skill_view(name="superpowers-{skill-name}")`

For example:
- `/superpowers-brainstorming` → `skill_view(name="superpowers-brainstorming")`
- `/superpowers-writing-plans` → `skill_view(name="superpowers-writing-plans")`
EOF

tree_hash() {
  (cd "$TREE" && find . -type f -print0 | sort -z | xargs -0 shasum -a 256) | shasum -a 256
}

# ---- run 1: the transform rewrites the stale tree -------------------------

output="$(bash "$SCRIPT" 2>&1)" || fail "transform run 1 exited non-zero: $output"

# 1) Every mapped colon-form reference now names the adaptation.
grep -q 'Use subagent-driven-development$' "$TREE/superpowers-writing-plans/SKILL.md" ||
  fail "superpowers:subagent-driven-development was not rewritten in writing-plans"
grep -qF -- '- **writing-plans** - Creates the plan this skill executes' \
  "$TREE/superpowers-subagent-driven-development/SKILL.md" ||
  fail "bold-list superpowers:writing-plans was not rewritten"
grep -qF -- '- **requesting-code-review** - Review dimensions for reviewer subagents' \
  "$TREE/superpowers-subagent-driven-development/SKILL.md" ||
  fail "bold-list superpowers:requesting-code-review was not rewritten"
grep -qF -- '- **test-driven-development** - Subagents follow TDD for each code-producing task' \
  "$TREE/superpowers-subagent-driven-development/SKILL.md" ||
  fail "bold-list superpowers:test-driven-development was not rewritten"
grep -qF 'Use test-driven-development for writing proper failing tests' \
  "$TREE/superpowers-systematic-debugging/SKILL.md" ||
  fail "superpowers:test-driven-development was not rewritten in systematic-debugging"
grep -qF 'You MUST understand test-driven-development before using this skill' \
  "$TREE/superpowers-writing-skills/testing-skills-with-subagents.md" ||
  fail "the recon stale line (support file beyond SKILL.md) was not rewritten"
grep -qF 'The terminal state is invoking writing-plans.' \
  "$TREE/superpowers-brainstorming/SKILL.md" ||
  fail "superpowers:writing-plans was not rewritten in brainstorming"

# ...including the pair whose adaptation name differs from the bare base name.
grep -qF 'Use mock-flow-hermes for the mock step.' "$TREE/superpowers-brainstorming/SKILL.md" ||
  fail "mapped value not honored: superpowers:mock-flow must become mock-flow-hermes"

# 2) Non-mapped references are untouched.
for untouched in \
  'superpowers:executing-plans' \
  'superpowers:finishing-a-development-branch' \
  'superpowers:using-git-worktrees' \
  'superpowers:verification-before-completion'; do
  grep -rq -- "$untouched" "$TREE" || fail "non-mapped reference was rewritten: $untouched"
done
count_code_reviewer="$(grep -c 'superpowers:code-reviewer' "$TREE/superpowers-requesting-code-review/SKILL.md")"
[[ $count_code_reviewer == 2 ]] ||
  fail "superpowers:code-reviewer (agent type, not a skill) must survive twice, found $count_code_reviewer"

# 3) Frontmatter name: lines are untouched.
for legacy in superpowers-writing-plans superpowers-subagent-driven-development \
  superpowers-systematic-debugging superpowers-requesting-code-review \
  superpowers-brainstorming superpowers-slash-commands; do
  grep -qxF "name: $legacy" "$TREE/$legacy/SKILL.md" ||
    fail "frontmatter name: line was modified in $legacy/SKILL.md"
done

# 7) Slash-commands dispatcher: adaptation-map lines present (one per pair),
#    mapped skill_view literal rewritten, non-mapped literal and the
#    {skill-name} fallback placeholder untouched.
slash="$TREE/superpowers-slash-commands/SKILL.md"
# shellcheck disable=SC2016  # literal markdown backticks, not command substitution
for line in \
  '- `mock-flow-hermes` replaces `/superpowers-mock-flow`' \
  '- `requesting-code-review` replaces `/superpowers-requesting-code-review`' \
  '- `subagent-driven-development` replaces `/superpowers-subagent-driven-development`' \
  '- `systematic-debugging` replaces `/superpowers-systematic-debugging`' \
  '- `test-driven-development` replaces `/superpowers-test-driven-development`' \
  '- `writing-plans` replaces `/superpowers-writing-plans`'; do
  grep -qxF -- "$line" "$slash" || fail "slash-commands adaptation map is missing: $line"
done
grep -qF 'skill_view(name="writing-plans")' "$slash" ||
  fail "mapped skill_view literal was not rewritten in slash-commands"
grep -qF 'skill_view(name="superpowers-brainstorming")' "$slash" ||
  fail "non-mapped skill_view literal was rewritten in slash-commands"
grep -qF 'skill_view(name="superpowers-{skill-name}")' "$slash" ||
  fail "the superpowers-{skill-name} fallback placeholder was rewritten"
# shellcheck disable=SC2016  # literal markdown backticks, not command substitution
grep -qF -- '`superpowers-{skill_name}`' "$slash" ||
  fail "the superpowers-{skill_name} construction instruction was rewritten"

# 8) Non-markdown files are never touched.
grep -qF 'superpowers:test-driven-development' "$TREE/superpowers-systematic-debugging/find-polluter.sh" ||
  fail "a non-markdown file was rewritten"

# 4) Idempotence: run 2 changes zero bytes.
hash_after_run1="$(tree_hash)"
bash "$SCRIPT" >/dev/null 2>&1 || fail "transform run 2 exited non-zero"
hash_after_run2="$(tree_hash)"
[[ $hash_after_run1 == "$hash_after_run2" ]] ||
  fail "not idempotent: run 2 changed the tree"

# 5) --check on the clean tree: exit 0, no writes.
bash "$SCRIPT" --check >/dev/null 2>&1 || fail "--check exited non-zero on a clean tree"

# Plant a stomp: a fresh upstream-shaped file with a stale mapped reference.
cat >"$TREE/superpowers-brainstorming/SKILL.md" <<'EOF'
---
name: superpowers-brainstorming
description: You MUST use this before any creative work
---

# Brainstorming

**The terminal state is invoking superpowers:writing-plans.**
EOF
hash_stale="$(tree_hash)"

# 5) --check on the stale tree: exit non-zero, names the stale file, no writes.
check_output="$(bash "$SCRIPT" --check 2>&1)" && fail "--check exited 0 on a stale tree"
printf '%s\n' "$check_output" | grep -q 'superpowers-brainstorming/SKILL.md' ||
  fail "--check did not name the stale file: $check_output"
[[ "$(tree_hash)" == "$hash_stale" ]] || fail "--check modified the tree"

# 6) --dry-run: reports the would-be rewrite, exits 0, no writes.
dry_output="$(bash "$SCRIPT" --dry-run 2>&1)" || fail "--dry-run exited non-zero: $dry_output"
printf '%s\n' "$dry_output" | grep -q 'superpowers-brainstorming/SKILL.md' ||
  fail "--dry-run did not report the stale file: $dry_output"
[[ "$(tree_hash)" == "$hash_stale" ]] || fail "--dry-run modified the tree"

# The fix run heals the stomp and --check goes green again.
bash "$SCRIPT" >/dev/null 2>&1 || fail "transform run over the stomped tree exited non-zero"
bash "$SCRIPT" --check >/dev/null 2>&1 || fail "--check still failing after the fix run"

# --lock-file override: point at a copy of the lock somewhere else entirely.
cp "$HOME/.agents/custom-skill-lock.json" "$tmp/alt-lock.json"
rm "$HOME/.agents/custom-skill-lock.json"
bash "$SCRIPT" --check --lock-file "$tmp/alt-lock.json" >/dev/null 2>&1 ||
  fail "--lock-file override failed --check on a clean tree"

# 10) The weekly wiring: update-skills.sh --install-only must heal a stomped
#     tree via its routing re-assert pass and log the drift LOUDLY. The routing
#     script is staged where update-skills.sh soft-gates on it.
cp "$tmp/alt-lock.json" "$HOME/.agents/custom-skill-lock.json"
mkdir -p "$HOME/.local/bin"
cp "$SCRIPT" "$HOME/.local/bin/assert-hermes-superpowers-routing.sh"
chmod +x "$HOME/.local/bin/assert-hermes-superpowers-routing.sh"
printf 'Stomped: use superpowers:writing-plans here.\n' >>"$TREE/superpowers-brainstorming/SKILL.md"
UPDATE_SKILLS="$REPO_ROOT/dot_local/bin/executable_update-skills.sh"
update_output="$(UPDATE_SKILLS_FORCE=1 bash "$UPDATE_SKILLS" --install-only 2>&1)" ||
  fail "update-skills.sh --install-only exited non-zero: $update_output"
printf '%s\n' "$update_output" | grep -q 'ROUTING DRIFT' ||
  fail "update-skills.sh did not log the routing drift loudly: $update_output"
grep -qF 'Stomped: use writing-plans here.' "$TREE/superpowers-brainstorming/SKILL.md" ||
  fail "update-skills.sh routing pass did not heal the stomped tree"
update_output="$(UPDATE_SKILLS_FORCE=1 bash "$UPDATE_SKILLS" --install-only 2>&1)" ||
  fail "second update-skills.sh --install-only exited non-zero: $update_output"
printf '%s\n' "$update_output" | grep -q 'superpowers routing: clean' ||
  fail "update-skills.sh did not report clean routing on a healed tree: $update_output"

# 9) Absent mirror dir: graceful skip, exit 0.
HOME="$tmp/fresh-home"
export HOME
mkdir -p "$HOME"
skip_output="$(bash "$SCRIPT" 2>&1)" || fail "absent mirror dir must exit 0: $skip_output"
printf '%s\n' "$skip_output" | grep -qi 'skip' || fail "absent mirror dir did not log a skip: $skip_output"

echo "hermes-superpowers-routing: OK"
