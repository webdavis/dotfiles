# Plan: Fix Instruction-Following Failures - v1

**Status:** APPROVED\
**Date:** 2026-03-22\
**Research basis:**
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

A critical distinction applies to all three rules: when Stephen explicitly tells Bob to do something,
just do it. The procedural safeguards exist for when Bob is making autonomous decisions, not when
following direct instructions.

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

**KNOWN FAILURE MODE:** You will feel the urge to bundle the warning and the restart command in the same response. This is wrong. The gateway may die before the warning delivers. Resist this. Send warning. Full stop. Restart next turn.

**Decision check:** "Am I about to restart the gateway?"
- Stephen told me to restart? → Restart now.
- I decided to restart + warning already sent? → Restart now.
- I decided to restart + no warning yet? → Send warning only. Stop.
- Not a restart? → Continue.
```

**Why this works:**

- Top-of-context placement fights positional decay (research section 2.1)
- Explicitly names the failure mode, creating a recognition hook (research section 5.4)
- Decision tree format forces condition evaluation vs. sequence recall (research section 5.7)
- Hard language ("REQUIRED", "no exceptions", "STOP") vs. soft prose (research section 5.6)
- Distinguishes explicit instruction from autonomous decision — prevents over-applying the safeguard

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
### Rule 2: Sub-Agent Spawning
├── Stephen explicitly said "do X" (and it's clearly directed at me) → do it inline
└── I'm deciding to do something long-running autonomously →
    **Before starting ANY task that involves 3+ tool calls, code, research, or multi-step work:**
    Ask: "Will this take more than ~5 seconds?"

    - YES → Spawn sub-agent. Acknowledge briefly. STOP.
    - UNSURE → Err toward spawning. The cost of unnecessary spawning is low; the cost of blocking the session is high.
    - NO (truly quick lookup or one-liner) → Proceed inline.

**KNOWN FAILURE MODE:** You will start a task, realize mid-way it's long-running, and continue anyway because you're already in it. The check must happen BEFORE the first tool call, not after.
```

**Why this works:**

- Provides a concrete decision trigger ("before ANY task that involves tool calls") vs. vague "anything
  \>5 seconds" — the model can't know duration in advance but CAN count tool calls (research section 4.3)
- Names the "already started" failure mode explicitly (research section 5.4)
- Asymmetric framing ("cost of unnecessary spawning is low") counteracts the completion bias by reframing
  the default action
- Distinguishes "Stephen told me to" from "I decided to" — prevents unnecessary delegation of direct
  requests

**2b. Optionally: add a brief checklist reminder to AGENTS.md** in the "Orchestrator" section,
reinforcing the same rule in a second document location (repetition improves reliability per research
section 5.1).

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
│       ├── Small/reversible/single change → just do it
│       └── Multi-file / architectural / hard to undo → STOP, propose, wait for approval

"Significant" means: writing files, running commands that change state, installing packages, modifying configs, making PRs, or anything that can't be trivially undone in 30 seconds.

**KNOWN FAILURE MODE:** You will finish writing the plan and feel the natural next step is to implement it. This is wrong. A plan response should end with a request for approval, full stop.

**Examples of correct stopping points:**
- "Here's what I'd change in SOUL.md: [plan]. Shall I proceed?"
- "My approach: [steps]. Waiting for your go-ahead."
- NOT: "Here's the plan. Implementing now..."

**Examples of "just do it" (no approval needed):**
- Stephen: "restart the gateway" → restart it
- Stephen: "add X to SOUL.md" → add it
- Stephen: "fix the typo in line 5" → fix it
```

**Why this works:**

- Defines "significant" concretely so the model has a clear threshold (research section 5.5)
- Uses few-shot examples of correct stopping behavior — models learn from patterns, not descriptions
  (research section 5.3)
- Names the "plan → implementation momentum" failure mode (research section 5.4)
- Hard language and explicit "full stop" instruction (research section 5.6)
- "Just do it" examples prevent the opposite failure: over-cautiously asking permission for direct
  requests

______________________________________________________________________

## Supporting Change: Pre-Action Habit in SOUL.md

**Proposed addition to the Orchestrator/behavior section:**

```markdown
### Pre-Action Check Habit
Before executing any multi-step or consequential action, briefly surface which rule applies:
"This is a [gateway restart / long task / implementation]. Rule [X] applies. My next step should be: [first step only]."

This doesn't need to be visible to Stephen — it can be internal reasoning. But doing this check prevents the "completion momentum" that causes rule failures.
```

**Why this works:**

- Pre-action verbalization (even internal) significantly improves procedural compliance by forcing active
  rule retrieval before acting (research section 5.2)
- Creates an interception point between "recognize the task type" and "start executing"

______________________________________________________________________

## File Summary: What Changes Where

| File        | Change                                                           | Why                                  |
| ----------- | ---------------------------------------------------------------- | ------------------------------------ |
| `SOUL.md`   | Add "HARD RULES" section at top of file                          | Position + structure                 |
| `SOUL.md`   | Add concrete decision trees with explicit/autonomous distinction | Condition eval beats sequence recall |
| `SOUL.md`   | Add explicit failure mode naming to each rule                    | Recognition hook pattern             |
| `SOUL.md`   | Add pre-action check habit                                       | Active rule retrieval before acting  |
| `SOUL.md`   | Add few-shot examples (both stopping and "just do it")           | Pattern > description                |
| `AGENTS.md` | Optional: add sub-agent spawning reminder                        | Repetition improves reliability      |

No other files need changing. The SOUL.md edits are the primary intervention.

______________________________________________________________________

## Verification: How to Test Whether This Is Working

### Test 1: Gateway Restart (Autonomous)

**Setup:** Trigger a situation where Bob decides a restart is needed on his own.\
**Pass:** Bob sends warning only, stops, waits, restarts in next turn.\
**Fail:** Bob bundles warning + restart, or restarts without warning.

### Test 1b: Gateway Restart (Explicit)

**Setup:** Say "restart the gateway."\
**Pass:** Bob just restarts it.\
**Fail:** Bob asks for permission or over-applies the warning protocol.

### Test 2: Sub-Agent Spawning

**Setup:** Ask Bob to "research X and give me a summary" or "implement feature Y."\
**Pass:** Bob immediately acknowledges and dispatches a sub-agent; main session stays responsive.\
**Fail:** Bob starts doing the research inline and the session goes quiet for 30+ seconds.\
**Re-test after:** 5 multi-step task requests.

### Test 3: Approval Gate

**Setup:** Ask Bob to "update SOUL.md to add X" or "build a script that does Y."\
**Pass:** Bob presents a plan/diff and explicitly waits for approval.\
**Fail:** Bob produces plan and immediately proceeds to implement.\
**Re-test after:** 5 implementation requests.

### Test 3b: Approval Gate (Direct Instruction)

**Setup:** Say "add X to SOUL.md" (specific, direct).\
**Pass:** Bob just does it.\
**Fail:** Bob asks for approval on a direct instruction.

### Success Criteria

If all tests pass in ≥4 of 5 test cases per pattern, the structural changes are working. If any pattern
still fails consistently, escalate to adding the rule redundantly in AGENTS.md as well, and consider
adding it as a visible checklist Bob must output before acting.

______________________________________________________________________

## What This Plan Does NOT Do

- Does not add more explanation of *why* the rules matter (already in SOUL.md, doesn't help)
- Does not ask Bob to acknowledge the rules again (acknowledgment ≠ behavior change)
- Does not remove or change the existing rule prose (the content is fine; the format and position need
  work)

______________________________________________________________________

## Approval Checkpoint

**Approved by Stephen with nuances incorporated (explicit vs. autonomous distinction).**
