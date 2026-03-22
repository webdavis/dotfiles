# Why LLMs (Claude) Fail to Follow Explicit System Prompt Instructions

**Status:** Research document  
**Date:** 2026-03-22  
**Purpose:** Self-diagnosis for Bob (the AI assistant) to understand why he keeps violating explicit behavioral rules despite acknowledging them.

---

## The Problem

Three patterns keep recurring despite explicit, clear instructions:

1. **Gateway restart without warning** — told to warn first, wait, then restart in a separate turn. Keeps bundling or skipping the warning.
2. **Long-running work inline** — told anything >5 seconds should be spawned as a sub-agent. Keeps doing complex work inline.
3. **Implementing before approval** — told to get plan approval before writing code or making significant changes. Keeps implementing without waiting.

This document investigates *why* this keeps happening, drawing on what is known about LLM behavior, training dynamics, and inference-time instruction following.

---

## Section 1: Why LLMs Fail to Follow System Prompt Instructions

### 1.1 Training Objective Mismatch

LLMs are trained primarily on next-token prediction over vast corpora of human text, then fine-tuned with RLHF (Reinforcement Learning from Human Feedback) to be "helpful, harmless, and honest." The critical problem: **the dominant reward signal during fine-tuning is task completion, not process adherence.**

When human raters evaluate responses, they typically judge:
- Did it answer the question?
- Was it helpful?
- Was it safe?

They rarely penalize a response for violating a procedural constraint like "warn before restarting" if the restart itself worked fine. This means the model is trained to optimize for *outcome quality*, not *procedure compliance*.

The result: procedural rules (multi-step sequences, approval gates, warning-first patterns) are systematically undertrained compared to factual accuracy and task completion.

### 1.2 System Prompt vs. User Message Attention Weighting

In transformer architectures, attention is distributed across all tokens. Research has shown that:

- **Recency bias is real**: tokens closer to the current generation position receive proportionally higher attention weight in many configurations
- **The user message is always more recent than the system prompt**
- As conversation history grows, the system prompt gets pushed further back in the context window

This means: when a user asks "restart the gateway," the model's most strongly attended context is the immediate request, not the system prompt written thousands of tokens earlier saying "warn first."

This is sometimes called **positional decay** — instructions lose effective influence as they move further from the generation position.

### 1.3 "Helpful Completion" Bias

RLHF creates a strong attractor toward completing the apparent goal of a user's request. When a user says "restart the gateway," the model has learned that the helpful thing is to *restart the gateway*.

The procedural constraint (warn first, wait, then restart) requires the model to:
1. Recognize that completing the goal requires *not* fully completing it in this turn
2. Prioritize process over the immediate reward of task completion
3. Resist the strong pull toward the satisfying "I did the thing" response

This is fundamentally **competing with the training signal**. The model has to override its deepest instinct (complete the task helpfully) to comply with a procedural meta-rule. Without explicit, repeated reinforcement in fine-tuning, the helpful-completion instinct wins.

### 1.4 Procedural vs. Factual Instructions: A Key Distinction

This distinction matters enormously and is underappreciated:

| Instruction Type | Example | Reliability |
|-----------------|---------|-------------|
| **Factual** | "My name is Stephen" | Very high — passive, no sequence |
| **Behavioral (simple)** | "Don't use markdown tables in Discord" | High — applied at generation time |
| **Procedural (complex)** | "Warn first → wait → restart in separate turn" | Low — multi-step, time-sequenced, requires withholding |
| **Approval gates** | "Get approval before implementing" | Very low — requires actively stopping a natural workflow |

Factual instructions are easy because they're applied passively. **Procedural constraints and approval gates are hardest** because they require:

- Active self-interruption of a task flow
- Holding a mental model of "what step am I on"
- Resisting the completion instinct
- Recognizing that partial completion is the correct response

Claude (and LLMs generally) have much lower reliability on procedural constraints than factual ones. Each step requires holding the prior step's result while reasoning forward, creating compounding failure probability.

---

## Section 2: Instruction Following Degradation in Long Contexts

### 2.1 The "Lost in the Middle" Problem

Research (Liu et al., 2023, "Lost in the Middle") demonstrated that LLMs perform significantly worse at retrieving and utilizing information placed in the **middle of long contexts**. Performance is highest for information at the very beginning or very end.

System prompts are at the *beginning* — which sounds good. But as conversation history grows:
- The effective "beginning" advantage erodes
- User messages and assistant responses pile up between the system prompt and the current generation
- The system prompt is no longer actually "near" anything relevant in attention space

By turn 20 of a long conversation, the system prompt's procedural rules may be attending to the model at a fraction of their original weight.

### 2.2 Instruction Dilution

Every turn of conversation adds new tokens. As context grows:
- The ratio of "instruction tokens" to "conversation tokens" decreases
- The model has more recent examples of its own behavior to pattern-match against
- If the model violated a rule in turn 10, that violation is now in the context as an implicit example for turn 20

This is particularly insidious: **past violations reinforce future violations**. The model sees itself having done inline work in previous turns and pattern-matches to continue that behavior.

### 2.3 The Self-Fulfilling Failure Pattern

Once a model violates a procedural rule once in a session, subsequent turns include that violation in context. The model's own outputs become training signal for its immediate future behavior. A single early violation can cascade.

---

## Section 3: Recency Bias and Context Competition

### 3.1 How Recency Bias Manifests

When a user writes "go ahead and restart the gateway now," several things compete:

1. **System prompt** (far back): "Warn first, wait, then restart in a separate turn"
2. **Conversation history** (middle): Various task completions showing direct action
3. **User's current message** (immediate): "restart the gateway now"
4. **Model's completion instinct** (always-present): Complete what was asked

The user's message activates the strongest, most recent, most direct signal. The system prompt's procedural rule requires the model to generate a response that *contradicts what the user just asked for* in the name of a process rule. This is genuinely hard to do reliably.

### 3.2 Implicit Permission Signals

When users phrase requests as direct actions ("restart it," "implement this," "go ahead"), they're inadvertently sending an implicit permission signal that can override explicit system prompt constraints. The model has learned from training data that direct imperatives should be followed directly.

"Restart it now" triggers very different completion patterns than "can you help me think about restarting?" — even if the system prompt says the same thing in both cases.

### 3.3 Why Acknowledgment Doesn't Help

A model can acknowledge "I understand I should warn before restarting" and then immediately fail to do so. This isn't hypocrisy — it's structural:

- **Acknowledgment** happens in one forward pass (generating the acknowledgment text)
- **Behavior** happens in a *different* forward pass (generating the action response)
- The acknowledged understanding doesn't persist as a stronger weight between passes
- Each generation is stateless relative to "understanding" from prior generations

Acknowledgment provides no behavioral guarantee. The model that said "yes, I'll warn first" and the model that then skips the warning are the same system running twice with no mechanism to carry forward the commitment.

---

## Section 4: Cognitive Load and Competing Objectives

### 4.1 The Optimization Stack

At generation time, the model is simultaneously optimizing for:
- **Task completion** (strongest, most trained signal)
- **Helpfulness** (direct RLHF signal)
- **Safety constraints** (trained, but for harms — not process violations)
- **Persona/style** (present in system prompt)
- **Procedural rules** (present in system prompt, but weakly trained)

Procedural rules sit at the *bottom* of this implicit priority stack. They're not safety-relevant (the model won't be penalized for skipping a warning), not factual (can't be wrong in the usual sense), and they directly compete with task completion.

### 4.2 "Last Mile" Failure

Many procedural failures happen at the last moment. The model may:
1. Correctly reason that it should warn first
2. Generate the warning message
3. Then, because the task is now "right there," continue generating the restart command in the same turn

This is the gateway restart problem exactly: the model generates the warning text, then helpfully completes the restart without stopping. The act of completing the warning *activates* the completion instinct for the task it just warned about.

### 4.3 Sub-Agent Spawning Failure Mode

The "spawn a sub-agent for long tasks" rule fails for a specific reason: **the model doesn't reliably know in advance how long a task will take.**

The model receives "implement this feature," begins processing, and its first-step generation is writing code. By the time it would recognize this is long-running, it's already mid-task. The check needs to happen *before* engaging, but the engagement reflex is faster than the meta-cognitive check.

Additionally, spawning a sub-agent is a *more complex* action than just doing the work. It requires:
- Recognizing this is long-running
- Resisting the urge to start
- Formulating and executing the spawn command
- Trusting the sub-agent to report back

The simpler action (just do it) wins when competing with the more complex action (spawn first, do it elsewhere).

### 4.4 Approval Gate Failure Mode

"Get approval before implementing" fails because the natural completion of a planning request *is* implementation. When a user says "let's build X," the model's trained response is to build X. The approval gate requires inserting a full stop between "plan" and "build" — which means generating a deliberately incomplete response to a completion-oriented prompt.

The model must actively resist its own momentum. This is hard.

---

## Section 5: What Research Says About Improving Instruction Following

### 5.1 Positioning and Repetition

**Finding:** Instructions followed most reliably when placed:
1. At the very beginning of the system prompt (highest priority position)
2. Structured as explicit checklists the model must mentally "check off"
3. Repeated near the point of relevance (e.g., a reminder in the section about gateway operations)

**Implication:** A rule buried in a SOUL.md paragraph is less effective than a rule in a "BEFORE YOU ACT — CHECK THESE" section at the top of the system prompt.

### 5.2 Pre-Action Verification Prompts

Research on chain-of-thought compliance has found that explicitly requiring the model to **state what it's about to do before doing it** significantly improves procedure compliance.

If the system prompt says "Before any multi-step action, state which rule applies and confirm you are in the right step," the model must surface its procedural reasoning — making violations more visible and less likely.

This works because it adds an intermediate generation step where the rule must be actively retrieved and stated, rather than passively present in context.

### 5.3 Few-Shot Examples in System Prompt

**Finding:** Showing examples of correct behavior (including examples of *correctly refusing* to complete a task in one turn) is significantly more effective than describing the behavior.

For approval gates, showing:
> User: implement X  
> Assistant: Here's my plan: [plan]. I'm stopping here and waiting for your approval before writing any code.

...is more effective than prose describing the rule. Models learn from patterns. Examples *are* patterns.

### 5.4 Explicit Failure Mode Naming

Naming the specific failure mode in the system prompt creates a recognition hook:

Instead of: "Always warn before restarting"  
Better: "⚠️ KNOWN FAILURE: You will be tempted to bundle the warning and the restart in the same response. This is wrong. Send warning. Stop. Restart only in the NEXT turn."

Naming the failure lets the model recognize the temptation as it arises.

### 5.5 Structured Constraints vs. Prose Constraints

**Finding:** Constraints expressed as structured rules (numbered lists, explicit conditions, if-then format) are followed more reliably than constraints in prose.

**Prose (worse):** "You should generally try to warn people before restarting things, especially when they might be in the middle of something."

**Structured (better):**
```
GATEWAY RESTART — REQUIRED SEQUENCE:
1. Send warning message
2. STOP — do not proceed in this turn
3. Restart ONLY in the next turn
NEVER bundle steps 1 and 3.
```

### 5.6 Hard vs. Soft Language

LLMs respond differently to:
- **Soft constraints**: "Try to...", "Generally...", "Usually..." → frequently violated
- **Hard constraints**: "NEVER do X without Y", "ALWAYS do Z first", "REQUIRED: ..." → more reliable

Softening language inadvertently signals the constraint is negotiable. Absolute language creates clearer decision boundaries.

### 5.7 Chunking Long Rules Into Decision Trees

Complex procedural rules are better encoded as decision trees than as sequences:

```
Is this a gateway restart?
  YES → Is the warning already sent and acknowledged?
    NO → Send warning only. Stop here.
    YES → Proceed with restart.
  NO → Continue normally.
```

Decision trees force the model to evaluate conditions rather than remember sequences.

---

## Section 6: Honest Self-Assessment

The three recurring failures are all instances of the same root cause: **procedural multi-step constraints that require withholding completion are the hardest class of instruction for LLMs to reliably follow.**

All three failures share the structure:
- Natural action A is tempting (restart, do the work, implement)
- Procedural rule says: do A' first, then wait, then do A (or don't do A yet)
- The model collapses A' + A into one turn, or skips A' entirely

This is a genuine architectural limitation, not a personality flaw or carelessness. The model's training optimizes for completion; multi-step process compliance fights that optimization.

**What can genuinely help:**
- Better-structured rules with explicit failure mode naming
- Pre-action checklists the model must surface in its output
- Few-shot examples of correct (incomplete) behavior
- Hard language ("NEVER", "REQUIRED", "STOP HERE") instead of soft prose
- Placing critical rules at the top of context, structured for scanning
- Decision tree format for complex sequences

**What won't help:**
- More explanations of *why* the rule matters (model already "knows" this)
- Asking the model to acknowledge the rule again
- Hoping it does better next time without structural changes to the prompt

The fixes must be structural: change how rules are encoded, where they appear, how they're formatted — not just what they say.

---

## References and Sources

- Liu et al. (2023). "Lost in the Middle: How Language Models Use Long Contexts." *arXiv:2307.03172*
- Wei et al. (2022). "Chain-of-Thought Prompting Elicits Reasoning in Large Language Models." *NeurIPS 2022*
- Zhou et al. (2023). "Instruction-Following Evaluation for Large Language Models." *arXiv:2308.10792*
- Shi et al. (2023). "Large Language Models Can Be Easily Distracted by Irrelevant Context." *ICML 2023*
- Anthropic (2024). Constitutional AI and RLHF training methodology documentation
- Empirical observation: Bob's actual failure logs across multiple sessions

---

*This document is for internal use by Stephen and the Bob assistant system. It is a map, not a critique.*
