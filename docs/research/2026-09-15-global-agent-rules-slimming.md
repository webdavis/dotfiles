# Global agent rules: what leaves, what stays

`.chezmoitemplates/global-agent-rules.md` renders into both `~/.claude/CLAUDE.md` and
`~/.codex/AGENTS.md` and loads into every agent turn on this machine. This is a proposal only: nothing in
the partial changed. The operator rules on each section below, one at a time, per the ledger's pass
method.

## The keep test

For every section: **would an agent that never read this text make a mistake on an ORDINARY turn**, one
that does not specifically touch the section's subject? If yes, KEEP. If the mistake only happens on a
turn that already needs the subject (a GitButler repo, a backup, a recap), the rule can live behind that
subject's own skill or command instead of in front of every turn. Rules that prevent a destructive or
irreversible action (data loss, a force-push, an unsafe git worktree convention that hides work from the
sidebar) KEEP regardless of length, because the ordinary-turn test does not apply to a rule whose whole
job is catching the rare turn that goes wrong.

## Measurement

Rendered both targets 2026-09-15:

```
CI=1 chezmoi --source "$PWD" execute-template --no-tty < private_dot_claude/CLAUDE.md.tmpl
CI=1 chezmoi --source "$PWD" execute-template --no-tty < private_dot_codex/AGENTS.md.tmpl
```

| Target                | Rendered lines | Chars  | Approx tokens |
| --------------------- | -------------- | ------ | ------------- |
| `~/.claude/CLAUDE.md` | 286            | 14,290 | ~3,572        |
| `~/.codex/AGENTS.md`  | 301            | 14,925 | ~3,731        |
| shared partial alone  | 265            | 13,133 | ~3,283        |

No tokenizer was installed in the worktree, so the token count is `chars / 4`, a standard rough estimate
for English prose; call it approximate. The partial is the bulk of both files (roughly 94-97% of their
content); the per-target difference is each file's own short harness section.

Per-section share of the 265-line partial (source lines, header comment excluded):

| Section                        | Lines | Share |
| ------------------------------ | ----: | ----: |
| Collaboration style            |    18 |    7% |
| Verification and sources       |    12 |    5% |
| Verification before completion |    10 |    4% |
| Destructive action gates       |    11 |    4% |
| Code discipline                |    10 |    4% |
| Shell scripts                  |     8 |    3% |
| Git commits                    |     9 |    3% |
| Git worktrees                  |    20 |    8% |
| GitButler                      |    24 |    9% |
| Work recaps                    |    71 |   27% |
| Pull request descriptions      |     7 |    3% |
| Task tracking                  |     8 |    3% |
| Tool preferences               |    13 |    5% |
| Open Neovim buffers            |    16 |    6% |
| Backups                        |     7 |    3% |
| Toolchain                      |    11 |    4% |
| Agents                         |     5 |    2% |

Work recaps alone is over a quarter of the file.

## Section verdicts

**Collaboration style** (18 lines) - KEEP. Shapes tone, commit granularity, acronym handling and the
em-dash ban on every single turn; missing it degrades every reply, not just a subject-specific one.

**Verification and sources of truth** (12 lines) - KEEP. The local-then-docs-then-search-then-
training-data ladder applies to any factual claim on any turn.

**Verification before completion** (10 lines) - KEEP. "Should work is not works" is the standing rule
against premature done-claims across every kind of task.

**Destructive action gates** (11 lines) - KEEP. Textbook case for the destructive-action exemption:
short, and every line exists to stop an irreversible command.

**Code discipline** (10 lines) - KEEP. YAGNI, no dead code, no third-party forks apply to nearly every
code-touching turn.

**Shell scripts** (8 lines) - MOVE, agrees with the ledger. Only bites on a turn that writes a shell
script. Destination: new skill `bash-style` (`~/.agents/skills/bash-style/`), since no existing skill
covers bash conventions. Stub:

```
## Shell scripts

Style rules (set -euo pipefail, array usage, avoided patterns) live in the `bash-style` skill;
invoke it before writing or editing a `.sh` file.
```

**Git commits** (9 lines) - KEEP. Short already, and the no-AI-trailer rule fired from a real incident
(an unwanted attribution line in a commit is visible externally and hard to walk back cleanly). Keeping
it in front of every turn costs 9 lines against a real recurring mistake.

**Git worktrees** (20 lines) - MOVE, agrees with the ledger, but the destination is the existing `herdr`
skill, not a new note: `~/.agents/skills/herdr/SKILL.md` already documents worktree commands (it has its
own worktree section). Fold this content in there rather than duplicating a second home. Stub:

```
## Git worktrees

Worktree creation and teardown mechanics (herdr vs plain `git worktree add`, when to register one
a harness already made, removal after merge) live in the `herdr` skill. Read it before creating,
opening, or removing a worktree.
```

**GitButler** (24 lines) - MOVE, disagrees with the ledger's silence on it (the ledger only names five
candidates; this is an obvious sixth). The `gitbutler` skill already exists and already carries command
recipes; this section's collaboration rules (don't touch another agent's commits, one branch per session)
belong there too, since the whole section is conditional on a repository that does not exist on this
machine yet. Stub:

```
## GitButler

Where `but status` succeeds, `but` is the version-control interface; everywhere else keep using
`git`. Full rules (never run `but setup` unprompted, one branch per session, amend vs fixup) live
in the `gitbutler` skill.
```

**Work recaps** (71 lines) - MOVE, agrees with the ledger; this is the single biggest win in the file.
The `pns-work-recap` skill already exists (triggers on `/pns:work-recap` or "recap"). Fold the exact
layout, the trigger list, and the delivery mechanics into it; a recap is never needed on an ordinary
turn. Stub:

```
## Work recaps

Format, triggers, and delivery (`pns recap git`, `pns recap agent --stdin`) live in the
`pns-work-recap` skill. Invoke it whenever a recap is due; never use Claude Code's own `/recap`.
```

**Pull request descriptions** (7 lines) - KEEP, unchanged. The ledger asked to reduce this to a pointer;
it already is one, at 7 lines pointing at `~/.claude/commands/pr.md`. No further cut available without
losing the pointer itself.

**Task tracking** (8 lines) - KEEP. Short, and the before/during/after discipline is a standing habit
meant to run on every non-trivial turn, not a subject-specific lookup; moving it into `todoist-cli` risks
the skill being invoked only when task syntax is already the question, dropping the habit itself.

**Tool preferences** (13 lines) - PARTIAL MOVE. Keep the one-line MCP-vs-CLI principle (applies whenever
any tool choice comes up); move the three per-tool preference bullets into the skills they name, since
those skills already exist and already claim the "use me for X" territory in their own descriptions:
`gh-axi` skill, `chrome-devtools-axi` skill, and the `home-assistant` / `home-assistant-best-practices`
pair. Stub:

```
## Tool preferences

Prefer local CLI tools over MCP servers when both work; MCP only for SaaS APIs without a
first-class CLI. Per-tool preferences (gh-axi over gh, chrome-devtools-axi over other browser
automation, the Home Assistant skill pairing) live in each named skill.
```

**Open Neovim buffers** (16 lines) - KEEP. A disk write against an open, unsaved buffer loses the
operator's side of the edit, the same class of harm as the destructive-action gates even though it is not
phrased as one.

**Backups** (7 lines) - KEEP, disagrees with the ledger. Already a 7-line stub-sized rule; a moved stub
would cost roughly the same 3-4 lines for near-zero net savings, and a new skill for one naming
convention is its own overhead (a `tiers` row, a `claudeDelivery` row, a symlink). Not worth a pass.

**Toolchain** (11 lines) - KEEP. Short list, prevents repeated unwanted migration suggestions (`mise`,
`fd`, `rg`); cheap to keep. **Agents** (5 lines) - KEEP, already minimal.

## Projected after-state

- **Every MOVE recommendation accepted** (Shell scripts, Git worktrees, GitButler, Work recaps, Tool
  preferences partial): partial drops from 265 to roughly **148 source lines**, about 56%.
- **Conservative, only the ledger's five named candidates** (Work recaps, Git worktrees, Pull request
  descriptions unchanged since already a stub, Shell scripts, Backups moved per the ledger's original
  wording though this document argues against it): drops to roughly **172 source lines**, about 65%.

Both are estimates from the stub text sketched above; actual counts will differ slightly once written.

## Ordered pass list

One section per pass, in file order. KEEP sections need no pass and are grouped below for completeness so
no section is silently skipped.

1. Collaboration style, Verification and sources, Verification before completion, Destructive action
   gates, Code discipline - KEEP, no action.
1. **Shell scripts - MOVE.** Create `~/.agents/skills/bash-style/SKILL.md` (new vendored skill: the full
   "Adding a skill" recipe in `docs/runbooks/agent-skills-store.md`). Touches: the new skill directory, a
   `tiers` row and `skillOverrides` entry in `dot_agents/custom-skill-lock.json`, a `hermesProfiles` row,
   a Claude symlink under `private_dot_claude/skills/bash-style`, and the partial (shrink to stub).
1. Git commits - KEEP, no action.
1. **Git worktrees - MOVE.** Fold content into the existing `~/.agents/skills/herdr/SKILL.md`. Touches:
   that file, and the partial (shrink to stub). No lock changes; `herdr` is already registered.
1. **GitButler - MOVE.** Fold content into the existing `~/.agents/skills/gitbutler/SKILL.md`. Touches:
   that file, and the partial (shrink to stub). No lock changes; already registered.
1. **Work recaps - MOVE.** Fold the layout, trigger list, and delivery mechanics into the existing
   `~/.agents/skills/pns-work-recap/SKILL.md`. Touches: that file, and the partial (shrink to stub). No
   lock changes.
1. Pull request descriptions, Task tracking - KEEP, no action.
1. **Tool preferences - PARTIAL MOVE.** Edit `~/.agents/skills/gh-axi/SKILL.md`,
   `~/.agents/skills/chrome-devtools-axi/SKILL.md`, and `~/.agents/skills/home-assistant/SKILL.md` to
   each state its own preference-over-alternatives. Touches: those three files, and the partial (trim to
   the one-line principle plus a pointer).
1. Open Neovim buffers, Backups (disagreement above, operator may override), Toolchain, Agents - KEEP.

Re-render both targets after every accepted pass and record the new line count before the next pass.
