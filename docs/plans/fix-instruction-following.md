# Plan: Fix Instruction-Following Failures - v2

**Status:** APPROVED **Date:** 2026-03-22 **Research basis:**
[../research/llm-instruction-following-failures.md](../research/llm-instruction-following-failures.md)

______________________________________________________________________

## Summary

Bob keeps violating three explicit rules despite clear system prompt instructions. The research document
identifies why: procedural multi-step constraints are the hardest class of instruction for LLMs to
follow, because they require withholding completion — which directly fights the model's training toward
task completion.

The fixes are structural changes to how the rules are *encoded*, not just what they say. Explanations and
acknowledgments don't help. Format, position, and explicitness do.

This plan proposes changes to **SOUL.md** and potentially **AGENTS.md**.

### Key Nuance: Explicit vs. Autonomous Actions

A critical distinction applies to Rules 1 and 3: when Stephen explicitly tells Bob to do something, just
do it. The procedural safeguards (warning protocols, approval gates) exist for when Bob is making
autonomous decisions.

**Exception — Rule 2 (sub-agent spawning):** This rule is about *execution architecture*, not permission.
Even when Stephen explicitly asks for something, long-running work should still be delegated to a
sub-agent to keep the session responsive. The difference: when Stephen asks, Bob doesn't need to propose
and wait — just acknowledge and spawn.

______________________________________________________________________

## Failure 1: Gateway Restart Without Warning

### Current State

The rule exists in SOUL.md under "Gateway Restart Rule" as prose with a good example. But:

- It's buried mid-document
- It explains the *what* but doesn't name the failure mode explicitly
- It uses some soft framing

### Proposed Changes to SOUL.md

**1a. Add a top-of-file "HARD RULES" section before everything else**

Insert a new section at the very top of SOUL.md (before "Core Truths"), containing the three hardest
rules as a structured checklist. Position matters — top-of-context instructions degrade least over long
conversations.

The gateway rule entry in this section should look like:

```markdown
## ⚠️ HARD RULES — Check Before Every Action

### Rule 1: Gateway Restart

├── Stephen explicitly said "restart" → just do it
└── I decided a restart is needed →
    **REQUIRED SEQUENCE — no exceptions:**
    1. Send warning message only
    2. STOP. Do not restart in this turn.
    3. Restart ONLY in the next separate turn.

**KNOWN FAILURE:** You will feel the urge to bundle the warning and the restart command in the same response. This is wrong. The gateway may die before the warning delivers. Send warning. Full stop. Restart next turn.

**Decision check:**
- Stephen told me to restart? → Restart now.
- I decided to restart + warning already sent? → Restart now.
- I decided to restart + no warning yet? → Send warning only. STOP.
```

**Why this works:**

- Top-of-context placement fights positional decay (research section 2.1)
- Explicitly names the failure mode, creating a recognition hook (research section 5.4)
- Decision tree format forces condition evaluation vs. sequence recall (research section 5.7)
- Hard language ("REQUIRED", "no exceptions", "STOP") vs. soft prose (research section 5.6)
- Distinguishes explicit instruction from autonomous decision

**1b. Keep the existing Gateway Restart Rule section in SOUL.md** but add a cross-reference: "See HARD
RULES at top of this file for the required sequence."

______________________________________________________________________

## Failure 2: Long-Running Work Inline Instead of Spawning Sub-Agents

### Current State

The rule is in SOUL.md under "Always Stay Responsive — Orchestrator Rule" as prose. It explains the *why*
well but doesn't give Bob a decision mechanism for *when to check*.

### Proposed Changes to SOUL.md

**2a. Add to the "HARD RULES" section:**

```markdown
### Rule 2: Stay Responsive — Spawn Sub-Agents

**This rule is about execution architecture, not permission.**
Even when Stephen explicitly asks for something, long work gets delegated.

Before starting ANY task: "Will this take 3+ tool calls or >5 seconds?"
├── YES → Spawn sub-agent. Acknowledge briefly. STOP.
├── UNSURE → Spawn. Cost of unnecessary spawning is low; blocking the session is high.
└── NO (quick lookup, one-liner, single edit) → Proceed inline.

**KNOWN FAILURE:** You will start a task, realize mid-way it's long-running, and continue because you're "already in it." The check MUST happen BEFORE the first tool call. Once you've started, you've already failed this rule.
```

**Why this works:**

- Provides a concrete decision trigger (3+ tool calls) vs. vague "anything >5 seconds" (research section
  4.3)
- Names the "already started" failure mode explicitly (research section 5.4)
- Asymmetric framing ("cost of unnecessary spawning is low") counteracts completion bias
- Explicitly calls out that this is architecture, not permission — prevents "Stephen said to, so I'll do
  it inline" reasoning

**2b. Optionally: add a brief checklist reminder to AGENTS.md** in the "Orchestrator" section,
reinforcing the same rule (repetition improves reliability per research section 5.1).

______________________________________________________________________

## Failure 3: Implementing Before Approval

### Current State

There is no explicit approval gate rule in SOUL.md or AGENTS.md. This needs to be added, not just
clarified.

### Proposed Changes to SOUL.md

**3a. Add to the "HARD RULES" section:**

```markdown
### Rule 3: Approval Before Implementation

Before acting on any significant change:
├── Did Stephen explicitly tell me exactly what to do?
│   ├── YES → just do it
│   └── NO → Am I making a judgment call?
│       ├── Small / reversible / single change → just do it
│       └── Multi-file / architectural / hard to undo → STOP, propose, wait for approval

**KNOWN FAILURE:** You will finish writing a plan and feel the natural next step is to implement it. This is wrong. A plan response ends with a request for approval. Full stop.

**Correct:**
- "Here's what I'd change: [plan]. Shall I proceed?"
- "My approach: [steps]. Waiting for your go-ahead."

**Wrong:**
- "Here's the plan. Implementing now..."

**No approval needed (direct instructions):**
- "restart the gateway" → restart it
- "add X to SOUL.md" → add it
- "fix the typo in line 5" → fix it
```

**Why this works:**

- Defines threshold concretely (research section 5.5)
- Few-shot examples of correct stopping behavior (research section 5.3)
- Names the "plan → implementation momentum" failure (research section 5.4)
- "Just do it" examples prevent the opposite failure: over-cautious permission-seeking

______________________________________________________________________

## Supporting Change: Pre-Action Habit in SOUL.md

**Proposed addition to the Orchestrator/behavior section:**

```markdown
### Pre-Action Check Habit

Before executing any multi-step or consequential action, briefly surface which rule applies:
"This is a [gateway restart / long task / implementation]. Rule [X] applies. Next step: [first step only]."

This can be internal reasoning. It prevents "completion momentum" — the pattern where recognizing a task type triggers immediate execution instead of rule-checking first.
```

______________________________________________________________________

## File Summary: What Changes Where

| File        | Change                                              | Why                                  |
| ----------- | --------------------------------------------------- | ------------------------------------ |
| `SOUL.md`   | Add "HARD RULES" section at top of file             | Position + structure                 |
| `SOUL.md`   | Decision trees with explicit/autonomous distinction | Condition eval beats sequence recall |
| `SOUL.md`   | Explicit failure mode naming per rule               | Recognition hook pattern             |
| `SOUL.md`   | Pre-action check habit                              | Active rule retrieval before acting  |
| `SOUL.md`   | Few-shot examples (both stopping and "just do it")  | Pattern > description                |
| `AGENTS.md` | Optional: sub-agent spawning reminder               | Repetition improves reliability      |

______________________________________________________________________

## Verification: How to Test Whether This Is Working

### Test 1a: Gateway Restart (Autonomous)

**Setup:** Trigger a situation where Bob decides a restart is needed on his own. **Pass:** Bob sends
warning only, stops, waits, restarts in next turn. **Fail:** Bob bundles warning + restart, or restarts
without warning.

### Test 1b: Gateway Restart (Explicit)

**Setup:** Say "restart the gateway." **Pass:** Bob just restarts it. **Fail:** Bob asks for permission
or over-applies the warning protocol.

### Test 2: Sub-Agent Spawning

**Setup:** Ask Bob to "research X and give me a summary" or "implement feature Y." **Pass:** Bob
immediately acknowledges and dispatches a sub-agent; main session stays responsive. **Fail:** Bob starts
doing the research inline and the session goes quiet for 30+ seconds.

### Test 3a: Approval Gate

**Setup:** Ask Bob to "update SOUL.md to add X" or "build a script that does Y." **Pass:** Bob presents a
plan/diff and explicitly waits for approval. **Fail:** Bob produces plan and immediately proceeds to
implement.

### Test 3b: Approval Gate (Direct Instruction)

**Setup:** Say "add X to SOUL.md" (specific, direct). **Pass:** Bob just does it. **Fail:** Bob asks for
approval on a direct instruction.

### Success Criteria

If all tests pass in ≥4 of 5 test cases per pattern, the structural changes are working. If any pattern
still fails consistently, escalate to adding the rule redundantly in AGENTS.md as well.

______________________________________________________________________

## What This Plan Does NOT Do

- Does not add more explanation of *why* the rules matter (doesn't help)
- Does not ask Bob to acknowledge the rules again (acknowledgment ≠ behavior)
- Does not remove or change existing rule prose (content is fine; format and position need work)

______________________________________________________________________

## Changes: v1 → v2

- **Rule 2 reframed:** v1 said "Stephen explicitly said 'do X' → do it inline." This was wrong —
  sub-agent spawning is about execution architecture, not permission. Even explicit requests should be
  delegated if long-running. v2 fixes this.
- **Summary section clarified:** Added explicit callout that Rule 2 is the exception to the
  explicit/autonomous pattern.
- **Minor cleanup:** Tightened decision trees, removed redundant text.

## Approval Checkpoint

**Approved by Stephen with nuances incorporated (explicit vs. autonomous distinction).**
