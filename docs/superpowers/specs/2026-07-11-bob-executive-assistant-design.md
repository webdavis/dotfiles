# Bob executive-assistant design (forzare — how Bob runs the day)

Date: 2026-07-11

> **Provenance (ported 2026-07-11).** This document was ported from
> `~/Documents/ADHD_Task_System_Research_20260521/Phase2_Surfacing_Engine_Spec.md` into the dotfiles
> superpowers specs on 2026-07-11 and is now the **canonical copy** — the user's **executive-assistant
> design** (originally titled "Phase 2 — Surfacing Engine Spec"). The research-folder source is retained
> as a read-only pointer (do not edit it). The 2026-07-03/04 conductor rulings (R1–R8) — adjudicated from
> code-verification evidence banked against the installed hermes-agent at
> `~/.hermes/hermes-agent` — are folded into §8b, §10, §12, §14, §15, §16, and §19; each touched passage
> is marked inline with the ruling id. Companion implementation plan:
> `docs/superpowers/plans/2026-07-11-bob-executive-assistant.md`.

> **The role and the system.** **Bob is the user's executive assistant** — schedule-manager, task-manager,
> ADHD (attention-deficit/hyperactivity disorder) manager — running as the **DEFAULT hermes-agent profile**
> (not a dedicated one). **forzare is the operating system for that role:** the ADHD task-management system
> this spec defines (Bob is the agent; forzare is the system — the name "forzare" is used for the system
> throughout). Owned-layer files live in `~/workspaces/Ivy/forzare/` (goals yardstick, dopamine menu,
> calibration state).

**Prepared:** 2026-05-22 · **Revised:** 2026-05-29 (mapped onto verified Hermes-Agent primitives) ·
**Status:** draft for review · **Ported + hardened:** 2026-07-11 (R1–R8)
**Premise:** Bob (Hermes) is the **boss of the user's schedule** — the executive assistant owns and drives
the day, not a passive "what now?" responder. Grounded in just-in-time adaptive intervention (JITAI) + the
Round-1 ADHD findings.

**Reading guide:** **Part I (§0–§8) = behavior** — the *what/why*; stable, the design contract. **Part II
(§9–§14) = implementation on Hermes primitives** — the *how*. **Part III (§15–§17) = reliability/ops.**
**Part IV (§18–§19) = forward path + open checks.**

**Verification note (2026-05-29).** Part II–III mechanics were checked against the live Hermes docs (cron /
kanban / plugins / skills / built-in-plugins / tools / configuration / webhooks / curator pages).
Config-key and CLI specifics are doc-derived — **⚠ verify exact keys + indentation against the live docs
before pasting into `config.yaml`.** Genuinely undocumented items are marked **OPEN** in §19 rather than
asserted.

---

# PART I — BEHAVIORAL DESIGN (the contract)

## 0. The one rule

At every decision point, **match task-side attributes to person-side state, and surface exactly ONE thing
— or nothing.**

**Precisely (W12/R5A1):** a response carries **at most one DO-NOW action _or_ one requested decision** — never
a list, never two asks. The **one bounded exception is the daily brief** (§2): it delivers read-only *planning
context* (weather / calendar / the ≤3) and still **closes on exactly ONE thing**. That one thing is the **head
item of a single unified decision queue** (`forzare/state/decision-queue.json`, §8a) when the queue is
non-empty — the queued decision then **replaces** the brief's do-now closing action (a requested decision and
a do-now action never both appear) — or the do-now action when the queue is empty. Because **every**
brief-time decision source (`@waiting` chases, missed-fixed re-decisions, stall decisions, capture-triage
re-raises, the monthly someday-sweep) funnels through that one **head-item-only** queue (§2 step 4 / §4c), the
one-thing count is satisfiable on **any** morning. Acceptance is mechanical — **count the actionable
imperatives + questions in a response**: everything but the brief must total ≤ 1; the brief's context lines
are not actionable, and it too closes on a single action-or-decision.

- **Task-side** (Todoist): `@deep`/`@light`/`@admin`, `duration`, `deadline`, `priority` (p1 = today's
  ≤3), `@waiting` (never surfaced as do-now), `@errand`; active = has a due date.
- **Person-side** (inferred, not measured): time-of-day, calendar gaps/load, activation state, location.
- **Provide-nothing is a first-class option** (JITAI receptivity — intervene less, but clearly).

**The backlog (~2,270 tasks) stays completely out of view.** Bob surfaces a single next action; the user
never sees the list. This is the load-bearing invariant — §9 firewalls Bob's *own* internal work-queue
(Kanban) away from the user for the same reason.

**Two kinds of failure — never conflated (load-bearing).** *Your* failure and *the system's* failure are
categorically different events, and Bob handles them in opposite ways. This separation is foundational —
every failure path in this spec derives from it.

| | **You fail a task / fall off schedule** | **The system fails** |
|---|---|---|
| **What it is** | didn't start it, rolled it, fell behind, missed a window | a pipeline stage crashes, a dependency dies, a job can't complete, the gateway is down |
| **What it means** | **normal** — the exact condition forzare exists to help with | **a software fault — forzare *itself* is broken** |
| **How Bob responds** | **gently, no shame:** provide-nothing-clearly, re-shape (decompose / if-then), never a guilt-wall, never scorekept | **loud and immediately:** the errors channel (`#forzare-errors`), never quieted, never receptivity-gated |
| **Where** | task channel (§4d / §6a / §7) | errors channel (`#forzare-errors`) (§9 / §16) |

(`#forzare-errors` is a **forzare-layer convention** — the channel id lives in `DISCORD_ERRORS_CHANNEL` and is
read by forzare's own out-of-band **ops watchdog** and deliver-strings, not by any hermes-core config key;
§12.4/§14.) **Detection latency — BEST-EFFORT ≈5 min (W8/X9), not a hard bound:** the watchdog's
`StartInterval` is DECIDED at 300s (§14), so its *polling target* is ≈5 min **while the host is awake and the
prior pass isn't still running** (launchd `StartInterval` semantics — missed intervals during sleep run at
wake, and a long pass isn't re-entered); a system failure's *delivery* is then retried from the durable spool
under a Discord outage. So **detection is best-effort 5-minute, never a guaranteed ceiling.** A native cron
failure summary may *additionally* land on the job's own channel as system-voiced text (never user-shame, §16).

Bob must **never** (a) dress a system failure up as *your* fault, (b) hide a system failure to "protect"
you, or (c) treat your ordinary task-slippage as a system alarm. Mislabeling either way breaks forzare: a
shamed user disengages (§6a "what-the-hell effect"), and a silently-swallowed outage means the externalized
prospective memory just vanished — the one thing worse than a visible degraded mode (Part III). The
delivery mechanism that enforces this split is the **two-channel invariant** (§9).

---

## 1. Decision points (when Bob acts)

1. **Morning brief** (cron, `15 5 * * *` — **fires every day**, Denver TZ; content is schedule-derived, §2)
2. **Transitions** (cron block-boundaries + the user's manual signals)
3. **On-demand** ("what now?")
4. **Sparse proactive nudges** (only when receptive)

All four paths funnel through **one delivery gate + the procedural single-writer discipline** (§12, R4) so
two paths can never double-fire "your next thing."

## 1a. Command surface — everything namespaced under `forzare-*`

**One namespace, discovery-by-prefix.** Every user-facing command is prefixed `forzare-` (except bare
`/forzare`). ADHD rationale: the user remembers **one** word — the system's name — and typing `/forzare`
autocompletes the *entire* command surface, so it's recognition-from-a-menu, not recall. The coined name
also makes the namespace **collision-proof** in Hermes' large shared skill list (nothing else namespaces
under `forzare`). Frequency-optimized: the most-used action (reporting state) is the *shortest* command
(bare `/forzare`); rarer discrete actions carry a `-suffix`. **All also work in plain language** (Hermes
description-driven activation, §3B) — the slash form is the power-user accelerant, not a requirement. *(The
`/forzare-*` handles are skills/bundles, not plugin commands (R5, §12); native `/forzare-*` slash-command
autocomplete depends on the mirroring caveat in §12/§19.)*

**The rule:** bare `/forzare` = *report my state* (the consolidated classifier); `/forzare-<action>` = *do
a specific thing*.

| Command | Tier | What it does |
|---|---|---|
| **`/forzare <free text>`** | daily hot path | Report a real-world state change — gym / work-shift / energy / location (§3B). Consolidated classifier; also auto-fires on phrases. **Stays bare** (most frequent → shortest). |
| **`/forzare-next`** | daily | Surface the next ONE thing within the current plan (§4). Also plain "what now?". |
| **`/forzare-today`** | occasional | The day's ≤3 + what's left (the `Today's 3` view, §4b). |
| **`/forzare-capture <thing>`** | occasional | Brain-dump → Todoist Inbox (staging), non-interrupting; the §8b pipeline places/researches it in the background. |
| **`/forzare-replan`** | occasional | **Re-orient/redraw the plan now, with no state change to report** — distinct from `/forzare <state>` (input-driven re-orient) and `/forzare-next` (one task within the existing plan). For "a block blew up, redraw the rest of the day" or re-triggering an ignored boundary prompt (§3, §5). *(Named `replan`, not `transition` — §3 "transition" is reserved for state-change-driven re-orientation, the opposite of this.)* |
| **`/forzare-morning-brief`** | occasional / recovery | Re-pull the brief (skimmed-and-lost), **or** manually run it after an outage — which also triggers the §8 defensive roll (missed-fire recovery), so it recovers morning *state*, not just the text. |
| **`/forzare-eod`** | occasional / recovery | Manually run end-of-day (normally auto-fires 23:00, §8). Idempotent — safe to run late or twice. |

**Cron** fires the brief / transitions / end-of-day **proactively** (§10/§11); **Kanban** runs the
background capture-processing pipeline (§8b). The manual commands above are for on-demand pulls, recaps,
and outage recovery, not daily use. Suffixes abbreviated where typed often (`-eod`, `-next`); autocomplete
shows each command's description regardless.

---

## 2. Morning brief — schedule-driven

The brief and the peak-window logic both read a **`work_schedule`** (skill config, §13): per-weekday work
blocks + an alternating-Sunday rule (anchor date). Everything derives from two computed values for *today*:
**is it a work day, and what is the work block?** So "current job," "off day," and a future job are just
different schedule *values* — not different code paths. There is **one** mechanism; the old "pre-job vs
post-job mode" split is replaced by this.

**Current schedule (2026-05):** work **Tue / Thu / Sat 15:00–23:00**, **plus alternating Sundays** (anchor:
**Sun Jun 7 = ON** → May 31 OFF, Jun 14 OFF, Jun 21 ON, …). Off days = Mon / Wed / Fri + the OFF Sundays.

**Overnight shifts (23:00–07:00) — planned and unplanned, no special machinery.** An overnight is just
another `work_schedule` block that happens to cross midnight; the schedule logic already handles it (a
known overnight → the following morning reads as a **sleep/recovery window**, not a deep-work peak; no
gym-activation nag if sleep displaces it). **The wrinkle: pick-ups.** The user may take an overnight
last-minute (a colleague calls out), so `work_schedule` can be *stale in real time*. This is handled by the
**manual-signal path (§3), not by pre-planning** — a recognized phrase like *"picked up a shift" / "at
work"* (added to the §3/§12 phrase set) **overrides the schedule for that block**: Bob treats the user as
working (no surfacing into the shift; provide-nothing), and the next morning as recovery. **The override is
persisted, not session-held:** the `/forzare` shift signal writes `forzare/state/schedule-override.json`
(block + date + recovery-morning flag, §8a) — the 5:15 brief and 23:00 end-of-day are fresh amnesiac cron
sessions and can only honor "next morning = recovery" by reading that file. The 5:15 brief firing *mid*-shift
reads it and **still FIRES — delivering the recovery/provide-nothing *variant* of the brief, not silence**
(content shaped to "you're mid-shift, rest after," never absent — §2's always-fires rule below) — **without
clearing** the override; the recovery flag is consumed by the first brief/engagement on the calendar day
*after* the block ends, then normal schedule resumes. No
forecasting, no calendar gymnastics — the same adaptation mechanism that handles "back from gym" handles
"called in for a night shift."

**Brief order — short, scannable, ends with ONE action:**

1. **Weather / outdoor-window block** — flags only on a trigger, for the day's relevant outdoor window
   (bike-to-gym; the work-commute window on work days):
   - **wind > 17 mph · any rain · < 50°F · > 90°F** → actionable prep ("Rain at 6am — fenders/jacket";
     "38° ride — layers").
   - **Quiet when clear:** "Clear ride both ways." Source: Open-Meteo / NWS (keyless). Thresholds = config
     (§13).
2. **Today's calendar** (fixed anchors from Google Calendar) — incl. the work block as fixed load on work
   days.
3. **The day's ≤3 must-dos** (`p1`) — chosen by §4c, fit into today's *free* windows.
4. **The single head decision (§0/R5A1 — the unified queue's ONE item).** Every deferred *decision*, from any
   source, is enqueued as a typed record in ONE owned-layer queue, **`forzare/state/decision-queue.json`**
   (§8a), by the amnesiac state runs that **never message**: the 02:00 reconcile enqueues **`waiting-chase`**
   records (any `@waiting` past its check-back date; most-overdue ordered first) plus its black-hole /
   staleness repairs; the EOD roll enqueues **`fixed-redecision`** records (the just-closed day's missed fixed
   items — the just-closed day is *today* for the on-time 23:00 fire, *yesterday* for a defensive-morning fire
   (CEILING, §8) —
   user-dated/timed/recurring, non-ledger and roll-excluded, §8; a *Bob lead-time* date on a deadline task is
   ledger-owned and rolls instead) and **`stall-decision`** records (any ledger task at `roll_count ≥ 2` that
   EOD marked but did not message, §7/§8/R4A10); the §8b capture pipeline enqueues **`triage-reraise`** records
   (a card awaiting a placement answer); the monthly someday-sweep enqueues **`sweep-candidate`** records and,
   past its stale threshold, one **`bankruptcy-offer`** record (§4c/X7); the **morning `eisenhower-plan`**
   enqueues a **`q1-conflict`** record when >3 real deadlines collide today (§4c step 6 — never a silent drop);
   and **EOD** enqueues a **`stale-p1`** record for any *user-set* `p1` older than 48h (§4c/§8/AA2 — flagged
   once, never auto-cleared). **Each record carries the ONE canonical schema (DD4 — defined once in plan Task B0,
   restated here identically): `{id, class, task_id|candidate_id|aggregate-key, proposed, status, enqueue_ts, gen,
   rev, head, journal_ref, answer?}`, `status ∈ {pending, tombstoned}` (no bare `acked` flag — ack TOMBSTONES,
   recording the user's `answer` on the retained record, KK3, below)** — **`id` is a
   STABLE, content-INDEPENDENT key, NOT hashed over `proposed`/content (AA4/BB2)**, keyed by the decision's natural
   identity per class: a **per-task** class keys
   on the task = `class + ":" + task_id/candidate_id` (`waiting-chase`, `fixed-redecision`, `stale-p1`,
   `stall-decision`, `triage-reraise`, `sweep-candidate`); an **aggregate** class that has no single task keys on
   its natural period = **`q1-conflict:<collision-date>`** (the day whose deadlines collide) and
   **`bankruptcy-offer:<YYYY-MM>`** (the sweep month) — so the SAME decision keeps ONE identity across
   re-touches. A producer that re-enqueues an already-present decision is a **no-op**; one that re-evaluates it to
   a *different* `proposed` **updates the existing record IN PLACE (same `id`) and increments `rev`** (a
   content-derived id would instead spawn a duplicate — the bug this fixes). **`gen`/`rev` contract (BB2):** a
   record starts `gen = 1`, `rev = 1`; every in-place `proposed`/content change or a promotion (below) increments
   `rev`; the producer's re-touch **retires any obsolete revision** so only the current record survives. **Ack
   TOMBSTONES the record IN PLACE (BB2/GG5 — supersedes the bare `acked` flag):** the ack flips the record's
   `status` to `tombstoned` in place — the full record is retained, `gen` unchanged, **no separate tombstone
   object** (one id = one record); **the ack RECORDS the user's answer on the tombstoned record in an `answer`
   field (KK3/JJ3 — e.g. `keep` / `drop` / `chase` / the settled decision).** A later **re-enqueue of a
   tombstoned `id`** is a *new occurrence* — it
   **reuses the same record**, resetting it to **`status = pending`, `gen + 1`, `rev = 1`, `head = false`, and a fresh `enqueue_ts`** (the `rev` counter resets with each new generation; the rollover clears the old promotion flag + re-stamps the occurrence clock, II6) — so a chase answered
   today, then genuinely overdue again next week, re-asks under `gen 2` rather than being suppressed forever by a
   stale ack. **PRODUCER ONCE-GUARD (KK3/JJ3 — the generalized re-ask rule):** a producer must **NOT re-enqueue a
   tombstoned `id` whose predicate STATE is unchanged**. Concretely, when a producer would re-enqueue an id that
   already has a tombstoned record, it re-opens a new generation **only if the predicate INPUT changed** — a
   genuinely new episode (e.g. for `stale-p1`: the user's `p1` was removed and then RE-SET, a fresh staleness
   episode); if the predicate is unchanged **and** the tombstone's recorded `answer` was **`keep`** (the user
   deliberately kept it as-is), the re-enqueue is a **NO-OP** — a stale `p1` the user chose to keep, or an
   unchanged sweep candidate already answered `keep`, is **flagged ONCE and never re-asked** until its predicate
   state actually changes. This is why `stale-p1` "flagged once, never auto-cleared" (§4c/§8) is a stable
   guarantee: the once-guard, not a per-day suppression list, prevents the nightly re-ask. **Total order = `(head DESC, class-rank, enqueue_ts, id)`** — **the `head` flag is the PRIMARY sort
   key, so promotion PARTICIPATES in the order (BB2)** rather than living in a side "head slot": a genuinely
   time-sensitive chase is promoted by setting `head = true` under the lock (`rev++`), which sorts it to the
   order's minimum without adding a second item — class-rank
   **`q1-conflict > waiting-chase > fixed-redecision = stale-p1 > stall-decision > triage-reraise >
   sweep-candidate > bankruptcy-offer`** (AA4/R6A10 — a same-day capacity `q1-conflict` outranks all;
   `stale-p1` ties with `fixed-redecision`; `bankruptcy-offer` is lowest), FIFO by `enqueue_ts` within a
   class-rank, `id` breaking exact ties. The brief **delivers EXACTLY the single HEAD `pending` record** (the
   order's minimum) as its one requested decision — never a list, never a second ask. Because that decision is a *requested decision*, it **replaces** the brief's do-now closing
   action (§0/W12: a decision and a do-now never both appear); on the user's answer the **live turn that
   receives it tombstones the head via compare-and-set on `{id, gen, rev}` of the record it actually showed** — if
   `gen`/`rev` moved (a producer re-touched or a new generation superseded it) the CAS fails and the turn re-reads
   rather than tombstoning a stale head (R5A5) — and the **next** record surfaces the following morning.
   **Generalized ack (CC10):** ANY record resolved *intra-day* — not only the shown brief head (e.g. a
   `stall-decision` the user settles mid-day via `todoist-surface`'s stall branch, §7) — is tombstoned by that
   live turn through the SAME CAS, so the next brief never re-asks a decision already made. **Every queue mutation
   runs under the SAME lock + atomic-replace contract as the lifecycle map/journal** (§8a's I/O contract, extended
   to `decision-queue.json`).
5. **Activation reminder:** *"Breakfast first, then gym"* (non-negotiable; the fragile morning is a JITAI
   **vulnerability state** — reinforce the routine, don't load deep work onto a collapsing morning).
6. **Closes on the one thing** — the head decision from step 4 when the queue is non-empty (which *is* the
   close), else a single do-now action — *"First: eat, then ride."* (§0/R5A1: decision-or-action, never both.)

**Response-structure rule — exactly ONE actionable line, MACHINE-READABLE by a schema marker (Z12/BB10).** The
brief's ONE actionable line — the queue-head decision *or* the do-now close — is emitted with a leading **`▶ `
schema marker**, and **no other line ever carries it**; the context lines are *rendered* non-actionable. So the
count is a mechanical `▶ `-marker count == 1 on **every** morning, in **both** queue states:

- **Queue non-empty:** the **queue-head decision (step 4) is the sole `▶ ` line.** The activation reminder
  renders as **non-actionable context** (*"Routine: breakfast → gym"*, a statement, not a command) and the
  weather block as **information** (*"Rain at 6am — fenders"*, phrased as a fact, not an instruction).
- **Queue empty:** the **closing do-now action is the sole `▶ ` line**, and the activation reminder **may BE
  that line** (*"▶ First: eat, then ride."*). Weather stays informational.

**Context lines carry a leading `· ` (GG12).** Every non-actionable line — weather, calendar, the ≤3, the
activation reminder when it is not the close — is emitted as a `· `-prefixed context line, so the gate can
distinguish context from a stray second imperative structurally: **any non-empty line OTHER THAN the single
`▶ ` line — whether it falls BEFORE or AFTER the marker — must start with `· `.** A bare imperative sentence, a
bullet, or a numbered step on EITHER side of the marker is therefore caught, not just verb-matched.

So the acceptance (§0) is mechanical and unambiguous: **count `▶ ` markers == 1 AND zero non-`· ` non-empty
lines OTHER THAN the marker line — checked BOTH PRE- and POST-marker (II7 parity with the plan's INV-B4-4) —
for BOTH the queue-empty and the queue-nonempty brief** (a build fixture asserts exactly
this; plan B4). The imperative/question verb regex stays only as **secondary evidence** — the `▶ ` marker count
+ the `· `-only-on-both-sides-of-the-marker check are the primary, machine-readable gate (BB10/GG12).

**The brief always fires at its time — receptivity shapes its *content*, never withholds it.** This includes
the mid-shift and post-overnight-recovery cases (§2 above): the brief still **delivers**, in its
recovery/provide-nothing *variant* (a shaped "rest, nothing to start now" message), rather than going absent
— a silent morning is indistinguishable from a dead Bob (Part III). A rough stretch (dense dismissals → low
receptivity) may lighten the ≤3 and drop optional blocks, but the daily anchor itself is exempt from the §6a
receptivity gate — a bad week is exactly when the clean reset matters
most. (The gate governs intra-day surfacing and nudges, §4/§6.) Fixed daily lines also **vary phrasing by
construction** (§7's rotation rule) so the anchor doesn't habituate to wallpaper.

**Peak / free windows derive from the schedule (the load-bearing rule):**

- **Work day** (currently Tue/Thu/Sat + ON Sundays, 15:00–23:00): free window = **morning → early
  afternoon** (post-gym until commute). **Deep-work peak = morning/midday.** Evening is work → **no personal
  `@deep` surfacing**; only quick `@admin`/`@errand` for any gap.
- **Off day**: morning (post-gym) **and** evening are both available; deep work can land in the evening.
- **Future post-job job:** set the schedule to that job's hours (e.g. ~09:00–17:00 + commute) → the same
  logic moves the personal peak to evenings. Flip the values on the job start date; no rewrite.

---

## 3. Transition system (combination)

**(A) Cron = time backbone (for things that recur on a clock).** Hermes cron fires the morning brief and
planned block-boundary prompts, runs a skill/script, delivers to Discord. See §10/§11 for what cron *kicks
off* vs what **Kanban** carries. **Cron timezone is configured** by the root-level `timezone` key (§15) —
the prior "TZ unverified" flag is **resolved**.

**(B) Manual signals = adaptation — ONE skill: `/forzare`.** The user reports a real-world state change and
Bob re-orients the day around it. This is a **single skill** (`forzare`, the system's namesake) that
internally **classifies** the report into a signal and dispatches — *not* one command per signal. ADHD
rationale: making the user recall `/gym-back` vs `/work-start` is exactly the working-memory load the system
exists to remove (and the choice-overload it avoids). One memorable entry point — the name of their own
system — covers everything.

- **Two input surfaces, one skill:**
  - **Manual:** `/forzare <whatever's going on>` — e.g. `/forzare back from the gym`, `/forzare picked up a
    shift`, `/forzare fried`. The only command to remember.
  - **Auto (voice-texting):** the skill's `description` is packed with the activation phrases, so Hermes'
    description-driven loading (verified §-below) auto-fires it on natural text — "back from the gym," "at
    work," "I'm wiped" — without the `/`.
- **The skill classifies into the signal set** (it owns these branches; the user never sees them):
  **activation** (leaving-for / back-from gym), **work/shift** (clock-in incl. picked-up overnight /
  clock-out — overrides a stale `work_schedule` and **persists the override to
  `forzare/state/schedule-override.json`** so cron sessions see it, §2/§8a), **energy** (fried / locked-in →
  force low / peak state), **location** (heading-out → `@errand`s). Then it drives the transition (reshuffle
  + surface), e.g. gym-back → activated → start the deep block now; clock-in → provide-nothing until shift
  ends.
- **Over-trigger guards:** the description scopes activation to *"the user reporting a change in their own
  availability / energy / location **right now**"* (not any passing mention of work/gym), and **on
  low-confidence classification Bob confirms in one line** ("sounds like you're starting a shift — yes?")
  before applying. State changes are cheap to reverse, so this errs toward acting. **When a live Discord-bound
  conversation exists, that one-line confirm is a native Discord button** (the clarify tool, §12 R1c) —
  one-tap yes/no; otherwise it's a plain one-line question.

**Activation reliability (verified 2026-06-27, Hermes skills docs).** Hermes surfaces every skill's
`name`+`description` to the model via `skills_list()` and loads full content with `skill_view(name)` "when
the agent decides it needs that skill" — i.e. description-driven, model-judged activation **does** exist
(it's relevance-judgment over the description, not regex). So the phrases-in-description approach works
natively — **no `pre_gateway_dispatch` phrase-matching hook is needed for this** (removed from §12). The
`/forzare` command is the deterministic backstop if the model ever misses a phrase.

**(C) Webhooks = optional later automation.** A phone Shortcut/geofence POSTs the same signals to Hermes'
inbound webhook (`platforms.webhook`, §12) so the user needn't text. Start manual.

**Morning timeline:** wake ~5:15 → breakfast → bike ~20m → gym → bike ~20m back → *"back from gym"* → deep
block. **Gym Mon/Tue/Wed/Fri/Sat/Sun; Thursday = gym-rest day** (no gym signal expected; don't nag for an
activation that isn't coming). **Note Thursday is a double-constrained day:** gym-rest *and* a work day
(15:00–23:00) → no morning activation signal *and* no evening deep window; treat it as a light/admin day.
(Gym schedule is config, §13, and is independent of the `work_schedule`.)

**Time-anchored backstop for the missing gym signal (don't hang the day on prospective memory).**
Remembering to *report* "back from gym" is itself a prospective-memory demand — the impaired function — and
a forgotten report would silently forfeit the day's *decaying* post-activation window (§6). So a cron check
at the configured gym-window end: if no activation signal has arrived, **one line — "Back from the gym?"** —
which doubles as the deep-anchor boundary prompt (signal already fired → the boundary prompt surfaces the
deep task instead; no double prompt). Skipped on gym-rest days (Thu), on post-overnight recovery mornings
(§2's override), and when the signal already came. One soft check, consistent with §6's "at most one" rule.
*(This is a cron-origin turn — no Discord-bound session — so its ask is a plain one-line question, not a
clarify button, §12 R1c caveat.)*

### 3a. Hyperfocus exit-ramps (transitioning OUT of deep work — the stopping problem)

Everything above handles *starting*; this handles *stopping*. ADHD hyperfocus means a deep block can overrun
its window — and on a work day the 15:00 commute is a **hard external deadline**, so a missed exit isn't
just lost time, it's a missed shift. Bob watches the *trailing edge* of a deep block, not just the leading
edge.

**When to ride it (don't interrupt):** the task is genuinely important, no hard commitment is being missed,
and basic needs are met. A protected deep block running long on an OFF day with nothing after it is
*success*, not a problem — leave it alone (provide-nothing still applies).

**When to ramp out:** a fixed commitment is approaching (esp. the work commute — back-compute from 15:00
minus prep+travel), the block has run ~2–3h+ with diminishing returns, or basic needs are unmet.

**How to ramp — graduated, never a hard yank** (hard stops fail for ADHD; ramps work):

1. **Soft pre-warning** at the window's trailing edge: *"~30 min until you need to leave for work — good
   point to find a stopping spot."*
2. **One-last-thing**: *"Time to wrap — what's the one small piece to finish or note so you can drop this
   cleanly?"* (closes the loop so re-entry is cheap).
3. **Hard-stop anchor** for immovable deadlines: a real reminder/alarm tied to the actual leave-time, framed
   as the deadline, not a nag. **V1 owner = the `calendar-write` skill (W13):** when the morning plan sees a
   fixed **work block**, `calendar-write` creates a **leave-time alarm on the 🤖 calendar** — an event with a
   **popup reminder at `block_start − commute_prep_minutes − commute_travel_minutes`** (back-computed from the
   block start). **The two constants are DECIDED config (X12):** `commute_prep_minutes: 30` +
   `commute_travel_minutes: 25` (hand-editable, §13/§19) — so today's 15:00 work block back-computes to a
   **14:05** popup (15:00 − 30 − 25 = 55 min prior). It is **idempotent by a stable event key** (so a re-fired
   brief updates rather than duplicates it) and is
   **created at morning-plan time**, tested across **create / update (leave-time moved) / cancel (block
   dropped) / recovery (a re-run finds and reuses the keyed event)** — each asserting the **exact** computed
   popup timestamp. A Fantastical mirror is a
   genuinely-optional post-V1 backup notifier (Phase H), **not** a V1 dependency — the 🤖-calendar alarm is the
   V1 hard stop.
4. **Capture the re-entry point** before the switch (see 3b) so the dropped deep task isn't lost.

Also interrupt **planning-as-procrastination**: if a "deep" session is really just re-planning/reorganizing
tasks, name it gently — *"this is planning, not doing — want to start the actual first step?"*

### 3b. Task-transition ritual (switching between tasks cleanly)

When Bob drives a transition (manual signal, block boundary, or surfacing the next thing after one
completes), it reduces the switching cost ADHD makes expensive:

- **Close the loop on what's being left:** capture the *next action* for the outgoing task (one line — feeds
  the surfacing context / `fs_path` re-entry, §5e) so it can be resumed cold.
- **Remove friction on what's next:** name the single first action and, where it can, pre-stage it (open the
  file/repo, surface the link) so starting is one step, not a setup project.

This is the in-the-moment companion to §5's planning — planning sets the anchors; the transition ritual makes
moving between them cheap.

---

## 4. In-the-moment / on-demand — one next action

From the now-eligible set, Bob surfaces ONE by matching state:

| State | Surfaces |
|---|---|
| Deep window + activated | one `@deep` task |
| Low energy / not activated | `@light` / `@admin`, a quick-win (≤5 min), or **nothing** |
| N-min gap before next event | active task with `duration ≤ N` |
| Out / errands | an `@errand` |
| Blocked (`@waiting`) | never surfaced as do-now |

Never the full list. After completion: *"Next?"* — user's call. Bob marks tasks done as they finish
(frequent small wins).

**Defer / snooze is a first-class outcome** (a JITAI primitive). When Bob surfaces one task and the user
says *"not now" / "later" / "tomorrow"*, the response splits by horizon — **one canonical rule set**
(mirrored in §8, the counter's single authority):

- **HARD RULE — agent date-writes are always date-only.** Bob **never** writes a time-of-day into
  `due.date`; a due-*time* is reserved for the user's real appointments. This is what keeps §8's
  fixed-vs-surfacing `"T"` heuristic sound — an agent snooze that wrote a time would make the task read as an
  appointment and silently exempt it from the roll + stall counter *forever* (the overdue-rot §7 exists to
  prevent).
- **HARD RULE — one centralized date-mutation layer, verb chosen by task state (W6).** Every agent
  date-write — capture dating (§8b), deadline lead-time (§4c), planning-pull promotion (§4c), snooze (§4),
  the nightly roll (§8) — goes through **one shared helper**, never a scattered ad-hoc `td` call, so the
  verb-selection rule is enforced in exactly one place and every path journals to the lifecycle ledger
  (§4d/§8a). The helper picks the verb from the task's *current* `due` state:
  - **Initially dating an UNDATED non-recurring task** (a capture, a planning-pull promotion, or a lead-time
    surfacing date on a task that carries no `due`) uses **`td task update --due <YYYY-MM-DD>`**. **Verified
    v1.75.3: `td task reschedule` REFUSES an undated task** — `Error: NO_DUE_DATE — Task "…" has no due date.
    Use "td task update --due" to set one.` — so `reschedule` *cannot* perform the initial dating. On an
    undated non-recurring task there is no recurrence rule and no existing time-of-day for `--due` to clobber,
    so the "replaces the whole due" caveat below does not apply.
  - **Re-dating an EXISTING date-only non-recurring task** (a snooze or roll of a task that already carries a
    Bob-written date) uses **`td task reschedule <ref> <YYYY-MM-DD>`** (verified v1.75.3 help: *"Reschedule a
    task (preserves recurrence)"*) — it preserves any recurrence rule and any existing time-of-day.
    **`td task update --due` is FORBIDDEN on an already-dated task**: it replaces the whole due string and
    *would* destroy a recurrence rule.
  - **Timed or recurring tasks are NEVER date-mutated by the agent** (`due.isRecurring == true`, or a `"T"`
    time in `due.date`). Recurring tasks are **roll-excluded by design** — recurrence owns its own next date
    (§8's secondary guard) — and a timed due is the user's appointment, not Bob's to move. So a "not now" on
    such a task is within-day suppression only.
- **Within-day defer ("not now" / "later today") = surfacing-suppression only — NO date write, NO label
  tick.** The task stays dated today, still in tonight's roll set; Bob just won't re-offer it this cycle (it
  re-enters at the next §3 block boundary / decision point). Still unfinished at 23:00 → the nightly roll
  provides its **single** tick. The counter measures being *carried to a new day* (§4d); a same-day "not now"
  isn't that.
- **Tomorrow defer = re-date to tomorrow (date-only) + tick at deferral.** Increments the task's
  `roll_count` in the private **lifecycle ledger** (§4d) at the moment of deferral; end-of-day then skips it
  (date now future) — no double-count. **The tick predicate is the date transition itself:** a snooze ticks
  **iff** it moves the due date from today/overdue → tomorrow. Stateless dedupe — after the move no second
  same-day snooze can match, and a task rolled last night then snoozed to tomorrow correctly escalates as a
  genuine 2nd consecutive carry (`roll_count` reaches 2 → §7).
- **Future-day defer (beyond tomorrow) names a *when* and exits clean.** Bob resolves "next week"/"Saturday"
  to a concrete **date-only** due (proposes one if the user doesn't) — postponement is valid, but it gets a
  landing spot. It leaves the roll set with **no tick** and **resets** the task's ledger entry (`roll_count →
  0`): a conscious future defer is *handled* — streak reset (§8).
- **No shame, no re-ask this cycle** — a deferred task is *handled*, not a failure. Repeated new-day carries
  — never a single defer — are what climb the §7 ladder.

---

## 4b. Filters (Bob's saved Todoist queries)

| Filter | Query | Purpose |
|---|---|---|
| **Today's 3** | `(today \| overdue) & p1 & !@waiting` | the must-do anchor; morning brief + "are the 3 done?" |
| **Active now** | `(today \| overdue) & !@waiting` | full eligible set for momentum-mode surfacing |
| **Follow-ups** | `@waiting & (today \| overdue)` | blocked items due for a chase |
| **Deep window** | `@deep & (today \| overdue) & !@waiting` | deep-work candidates for peak windows |
| **Errands** | `@errand & !@waiting` | location-dependent tasks |

**Label-name notation (verified `td` v1.75.3).** The stored label names are **unprefixed** — `deep`,
`light`, `admin`, `errand`, `waiting` (a task's JSON `labels` array holds `["deep"]`, not `["@deep"]`). The
`@` written throughout this spec is **prose / filter-query notation only**: inside a Todoist filter query
`@waiting` is how you *reference* the label, but `td label create --name` / `td task update --labels` take
the bare name. Command examples strip the `@` accordingly.

`p1` is reserved for the daily ≤3 must-dos — Bob marks at most 3 tasks `p1`/day (set each morning; at day's
end Bob clears **only the ids HE set** — the plan record's `selected_ids`, §4c/§8 — and **never touches a `p1`
the user set directly** (AA2)). A **user-set `p1` counts toward the ≤3**: Bob assigns `max(0, 3 −
user_p1_count)` of his own, and a user `p1` older than 48h is flagged as **one `stale-p1` decision-queue item**
(§2 step 4/§8), never auto-cleared. The ≤3 is maintained by **assignment discipline, not the query** (Todoist
can't cap a filter's count). The ≤3 is a **floor, not a ceiling** — after the 3,
momentum-mode keeps feeding one-at-a-time from `Active now`. Quick-wins (duration ≤5 min) are filtered
client-side by Bob (duration isn't a reliable filter operator).

**`@waiting` set-time invariant:** applying `@waiting` **always sets a check-back due date + a blocker note
at the same moment** (what/whom it's waiting on) — never the bare label. A dateless `@waiting` is invisible
to the `Follow-ups` filter (it queries `today | overdue`) — the exact black-hole the lifecycle exists to
prevent; the §8 reconcile detects and repairs violations.

## 4c. Daily p1 selection (the morning narrowing)

The algorithm Bob runs each morning to pick ≤3 from the active pool — **fitted to today's actual capacity**,
not just ranked:

1. **Pool** = `Active now` = `(today | overdue) & !@waiting`. **Groom-on-read (the trigger with an owner):**
   tasks enter this pool from anywhere — Bob's flows *or the user dating a task directly in Todoist* — so
   **every pool read detects tasks missing a load-label or duration**. The two daily cron reads (this brief +
   EOD §8) are the authoritative groomers (idempotent — the read *is* the activation trigger); an on-demand
   §4 read answers *first* using the fallback — **ungroomed = treat as `@light`, unknown duration = eligible
   for light/admin surfacing but never gap-fit or capacity-fit** — and grooms opportunistically after. No
   active task is ever silently invisible for missing data.
2. **Compute today's free windows** from §2 (the `work_schedule` + gym schedule + fixed calendar anchors):
   the concrete blocks actually open today, and whether a **deep window exists** — work day → morning/midday
   only (evening is work); off day → morning + evening; Thursday → neither morning-activation nor evening, so
   a light/admin day.
3. **Rank Q1 first** — anything with a real deadline today/overdue.
4. **Q2 against the 5 goals** (the importance yardstick — read at p1 time from the owned vault note
   `~/workspaces/Ivy/forzare/goals.md`; per §6's owned-writable layer, **not** the frozen memory snapshot and
   **not** Todoist Goals — see decision note): protect **one** deep-work investment, but only if today
   actually has a deep window to hold it.
5. **Capacity / window fit (the schedule coupling).** Every p1 candidate must fit a free window that exists
   today. A `@deep` needing a ~2h block is **not** a valid p1 on a day whose only open slots are short —
   defer it to a day with the window, or pick its smaller next-action instead. On a no-deep day (e.g.
   Thursday), the ≤3 bias to `@light`/`@admin`. The ≤3 is a capacity budget, not just a priority list.
6. **Cap at 3 — and USER-set p1s count toward the budget (AA2).** Bob assigns at most `max(0, 3 −
   user_p1_count)` of his own p1s, so a day the user already flagged two p1s leaves Bob one slot — the ≤3 is a
   shared budget, not "3 Bob-p1s on top of the user's." If >3 real deadlines collide, Bob enqueues a
   **`q1-conflict`** decision-queue record (§2 step 4; §7; INV-5: ask before a big reprioritization) — never a
   silent drop. A user-set `p1` older than 48h surfaces once as a **`stale-p1`** record (never auto-cleared) —
   **enqueued by EOD (`eod-roll`, §8), the SOLE `stale-p1` producer; this morning pass does NOT enqueue it**, it
   only reads the head like every other brief-time decision (§2 step 4).
7. **Idempotent — via a PER-DAY PLAN RECORD, not a p1 count (Y13).** The morning narrowing is guarded by an
   owned-layer **`forzare/state/plan-of-day.json`** — `{date, created_ts, selected_ids[], anchor, writes: {p1_set,
   anchor_placed, alarm_set}}` — written *as the narrowing executes*. On entry the plan step reads it: if a
   record for **today** already exists, the day is already planned, so it **resumes only the missing writes**
   (each `writes.*` flag is that write's done-marker) and never re-runs a completed one or tops p1 up past the
   recorded `selected_ids`. This **distinguishes Bob-owned p1 from user-set p1** — only the ids in
   `selected_ids` are Bob's, so a `p1` the *user* set directly in Todoist is left untouched, which the old
   "any p1 present" heuristic could not tell apart (a user's own p1 would have wrongly read as "already
   planned"). A re-fire or the ±2h catch-up (§8) therefore converges to the same day-plan idempotently,
   completing any write the first pass didn't (§15). *(This replaces the earlier "any p1 present" guard; the plan
   record names exactly which ids Bob set today — which is ALSO what scopes the EOD p1-clear to Bob's own ids
   and leaves a user-set p1 untouched, AA2.)*

**Someday→active promotion — the pool's designed inflow.** The active pool only *drains* (completions,
drops, future-defers) unless something dates tasks INTO it; without a designed inflow, captures externalized
to the system are functionally lost — the exact prospective-memory failure forzare exists to remove. Three
mechanisms date tasks in (each firing activation-time grooming, below):

- **Deadline lead-time (automatic).** Any task carrying a real `deadline` gets a computed **surfacing due
  date** = deadline − duration-aware padded lead time — **date-only** (so it rolls normally as
  surfacing-dated; the immovable date stays in `deadline`). Steeper ADHD temporal discounting means the
  **system** computes lead time; it never trusts future-self to notice a deadline approaching.
- **Capture dating (§8b stage 1 — placement).** Captures that state or imply timing are dated at placement (§8b).
- **Planning pull (goal-matched).** When the dated Q2 pool is thin, this morning narrowing (and EOD's
  pre-stage) pulls goal-matched candidates from the someday pool — matched against the §goals yardstick — and
  dates them. Deliberate, small (one or two), and one of the two places the someday backlog re-enters view;
  the backlog itself stays out of sight (§0). **Bounded, user-approved freshness policy:** the pull is
  conservative until the ~2,270-task backlog has been relevance-combed (a separate, still-pending pass) — an
  un-combed backlog would surface stale junk, so early on the pull proposes rather than auto-dates.

**Backlog re-decision — the monthly someday-sweep (the second, deliberate re-entry path).** Left alone the
someday pool only grows, and a ~2,270-item backlog is an ADHD wall by its mere existence (§0). So **once a
month** a **SMALL batch (≤5)** of the *oldest / most-stale* someday items is proposed as one-line **keep /
drop / promote** decisions — decide-in-context, one line each, **no walls and no shame**.

**The sweep feeds the ONE unified decision queue, delivered one-at-a-time (X7/R5A1 — not a ≤5 list-dump).**
The old "feed the ≤5 into the brief" phrasing would surface a five-item checklist in a single brief, which is
exactly the multi-decision wall the one-thing rule forbids (§0/W12) and the user's stated no-batch preference.
Instead the sweep is just **one producer among several** for the single head-item-only queue (§2 step 4):

- **Owner + write (state-only, R4A6):** a monthly cron (`0 5 1 * *`, brief-time on the **1st**) runs the
  `followups-sweep` skill in its **SWEEP mode** — it selects the ≤5 oldest/most-stale candidates from the
  **sweep pool = union(oldest UNDATED someday items, long-cycling DATED actives so defined above —
  `roll_count ≥ 10` and no progress ≥ 30 days, R7A5)** and **enqueues them to the unified
  `forzare/state/decision-queue.json`** (§8a) as **`sweep-candidate`** records in the ONE canonical schema (DD4,
  §2 step 4/plan B0) — `id = "sweep-candidate:" + candidate_id`, `proposed ∈ {keep, drop, promote}`,
  `status: pending`. It **never messages** — the monthly job is `--deliver local` (§12.4).
- **Delivery = the brief's single HEAD item only (via the MORNING BRIEF, never a second proactive message,
  R2A20/R5A1):** each morning the brief reads the unified queue and emits **only the single HEAD `pending`
  record** as its one requested decision (§2 step 4) — which **replaces that day's do-now action**, so the
  one-decision invariant is preserved. On the user's answer the **live turn that received it TOMBSTONES the head
  via the compare-and-set on `{id, gen, rev}`** (§2 step 4/BB2) and the **next** record surfaces the following
  morning (R5A5). One anchor message a day
  still holds; the sweep's ~5 candidates drain over ~5 days *behind* whatever higher-priority
  `waiting-chase`/`stall-decision`/`fixed-redecision`/`triage-reraise` records are already queued, one
  decision each.
- **Task bankruptcy — a REVERSIBLE, TWO-CLASS opt-in clear (DECIDED threshold > 25 candidates, §19; Y3/Z13).**
  When SWEEP mode finds the stale set exceeds 25, it additionally enqueues an explicit, opt-in **"task
  bankruptcy"** offer as one **`bankruptcy-offer`** queue record (§2 step 4 — the lowest class-rank, so it
  never jumps ahead of a live chase/decision; "~N of these haven't moved in ages — clear them all?"). **The batch
  operation is NEVER a delete / complete / archive** (v1 has no destructive bulk op). A single yes **"clears the
  tail," applying the op that fits each item's CLASS** — the sweep pool spans two:
  - **Stale DATED actives — DEFINED so the class is actually REACHABLE (R7A5):** a **ledger entry with
    `roll_count ≥ 10` AND no progress ≥ 30 days** (both DECIDED, §19) — a long-cycling active, not "a due that
    never moved" (that phrasing was a contradiction: the nightly roll re-stamps `written_due` every night, so a
    surfacing due *always* moves). → **UNDATE**: strip the due from the frozen set so they drop back to the
    **hidden someday pool** and leave the active view.
  - **Undated someday items** (genuinely never dated — an UNDATE is meaningless, there is no due to strip) →
    **RETIRE**: append the id to the **sweep-exclusion list** (`forzare/state/sweep-exclusion.json`, §8a) so the
    monthly sweep never re-proposes it. Retire writes **no label, deletes nothing, re-parents nothing**; it is
    **reversible by deleting the exclusion entry**, and the task stays exactly where it is (hidden, undated).
  Neither class destroys a single task. This is deliberate — **two distinct incidents** motivate the
  reversibility rule, cited separately: the **Todoist parent-delete cascade** (deleting a parent silently
  deletes its subtasks; STATUS:76, 3 videos — `td activity --json` is the recovery path) and the **separate
  2026-05-20 Obsidian-vault refactor** (a bulk rename/merge that deleted 31 files). Before the op the exact **id
  set is FROZEN and journaled** (a `bankruptcy` journal record naming every id + its class/op); the confirmation
  **names the operation** ("undate N dated + retire M someday — reversible"); a **bounded summary** is shown
  (count + a few sample titles, never the whole list); and the clear is **idempotent on partial failure** — a
  re-run reads the frozen journaled set and completes only the ids not yet undated/retired, so an interrupted
  clear resumes cleanly and never double-processes. A **before/after set-membership test** (plan B5) asserts
  each dated id left the active view and each undated id entered the exclusion list. The frozen id list **is**
  the recovery/backup record. Still a proposal; nothing leaves view without the user's word, and nothing is ever
  deleted.

**Goals-backend decision (2026-06-02): yardstick stays an owned file; Todoist Goals (Beta) evaluated +
rejected.** Goals is the right *category* (a semantic objective — name/description/deadline, agent-readable
via `td goal list --json`), but a worse fit: its single free-text `description` can't hold the per-goal
Eisenhower quadrant + current sub-focus this ranking depends on; several of the 5 goals (Homelab, Casually
Concerned) have no natural deadline; and it's a beta, Pro-gated, REST-API-undocumented surface — a fragile
foundation for a daily-read core dependency, against the owned/open-format longevity principle (same reason
Marvin was rejected). **One copy only** — the vault note; no Todoist-Goals mirror.

**Build prerequisite:** `goals.md` must be created from the existing `current-goals` memory content —
**carry over each goal's Eisenhower quadrant** (Podium=Q1, Essential Developer=Q2, Casually Concerned=Q2,
Homelab=Q2, Karl=Q2) **and current sub-focus**, because step 4's Q2 ranking reads them. Until `goals.md`
exists, p1-selection falls back to reading `current-goals` from memory.

**Duration is a required field — estimated with ADHD time-blindness factored in.** Steps 2/5 (capacity fit)
and the §4 gap-filler row (`duration ≤ N`) both depend on tasks carrying a `duration` (Todoist native field,
minutes). A task with no duration silently can't be capacity-checked or gap-fit — so duration is a **data
prerequisite, not optional**: Bob sets/refreshes it (the research confirms duration is *agent-estimable*)
when a task enters the active pool (gets a date) or during grooming — not a mass-backfill of the whole
someday backlog.

- **Factor in ADHD time perception.** The research is explicit that **time perception is reliably impaired
  in ADHD** (Barkley — Round-1), which manifests as chronic **under-estimation** (time-blindness / planning
  fallacy). So Bob **biases duration estimates upward** to counter this, rather than taking a naive best-case
  guess.
- **Treat every estimate as approximate.** Round-2 is direct: agent duration estimates *will* be imperfect —
  so capacity-fit (step 5) must **tolerate error**, and a blown estimate is **never framed as failure**
  (Round-1 self-forgiveness / no-shame; INV-4). An over-run just feeds the §4d roll, it doesn't get scored.
- **≤5 min = quick-win** (Round-1 immediate-reward) — surfaced as paralysis-breakers in low-energy states
  (§4).
- The exact upward-bias factor is a **config default, tunable from observed over/under-runs** — the research
  prescribes *direction* (pad against under-estimation) + *tolerance*, not a fixed multiplier, so don't
  hardcode one as "research-backed." Calibrate it from real completion data over time (the same owned-layer
  learning loop as §6).

**The cognitive-load label is the same kind of prerequisite — and even more load-bearing.** Surfacing
(§4/§6) matches person-state to a task's `@deep`/`@light`/`@admin`; a task with **no** load-label can't be
matched to any state, so it's invisible to the engine — a worse failure mode than a missing duration. So the
load-label (and a quick **verb-first cleanup** — a raw "milk" → "Buy milk") is set on the **same trigger and
rule as duration**: when a task **enters the active pool (gets a date)** — via any of the §-above inflows or
groom-on-read — not as a mass-backfill of the someday backlog. Captures: §8b stage 1 (placement) dates time-bound
captures at placement (grooming fires then); genuinely timeless captures rest undated as someday until
promoted (above).

**Activation-time grooming has FOUR elements — the fourth is the atomicity gate:** (1) load-label, (2)
duration estimate, (3) verb-first cleanup, (4) **next-action check** — if the item is vague or project-sized
("sort out insurance"), Bob **extracts or decomposes a concrete first step himself** (agent-proposed, same
lever as §6a's if-then; he asks the user only when the split is genuinely ambiguous — one decision,
decide-in-context). Until it passes, the item is **not surfaceable as a do-now** — but never silently hidden:
if extraction can't complete, it's flagged in the morning brief. Without this gate, a vague dated item
surfaces as-is and only gets decomposed *after* stalling twice (§7) — decompose-on-entry beats
decompose-on-failure.

## 4d. Stall tracking — the consecutive-roll counter (private lifecycle ledger)

The §7 stall ladder needs to know **how many consecutive nights a task has been carried forward without
progress** — and that must survive fresh (amnesiac) cron sessions.

**Why this can't ride on "overdue".** §8's end-of-day loop **rolls forward the unfinished "meant-to-do-today"
tasks** (reschedule → due tomorrow; *excluding* fixed items — user-dated/appointments/recurring/future-dated;
ledger membership governs, §8/W6) so
overdue never piles into a guilt-wall (INV-4). But that reschedule means a task dodged for 5 days looks
identical to a brand-new task due tomorrow — overdue is *consumed* by reconciliation and can't double as the
stall count. So the stall memory must live somewhere the nightly roll doesn't wipe.

**Stored in a private lifecycle ledger, NOT on the task.** The counter lives in the owned layer as
`forzare/state/task-lifecycle.json` (§8a), a map keyed by Todoist **task id**, each entry
`{written_due, roll_count, last_escalated, kind}`:

- **`written_due`** — the due date Bob last *wrote* on the task (a date-only agent surfacing date).
- **`roll_count`** — consecutive nights carried without progress.
- **`last_escalated`** — the date the §7 escalation last fired for this task (re-nag guard).
- **`kind`** — **the provenance of the written date (X5), which governs roll membership.** One of:
  - **`surfacing`** — a date Bob wrote purely to make the task *show up* (a snooze, a planning-pull
    promotion). **Rolls.**
  - **`leadtime`** — a deadline-derived surfacing due (§4c lead-time; the immovable date stays in `deadline`).
    **Rolls.**
  - **`waiting_checkback`** — the check-back date the `@waiting` set-time invariant (§4b) stamps. **Never
    rolls, never ticks** — it is a chase reminder owned by the follow-ups path (§8's reconcile → §2 step 4),
    not a do-now that stalls.
  - **`user_fixed`** — a date the *user explicitly stated* that Bob merely transcribed (a capture like
    "Saturday", §8b stage 1). **Never rolls** — the user chose the day; it is theirs, not Bob's to move.
  **Only `surfacing` + `leadtime` join the roll set.** `waiting_checkback` and `user_fixed` are journaled (so
  a user edit still voids them via the divergence test, and calibration can exclude Bob's write — §6a/X11) but
  are excluded from the nightly roll and the stall counter by `kind`.

Rules:

- **Every agent date-write records `written_due`.** When Bob dates or re-dates a task (surfacing date, snooze,
  lead-time), it stamps the value it just wrote.
- **The roll set is the ledger entries with `kind ∈ {surfacing, leadtime}` (X5) where the task's *current*
  `due.date` still equals `written_due`**
  (self-healing provenance) — **this is the definition** (V1; §8 is the single authority). A `waiting_checkback`
  or `user_fixed` entry is excluded by `kind` before the date test even runs. If current and `written_due` differ,
  **the user re-dated the task** since Bob touched it → the entry is **void** and the task is treated as
  *fixed* (user-owned date, never auto-rolled). So **user-dated tasks never roll**, with no label bookkeeping
  to get stale — the divergence *is* the signal. The task-field checks (`due.isRecurring`, a `"T"` time,
  future date) are **secondary sanity guards** applied *within* this set, not the membership test. **A
  deadline-bearing task with a Bob-written lead-time surfacing due IS a ledger entry and DOES roll** (§4c/§8);
  `deadline != null` marks "fixed" only for a task with **no** ledger entry (a user-dated deadline-day task).
- **A roll increments `roll_count`.** At `roll_count == 2` (2nd consecutive carry) Bob fires the §7
  escalation; `last_escalated` is stamped so the same stall isn't re-nagged the next night.
- **Reset on progress** (`roll_count → 0`, entry effectively cleared) on the same triggers as before —
  completion / subtask done / user comment / user-reported "touched" (§8) — and on a conscious
  beyond-tomorrow defer.
- **Prune on terminal state:** a completed or deleted task's entry is dropped (detected from the activity
  log, §8a), so the ledger stays roughly the size of the small active/rolled set, never the ~2,270 backlog.

**The lifecycle store SPLITS into two (Y5/X11) — a prunable MAP + an append-only JOURNAL.** The roll state and
the mutation history have different lifetimes, so they live in two owned-layer stores, not one:

- **Lifecycle MAP — `forzare/state/task-lifecycle.json`** (the roll state, above): `{written_due, roll_count,
  last_escalated, kind}` keyed by task id. **Pruned on terminal state** — a completed/deleted task's map entry
  is dropped (detected from the activity log, §8a), so the map stays roughly the size of the small
  active/rolled set, never the ~2,270 backlog.
- **Mutation JOURNAL — `forzare/state/mutation-journal.jsonl`** (append-only, typed): **every** Bob-authored
  mutation, one line in the **single unified record shape (CC8 — the same fields in §4d, §8a, and plan B0; the
  two earlier partial schemas merged):** **`{ts, created_ts, type, target, op, args, old_value, intended_value,
  external_marker?, reconcile_date, commit_state}`** (`created_ts` = when the intent was first journaled — the
  clock the crash-heal keys its propagation-window checks off, GG4) — `type ∈ {date-op, p1, label, comment, calendar,
  description, task.add, task.complete, waiting-clear, undate, retire, bankruptcy}` (X11/R5A11/Z3/BB3/FF1): `date-op` (the
  surfacing/lead-time/roll/snooze
  date-writes above), `p1` (a morning `p1`-set or the EOD clear), `label` (a grooming load-label or the
  `@waiting` label write), `comment` (an auto-repair or state comment), `calendar` (a 🤖-calendar write,
  §5c/§3a), **`description`** (added R5A11 — the §7/X13 if-then cue Bob persists to a task's *description*; the
  Todoist activity log reports that write as a bare **`updated`** event, so it MUST be journaled or W7's
  exclusion would misread it as a user touch), and — **added Z3, aligning the enum with the intent-op
  vocabulary** — **`task.add`** (a capture-create; without it a Bob-authored `added` event reads as a user
  touch) and **`task.complete`** (a Bob-authored completion); and — **added BB3, so the composite + bankruptcy
  ops are first-class** — **`waiting-clear`** (the 02:00 unblock's composite clear-`@waiting` + re-date +
  `kind`-flip, §8a), **`undate`** (a bankruptcy UNDATE of a stale dated active, §4c), and **`retire`** (a
  bankruptcy RETIRE onto `sweep-exclusion.json`, §4c); and — **added FF1** — **`bankruptcy`** (the frozen id-set
  snapshot the sweep journals before a bankruptcy clear — naming every id + its class/op; the queue's
  `bankruptcy-offer` record carries NO frozen field and references this journal record by storing its
  `journal-uuid` `external_marker` in the queue record's **`journal_ref`** field — the ack matches that EXACT uuid,
  never a month search (II3), §4c). **pending→commit→heal ordering is defined for
  every `type`, each with a type-specific verification predicate** (§8a/V2/Z3/BB3 — the journal-then-commit write
  order and the crash-heal re-verify apply to each op class, not just date-ops).
  Entries are **retained through the calibration correlation window — 45 days (DECIDED, §19) — then pruned**;
  a task's **completion prunes the MAP, never the journal window** (the calibration reducer still needs the
  recent journal to exclude Bob's own writes even after the task is done).

**This split is what makes the §6a/W7 calibration-exclusion claim TRUE:** because Bob writes to Todoist as the
same account, `td activity`'s `initiatorId` can't separate his writes from the user's, so the correlator
excludes any activity event that matches a **journaled** forzare write (§6a). Journaling **all** Bob mutations
— not just date-ops, and now including the `description` if-then write (R5A11) — is the only way that
exclusion is complete (a label groom, auto-repair comment, or if-then cue would otherwise read as user
"initiation"). The journal is written on **real** runs; under dry-run the intended writes go to
`dryrun-intents.jsonl` instead (§17), never the journal or the map.

**Increment is roll-driven, deduped per day.** `roll_count` ticks when a task is *carried to a new day without
progress* — via one of two paths that never both count the same task: (a) end-of-day's nightly roll (§8) for
silent non-completion, or (b) an explicit **snooze-to-tomorrow** (§4), which ticks at the moment of deferral
(the today→tomorrow date transition *is* the dedupe) and is then **skipped** by that night's end-of-day (date
already future). A **within-day** defer never ticks — the nightly roll is its single site; a
**beyond-tomorrow** defer exits the roll set with no tick and **resets** the entry (handled). Net: **at most
one increment per task per day**, no scattered bookkeeping. (A task parked far in the future isn't rolled
until its date comes back around — scheduled ≠ stalling.) The precise increment/reset rules live in §8 (the
single authority); this is the summary.

**Why a private ledger, not marker labels — and the rejected alternative.** The earlier design stored two
lifecycle labels (`@rolled`/`@stalled`) on the task itself. **Rejected**, because a marker label is
assertion-only provenance: it records *that Bob thinks the task rolled* but **can't self-validate against a
user edit** — if the user re-dates the task in the Todoist app, the label lingers and the streak reads wrong
until some reconcile notices, and the "rolled/stalled" state is **visible on the user's task** (a standing,
if muted, failure marker — the no-shame concern, INV-4). The ledger instead derives "did this actually roll?"
from `current due == written_due`, which *self-corrects* the instant the user touches the date, keeps every
failure-shaped signal **off the user's surface entirely**, and shrinks the `--labels` read-modify-write
clobber race (fewer full-set label writes per task). **The user can overrule this at PR review** — if a
visible, filterable state is wanted back, restore the two marker labels and drop the ledger; the trade is
losing the self-healing provenance and re-introducing a visible failure marker.

**Grooming label writes still use full-set replace (verified `td` v1.75.3 — `--labels` REPLACES the set,**
`--no-labels` clears all**).** There is no additive label flag, so any label mutation Bob *does* make — a
grooming load-label, or the `@waiting` lifecycle label — is a read-modify-write: read `labels[]` from the task
JSON, add/strip the one label, preserve `deep`/`admin`/etc., write back with
`td task update <id> --labels "<full,set>"`. A partial write drops the rest, so never write a partial set.
(With the roll/stall labels gone, these writes are rarer and the clobber window is smaller.)

**`td` gotchas (verified v1.75.3):** create still prints the ID even under the global `-q` (so: create via
`--json`, parse `.id`); `td task view` on a just-deleted task can still return it (read-replica lag — verify
deletions via `td task list`); td JSON is **camelCase** (`projectId`, `due.isRecurring`), and list output is
an **envelope** — `{ "results": [...] }` (parse `.results[]`, never bare `.[]`).

**Vocab impact:** the label vocabulary **stays at the 5 surfacing labels** (`deep`/`light`/`admin`/`errand`/
`waiting`) — the ledger adds **no** task-visible label. `@waiting` is unchanged.

**Threshold = DECIDED: escalate on the 2nd consecutive roll** (`roll_count == 2`). Locked, not a runtime
knob — fast catch, and the escalation is a gentle decompose/if-then offer (§7), not a nag, so 2 is safe.
(Because the count is an integer in the ledger, pushing to 3+ later is a one-line change, not a redesign.)

---

## 5. Planning layer — Eisenhower triage feeds the schedule

The **agent** applies Eisenhower when building the day (the user never ranks live):

- **Q1** urgent+important → `deadline` / `p1` today.
- **Q2** important-not-urgent → deep-work investments → **protect peak windows** (the biggest ADHD win).
- **Q3** urgent-not-important → batch into low-energy admin gaps.
- **Q4** neither → someday / drop.

→ Bob writes **time-blocks to Google Calendar**. **Importance shapes the plan; state drives the moment.**

### 5a. Time-blocking philosophy — "ANCHOR, don't fill" (the precise rule)

A fully-blocked calendar is **wrong for this user**, and the research says why: importance/urgency planning
"falls apart fast" for ADHD (it assumes intact planning + sequencing, and everything feels equally urgent),
and activation is driven by **state — interest/autonomy (SDT), urgency (delay-aversion), novelty (optimal
stimulation), effort-cost — not importance** (the calibration research's supersession of the Round-1 INCUP
framing). A rigid full plan therefore (a) fights the wiring, and (b) slips the moment one block runs over →
an overdue-style guilt-wall (INV-4). So Bob does **not** schedule the day block-by-block.

But the *opposite* extreme is also wrong: a calendar block **is** an implementation-intention ("when 9:00
comes → start X"), and if-then is the **single strongest lever in the system** (d=0.65 is the *overall*
implementation-intention effect, Gollwitzer & Sheeran 2006; d=0.99 in self-regulation-impaired samples,
**Toli et al. 2016**; the ADHD-specific evidence is in **children** and reports no pooled *d* — so
"ADHD-specific" names the population, not that figure). Some blocking is the highest-value thing Bob does.

**The resolution — block only these, leave the rest fluid:**

1. **Fixed external commitments** (work block, appointments) — these already exist; Bob treats them as
   immovable load, doesn't create them.
2. **ONE protected deep-work block, only if today has a deep window** (§2/§4c). This is the Q2
   investment-protection — the biggest ADHD win (INV-13). One block, in the peak window, for the day's single
   most important deep task. Not two, not a stack.
3. **(Optional) one batched admin/light block** on a no-deep day (e.g. Thursday), if there's a cluster of
   `@admin` worth grouping.

**Everything else stays UNSCHEDULED — deliberately.** The fluid time between anchors is where §4 surfaces ONE
task at a time by state (the interest/urgency-driven engine). Bob does **not** put Q1 deadlines or Q3 admin
on the calendar as individual blocks — those live in Todoist with due dates and get *surfaced*, not
*scheduled*. The rule in one line: **block the protected deep window + accept fixed commitments; surface
everything else.**

**Why this is the ADHD-correct cut:** it externalizes the *one* structure that matters (the protected
investment) as an if-then anchor, while leaving the rest of the day responsive to real-time state instead of
a brittle pre-plan. Anchors give scaffolding; fluid time respects state/activation-driven motivation.

### 5b. Planning cadence (when §5 runs)

- **Morning brief (§2):** build today's plan — accept fixed commitments, place the one deep anchor if a deep
  window exists, set the ≤3 p1 (§4c).
- **End-of-day (§8):** pre-stage tomorrow's candidate anchor + ≤3 (proposal; confirmed/replaced in the
  morning).
- **Transitions (§3):** do **not** re-block the calendar — a transition re-*surfaces* (picks the next one
  thing) within the existing anchors; it doesn't redraw the plan. (Re-planning mid-day = the rigidity trap
  again.)

### 5c. Calendar write contract (trust / least-surprise)

Bob writes **only** to a dedicated "Bob"/🤖 calendar — **never** the user's primary; **never** edits or
deletes user-created events. Blocks are **movable proposals**, not commitments the user must obey — with
**one carve-out:** a **user-confirmed fixed event** captured via §8b's calendar pre-check (a real appointment
the user approved onto the 🤖 calendar) is **immovable load**, treated by §2/§5 exactly like a
primary-calendar anchor. Same firewall principle as §9 (Kanban private) and §4d (don't touch the user's
comments): **the agent writes to its own lane, never mutates the user's data.**

### 5d. Q2 is derived, not labeled (no `@q2`)

"Which active tasks are Q2 investments?" is computed at plan-time by matching the active pool against the
goals yardstick (§4c, the owned `goals.md`) — Bob already does this to pick the deep anchor. So **no `@q2`
label** is added (the label vocabulary stays at the **5 surfacing labels** — no lifecycle markers either;
the roll counter is the private ledger, §4d). Rationale beyond vocab economy:
importance-based *tagging* is exactly the weak-for-ADHD scheme the research warns against — Q2 is a
*planning-time computation*, not a persistent property of the task.

### 5e. `fs_path` bridge

The vault↔Todoist **`fs_path` bridge** (MASTER) resolves a surfaced task to its working files: Todoist
project → Obsidian folder-note → `fs_path` → the real files. Surfacing carries that context so "start X"
lands the user *in* the work, not hunting for it. **Container/group metadata is DECIDED to live here** — in
the vault folder-note frontmatter ("**Option C**", MASTER build status): per-project/group status
(job-application state, etc.) is NOT held as grouped Todoist labels and NOT duplicated in the agent (no
split-brain); Bob reads it across the same bridge.

---

## 6. Energy = passive inference (never a required input)

Bob infers state from signals already available — **gym/activation signal** (best tell), **today's free
windows derived from the `work_schedule`** (§2: a *short-lived* post-gym activation window in the morning —
never an all-day "morning peak," §6a below; evening deep-capacity only on OFF days — work days own the
evening), the post-lunch dip, calendar load, recent completion vs stalling. Asks only when
ambiguous (light, occasional); user may volunteer ("I'm fried"/"locked in"). Calibrates from observed
patterns over time. **No "rate your energy" gate before tasks.**

**Receptivity decision rule** (when to act vs stay quiet), not an energy gate. State maps **directly onto the
cognitive-load labels** — `@deep`/`@light`/`@admin` ARE the energy-commitment axis, so inferred state selects
which label-class is eligible (no separate energy scale exists or is needed):

- **Activated / peak** (post-gym, "locked in", recent completions, clear calendar) → **`@deep`** eligible.
  The *only* state in which deep work surfaces. **⚠ The post-gym activation boost is SHORT-LIVED** — Mehren
  2019 observed the acute-exercise executive-function benefit in ADHD adults at **~10 min post-exercise** and
  found **no effect on the ~33-min measure** (the authors note the effect's limited duration). So the earlier
  "**~1–2h** working window" claim is **REMOVED** — it was never a measured value. So "back from gym" is a
  *briefly* elevated window, not an all-day flag. **v1 seeds a deliberately conservative prior — a boost
  window of roughly ≤~30 min — explicitly labeled *pending personal data*** (the learned per-person curve,
  §6a, refines the real duration from initiation data; it may run shorter or longer than the seed). Surface
  the day's hardest `@deep` work into the window *right after* activation, not at 4pm because the gym happened
  at 7am. (Corrects the earlier "morning peak always" framing.)
- **Mid / steady** → **`@light`** (engaged but not deep — reading, watching, reviewing).
- **Low / depleted** (no activation, "fried", post-lunch dip, dense calendar, recent stalls) → **`@admin`**
  (shallow/mechanical) or a ≤5-min quick-win, or **nothing**. Never `@deep`.
- **Ambiguous** → bias one class lighter; at most one soft check, never a battery of questions.
- (`@errand` is orthogonal — location-gated, §4. `@waiting` never surfaces as do-now.)

**Dopamine menu (the concrete mechanic for interest-based motivation).** The spec leans on "interest, not
importance" but otherwise has no lever for it. The dopamine menu is an **owned vault note**
(`~/workspaces/Ivy/forzare/dopamine-menu.md`) listing the user's pre-chosen stimulation/reward options tiered
by effort, that Bob can draw from when state is low or a boring task needs a hook:

- **Quick (1–5 min):** between-task resets / paralysis-breakers — surfaced in the low-energy branch (§4)
  instead of a `@deep` task.
- **Longer (10–30 min):** real breaks, esp. as the *reward* end of a hyperfocus ramp-out (§3a) or a
  post-completion win.
- **During boring tasks:** pairing hooks (music/podcast, timer challenge, standing) Bob can suggest alongside
  an `@admin` surface — temptation-bundling.
- **Use-sparingly:** the doom-scroll options, named honestly so they're a bounded choice, not a default.

Pre-chosen (not invented in the moment), human-edited, lives in the same owned layer as the goals yardstick +
calibration state. Bob *offers*, never forces.

Calibration state (observed patterns, what-worked) lives in an **owned layer** Bob can write — not the frozen
MEMORY.md snapshot (read-only, capped). The **5-goal yardstick** (§4c) + the **dopamine menu** live in this
same owned layer (`~/workspaces/Ivy/forzare/`). The calibration *learning* state has its own home + shape,
below.

## 6a. Calibration layer (evidence-based; how Bob learns your patterns)

Grounded in peer-reviewed evidence (JITAI framework, micro-randomized-trial / receptivity literature, ADHD
reward + procrastination science, non-stationary-bandit math). Sources in the companion research synthesis
(2026-06-27). The whole layer is built so **the user is never the sensor** — every input is passively
observed from actions, never self-reported.

**Why passive, restated as evidence (not just preference):** adults with ADHD reliably *mis*-estimate their
own state, and the impairment is **attention-specific** (Mayer 2021) — i.e. self-rated attentional energy is
precisely the unreliable signal. This is the empirical backing for the no-ask-energy invariant (INV-6):
asking would be both burdensome *and* low-quality data. Bob monitors; the user acts.

**The data model — log each surfacing decision as a micro-trial.** Every time Bob surfaces a task **or
deliberately withholds one**, append one record to the owned-layer calibration store
(`~/workspaces/Ivy/forzare/calibration/`):

- **context at decision time:** day_type (work/off), time-of-day bucket, minutes_since_activation (post-gym),
  calendar load / gap-to-next-event, recent completions today, recent stalls today,
  minutes_since_last_surfacing, surfacings_today.
- **action:** the task surfaced (+ its `@deep`/`@light`/`@admin` + duration_est + due-proximity +
  consecutive-roll count) **OR `provide-nothing`** — logging restraint is essential: it's the control
  condition *and* it keeps "provide-nothing-is-valid" measurable.
- **proximal outcome (the label):** initiated? (+ latency), completed / partial / rolled-again /
  dismissed-without-action.

This MRT-shaped log is what turns ordinary use into learnable data — Bob estimates *which contexts predict
initiation*, not guesses. **Not logged:** no energy/mood/satisfaction ratings (per above).

**What Bob computes from it:**

1. **Per-person time-of-day × cognitive-load initiation curve** — does *this* user actually start `@deep`
   better mornings or evenings (chronotype is person-specific — learn it, don't assume "nights are better").
2. **Activation-decay function** — the lift in P(initiate `@deep`) vs minutes_since_activation, fit as a
   *decaying* boost (the §6 ⚠ correction, learned per-person).
3. **Receptivity score** — from the validated cheap features (recent dismissals, over-prompting, calendar
   load). High recent-dismissal density ⇒ withhold. **Made computable (V8) — observable proxies, no
   self-report:**
   - **initiation** = the surfaced task shows **USER** progress within **N minutes of surfacing** (**N = 30,
     DECIDED**) — re-scoped to attribution-reliable signals only (**W7**): a **completion**, a **subtask
     completion**, or an **explicit user response**. **Every forzare-authored mutation is EXCLUDED from
     initiation attribution** — Bob writes to Todoist as the *same* account, so `td activity`'s `initiatorId`
     can't tell Bob's writes from the user's; instead the **append-only mutation journal**
     (`forzare/state/mutation-journal.jsonl`, §4d/X11/Y5 — the store now split off the lifecycle map, recording
     every typed write) records every write Bob makes, and any activity event matching a journaled forzare write
     is filtered out before scoring. **The correlator excludes GENERICALLY over the journal `type` enum — every
     Bob-authored type (`date-op`/`p1`/`label`/`comment`/`calendar`/`description`/`task.add`/`task.complete`/
     `waiting-clear`/`undate`/`retire`/`bankruptcy`), not a hardcoded subset (GG13)** — so a new op type can never
     silently re-open a false-positive. The
     exclusion is **complete only because the journal records every op type past date-ops** (X11/R5A11): a
     label groom, an auto-repair comment, or the §7/X13 **if-then `description` write** (which Todoist reports
     as a bare `updated`) is journaled too, so none can read as user "initiation." A raw `updated`/`comment`
     event is therefore **not** counted as initiation unless it is a completion or a genuine user touch the
     journal did not author (this kills the false-positive where Bob's own morning `p1`/label groom would read
     as the user "starting" the task);
   - **dismissal** = an explicit defer reply (§4) **OR no activity by the next decision point**;
   - **the v1 receptivity rule (deterministic):** **withhold** (provide-nothing) when the **count of
     dismissals in the trailing 24h ≥ D (D = 3, DECIDED)** **OR** **`surfacings_today` ≥ S (S = 8,
     DECIDED)**; otherwise **proceed**. This is the v1 rule; the calibration loop *refines* the thresholds
     per-person **post-V1** (they start as the decided seeds, §19). *(Implementation notes for the plan's
     calibration task (X11): the `td activity` query is **paginated by cursor** — loop the cursor until it is
     exhausted, never read only the first page (fixtures use the **two-page cursor stub**, R6A8 — a real cursor
     token on page 1 and the genuine user comment on page 2, so a first-page-only read demonstrably misses it); and **comment**
     events come from a **separate** `--type comment` activity query than completed/updated, so both must be
     paged and both cross-checked against the journal, including a **comment-only-progress** case — a genuine
     user comment the journal did NOT author counts as a touch, a Bob-journaled comment does not.)*
4. **Per-task aversiveness signal** — rising `consecutive-roll count` ⇒ the task is aversive/ambiguous ⇒
   change the intervention *form* (decompose / attach an if-then plan), **not** the nag frequency.
5. **Duration-estimate correction** — observed time-to-complete vs `duration_est` per load-class → the
   per-person bias factor feeding §4c.
6. **Habituation index** — the trend in initiation-given-surfacing over rolling weeks; a decline is the early
   warning to vary form/timing/phrasing.

**How it feeds the engine:** the receptivity score gates §4/§6 (low ⇒ provide-nothing or one `@light`); the
initiation curves + activation-decay weight §4c p1-placement (route `@deep` into *this person's* learned
high-capacity window); the duration correction tightens capacity-fit; the aversiveness signal drives the §7
stall response.

**Update math (deliberately simple — the evidence says don't over-engineer):**

- **Recency-weighted, constant step-size:** `estimate ← estimate + α·(observed − estimate)`, **α = 0.15
  (DECIDED)** — effective memory ≈ last ~7 relevant observations; recent behavior dominates, model tracks
  drift (Sutton & Barto §2.5). Fixed, not a runtime knob: behavior is deterministic given the logged data +
  this α. (Real-world RL gives only modest, conditional gains and underperforms a good warm-start for *weeks*,
  Mishra 2021 — a recency-weighted score with good priors captures most of the value.)
- **Cold-start via priors, then specialize** (pool-then-personalize): seed new/sparse cells with stated
  population priors (evening-chronotype tilt, the post-activation EF boost, the work-day window shape) and let
  per-person data take over as it accrues. **Priors are written down + auditable**, not hidden.
- **Cadence:** receptivity/rate decisions live at each decision point; per-person curves updated daily;
  habituation reviewed weekly.

**Anti-patterns the evidence forbids (do NOT build these):**

- **No energy/mood/state self-ratings** (Mayer 2021 — unreliable + burdensome; violates INV-6).
- **No streaks / streak-loss / points / leaderboards / failure-tracking** — the "what-the-hell effect" (one
  slip → total abandonment, Polivy & Herman) + gamification-harms literature; this is the *evidence* behind
  the no-shame invariant, not just ethics. *(Directly implicates the `hermes-achievements` plugin — R6b/§14
  requires it OFF before go-live.)*
- **No self/identity-directed or loss-framed feedback** — >⅓ of feedback interventions *reduce* performance
  when aimed at the self (Kluger & DeNisi); criticism internalizes as shame in ADHD adults (Beaton 2022).
  Keep feedback **task-level, gain-framed, immediate**.
- **No escalating nags on stalled tasks** — re-shape (decompose / if-then), don't pressure; pressure adds
  aversiveness and feeds avoidance (Steel; Sirois & Pychyl).
- **Don't over-prompt or freeze the policy** — a fixed surfacing policy habituated to *zero effect in ~4
  weeks* in HeartSteps; govern prompt rate, vary, back off as the habituation index rises.

**Two evidence-backed mechanisms to USE (positive, not just prohibitions):**

- **Implementation intentions (if-then), agent-proposed** — the highest-leverage low-burden lever (d=0.65 is
  the *overall* effect (Gollwitzer & Sheeran 2006) and d=0.99 the self-regulation-impaired-samples figure
  (**Toli et al. 2016**); the
  ADHD-specific evidence — response-inhibition normalization — is in **children** and reports no pooled *d*).
  When a task stalls or is ambiguous, Bob attaches a concrete "when <cue>, I <first action>" — this is the
  same if-then mechanism §3/§5 already lean on, now applied at the task level. *(Agent-proposed if-then is
  reasoned extrapolation — the studies test user-formed plans — flagged as such.)*
- **Immediate, frequent, task-level, gain-framed reinforcement on completion** — the ADHD reward profile
  responds to immediacy + frequency + response-specificity (Luman 2005; Volkow 2011). A plain "✅ done — that's
  the hard one cleared" beats delayed/effusive praise.

**Calibration parameters — all DECIDED (no runtime tuning):**

- **α = 0.15** (above). **Habituation threshold** = if initiation-given-surfacing for a recurring prompt
  drops below ~50% of its 4-week-prior rate, vary form/timing (a decided rule, not a dial).
- **"provide-nothing" is NOT a base-rate / probability** — it is purely the deterministic output of the §6a
  receptivity gate (low receptivity → withhold). There is no random withholding; given the same signals, the
  same decision. (This resolves the old "tune the base rate" note *toward* determinism.)
- **Stimulant time-course = a config prior, modeled conservatively (CAL Q1-1 — a strong capacity finding).**
  The user takes an extended-release **methylphenidate** daily plus an immediate-release booster PRN; the
  **exact formulation, dose, and dose time are recorded from the user at implementation** (one question then,
  never re-asked) rather than asserted here — the doc carries the *shape* of the prior, not clinical
  specifics. Disposition:
  - **Daily extended-release → a written-down prior** in the owned layer (`forzare/calibration/priors.md`:
    formulation · dose · dose time, default = the 05:15 wake anchor, hand-editable). The time-of-day capacity
    curve is a **coverage RANGE with explicit uncertainty**, NOT a monotonic "ascending all day" curve:
    onset ~1 h post-dose, a broad plasma plateau/peak in the mid-hours (extended-release methylphenidate
    labels put peak concentration roughly **6–10 h** post-dose, then decline — *from training, not verified:*
    confirm against the specific product), and a **late-afternoon/evening wear-off dip** to watch for. Two
    caveats the prior encodes: **peak plasma ≠ peak cognition** (the behavioral effect is domain- and
    dose-dependent, not a clean tracking of blood level), and an **afternoon offset/rebound** window where
    initiation can sag. The learned per-person curve then refines all of this from real initiation data
    (pool-then-personalize, like every prior; auditable, not hidden).
  - **PRN booster → a volunteered signal, never asked** (INV-6 stands — no "did you medicate?" gate). "Took a
    booster" classifies through the `/forzare` energy branch → treat as a ~3–4 h capacity lift from report
    time; logged in the decision record's context like any state signal.
  - **Capacity input only.** Bob never gives medication advice, never nags about dosing — the prior shapes
    *when `@deep` is offered*, nothing else.
- **Sleep/circadian inputs:** OUT of scope for v1 (Bob has no passive sleep source). The post-overnight-shift
  recovery handling (§2) covers the one case that matters without needing sleep sensing. Not deferred —
  explicitly excluded.
- Remaining genuine unknowns are **tool-contract facts** (not behavior choices), in §19 — verified against
  docs/CLI at build, can't introduce nondeterminism in the design.

---

## 7. Nudge / silence rules + stall escalation

The governor on Bob's voice: when to speak, when to stay quiet, what to do on a stall, and what to do on a
win. A thin policy layer over machinery built elsewhere (§6a receptivity, §4d lifecycle ledger, §8 nightly roll).

**Standing rules:**

- Provide-nothing is valid; intervene less but clearly (JITAI receptivity — the §6a receptivity gate computes
  "stay quiet"). A fixed/over-eager prompt policy habituates to zero effect in ~4 weeks (§6a, HeartSteps) —
  silence is a feature, not a failure.
- **Ask before a big reprioritization** (INV-5) — never silently reshuffle the day; structural changes get an
  explicit yes.
- **No-shame is a *task-channel* rule, not a system one** (§0 two kinds of failure; two-channel invariant,
  §9). It governs how Bob speaks to the user about *their* tasks; it never licenses quieting a *system/pipeline*
  failure — those are always loud on the errors channel (§9/§16). **The user's task-slippage and a system
  fault are different events** — a quiet stall is mercy; a quiet outage is a bug.
- **Vary surface form by construction — don't wait to detect decay.** Every recurring prompt shape (nudges,
  stall re-engages, completion beats, the brief's fixed lines §2) rotates its phrasing/format via a
  deterministic design-time rotation over listed form/framing axes — never the same form twice in a row
  (decide-before-runtime compatible; a finite verbatim phrase list would itself re-habituate). The **semantic
  contract stays fixed**: same decision options, no-shame frame, one thing only. Rationale: a fixed prompt
  policy decays to ~zero effect in ~4 weeks (§6a, HeartSteps) — so variation triggered *by* the §6a
  habituation index (a ~50%-drop detector over a 4-week baseline) would arrive at or after full habituation.
  The index stays as **backstop detection**, not the trigger; the §6a receptivity/rate governor already
  covers the over-prompting driver.
- Firm and directive (the user wants a boss), within these bounds.

**Reinforce completions (the positive half — don't only speak on stalls).** On a completion, Bob gives
**immediate, task-level, gain-framed** acknowledgement of the concrete thing just finished — e.g. *"✅ done —
that's the hard one cleared."* The ADHD reward profile responds to immediacy + frequency + response-specificity
(§6a: Volkow 2011, Luman 2005), and frequent small completions are the reinforcement that sustains engagement
(Round-1 Finding 5). **NOT** effusive praise, streaks, points, or scores (§6a anti-patterns — those backfire).
One genuine, specific beat, then move on.

**Stall ladder (no shame at any rung).** Driven by the **consecutive-roll counter** (§4d) — how many nights a
task has been carried forward without progress, read from the private lifecycle ledger (`roll_count`):

1. **Rolled once** (`roll_count == 1`, carried 1 night) → surface normally. One miss is just "didn't get to
   it" — **no special treatment, no flag** (a single carry-over must never read as failure). Nothing is
   visible on the task; the count lives only in the ledger.
2. **Stalled** (`roll_count == 2`, 2nd+ consecutive roll; `last_escalated` guards re-nag) → **change the
   *form* of the intervention, not the pressure** (§6a, evidence-backed): surface as a *decision*, not a nag —
   *"X has been carried a few days — want to break it down, pick a real time, or drop it?"* The default,
   highest-leverage move is to **lower the activation barrier**, two evidence-based forms (§6a):
   - **Decompose** to a tiny concrete first step (raises expectancy — TMT), and/or
   - **Attach an if-then plan** ("when I sit down after the gym, I open the doc and write one sentence") — the
     single best-evidenced low-burden lever (d=0.65 overall, Gollwitzer & Sheeran 2006; d=0.99 in
     self-regulation-impaired samples, **Toli et al. 2016**; the ADHD-specific response-inhibition evidence is
     in children, no pooled *d*).
   Dropping it is always an offered, shame-free option. The choice is the user's. **Never** re-surface a
   stalled task with more frequency or firmer pressure — pressure adds aversiveness and feeds avoidance (§6a:
   Steel; Sirois & Pychyl). *(When a live Discord session is present, this decision is a clarify-button set —
   Break down / Pick a time / Drop — §12 R1c; at brief time it's a plain one-line question.)*

   **Named owner (X13) — the stalled-task branch lives in `todoist-surface`.** When `todoist-surface` would
   surface a task whose ledger `roll_count ≥ 2`, it emits **this decision as the one thing** instead of the
   task (no separate nag path). A chosen **if-then is agent-proposed** — Bob composes a concrete
   "when `<cue>`, I `<first action>`" situational cue and **persists it to the task's *description*** via the
   centralized mutation layer (§4/W6, journaled with `type: description`, X11/R5A11 — the Todoist activity log
   shows it only as a bare `updated`, so journaling it is what keeps the W7 calibration exclusion complete), so
   the plan rides with the task and re-entry is cheap. **Research traceability (X13):** the if-then lever is the highest-evidenced
   low-burden mechanism (d=0.65 overall, Gollwitzer & Sheeran 2006; d=0.99 in self-regulation-impaired samples,
   Toli et al. 2016; ADHD-specific response-inhibition evidence in children, no pooled *d* — §6a). *(This is
   the intra-day path; the same stall decision is also **delivered at the morning brief** as a `stall-decision`
   record in the unified decision queue — §2 step 4/§8a/R5A1 — because EOD only **marks** the `roll_count == 2`
   escalation as state (enqueuing the record) and never messages at 23:00, R4A10.)*

- **Self-forgiveness reduces procrastination; shame deepens it** (Round-1 Finding 9; §6a). Overdue **never
  accumulates into a wall** — in *either* class: the nightly roll (§8) sweeps surfacing-dated tasks forward,
  and missed **fixed** items (user-dated/timed/recurring — non-ledger, hence roll-excluded, §8/W6) each get a
  one-line **morning re-decision** (§8/§2) instead of rotting. Bob never scorekeeps failures, and the count lives in the private
  lifecycle ledger (§4d), never on the task and never as a visible failure score.

---

## 8. End-of-day loop + reconciliation + capture (the closing loop)

**End-of-day** (idempotent cron, **fixed 23:00 daily**, Denver TZ §15): the user is reliably up at 23:00
(work runs until 23:00), so a fixed time is simplest — no schedule-aware computation needed. The *report* half
passes through the §6a receptivity gate, so if 23:00 lands mid-activity it stays quiet and surfaces when the
user next engages; the *state* half (roll/counter/p1-clear) is harmless bookkeeping safe to run anytime
(idempotent, §-below). **Overnight shifts (23:00–07:00):** running the state half at the *start* of an
overnight is fine — it just advances dates; the date-stamp guard prevents a double-roll, and the morning brief
treats a post-overnight morning as recovery, not deep-work, *when the shift is known* (see overnight handling
below).

- Report the day's **completions as wins** — no scorecard, no list of misses.
- **The roll set is DEFINED BY THE LEDGER, full stop (V1 — the single authority).** A task rolls **iff**:
  (a) it has a **lifecycle-ledger entry whose `kind ∈ {surfacing, leadtime}`** (§4d/X5 — a `waiting_checkback`
  or `user_fixed` entry is excluded here, before any date test), **and** (b) the
  task's **current `due.date` still equals the entry's `written_due`** (the self-healing provenance test —
  if they diverge the user re-dated it, so the entry is void and the task is *fixed*), **and** (c) it is
  **date-only** and **today/overdue** and **not done**. That is the whole definition. **A surfacing/leadtime
  ledger entry is what makes a date "surfacing" not "fixed"** — a date Bob wrote to make the task show up,
  versus a date the user owns or a chase reminder. So a **user-dated task never rolls** (no ledger entry, a
  `user_fixed`/`waiting_checkback` entry, or `current != written_due`), with no
  field-heuristic needed to catch it — the `kind` and the divergence *are* the signal.
- **The four field checks are now SECONDARY sanity guards, not the definition** (verified `td` JSON, §19):
  within the ledger-defined set, still skip any task that reads as a fixed commitment — `due.isRecurring ==
  true` (let recurrence set its own next date; and a `td` date-write would destroy the rule — §4/R2A18),
  `"T" in due.date` (an appointment time-of-day — which Bob's date-only hard rule means Bob never writes,
  so a "T" on a ledger task signals the user re-timed it), or `due.date` is a future day. These guard against
  a stale or corrupted ledger entry; they don't *define* membership.
- **Deadline-bearing tasks — the contradiction RESOLVED (V1).** A task carrying a real `deadline` gets a
  **Bob-written lead-time surfacing due** (§4c) — that due **is in the ledger and DOES roll** like any other
  surfacing date (the immovable date stays untouched in `deadline`; only the surfacing due moves). The old
  blanket "`deadline != null` ⇒ exclude" was wrong: it would freeze exactly the lead-time dates Bob creates
  to surface deadline work. **`deadline != null` only means "fixed" for a task with NO ledger entry** — i.e.
  a task the *user* dated on its deadline day. Ledger membership decides; the deadline field alone does not.
- **When uncertain, leave it and flag for the morning** rather than silently moving it (a wrongly-moved
  user date is far worse than a wrongly-kept one). The ledger test removes almost all uncertainty — a task
  with no entry, or a diverged entry, is simply never touched.
- **Fixtures (mirrored in plan B7):** a **user-dated task** (no ledger entry) never moves · a **Bob
  lead-time date on a deadline-bearing task** rolls (entry present, `current == written_due`) · a **user
  re-dated** task (`current != written_due`) never moves, entry voided.
- **p1-clear is BOB-OWNED — scoped to the day's plan record, NOT the whole p1 set (AA2 — its own step, not
  scoped to the roll set either) — AND ownership is re-checked AT CLEAR TIME (EE6).** At EOD, clear `p1` from
  the ids in today's `plan-of-day.json` `selected_ids` — the p1s *Bob* set this morning (roll-excluded ones
  among them included; clearing moves no dates, so it can't corrupt the schedule) — **but membership in
  `selected_ids` is morning-time evidence, and ownership can change intra-day: a user who re-set the priority on
  a selected id during the day has RE-TAKEN ownership.** So, per selected id, EOD first checks the day's activity
  stream for an **intervening user priority event** — a priority-change event on that task **whose `ts` is later
  than the plan record's `created_ts` (FF6 — the plan-write moment is the cutoff; Bob's OWN morning p1-set events
  DO postdate `created_ts`, so it is the journal cross-check below, NOT the cutoff, that excludes them)** that is
  NOT matched by a Bob-journaled `p1.set` in the mutation journal (the
  journal is what distinguishes Bob's own morning write from the user's touch; W7's exclusion machinery, §6a).
  **(Verified: `td activity` `updated` events on a priority change carry `priority`/`lastPriority` in
  `extraData`, FF6 — so a priority-change event is detectable in the stream.)** **No intervening user event ⇒
  clear; intervening user event ⇒ SKIP the clear + enqueue ONE queue flag** (a `stale-p1`-class record for that
  id, so the user decides — never a silent wipe of a priority they deliberately re-asserted). **A `p1` the USER
  set directly in Todoist is NEVER cleared by Bob** (it is not in `selected_ids`) — the old "clear every
  unfinished p1 unconditionally" step would have wiped the user's own priorities nightly. A user `p1` still
  present after 48h is instead enqueued **once** as a `stale-p1` decision-queue record (§2 step 4/§4c) for the
  user to decide — never auto-cleared. **"Once" is enforced by the §2-step-4 producer once-guard (KK3/JJ3):**
  after the user answers (tombstone records the `answer`), EOD does **not** re-enqueue the same `stale-p1` id on
  a later night while its predicate is unchanged (the same user `p1` still older than 48h, answered `keep`); a
  new episode re-asks only if the `p1` was removed and RE-SET. So a stale p1 the user chose to keep is flagged
  exactly once, never nightly. Because the plan record names exactly which ids Bob set (§4c/Y13), the
  morning idempotency guard no longer needs "p1 present ⇒ assigned today."
- **Missed FIXED items don't rot (the roll-excluded overdue path).** EOD *enumerates* the just-closed day's
  roll-EXCLUDED overdue (the just-closed day = CEILING — *today* for the on-time 23:00 fire, *yesterday* for a
  defensive-morning fire, §8's range) — the **non-ledger** set (no entry, or `current != written_due`), plus any ledger task
  the secondary guards skip (`"T"` due / recurring); **`deadline != null` marks an item fixed only when it has
  no ledger entry** (W6 — a Bob lead-time due on a deadline task rolls instead) — and **enqueues each as a
  `fixed-redecision` record to the unified decision queue** (§2 step 4/§8a/R5A1: do late / reschedule / drop —
  a decision, never a do-now; EOD itself reports no misses, per the no-scorecard rule), delivered as the
  brief's head item if it reaches the front. No new *derived* state: "overdue + fixed-shape" is derivable per
  run from the ledger test + task fields (§8a's derived-state model), and the queue record only holds the
  pending decision. Until re-decided, such items are excluded from momentum-mode do-now surfacing.
- **This is the counter's primary increment site (§4d)** — the nightly path for silent non-completion: for
  each task **in the roll set**, increment its `roll_count` in the lifecycle map; at `roll_count == 2`
  **MARK the §7 escalation as state** (stamp `last_escalated`) **and enqueue a `stall-decision` record to the
  unified decision queue** (§8a) — **EOD never messages the stall at 23:00** (the no-scorecard rule holds);
  the decision is *delivered* as the brief's head item (§2 step 4/R4A10/R5A1), or intra-day by
  `todoist-surface`'s stalled-task branch (§7/X13) if the task would surface first. Rolling also re-stamps
  `written_due` to the new
  (tomorrow) date, so tomorrow's roll-set test (`current due == written_due`) still holds unless the user
  intervenes. **Reset rule (precise):** reset `roll_count → 0` when a task shows **progress or completion**
  since it was last surfaced — any of: completed · a subtask completed · a user comment added (all three
  detected from the activity log, §8a) · **user-reported progress ("touched/started")** — which has an
  immediate write, not a deferred signal: **the parent resets the ledger entry at the moment of the report**,
  so EOD needs no extra memory and tonight's roll correctly restarts the streak at `roll_count == 1` if the
  task still doesn't finish. (A user *re-dating* the task in Todoist needs no explicit reset — the
  `current due != written_due` divergence voids the entry automatically, §4d.) **Snooze interplay (canonical
  — mirrors §4 exactly):**
  *within-day* defer = no date write, no tick — tonight's roll is its single increment site if still
  unfinished; *tomorrow*-snooze = ticked at deferral (the today→tomorrow date transition is the dedupe) — EOD
  skips it; *beyond-tomorrow* defer = exits the roll set, no tick, **clears** the streak (handled, not
  stalling). A task merely *carried to a new day with no progress* is the one that accrues.
- Optionally pre-stage tomorrow's candidate ≤3 from the active pool (Bob proposes; confirmed/replaced in the
  morning).

**Missed-fire safety (degraded mode) — corrected to the VERIFIED cron recovery code (V3,
`cron/jobs.py:1456-1492`).** The earlier "±2h bounded, else **skipped**" claim was **false** and is replaced.
What the code actually does when the gateway was down and a recurring job's time passed:

- **Within the catch-up grace window** (`_compute_grace_seconds` = half the schedule period, clamped
  120s–7200s; a **daily** job → the full **2h** cap) → the missed run is treated as **due now and replays**
  (plain catch-up).
- **PAST the grace window** → the code **fast-forwards `next_run_at` to the next future occurrence AND still
  executes the job ONCE now** (`due.append(job)` after the fast-forward, `jobs.py:1462-1492`) — it does
  **not** skip. So an outage of *any* length ends in **exactly one recovery fire**, not a silent miss. This
  is the opposite of the old claim and it *strengthens* the guarantee.

Because a late fire can therefore run at an arbitrary recovery time, the roll must not key off the wall clock:

- **The eod-roll takes an EXPLICIT reconciliation range, not a single wall-clock day (R3A9/W5).** The days
  to close are the half-open range **`(last-reconcile.stored .. CEILING]`** — every day strictly after the
  last-stamped day, up to and including **CEILING**. The **roll destination is `CEILING + 1`**, and the stamp
  advances to **`CEILING`**.
  - **CEILING is set by invocation mode against the Denver-local 23:00 cutoff (X6 — this REPLACES the flat
    "never ≥ today" rule, which was ambiguous for the 23:00 fire itself).** The eligibility principle is
    unchanged (a day is closable only once its own 23:00 has passed), but stated precisely by wall-clock:
    - **At/after today's 23:00 Denver cutoff — the on-time 23:00 EOD (or a ≤2h same-night catch-up):** today's
      task-day is over, so **CEILING = today**; destination = tomorrow; stamp = today.
    - **Before the cutoff — a recovery/defensive morning fire (the 5:15 brief's defensive roll), or a manual
      `/forzare-eod` run earlier in the day:** today is still in progress and un-closable, so **CEILING =
      yesterday**; destination = today; stamp = yesterday.
    Equivalently: **CEILING = today iff the current Denver wall-clock is at/after today's 23:00 cutoff, else
    yesterday** — so a just-past-midnight late fire (Denver "today" has already rolled to D+1, whose cutoff
    hasn't passed) still closes exactly day D. (The bootstrap seed — `last-reconcile.json` = **Denver
    yesterday** at install, plan A1 — makes the first real 23:00 fire close exactly one day.)
  - **Multi-day outage drain — the WHOLE gap in ONE pass.** After an outage the recovery/defensive fire
    (morning) sets **CEILING = yesterday** and reconciles **every** day in `(stored .. yesterday]` in a single
    pass; the surviving unfinished tasks roll to **today** (= CEILING + 1); the stamp advances to **yesterday**.
    Crucially, **`roll_count` ticks EXACTLY ONCE per task for the entire gap**, not once per skipped day — an
    outage is *Bob's* failure, not the user's avoidance, so it must never inflict multi-tick stall-shame
    (§0/§7). One task carried across a 4-day outage reads as a single new-day carry, not a 4-rung escalation.
- **Duplicate-fire / already-reconciled no-op (W5, derivation).** The first candidate day is `stored + 1`; if
  that **exceeds CEILING** (i.e. `stored ≥ CEILING` — every day through the ceiling is already stamped), the
  run's roll set is **empty and it is a no-op** (it logs an *"already-reconciled"* record and advances no
  dates, no counters, no stamp). So `{on-time fire, ≤2h catch-up, past-grace recovery fire, defensive morning
  run}` reconcile a given day **exactly once**, independent of when the job actually fired — a second attempt
  on an already-stamped range simply finds `stored ≥ CEILING` and no-ops. Never double-rolls.
- **Ordering when a stale EOD *and* the morning brief are both due:** the **EOD state half runs first**
  (roll/counter/p1-clear/stamp), then the brief builds the day. The brief's defensive check already
  **waits on the stamp** — it runs the roll itself only if `last-reconcile.json` shows yesterday is still
  un-closed, so the two never race to reconcile the same day.

This removes the dependency on the gateway being alive at exactly 23:00 — and, unlike the old text, it does
so *without* assuming any outage-length ceiling.

**`@waiting` nightly reconciliation** (idempotent cron — e.g. `0 2 * * *` Denver) — **state-only: the 02:00
run never messages the user.** Everything it finds is **enqueued as `waiting-chase` records to the unified
decision queue** (§2 step 4/§8a/R5A1), delivered as the brief's head item when it reaches the front; only a
genuinely time-sensitive chase may be promoted into the head slot (still one item). What it does:

- **Mark chase-due:** any `@waiting` past its check-back date → enqueue a `waiting-chase` record. **The 02:00
  producer enqueues MOST-OVERDUE-FIRST with strictly increasing `enqueue_ts` (R7A11):** it sorts the chase-due
  set by check-back-overdue descending and stamps each record a monotonically increasing `enqueue_ts` in that
  order, so the queue's own total order (FIFO by `enqueue_ts` within the `waiting-chase` rank, §2 step 4)
  delivers the most-overdue chase first — the ordering promise is a property of the enqueue sequence, not a
  re-sort at read time.
- **Repair the §4b set-time invariant:** any `@waiting` with *no* check-back date or blocker note (the
  black-hole case) gets a near-term check-back date auto-set + an "auto-repaired — blocker unclear" flag, then
  enqueued as a `waiting-chase` record for a one-line brief decision. Silent-first: repair now, ask once,
  never let it vanish.
- **Unblock detection — gog calendar + Todoist activity log ONLY (R5A12).** Check each blocker note against
  the two signals an **amnesiac cron session has a verified read path to** — **gog calendar** (the awaited
  event passed?) and the **Todoist activity log** (the blocking task completed? a new comment?, queried on the
  `--type task` AND the separate `--type comment` streams, Z14) — and on a detected unblock, silently clear
  `@waiting` + re-date so it rejoins the normal pool. **The re-date rewrites the ledger `kind`: `waiting_checkback
  → surfacing`** (Z14) — the entry was a chase reminder (never rolls), and once unblocked it becomes a
  surfacing date that **rolls normally**; the re-date goes through the centralized helper, so the new `kind`
  and `written_due` are stamped together and the entry joins the roll set from that night on. **"Recent Discord
  context" is NOT a 02:00 signal** — an amnesiac cron session has no verified read path to chat history, so
  the nightly scan never relies on it. (Email is added as a signal only if Bob later gains read access.) Bob
  still clears **opportunistically mid-conversation** whenever the user or a signal reveals the unblock during
  a *live turn* (which does have the conversation in context) — the nightly scan is the floor, not the only
  path.
- **Staleness sweep:** ~14 days *since the label was applied* (from the activity log) with no movement →
  enqueue a `waiting-chase` record for the same brief slot as a decision (still waiting? chase harder? drop?).

The agent owns the label end-to-end; the user never manages it.

**Low-friction capture:** a one-liner to Bob → a **structured** `td task add "<raw text>"` to the Todoist
**Inbox** (NOT `td task quickadd` — verified v1.75.3: `quickadd`/`qa` runs the natural-language parser that
would pull a date out of the phrase; `td task add` with a positional body stores the text verbatim and dates
only via an explicit `--due`). The Inbox `td task add` therefore **stores raw and NL-parses no date** —
within stage 1, classification comes first, dating second (both are the PARENT's synchronous stage-1 work),
so a phrase like "call the dentist Tuesday" isn't silently turned into an
appointment before Bob has decided task-vs-event. **Inbox is staging, not a store** — a brief transit lane;
the *project* is the canonical home (§8b places it there). The instant Inbox write is the capture's
**nothing-lost ack** — it guarantees nothing is lost even if later processing fails. **The PARENT's synchronous
work is the whole of stage 1 (the Inbox write + placement/classification + dating) plus the bounded `specify`
attempt (stage 2)** — those are the decide-in-context moments, resolved while the user's attention is here
(AA5/BB1/GG10). **Only the research stages — verify, research, split (3–5) — run in the background** (§8b). **Triage** is the *exception*, not the resting state: an Inbox item becomes triage only when
the pipeline needs the user's input (an ambiguous placement or a not-yet-existing project, §8b cases 3–4).
Capture never interrupts the current surfaced task.

## 8a. State & persistence model (where everything lives)

**Bob is almost stateless by design.** An amnesiac fresh session must never trust its own memory — it
re-derives ground truth from Todoist each run. Only a few things persist, in distinct homes. This is
the authoritative map of what reads/writes what; the rest of the spec references it.

| State | Home | Read | Written | Persisted? |
|---|---|---|---|---|
| **Lifecycle MAP** (`roll_count`, `written_due`, `last_escalated`, **`kind`** per task id, §4d/X5) | owned layer `forzare/state/task-lifecycle.json` | EOD roll · morning brief · §4 defer · §7 escalation | agent date-writes stamp `written_due` + `kind` (surfacing/leadtime/waiting_checkback/user_fixed); EOD roll / §4 tomorrow-snooze increment `roll_count`; progress/touched/complete reset; **entry pruned on terminal state** | **yes — small, off the user's tasks (roughly the active/rolled set)** |
| **Mutation JOURNAL** (append-only typed op-history, §4d/X11/R5A11/Y5/**Z3/BB3/CC8**) | owned layer `forzare/state/mutation-journal.jsonl` | §6a calibration exclusion (W7) | **every Bob mutation appends one line in the unified shape** `{ts, created_ts, type, target, op, args, old_value, intended_value, external_marker?, reconcile_date, commit_state}` (CC8 — merged; `created_ts` = intent-journaled time, the propagation-window clock, GG4), `type ∈ date-op/p1/label/comment/calendar/description/**task.add**/**task.complete**/**waiting-clear**/**undate**/**retire**/**bankruptcy** (Z3/BB3/FF1 — `bankruptcy` = the frozen id-set snapshot for a clear); **pending→commit→heal per op type with a type-specific predicate** (§8a/V2/Z3/BB3) | **yes — retained 45 days (calibration window), then pruned; NOT pruned on task completion** |
| **Date provenance** (`current due == written_due`?) | derived: task `due.date` (Todoist) vs ledger `written_due` | every roll-set test (§4d/§8) | n/a — *derived per run* (divergence ⇒ user re-dated ⇒ fixed) | no |
| **Progress-since-surfaced** (reset trigger) | Todoist **activity log** | TWO paginated queries: `td activity --since <d> --type task --json` (completed/updated/added events) **plus a SEPARATE** `--type comment` query (comments are NOT in the `--type task` stream, §6a/Z14) — both cursor-paged to exhaustion | n/a — *derived* | no (queried) |
| **Roll-set membership** (the ledger IS the definition, §8/V1) | ledger entry + `current due == written_due` (Todoist task fields as the secondary sanity guard) | every roll-set test (§4d/§8) | n/a — *derived per run* from ledger∩divergence-test | no |
| **Last-reconcile date** (idempotency + the explicit reconciliation range, §8/V3/R3A9) | owned layer `forzare/state/last-reconcile.json` | morning brief + end-of-day (days closed = the range `(stored .. CEILING]`; **CEILING = today at/after the 23:00 Denver cutoff, else yesterday** — §8/W5/X6) | end-of-day (+ defensive morning run) stamps CEILING; **seeded = Denver yesterday at install** (plan A1) | **yes — one tiny file** |
| **Unified decision queue** (ALL brief-time decisions, §2 step 4/§4c/X7/**R5A1/Y1/Z2/AA4/BB2**) | owned layer `forzare/state/decision-queue.json` (`{id, class ∈ q1-conflict\|waiting-chase\|fixed-redecision\|stale-p1\|stall-decision\|triage-reraise\|sweep-candidate\|bankruptcy-offer, task_id\|candidate_id\|aggregate-key, proposed, status, enqueue_ts, gen, rev, head, journal_ref, answer?}` per record (the ONE canonical schema, DD4/§2 step 4/plan B0; `status ∈ {pending, tombstoned}`; **`journal_ref` = nullable mutation-journal uuid the ack consumes, populated for `bankruptcy-offer` + the ambiguous-window `triage-reraise`, II3 — the ack reads the frozen snapshot by this EXACT uuid, never a month search**; **`answer` = the user's recorded decision (`keep`/`drop`/…) written on the tombstoned record so the producer once-guard never re-asks an unchanged-predicate `keep`, KK3/JJ3**); **`id` = STABLE content-INDEPENDENT key — per-task classes `class:task_id`, AGGREGATE classes `q1-conflict:<date>` / `bankruptcy-offer:<YYYY-MM>` (BB2); total order `(head DESC, class-rank, enqueue_ts, id)`** with promotion participating in the order via the `head` flag, class-rank q1-conflict>waiting-chase>fixed-redecision=stale-p1>stall-decision>triage-reraise>sweep-candidate>bankruptcy-offer, AA4/R6A10) | the morning brief (emits ONLY the single HEAD `pending` record as its one decision, replacing the do-now close) | **producers (state-only, never message):** 02:00 `waiting-reconcile` (waiting-chase + repairs) · EOD `eod-roll` (fixed-redecision + stall-decision + stale-p1) · morning `eisenhower-plan` (q1-conflict) · §8b capture pipeline (triage-reraise) · monthly SWEEP `followups-sweep` (sweep-candidate + bankruptcy-offer) — **producers dedup by `id`: a re-enqueue of an unchanged decision is a no-op, a changed `proposed` updates IN PLACE + `rev++`; ALL mutations under the same lock/atomic-replace contract as the map/journal (Z2)**. **Ack = a LIVE-ONLY compare-and-set on `{id, gen, rev}` that TOMBSTONES the record (R5A5/Z2/BB2):** the live turn that resolves a decision (the shown brief head OR any record settled intra-day, CC10) CAS-flips the record's `status` to `tombstoned` IN PLACE (`gen` unchanged, no separate object, GG5); a moved `gen`/`rev` fails the CAS and the turn re-reads; a re-enqueue of a tombstoned `id` **reuses that record**, resetting it to `status=pending`, `gen+1`, `rev=1`, `head=false`, fresh `enqueue_ts` (II6); **never a dry-run write** | **yes — one small queue file** |
| **Sweep-exclusion list** (bankruptcy RETIRE for undated items, §4c/**Z13**) | owned layer `forzare/state/sweep-exclusion.json` (a set of task ids the monthly sweep never re-proposes) | monthly SWEEP `followups-sweep` (excludes these from the candidate pool) | RETIRE appends an id (reversible by deleting the entry; no label/delete/re-parent) | **yes — one small file** |
| **Per-day plan record** (idempotency guard, §4c/§15/**Y13**) | owned layer `forzare/state/plan-of-day.json` (`{date, created_ts, selected_ids[], anchor, writes:{p1_set, anchor_placed, alarm_set}}`; `created_ts` = plan-write time, the EE6/FF6 intervening-user-event cutoff) | the morning brief's `eisenhower-plan` (resume-missing-writes guard; distinguishes Bob-owned p1 from user-set p1) | written *as the narrowing executes* — each `writes.*` flag set when that write lands; a re-fire resumes only the missing writes | **yes — one tiny file per day** |
| **Tomorrow pre-stage** (EOD proposal → morning brief, §8/R2A8) | owned layer `forzare/state/tomorrow-prestage.json` (≤3 candidate task ids + one anchor candidate) | the morning brief (consumes it, then **clears** it) | end-of-day's `tomorrow-prep` writes it (proposal only — **no** `p1`, **no** calendar write) | **yes — one tiny file, cleared each morning** |
| **Schedule override + gym activation** (§2/§3/§6) | owned layer `forzare/state/schedule-override.json` (shift block · date · recovery-morning flag · today's `activation` field) | morning brief + end-of-day + gym-window-end check + the `/forzare` skill | shift override set by `/forzare` shift signal (consumed on the day *after* the block ends; a mid-shift 5:15 brief reads without clearing); the **date-scoped `activation` field** is set when the gym-back signal fires, so the gym-window-end cron (§3, an amnesiac session) knows the signal already came | **yes — one tiny file** |
| **Dry-run intents** (§17/R3A1 — the ONLY file a dry-run writes) | owned layer `forzare/state/dryrun-intents.jsonl` | staging assertions (the intent RECORD is the evidence) | appended by every mutating skill *instead of* its real write when the dry-run instruction is active; truncated at go-live | **staging-only — never present in a live run** |
| **Go-live flag** (staging↔live boundary, §14 scan (d)/CC3) | owned layer `forzare/state/go-live.json` (`{gone_live: bool, ts}`) | the forzare-ops watchdog's ritual-absence scan (§14 scan (d) — **pre-go-live it LOGS informationally, post-go-live it ALERTS**), and any skill that must know it is live | written **once at go-live** (plan G1 Step 4); absent/`false` ⇒ still staging | **yes — one tiny flag file** |
| **Staging test-overrides** (CC4 — test-only fields, honored ONLY in staging) | injected into `forzare/state/schedule-override.json` under the dry-run/staging directive: **`pinned_schedule`** (a fixed work/off schedule for eisenhower-plan/brief-assemble), **`synthetic_weather`** (a fixed forecast/breach for weather/brief-assemble), **`FORZARE_NOW`** (a wall-clock override for eod-roll's cutoff/ceiling math and any time read), **`activity_stub`** (a path to a synthetic `td activity` stream for followups-sweep's 30-day-inactivity eligibility reducer, GG11) | eisenhower-plan · brief-assemble · eod-roll · weather · followups-sweep — **each honors these ONLY when the staging directive is active; ignored in production** (§8a/CC4/CC12/GG11) | written by a staging test harness (never a live run) | **staging-only — never read in a live run** |
| **Fire times** (§19 decided config) | `forzare/` config + fixed 23:00 | read | hand-edited | config |
| **Calibration log** (§6a — learning) | owned layer `forzare/calibration/` | aggregate analysis (daily/weekly) | appended per surfacing decision | **yes — the learning dataset** |
| **Goals yardstick** (§4c) | owned layer `forzare/goals.md` | p1 time | hand-edited ~quarterly | yes (human-owned) |

**The two kinds of memory, kept separate:** *control state* ("what's true now" — the lifecycle ledger,
last-reconcile date, schedule override) is read every run and acted on immediately; the *calibration log*
("what tends to work for me") accumulates and is analyzed in aggregate to tune decision rules (§6a) — never
user-facing, never self-report. **Everything Bob persists now lives under `forzare/`** (lifecycle ledger,
state-stamp, calibration, goals, dopamine-menu) — nothing failure-shaped is written **on the user's Todoist
tasks** anymore (§4d). Bob holds no other *knowledge* state between sessions.

**Ledger I/O contract (V2/Y5/Z2 — the lifecycle MAP, mutation JOURNAL, and decision QUEUE are crash-safe,
single-writer stores).** The `task-lifecycle.json` map, the `mutation-journal.jsonl` journal (§4d/Y5), **and the
`decision-queue.json` queue (§2 step 4/Z2)** are touched by concurrent paths (the 23:00 EOD roll, a live
`/forzare` snooze, a §7 escalation, a producer enqueue vs a live ack), so their reads/writes go through **one
shared helper** with these guarantees — never ad-hoc `json.load`/`json.dump` scattered across skills. **The
journal-then-commit write order + healing rule below are defined for EVERY journal op type** (`date-op`, `p1`,
`label`, `comment`, `calendar`, `description`, **added Z3 — `task.add` + `task.complete`**, **added BB3 —
`waiting-clear` + `undate` + `retire`**, **added FF1 — `bankruptcy`** (the frozen id-set snapshot) so the composite unblock and the bankruptcy ops are journaled too and
can't read as user initiation under W7; the enum now aligns with the intent-op vocabulary), not only date-ops;
the date-op sequence is the worked example. **The queue obeys the same lock + atomic-replace, and its ack is a
compare-and-set on `{id, gen, rev}` that TOMBSTONES the record IN PLACE** (Z2/BB2/GG5 — the `status` flips to
`tombstoned` on the retained record, `gen` unchanged; a re-enqueue of a tombstoned `id` **reuses that record**,
resetting it to `status=pending`, `gen+1`, `rev=1`, `head=false`, fresh `enqueue_ts` (II6)):

- **Exclusive lock — ONE state-layer sentinel, crash-auto-released (KK1/JJ11 — supersedes the mkdir/PID/TTL
  machinery).** Every read-modify-write holds ONE lock for the whole **map+journal+queue** critical section — a
  single state-layer-wide sentinel file **`task-lifecycle.lock`** — acquired through the **B0 helper's python
  shim via `fcntl.flock(fd, LOCK_EX)`**. **`fcntl.flock` is verified AVAILABLE on this macOS host** (`python3 -c
  'import fcntl'` succeeds; what is absent is the shell `flock(1)` binary, not the syscall — the earlier "flock
  verified ABSENT" reading conflated the two). Because the advisory lock is held on an OPEN FILE DESCRIPTOR, the
  **kernel releases it automatically the instant the holding process exits or is SIGKILLed** — so there is **no
  PID file, no TTL, no stale-lock sweep, and no `mkdir` sentinel**: a crashed holder can never leave the lock
  wedged, and the crash-heal contract composes with it (the next run simply re-takes an already-released lock).
  A live snooze and the nightly roll still can't interleave a lost update, because the whole critical section is
  serialized under the one lock.
- **Atomic writes.** Write to a temp file in the same directory, `fsync`, then `os.rename` over the target
  (atomic on the same filesystem) — a crash mid-write never leaves a truncated/corrupt ledger; the reader
  sees either the old file or the new one.
- **Operation record lives in the JOURNAL, not the MAP (AA3 — the B0 contradiction swept; ONE unified shape,
  CC8).** Each mutation is a single **journal line** in the shape stated identically in §4d, here, and plan B0 —
  **`{ts, created_ts, type, target, op, args, old_value, intended_value, external_marker?, reconcile_date, commit_state}`**
  (CC8 merges the two earlier partial schemas; `created_ts` is the propagation-window clock, GG4) — never a history array on the map entry, so **the MAP keeps its
  4-field schema** (`written_due, roll_count, last_escalated, kind`). The journal record is the idempotency
  substrate under the date-stamp (a run reads the journal to tell whether *it* already applied a given day's
  roll).
- **Write ORDER — journal, then commit (decided).** For each mutation the sequence is: **(1)** write the
  *intent* first (journal the pending record — `{old_value, intended_value, external_marker?, reconcile_date}`
  with a `pending` flag), **(2)** perform the real write via the centralized layer (for a date-op: the
  state-chosen verb §4/W6 — a roll re-dates an already-dated task, so `td task reschedule`; the same helper
  uses `td task update --due` when *initially* dating an undated task), **(3)** commit the entry (flip
  `pending`→committed, stamp the new value). A crash **between (1) and (3)** leaves an *intent without a
  commit*.
- **THREE-WAY healing rule (AA3 — absent / intended / OTHER).** On the next run, any `pending` (un-committed)
  entry is **re-verified against Todoist** by reading the target's live value and comparing it against BOTH the
  journaled `old_value` and `intended_value` (and the `external_marker` where one exists):
  - **matches `old_value` (write ABSENT)** — the write never landed → **re-apply** step (2), then commit
    (replay).
  - **matches `intended_value`** — the write landed → **commit** the entry (heal forward).
  - **matches NEITHER (OTHER / user-changed)** — a value the user set since Bob journaled the intent →
    **ABORT the replay and FLAG it (never overwrite user state):** void the entry (the §4d divergence rule) and
    surface it as a decision/error rather than silently clobbering the user's value. This is the load-bearing
    difference from a two-way heal — a silent replay onto a user-changed value would destroy the user's edit.
  Net: no op is ever double-applied, none is silently lost across a crash, and a user's concurrent edit is
  never overwritten.
- **PER-TYPE healing predicate (Z3/AA3 — the three-way heal is defined for EVERY `type`, each with a
  type-specific "which of old/intended/other does the live value match?" check):**
  - **`date-op`** — live `due.date` vs journaled `old_due` / `new_due` (the divergence test above).
  - **`comment`** — re-read the task's comments; landed iff a comment with the journaled content exists on the
    target task (content + task lookup); absent iff no such comment.
  - **`calendar`** — look the 🤖-calendar event up **by its stable event key**; landed iff the keyed event
    exists with the intended fields; the key IS the `external_marker`.
  - **`label` / `p1` / `description`** — compare the task's **current value** to `old_value`/`intended_value`
    (label set contains/omits the target label · `p1` present/cleared · description equals the if-then cue).
  - **`task.add` — NO native idempotency (verified: `td task add` exposes no dedup/idempotency flag, only
    `--dry-run`), so dedup is ONE explicit five-step state machine (EE1) — a pre-persisted journal intent + a
    create-time marker + an immediately-journaled returned id, NOT a content search (BB3 — corrects AA3's
    content+project search, which mis-heals on a collision, a rename, or a project move).** The five steps, in
    order under the lock: **(1)** the helper generates a `journal-uuid` and **persists the intent
    `{uuid, content, project}` as a `pending` journal record (uuid as the `external_marker`, `created_ts` stamped)
    BEFORE the API call** (a `pending` write, NOT a commit — the healer scans exactly these `pending` records, GG4/FF3); **(2)**
    the **`td task add` API call**, which carries a hidden **`⟦fz:<journal-uuid>⟧`** line in `--description`, so a
    task that lands atomically carries the marker; **(3)** the returned **task id is journaled into the intent
    record IMMEDIATELY** on return; **(4)** the marker is **stripped** from the description on commit-verify (so
    the user never sees it); **(5)** the entry is **committed**. **Healing on the next run, for any `pending`
    `task.add` intent (this is the one machine — EE1 supersedes the earlier absent⇒replay / no-marker⇒abort
    contradiction, which conflated two different windows):**
    - **journaled id present (step 3 done)** ⇒ **verify BY ID** — the id resolves ⇒ strip marker + commit; the id
      no longer resolves (the create was rolled back) ⇒ replay step (2).
    - **no journaled id, marker FOUND** (search Todoist for the `⟦fz:<journal-uuid>⟧` marker) ⇒ the create landed
      but we crashed before journaling the id ⇒ **resume from step 3** (journal the id, strip marker, commit). A
      marker found on a task whose content/project has since diverged still heals correctly — the marker, not the
      content, is the identity.
    - **no journaled id, marker NOT found, AND the intent is OLDER than the API-propagation window** (its
      `created_ts` predates the **120s** propagation window, §19, so a landed task should already be searchable)
      ⇒ **REPLAY FAILS CLOSED (II5): replay ONLY after TWO consecutive empty-SUCCESS marker searches ≥30s apart
      both confirm absence.** A **single** miss, or **any search ERROR**, ⇒ do NOT replay — **ABORT + enqueue the
      one-tap `triage-reraise` re-confirm** instead. Elapsed time alone never authorizes a replay: an indexing lag
      past the window must not be misread as a true absence (the duplicate is the worse failure).
    - **no journaled id, marker NOT found, but the intent is WITHIN the 120s propagation window** — **the ONE ambiguous
      window** (the API may have returned and the id-journal crash raced the create; a search miss could be index
      lag rather than a true absence) ⇒ **AT-MOST-ONCE decided: ABORT + enqueue a one-line queue decision, NEVER
      auto-duplicate.** A duplicate is the worse ADHD failure (two identical tasks re-inflate the backlog and
      re-fragment attention), so Bob refuses to replay under ambiguity and instead asks the user to re-confirm the
      capture in one tap (a `triage-reraise`-class `decision-queue.json` record whose `id` is
      **`triage-reraise:<journal-uuid>`** — keyed on the journal-uuid since no `td` task id exists yet, FF10, §2 step 4).
    So past the window a replay fires **only on a DOUBLE empty-success search** (II5) and within the window it never
    fires — both paths **fail closed toward the one-tap re-confirm**, never toward a blind duplicate. There is **no Todoist-side
    idempotency key** to look up (the Inbox-task-id idempotency key is Kanban's, on the *card*, not the `td` task).
    This replaces §8b stage-1's content+project dup-guard with the same state machine.
  - **`task.complete`** — read the task's state: landed iff the task is completed.
  - **`waiting-clear` (composite, AA3)** — the 02:00 unblock's clear-`@waiting` + re-date + provenance-flip
    (`kind: waiting_checkback → surfacing`, §8/Z14) is journaled as **ONE composite pending transition**, healed
    atomically: landed iff `@waiting` is absent AND the new surfacing `due.date` equals `intended_value` AND the
    map `kind` is `surfacing`; a partial (any one of the three still in its old state) re-applies the whole
    transition; a user-shaped OTHER voids + flags. Never a half-applied unblock.
  - **`undate` (bankruptcy UNDATE, BB3)** — the stale dated active's due is stripped: landed iff the task's
    `due` is now null; absent iff it still carries `old_value`; OTHER (a user re-dated it since) voids + flags.
  - **`retire` (bankruptcy RETIRE, BB3)** — the undated someday id is appended to `sweep-exclusion.json`: landed
    iff the id is present in the list; absent iff not (re-append). A state-file op, so its "live value" is the
    exclusion list, not a Todoist field.
  - **`bankruptcy` (the frozen id-set snapshot, KK7 — COMMITTED-AT-WRITE, NO pending phase; the generic
    three-way heal does NOT apply to this type).** The snapshot is written as **ONE atomic append of the
    COMPLETE record** (every frozen id + its per-item op + the `journal-uuid`), never a journal-then-commit
    pair — so it is either **wholly absent or wholly present**, never half-written. There is therefore no
    `pending` state to heal three ways; recovery is binary: **absent ⇒ the offer never froze** (no cohort was
    committed, safe to re-offer — the next sweep re-freezes from the live sweep pool); **present (complete) ⇒
    recovery reads the frozen cohort back and drives the per-item `undate`/`retire` ops** (each of which heals
    under its OWN predicate above). **Immutable-cohort recovery invariant:** once committed the frozen id set is
    **read-only** — a resumed clear reads exactly the ids that were frozen and never re-derives a different
    cohort, so an interrupted clear resumes over the SAME cohort and never processes a task the offer did not
    name. (The generic three-way `date-op`-style crash fixture for this type is REMOVED — a committed-at-write
    snapshot has no absent/intended/OTHER live value to compare, KK7.)
  In every case (except the committed-at-write `bankruptcy` snapshot above): `intended_value` match ⇒ commit;
  `old_value` match ⇒ re-apply then commit; OTHER ⇒ abort + flag (void), never overwrite user state.
- **Fixtures (mirrored in the plan's ledger-I/O task):** crash after (1)/after (2)/after (3); **one
  after-write crash fixture per journal `type`** (`date-op`/`comment`/`calendar`/`label`/`p1`/`description`/
  `task.add`/`task.complete`/`waiting-clear`/`undate`/`retire`) **exercising ALL THREE heal outcomes** — live
  value = intended ⇒ commit · = old ⇒ re-apply · = OTHER (a seeded user-changed value) ⇒ **abort + flag, user
  value NOT overwritten** (AA3); the `task.add` fixture asserts the **five-step state machine with ONE FIXTURE
  PER HEALING WINDOW (EE1)**: (i) **journaled-id present** ⇒ verify-by-id commits; (ii) **no id, marker FOUND**
  under the **collision / rename / move** cases — a same-content sibling (collision), a task renamed after create,
  and a task moved to another project — each of which the marker still resolves where a content+project search
  would mis-heal ⇒ resume-from-step-3; (iii) **no id, no marker, intent PAST the propagation window** ⇒ REPLAY only
  after a **DOUBLE empty-success marker search ≥30s apart (II5)**, with paired negatives (a single miss and a
  search error each ⇒ ABORT + re-confirm, ZERO replay); (iv) **no id, no marker, intent WITHIN the window** (the ambiguous window) ⇒ **ABORT +
  enqueue a one-line re-confirm decision, asserting NO auto-duplicate `td task add`** (at-most-once); the
  `waiting-clear` fixture asserts
  the **composite** transition heals atomically (a partial re-applies the whole clear+redate+flip); the `undate`
  and `retire` fixtures assert the bankruptcy ops heal (undate landed iff due null · retire landed iff on the
  exclusion list); the **`bankruptcy` snapshot fixture is COMMITTED-AT-WRITE-shaped, NOT three-way (KK7)** — a
  crash *before* the atomic append leaves the snapshot **absent ⇒ the offer is re-offered** (no cohort
  committed), and a crash *after* it leaves the snapshot **complete ⇒ recovery reads the SAME frozen cohort and
  drives the per-item ops idempotently** (immutable-cohort: the resumed clear never re-derives a different set);
  a **SIGKILL-mid-critical-section** fixture asserts the `fcntl.flock` lock is **auto-released** by the kernel so
  the next run re-takes it and heals with no stale-lock sweep (KK1); a concurrent **EOD roll × live snooze** on the same task id (the lock serializes them; the
  loser re-reads and no-ops via the same-day dedupe); and **decision-queue concurrency fixtures (Z2/AA4/BB2):**
  a **producer race** (two producers enqueue the same `id` → one record, the second a no-op), a **duplicate
  reconcile** (a re-enqueue of an unchanged decision is a no-op), an **in-place update** (a producer re-touches
  an existing `id` with a *different* `proposed` → the SAME record updates and `rev` increments, no duplicate),
  an **ack-vs-promotion race** (the head's `gen`/`rev` moves between read and ack → the CAS fails and the turn
  re-reads rather than tombstoning a stale head), an **ack-then-reenqueue** (ack tombstones `{id, gen 1}`; a
  later re-enqueue of the SAME `id` opens a fresh `gen 2`, `rev 1` record — not suppressed by the stale ack), a
  **delayed-answer** (a chase acked a day after it was shown ⇒ the tombstone prevents a re-ask; a genuinely new
  occurrence re-asks under `gen 2`), and a **non-head intra-day resolution (CC10)** — a `stall-decision` settled
  mid-day via `todoist-surface` ⇒ the same live CAS tombstones it ⇒ the next brief does NOT re-ask it.

**The one transient exception — in-flight agent work.** A capture-processing job (§8b) that's mid-flight is
execution state, not knowledge — it lives on the **Kanban card** (`~/.hermes/kanban.db`, §10), Bob's private
work substrate. It is **not** in the table above because it isn't ground truth Bob re-derives; it's a job in
progress. Crucially, **Kanban has no mid-run resume — a crash restarts the whole card from its first stage,
not the failed step** (verified, §19), so every stage is written **idempotent + check-before-create** (§8b
dup-guard): a restart re-reads the Inbox task (stage 1's durable output, written by the parent) and re-runs
the rest, producing no duplicate Todoist task. The durable ground truth a restart re-derives from is still
just the Inbox task + the card's own input.

## 8b. Capture-processing pipeline (brain-dump → placed, researched, optionally split)

A capture is rarely "done" when it lands — a one-liner usually needs a home, sometimes research, sometimes
splitting into a task + subtasks. That processing is **staged, background, and gated** so the **parent agent
stays free for the user** (decide-in-context: Bob must be available *now*, not blocked on plumbing).

**Trigger + ownership — placement INTELLIGENCE moves to the parent (AA5, decide-in-context).** The user
captures (`/forzare-capture` or plain language). **Parent Bob** does, synchronously: **stage 1** (the Inbox
write, instant ack, nothing-lost) **AND the placement/classification decisions (the four routing cases below)**
— because those are exactly the *decide-in-context* moments the ADHD design wants resolved while the user's
attention is here, not deferred to a background subagent. The parent then **creates the card** and runs a
**bounded `specify` attempt** (see the kickoff below — short on the happy path; on timeout it degrades to
"capture saved; processing delayed" + a persisted cron retry). **The remaining research stages (3–5) run in the
BACKGROUND**, as fresh-context **default-profile** work (§10), without holding the conversation.

> **Persona vs. profile (load-bearing for every Kanban `--assignee` / `default_assignee` below).** "**Bob**"
> is the **persona** — the `SOUL.md` character and the `bob → hermes -p default` wrapper alias — running as
> the hermes-agent **`default` profile** (verified: `hermes profile list` shows `default` as the marked
> default; there is **no** profile literally named `bob`). So every assignee and `default_assignee` in this
> spec is the profile **`default`**, and Kanban's own no-assignee fallback already resolves to `default`.

**Concrete kickoff — the Inbox ack loses nothing; `specify` is a BOUNDED synchronous act, supervised by a
persisted retry (BB1 — corrects the AA5 "detached fire-and-forget" framing, which claimed a supervision Hermes
cannot give a non-dispatched call).** Stage 1's Inbox write is the instant, nothing-lost ack. The PARENT then
makes the placement decision, creates the card, and runs **ONE bounded `specify` attempt** — it does not
fire-and-forget it:

1. **`hermes kanban create "fz-capture: <title>" --triage --created-by forzare --idempotency-key <inbox-task-id> --assignee default
   --max-runtime 900 --skill forzare-capture-pipeline`** — parks the card in the **`triage`** column (a
   `--triage` card is **not dispatchable**; verified — the dispatcher spawns only `ready` cards,
   `has_spawnable_ready`, §18). The **`--created-by forzare` stored column is the forzare-card discriminator** (MM1 —
   the private board is shared across profiles, §9): `--created-by` is a stored, IMMUTABLE, filterable per-card
   column (`kanban_db.py:1019`, verified `--help`, FF9/HH5), so both watchdog scans read `created_by == "forzare"`
   from `hermes kanban list --json` (the CLI exposes no `--created-by` filter flag — the scans jq the JSON field).
   The **`fz-capture: ` title prefix is now DISPLAY-ONLY** — human-legible, but NO LONGER a discriminator, because
   `specify` **provably rewrites the title** (`specify_triage_task` atomically updates title/body, `kanban_db.py:4574`),
   so a title-prefix scan would miss every specified card; the earlier "chosen discriminator" framing is swept
   (JJ8/HH5/MM1). Both the watchdog stale-triage AND run-failure scans (§14 (b)/(e)) filter on `created_by ==
   "forzare"` so non-forzare triage/failure cards never alarm. `title` is a
   **REQUIRED positional** (verified `hermes kanban create --help`; a
   bare `create --triage` fails), seeded from the parent's placement decision. **`--max-runtime 900` (DECIDED,
   Y7/§19)** caps each card at 900s; on exceed the dispatcher SIGTERMs→SIGKILLs (5s grace) and re-queues the
   whole card (`timed_out`, §16). **The idempotency key is the Inbox TASK ID** (stage 1's durable output), not a
   fresh uuid — so a full parent retry *or* a Kanban no-resume restart both re-derive the **same** key and look up
   the **same** card. This key dedupes **best-effort**: hermes' own `kanban create` runs the idempotency lookup
   BEFORE the write transaction (`hermes_cli/kanban_db.py:2385-2389` — "Race is acceptable: two concurrent
   creators with the same key might both insert … the next lookup stabilises"). **The design guarantee that
   forzare never hits that race is that the PARENT is the single capture-writer, SERIALIZED per Inbox task id** —
   only one `kanban create` is ever in flight for a given key (a retry runs only after the prior attempt
   returned), so the concurrent-insert window never opens (§15). Stage 1 writes the Inbox task first, so the id
   exists before the card. **The `forzare-capture-pipeline` skill is ATTACHED via `--skill`** (W4) — the card's stages are that
   one installed skill's content (loaded through `job.skills`, the same expansion path as cron `--skill`,
   §11/W1), not free-form improvisation; it is not a bespoke plugin. **Create is instant; the bounded `specify`
   attempt (item 2) follows on the parent's path — short on the happy path.**
2. **`hermes kanban specify <task_id>` — a BOUNDED synchronous attempt, supervised by a persisted cron retry
   (BB1, NOT a detached fire-and-forget).** The parent runs `specify` with a short bound (a cheap Haiku
   `auxiliary.triage_specifier` call, §14, so the happy path completes in ~a second): it concretizes the terse
   one-liner ("insurance thing") into a real title + body **and performs the `triage → todo` transition that
   PERMITS dispatch** (verified `kanban_specify.py` / `specify_triage_task`, `kanban_db.py:4574` — atomically
   updates title/body and sets `status = todo`; verified it **requires** the card be in `triage`, so it is what
   releases the card, and `auto_decompose: false` means nothing else auto-specifies it, §14). **On success** the
   card is released and the parent returns. **On failure or timeout** the card **STAYS in `triage`** — **never
   marked `blocked`** (a parent-run `specify` is NOT a dispatcher-claimed worker, so it emits no
   `gave_up`/`crashed`/`timed_out` event and Kanban cannot auto-retry it; the earlier "retried on transient
   failure" / "raises a failure event" claims were **ungrounded and are DELETED**) — the parent says **one honest
   line, "capture saved; processing delayed"**, and schedules a **ONE-SHOT `--no-agent` cron job that retries
   `hermes kanban specify <id>`** (verified `hermes cron create --no-agent --script`): a *persisted, genuinely
   supervised* retry whose own non-zero exit lands in the watchdog's failed-run scan (§14 scan b). The
   **stale-triage watchdog scan (§14 scan e)** — any forzare capture card still in `triage` past its create time
   + a 30-min grace ⇒ an errors-channel alert — is the final backstop. `specify` is still MANDATORY (a vague card
   never routes on a raw fragment); its supervision is now the **bounded attempt + the persisted cron retry + the
   stale-triage backstop**, not a claim Hermes can't back.

**CLI transport is a HARD RULE of the pipeline skill's contract (Z1).** The `forzare-capture-pipeline` skill
creates every card through the **CLI `hermes kanban create`** — **never** the in-gateway kanban *tool* — because
the CLI create path is **verified subscription-free** (`hermes_cli/kanban.py` never calls
`_maybe_auto_subscribe`), whereas the tool path on a platform-bound session auto-subscribes the chat to the
card's terminal events (`tools/kanban_tools.py:843,858-898`, gated by `kanban.auto_subscribe_on_create`, default
`True`). Belt-and-suspenders, the config pins that key **`false`** (§14 drift #3) so even a stray tool-path
create writes no subscription row. The acceptance is mechanical: after a test capture, the kanban subscription
table (`kanban_notify_subs`) has **no row** for the created card (plan D1).

**NO card subscription — the `notify-subscribe` callback design is DELETED (Y2).** The earlier "step 3"
`hermes kanban notify-subscribe` was both a firewall breach and a category error, verified against
`hermes kanban --help`: `notify-subscribe` "Subscribe a gateway source to a task's **terminal events**"
(completed / blocked / gave_up / crashed / timed_out) — there is **no decision-event routing** and **no
reply-to-card correlation** (the claimed "terminal + decision events" does not exist), the subscribed events
land on the **home (task) channel** — exactly the agent-plumbing leak §9 forbids — and a card is dispatchable
*before* a subscribe call could even land (a routing race). So **capture cards carry no subscription at all.**
Instead, when a stage needs the user (cases 3–4 below) the card **blocks awaiting-user** and the pipeline
**enqueues a `triage-reraise` record to the unified decision queue** (§2 step 4/§8a/R5A1) — a state-only
write, no message. The brief delivers it as its head item when it reaches the front, and any live
Discord-bound turn re-raises it opportunistically (the parent owns the inline clarify-button / one-line ask,
§12.1c). On the user's answer the live turn writes it onto the card (a comment/field keyed by card id) and
**unblocks** the card (`hermes kanban unblock`), which lets the dispatcher resume the pipeline; the same turn
TOMBSTONES the queue record via the `{id, gen, rev}` CAS (R5A5/BB2). **Pipeline FAILURES** (a crashed / timed-out / gave-up card) reach
`#forzare-errors` via the **forzare-ops watchdog** (§14/§16), never a card subscription — so no message issues
on the user-facing TASK channel before Phase G go-live (the `#forzare-errors` route + the Checkpoint-A/F1 send
probes are sanctioned staging traffic, §16).

`create` and `specify` are first-class `hermes kanban` verbs (verified flags + watcher behavior). Stage 1's
Inbox write is the instant nothing-lost ack; the bounded `specify` attempt runs on the parent's path (short on
the happy path, or degrades to "capture saved; processing delayed" + a persisted cron retry on timeout); the
research stages 3–5 run async in the background.

**The five stages — each gates the next, so the pipeline short-circuits (AA5: PLACEMENT is stage 1, on the
parent; `specify` is the parent's bounded second act, BB1):**

| # | Stage | Does | Gate to next |
|---|---|---|---|
| 1 | **Place + DECIDE placement** (PARENT, sync — AA5) | Structured `td task add "<raw>"` to **Inbox** (staging) — **no date parsing** (never `quickadd`) — **AND** the task-vs-event pre-check + the 4 routing cases (decide-in-context) + dating time-bound captures via the centralized date-mutation layer (§4/W6). Idempotent: skip if this capture is already there. | placed → card create + specify · event → 🤖 calendar, **done** · needs user input → **block awaiting-user** (cases 3–4) |
| 2 | **Specify** (PARENT, BOUNDED sync — BB1) | Concretize the title/body via `auxiliary.triage_specifier` + the `triage → todo` transition that releases the card; on timeout/failure the card stays `triage`, the parent says "capture saved; processing delayed", and a one-shot `--no-agent` cron job retries `specify` (its failure → watchdog scan b; stale-triage scan e is the backstop, §14). | → 3 |
| 3 | **Verify + research-decision** (background) | Confirm the placement is sane; decide **does this need research before it's actionable?** | research-worthy → 4 · not → **STOP (done)** |
| 4 | **Research** (background) | Investigate (web / vault / `/deep-research` as warranted); decide whether the result implies subtasks. | implies subtasks → 5 · not → **STOP (done)** |
| 5 | **Split** (background) | Rewrite as one task + concrete subtasks from the research verdict. | → done |

**Stage 3 *is* the gate (the research-worthiness verdict) — there is no separate "full-pipeline-vs-not" switch.** Most captures stop at
stage 3: an obvious, self-contained task the parent already placed is finished; only genuinely research-worthy items
walk the whole ladder. The gate is a per-item verdict, deterministic given the item — not a global mode.

**Placement pre-check — task vs calendar EVENT (PARENT, stage 1, before any project routing).** A capture that *is* a fixed-time
event or routine ("dentist Tue 2pm") does **not** become a Todoist task — it routes to **Bob's 🤖 calendar**
(§5c), propose-and-confirm inline like case 3. The dup-guard extends to the calendar write (check the 🤖
calendar first — a Kanban restart re-runs the card and must not duplicate the event); the stage-1 Inbox item
is completed/cleared once the event exists (staging honored, nothing lost). A user-confirmed fixed event is
**immovable load** (§5c carve-out), not a movable proposal. Only start-decision items proceed to the project
routing below.

**Placement also DATES time-bound captures (PARENT, stage 1)** (this + §4c's promotion inflows replace the old
"captures are placed undated" blanket; because the raw Inbox write stored the text verbatim, **the parent's
placement step is the only place a date is written** — classify first, date second). Each write records its ledger **`kind`** (§4d/X5): a
**hard time bound** ("submit by the 15th") → `deadline` + computed date-only surfacing due, **`kind:
leadtime`** (rolls; §4c lead-time rule); a **plain day the user explicitly stated** ("Saturday") → date-only
due, **`kind: user_fixed`** (the user chose the day — it **never rolls**, §4d/X5); **implied-but-vague** timing
("before prices jump") → **propose the concrete date inline** (case-3 style) — never silently invent one, and
a user-confirmed proposal is likewise `user_fixed`; **genuinely timeless** → rests undated as someday
(§4c's planning pull is its designed way back in). Dating a capture fires activation-time grooming (§4c) right
then.

**Placement routing — four cases, all resolved INLINE by the PARENT (stage 1, decide-in-context — AA5):**

1. **Explicit project** — the user named it at capture ("…to Homelab") → route there, no decision.
2. **Obvious** — exactly one project clearly fits → route, no decision.
3. **Ambiguous** — more than one plausible home → **propose-and-confirm inline** ("Captured X — put it in
   Homelab?"). One fast yes/no while context is fresh. *(Live Discord session → a clarify button, §12 R1c.)*
4. **New project needed** — nothing fits → **ask inline** ("X doesn't fit any project — make one called Y?").
   **Never auto-create a project** (firm rule).

Cases 3–4 need the user; because **the PARENT owns placement (AA5)** and the parent IS the Discord-bound turn,
the ask is inline, decide-in-context, on the live session (a clarify button where one exists, §12 R1c). The
**primary path is decide-now** — ask while it's fresh. If the user is unresponsive, the parent creates the card
in a **held state and enqueues a `triage-reraise` record to the unified decision queue** (§2 step 4/§8a/R5A1/Y2
— no card subscription), durably holding "awaiting placement decision" while the Inbox task stays put; the
record surfaces as the brief's head item, and any live turn re-raises it opportunistically and, on the answer,
writes the placement onto the card + `hermes kanban unblock`s it. Durable block-and-wait is the *fallback*, not
the default. *(The mid-flight-decision path is the decision-queue re-raise — there is NO callback transport
onto the card, Y2/AA5.)*

**Dup-guard / idempotency (the check-before-create rule) — forced by the no-resume caveat.** Kanban restarts a crashed card from its
first stage, not the failed step (§19). So **every stage is check-before-create**: stage 1 skips if the
capture is already in Inbox — detected by the **`⟦fz:<journal-uuid>⟧` healing-marker search** (BB3), not a
content match, so a renamed or moved capture still de-dupes and a same-content sibling does not false-positive —
which also covers a full parent-level retry, and skips re-routing an already-placed task; stage 5 skips subtasks
that already exist. A restart converges to the same single task — never a duplicate. (Also why placement is one
*move*, not create-then-move — fewer mutations to make idempotent.)

**Failures are loud (the two-channel rule, §9/§16).** A stage that errors, or a card that can't complete, is
a *system* failure → it surfaces on the **errors channel (`#forzare-errors`)**, never silently dropped. The
no-shame/receptivity gate governs *task nudges to the user*, not pipeline health. The captured item is safe
regardless — stage 1 already persisted it.

---

# PART II — IMPLEMENTATION ON HERMES PRIMITIVES (the how)

> ⚠ Mechanics doc-derived (2026-05-29) + **Kanban/cron facts code-verified against upstream
> NousResearch/hermes-agent 2026-06-30** (corrections in §19) + **delivery/plugin facts code-verified against
> the installed hermes-agent 2026-07-03/04 (R1–R8, §12/§19)**. Confirm exact `config.yaml` keys/flags against
> live docs before building; genuine unknowns stay **OPEN** in §19.

## 9. Architecture firewall — Kanban is Bob's PRIVATE substrate, never a user surface

**The four-layer model (the mental picture for all of Part II):**

| Layer | Role | What it is |
|---|---|---|
| **Cron** | the **clock** | fires recurring rituals on a wall-clock; holds no logic of its own |
| **Bob** | the **brain** | decides + acts; the parent stays available to the user at all times |
| **Kanban** | the **coordinator** | sequences *durable background agent-work*; an **intermediary that leans on Hermes features (subagents, parent/child, retry) to reach the endpoints — not a store, not an endpoint** |
| **Todoist** | the **user task store** | the canonical home for tasks; an **endpoint** (alongside the vault + Calendar) |

Kanban moves work *toward* the endpoints; the data lands in Todoist / the vault / Calendar. Bob never persists
user data *in* Kanban.

**The boundaries that follow:**

- **Todoist = the ONLY user task store.** The user's tasks, the 5-label vocab, the 3 native fields, `p1`,
  due-dates — all stay in Todoist, accessed via the **`td` CLI** (the standardized local toolchain;
  prefer-local-CLI rule). Ranking and *timing of user tasks* live here. (Kanban has no due-date/deadline
  concept and its `priority` is an opaque int — it **cannot** be the user-task store.)
- **Hermes Kanban (`~/.hermes/kanban.db`) = Bob's own background work-items only** — e.g. the
  capture-processing pipeline (§8b), a durable research fan-out, a triage→done backlog. It is in-flight agent
  execution state, **never shown to the user.** (The morning brief, end-of-day, and `@waiting` reconcile are
  **not** Kanban jobs — they're cron-kicked skill-bundle runs, §11.)
- **Why it must be firewalled — and the TWO channels.** Creating a card from a platform-bound session
  auto-subscribes the originating chat to that card's **terminal events** (completed/blocked/gave_up/crashed/
  timed_out) — verified: the in-gateway kanban **tool** create path calls `_maybe_auto_subscribe`
  (`tools/kanban_tools.py:843,858-898`), gated by **`kanban.auto_subscribe_on_create` (default `True`,
  `hermes_cli/config.py:1348`)**. **Two guards keep it off the task channel (Z1):** (1) forzare's capture flow
  creates cards **only via the CLI `hermes kanban create`**, which is **subscription-free** (verified — the
  CLI create path in `hermes_cli/kanban.py` never calls `_maybe_auto_subscribe`); and (2) as belt-and-suspenders
  the config sets **`kanban.auto_subscribe_on_create: false`** (§14 drift #3) so even a stray tool-path create
  writes **no** subscription row. Separately, `hermes kanban notify-subscribe` only ever routes those
  **terminal** events, never decision events (verified `hermes kanban --help`; the reason forzare adds **no**
  card subscription and routes decisions via the unified queue + failures via the watchdog, §8b/§14/Y2). These
  must **not** leak onto the user's **task channel** (the one delivery gate, §12) — a stream of agent plumbing
  is the opposite of one-thing-or-nothing. **But "firewalled" means routed, not silenced:**
  - **Task channel** (§12 gate): one-thing-or-nothing, **no-shame, receptivity-gated.** Routine Kanban
    plumbing never appears here. No-shame governs *this* channel only — it's about the *user* failing a task,
    never about the system.
  - **Errors channel** (§16/§17): **system/pipeline failures surface LOUDLY here, always — never suppressed,
    never receptivity-gated.** A failed capture job, a crashed step, a dead dependency is a software fault, not
    a user shame event. *Quieting a pipeline failure is a software-engineering no-no.* (This is the
    **two-channel invariant** — the delivery mechanism for the §0 "two kinds of failure" separation: a
    *system* failure is never the user's fault and never quieted.)
  - So the earlier "suppress internal-job terminal events" framing was a category error and is **removed**:
    route routine plumbing away from the task channel; route *failures* loudly to the errors channel.
- **The user sees exactly one *task* thing, via the one delivery gate (§12)** — not the board, not the queue.
  System health is the separate errors channel.
- **REST surface is authenticated — verified (the docs implying an open REST surface are wrong).** The Kanban
  REST endpoints require auth, so the private board isn't incidentally exposed; the firewall is a design
  boundary, not a security patch over an open port.

## 10. Primitive decomposition (Bob-only) — the four layers, mapped to Hermes primitives

The §9 four-layer model (cron / Bob / Kanban / Todoist), expanded to the actual Hermes primitives. **Cron,
Kanban, and Todoist map one-to-one with rows below; the *Bob (brain)* layer is realized by the remaining
rows** — skill bundles (logic), the `/forzare` skill (signals), and cron-native + clarify-tool delivery (no
bespoke plugin, R5), all reaching the store via the `td` CLI:

| Layer | Hermes primitive | Carries |
|---|---|---|
| **Timing (the clock)** | **Cron** (`~/.hermes/cron/jobs.json`; gateway ticks 60s) | *Kicks off* every recurring ritual: morning brief, end-of-day, `@waiting` reconcile, block-boundary prompts. **Cron is the only clock** — Kanban has **no wall-clock firing at all** (the docs' `scheduled_at` is fictional; verified §19). Also carries **scheduled Discord delivery** (`deliver="discord[:channel_id[:thread_id]]"`, R1a). |
| **Coordination (durable background work)** | **Kanban** (private board, §9) | Durable **background** multi-step agent jobs — the **capture-processing pipeline (§8b)**, research fan-out, triage→done backlog. The **intermediary that leans on Hermes features (subagents, parent/child, retry) to reach the endpoints — not a store** (§9). Single assignee = the **`default` profile** (persona "Bob"); **manual orchestration** (not auto-decompose — that's multi-profile fan-out). **The brief is NOT here** — it's cron + a skill bundle (§11). |
| **Logic (reusable)** | **Skill bundles** (§13) | `/forzare-morning-brief`, `/forzare-replan`, `/forzare-eod` compose small skills (`todoist-surface`, `weather`, `calendar-read`, `calendar-write`, `eisenhower-plan`, …). Command surface: §1a. |
| **State signals** | **The `/forzare` skill** (§3B) | Native description-driven activation (manual `/forzare` + auto-fire on phrases) — classifies + dispatches. No hook. |
| **Delivery + on-demand pulls** | **No custom plugin by default** (§12, R5) | Cron-native Discord delivery for scheduled rituals (R1a) + **clarify-tool native buttons** for inline asks (R1c) + **`hermes send --to discord:<channel>`** for the errors channel (R2); the `/forzare-*` on-demand handles are **skills/bundles** (description-driven), not plugin commands. **Optional micro-shim plugin** only if native `/forzare-*` slash-command autocomplete proves necessary for recognition-over-recall — it registers command **names only** (no delivery, no hooks, no lock; §12/§19). |
| **Todoist access** | **The `td` CLI** (shelled out), taught by the existing **`/todoist-cli` skill** | Bob runs `td …` directly; it learns the command surface by invoking the installed **`/todoist-cli`** skill (single source of `td` knowledge — don't duplicate `td` usage into the forzare skills). `td` is a CLI, **not** an MCP server — no `mcp_servers` entry. Local toolchain, per the prefer-local-CLI rule. |

**Replaces the old §8 framing** ("a Hermes skill + script(s) driven by cron + the dispatch hook") — cron is
the clock for every recurring ritual (incl. the brief) and the scheduled-delivery path; Kanban carries
durable **background** work (the §8b capture pipeline); bundles hold the logic; **delivery is cron-native +
clarify buttons + `hermes send`, with no bespoke plugin** (R1/R2/R5).

## 11. Morning brief — a cron-kicked skill bundle (NOT a Kanban job)

The brief is **cron kicking off Bob to run the `forzare-morning-brief` skill bundle** (§13) in one agent turn
— not a Kanban parent-child graph. **The bundle is ATTACHED to the cron job via `--skill forzare-morning-brief`
(load-bearing, W1), NOT typed as a `/forzare-morning-brief` slash-command in the prompt.** Verified
(`cron/scheduler.py:1690-1889`, `_build_job_prompt`): the cron path expands **`job.skills`** (the `--skill`
list — a bundle slug there loads its member skills' full content) and appends the job's free-text `prompt`
only as inert *"instruction alongside the skill invocation"* text — a slash-command string in the prompt is
**never executed**. So a ritual whose prompt is a bare `/forzare-morning-brief` would run with **no skill
loaded**; every ritual cron job must carry its bundle/skills on `--skill`, with the prompt reserved for the
dry-run directive (§17) and any run-specific instruction. Two verified facts make Kanban the wrong tool here: there is **no real
workflow primitive** (`workflow_template_id` is a half-built filter/tag column, §19) and **no mid-run resume**
(a crash restarts the whole job, §19). A parent-child card graph therefore buys little durability for a short,
fast, daily sequence while adding exactly the plumbing the user must be firewalled from (§9).

- **Cron job** (~5:15, §1) runs the bundle. **Re-fire safety is app-level** (not a cron flag): the brief
  checks whether today's plan already ran — the §4c per-day plan record (`plan-of-day.json`, Y13) + the §8
  date-stamp — before mutating, so
  a re-fire or the ±2h catch-up (§8) is a no-op. **Time bound = the cron INACTIVITY timeout**
  (`HERMES_CRON_TIMEOUT`, default 600s *idle*, verified `cron/scheduler.py`): the turn may run for a long
  wall-clock time while active and is killed only after 600s with no activity; `script_timeout_seconds` does
  **not** bound it (that caps only an optional pre-run `--script`). Iterations are capped by `agent.max_turns`
  (default 90).
- **Ordering is a PROMPT CONTRACT, not a structural guarantee (V7/R2A23).** A bundle is a **skill-loader**,
  not a sequencer — verified `agent/skill_bundles.py:286-340` loads every listed skill into one turn under a
  header ("Treat every skill below as active guidance for this turn") plus an author-supplied **`instruction`**
  string; there is no engine step-ordering. So the order below is enforced by the **bundle's mandatory
  instruction block** (§13) — a prompt the agent follows — not by the loader: **defensive roll** (`eod-roll`,
  only if yesterday's state is stale, §8) → weather → calendar → active-tasks → **plan** (`eisenhower-plan`:
  set ≤3 `p1` + place the one deep anchor via `calendar-write`, §4c/§5a) → **follow-ups sweep** (§2 step 4:
  chases, fixed-item re-decisions, triage re-raises) → activation-reminder → assemble, then **deliver** via
  cron's scheduled Discord path (R1a; delivery is *not* a bundle skill) — all inside the one run, each step
  degrading **visibly** on failure (§16). The **defensive-roll-before-p1** rule is carried explicitly in that
  instruction (and restated in the `brief-assemble` skill) so the roll is never skipped or reordered after a
  `p1` write. Acceptance verifies the **actual tool trace** (§13), not just that the skills loaded.
- **`deliver` is the only user-facing step**, via cron's scheduled Discord delivery (§12).
- **Durability comes from idempotency, not Kanban:** the brief is safe to re-run (§15), and its state-mutating
  half (the nightly roll) is idempotent + date-stamped with a defensive morning re-run (§8). That is the whole
  recovery story — no card graph needed.

**Kanban is reserved for genuinely-durable *background* work** (the capture pipeline, §8b) where the parent
must stay free and stages run async — not for a foreground ritual the user is waiting on.

## 12. Delivery + interaction contract (no custom plugin by default)

**R1/R4/R5 rebuild — the delivery layer is now headless-native, with zero bespoke plugin code by default.**
The prior draft leaned on a custom `bob-surface` plugin whose delivery primitive was
`ctx.inject_message(...)` behind a single-surface *lock*. **Both are gone:**

- **`ctx.inject_message` is DEAD in gateway mode** (verified `hermes_cli/plugins.py:409-433`): it requires an
  interactive CLI `_cli_ref`, returns `False` headless while logging "not available in gateway mode," and even
  in CLI mode returns "queued" (not "delivered"). It **cannot** be forzare's delivery primitive on a headless
  gateway host. Removed.
- **The `bob-surface` plugin dissolves (R5).** There is **no custom plugin code by default.** The `/forzare-*`
  command surface is **skills/bundles invoked by name or plain language** (Hermes description-driven
  activation, §3B/§13). See the native-mirroring note + micro-shim fallback below.

### 12.1 The three verified headless-native delivery paths (R1)

**(a) Scheduled rituals** (brief / end-of-day / block-boundary prompts) → **cron Discord delivery.**

- `deliver="discord"` targets `DISCORD_HOME_CHANNEL` (verified `cron/scheduler.py:709-745`; env map at
  `:217`).
- `deliver="discord:<channel_id>[:thread_id]"` targets any specific channel/thread (`:839-870`).
- **Verified fully headless:** the gateway hands live adapters + its event loop to the cron scheduler
  (`gateway/run.py:18151-18161`) and delivery goes through `adapter.send` (`gateway/delivery.py:448`), with a
  standalone HTTP fallback. **No CLI is involved anywhere.**

**(b) Ad-hoc proactive nudges** (the sparse §1/§7 nudge) → **a one-shot cron job / `trigger_job`** (verified
`cron/jobs.py:774`, `:1133`) — uniform with the briefs path, same delivery machinery.

**(c) Inline asks** (§8b placement cases 3–4, §4 defer resolution, §3 low-confidence confirms) **when a live
Discord-bound conversation exists** → **the clarify tool → native Discord buttons.**

- `tools/clarify_tool.py:20` — max 4 choices + an auto-appended "Other"; adapter `send_clarify` at
  `plugins/platforms/discord/adapter.py:4778-4892`.
- A button-tap **resolves the blocked turn + the embed self-updates** (`:6535-6606`);
  `agent.clarify_timeout` default 600s; the view timeout is 300s; auth-gated.
- **One tap beats typing — an ADHD win** (recognition over recall, no free-text working-memory load).
- **CAVEAT (load-bearing):** **cron-origin turns have NO Discord-bound session and get no buttons** (verified
  `gateway/run.py:15585`, `16064-16127`). So: pipeline subagents route their asks **through the parent
  conversation** (already the design, §8b) — the parent is the Discord-bound turn; and **brief-time decisions
  fall back to plain one-line questions**, not buttons. Buttons are an accelerant on the live-session path, not
  a universal mechanism.

### 12.2 The [SILENT] guarantee (R3 — code-level, cross-note §16)

**Two suppression paths, different strictness (verified) — scope every claim to its path:**

- **Gateway (live-session) delivery = EXACT WHOLE-RESPONSE match, success-only.** Suppression happens **only**
  when the entire response canonicalizes (strip + upper-case + collapse whitespace, ≤64 chars) to one of
  `[SILENT]` / `SILENT` / `NO_REPLY` / `NO REPLY`, **and** the turn did not fail (verified
  `gateway/response_filters.py:13-52`, `is_intentional_silence_agent_result` returns `False` when
  `agent_result["failed"]`; `gateway/delivery.py:30-40`). A partial `[SILENT] …` prefix does **NOT** suppress
  on this path, and a failed turn is structurally un-silenceable — exactly what §0/§9 require of the errors
  half.
- **Cron delivery = MORE LENIENT (whole-response, first line, last line, or `[SILENT]`-prefix).** The cron
  scheduler recognizes the same token set as **whole-response, first-line, or last-line**, and additionally
  suppresses any response that **`startswith("[SILENT]")`** (verified `cron/scheduler.py:244-287` —
  `_CRON_SILENCE_TOKENS`, the `upper.startswith("[SILENT]")` branch). So `"[SILENT] no changes"` *is* swallowed
  on the cron path though it would be delivered on the gateway path.

The "exact-match only" guarantee therefore belongs to the **gateway** path; the **cron** path is the lenient
one. The dry-run/staging path (§17) leans on the cron behavior: a ritual whose output is (or begins with) the
sentinel exercises the full pipeline + writes the cron audit log without messaging the user. **Test matrix to
cover at build** (§17): exact-match, prefix, first-line, last-line, a *failed* turn (must NOT be silenced on
either path), and an ordinary substantive turn (must deliver) — **verified by DIRECT probes of the installed
filter functions** (`cron.scheduler._is_cron_silence_response`,
`gateway.response_filters.is_intentional_silence_*` — plan E1, R3A7), never by asking a staged agent to echo
the sentinel; the delivered-vs-suppressed live observation belongs to go-live day 1 (plan G1).

### 12.3 Single-writer discipline (R4 — procedural, NOT a mutex)

The old plugin-held single-surface *lock* is gone with the plugin. The replacement is **honestly procedural,
not a lock:**

- **Cron is the only proactive writer.** Jobs are serialized by the clock (the 60s tick fires them in order),
  and **cron deliveries land in their own cron session — never spliced into the user's live chat** (verified).
  Two scheduled rituals cannot interleave into one "next thing."
- **Plus the §6a receptivity gate** — low receptivity → provide-nothing, so the proactive rate is
  self-limiting.
- **Plus §0's one-thing rule** — the surfacing logic itself never emits more than one task.
- **Plus the daily session boundary (R6/§14 root `session_reset`)** — the session resets fresh at 04:00 (the
  root `default_reset_policy`, so it covers the Discord task channel), bracketed between the 23:00 end-of-day
  and the 5:15 brief, so no stale context bleeds across days.

Together these make double-firing **rare**, but — stated honestly — they are **not a mutex**. The real
invariant is **"at most one DO-NOW action _or_ one requested decision per RESPONSE, receptivity-gated"**
(the §0/W12 wording — the daily brief is the bounded planning-context exception that still ends on one
action), not "at most one across all concurrent paths at all times." A live-session turn and a cron ritual *can* in principle each emit their (single) next
thing in the same narrow window; nothing here structurally forbids it the way a lock would. That residual is
accepted for v1 (below) and booked as a post-V1 hardening candidate (§18a) — the earlier claim that this
gives "the same guarantee as a mutex" is **withdrawn** as overstated.

**Residual interleave — the per-channel delivery LEASE is REJECTED for v1 (YAGNI).** A live-session turn and a
cron ritual *could* in principle both emit a "next thing" in the same narrow window. A per-channel delivery
lease would close that, but it is exactly the bespoke machinery R4 removed — so it is **not built for v1**.
Documented honestly: the residual live-turn × cron interleave is **accepted as rare and benign** (both paths
are receptivity-gated and each emits at most one thing, §0/§6a — the worst case is two short messages moments
apart, not a wall), and is **booked as a post-V1 hardening candidate** (§18a) should it ever prove annoying in
practice.

### 12.4 Two delivery channels — the §9 two-channel invariant

- **Task channel (idle-only, rate-limited, receptivity-gated):** proactive *task* output goes to Discord via
  the configured **home channel** (`DISCORD_HOME_CHANNEL`), through cron's scheduled delivery (R1a) or a
  live-session reply. `[SILENT]`-only responses are logged but not delivered (§12.2) — for **dry-runs +
  routine internal-job chatter only**, never for failures (§16/§17). **Suppression is DELIVERY-only** — it
  silences the *message*, not the skill's store writes; genuine side-effect-free staging needs the **dry-run
  instruction** on top (writes redirected to `forzare/state/dryrun-intents.jsonl`, §17/V4/R3A1).
- **Errors channel (always loud) — the dedicated `#forzare-errors` Discord channel**
  (`DISCORD_ERRORS_CHANNEL`): **system/pipeline failures** go here and are **always delivered — never
  `[SILENT]`, never receptivity-gated, never rate-suppressed.** The out-of-band alert primitive is
  **`hermes send --to discord:<channel>`** (R2) — a CLI that needs **no LLM, no agent loop, and NO RUNNING
  GATEWAY** for bot-token platforms (verified via `hermes send --help`); the relay's phone/local push stays as
  **belt-and-suspenders**. A *separate, dedicated* channel (not the home channel) is the design, on purpose:
  **any unread message in `#forzare-errors` means something is broken** — the channel name + its unread badge
  *is* the alert, so the user sees it and checks immediately, with zero mixing into the normal task stream.
  (Degraded fallback if `DISCORD_ERRORS_CHANNEL` is unset: the **forzare-ops watchdog / deliver-strings**
  — the only consumers of that forzare-layer key — fall back to the home channel with a `⚠ ERROR` prefix;
  this fallback is *their* logic, not a hermes-core behavior.) Quieting a pipeline failure is forbidden; this
  is the loud half of the two-channel rule.
  - **Scope — errors-only.** `#forzare-errors` carries **nothing but forzare's own system/pipeline failures.**
    Explicitly **not** here: the user's task/schedule slippage (that's the no-shame task channel — §0 two kinds
    of failure), routine status / health / heartbeat pings, "recovered/back-to-normal" notices (the
    best-effort ≈5-min detection target, W8/X9, applies to *failures* — recovery chatter never appears here),
    observability
    traces (the cron audit log `~/.hermes/cron/output/`, §17; Langfuse tracing is post-V1, §18a), and any
    other system's logs (forzare is Bob-only anyway, §18). The channel is therefore **silent when healthy** —
    which is the entire reason an unread message there is a trustworthy alert. The instant it becomes a mixed
    feed, "unread = broken" stops being true.

### 12.5 Native slash-command mirroring + the micro-shim fallback (R5)

Hermes mirrors **plugin-registered commands** to native Discord slash commands
(`plugins/platforms/discord/adapter.py:3625-3733`), while **skills consolidate under a single `/skill`
group.** So the default (skills/bundles, no plugin) gives description-driven + plain-language activation but
**not** native `/forzare-*` autocomplete. If native `/forzare-*` autocomplete proves necessary for
recognition-over-recall (§1a), the **documented fallback** is a **micro-shim plugin** that does **ONLY** one
thing: register the command **names** so they mirror to native slash commands. It carries **no delivery, no
hooks, no lock** — those all stay on the cron-native / clarify-button / `hermes send` paths above. §19 records
the build-time check: *do bundle invocations mirror natively?* Build the shim only if the answer is no and
recognition suffers.

## 13. Skill bundles + declarative config + skill-integrity gate

- **Bundles** (`~/.hermes/skill-bundles/<slug>.yaml`): `skills:` (required, non-empty list) + optional
  `name`/`description`/`instruction`. `/<bundle> [instruction]` loads all listed skills at once (text after
  the command is attached as the instruction). Bundles **don't install** skills and **silently skip missing
  ones** — so the atomic skills must exist first.
- **Every forzare bundle carries a MANDATORY `instruction` block (V7) — this is where sequencing lives.**
  Because the loader does not order or gate skills (§11), each bundle's `instruction:` string is a required
  contract stating: **(1) mode** (`morning` | `eod`), **(2) the ordered step list**, **(3) mutation
  boundaries** — for EOD explicitly *"writes NO `p1`, NO calendar"*; for morning *"run the defensive
  `eod-roll` and the roll-set check BEFORE any `p1` write"* — and **(4) failure handling** (degrade visibly,
  never silent; §16). The `instruction` is not optional decoration for these three bundles; it is the only
  thing that makes the run deterministic.
  - `/forzare-morning-brief` = `eod-roll` (defensive missed-fire roll first, §8) · `weather` ·
    `calendar-read` · `todoist-surface` · `eisenhower-plan` · `followups-sweep` (§2 step 4: chases + fixed
    re-decisions + triage re-raises) · `activation-prompt` · `calendar-write` · `brief-assemble`. **The
    morning run is where the day's plan is WRITTEN** — `eisenhower-plan` sets the ≤3 `p1` (§4c) and
    `calendar-write` places the ONE protected deep anchor if a deep window exists (§5a). (These two were
    missing from the earlier composition, so the morning bundle couldn't actually build the day.)
  - `/forzare-replan` = `calendar-read` · `todoist-surface` · `eisenhower-plan` · **`calendar-write` (KK5/JJ5 —
    required in the bundle because replan MOVES Bob's own 🤖-calendar proposals, and every 🤖-calendar write
    goes through `calendar-write`'s contract, §5c: own lane only, movable proposals, never a user event)** in
    **`replan` mode (W10)** —
    redraw **only the REMAINING day** (from now to end-of-day), from the current plan + active pool; no
    state-detect (that's the `/forzare` state path). Replan **may move Bob's own 🤖-calendar proposals** (via
    `calendar-write`, inside its §5c contract) and
    **may PROPOSE `p1` changes**, but **never silently applies** a `p1` change — applying one requires the
    user's explicit yes (INV-5) — and it **never touches fixed anchors** (the work block, user-primary events,
    a §5c user-confirmed fixed event). **Partial-day acceptance:** a mid-afternoon replan re-plans only the
    hours left, leaving the morning's completed/passed blocks untouched.
  - `/forzare-eod` = `eod-roll` (the roll + **Bob-owned p1-clear** — only the day's `plan-of-day` `selected_ids`,
    never a user-set p1, AA2 — + lifecycle ledger ticks + the `stale-p1` enqueue + last-reconcile
    stamp, §8) · `todoist-surface` · `daily-reflect` · `eisenhower-plan` · `tomorrow-prep`. **`calendar-write`
    is NOT in the EOD bundle (R2A8):** EOD writes **no** calendar — `tomorrow-prep` only records a candidate
    anchor to `forzare/state/tomorrow-prestage.json` (§8a); the **morning** run is the sole place the deep
    anchor is placed on the 🤖 calendar. **At EOD `eisenhower-plan`/`tomorrow-prep` only PROPOSE** tomorrow's
    candidate ≤3 + anchor — **neither writes `p1`** (that is exclusively the morning run's job; this removes
    the earlier contradiction where both the morning brief and EOD appeared to set `p1`, and the earlier
    double where EOD also wrote the calendar).
  - **`eisenhower-plan` is one skill with THREE modes by caller (W10):** **`morning`** — plan-time ranking
    that **writes** the ≤3 `p1` + places the one deep anchor; **`eod`** — **proposal only**, writes no `p1`
    and no calendar; **`replan`** — redraw the **remaining** day only, may move Bob's 🤖-calendar proposals and
    **propose** (never silently apply) `p1` changes, never touching fixed anchors. The caller (bundle)
    determines which.
  - The **02:00 `@waiting` reconcile** is owned by a dedicated **`waiting-reconcile`** skill (mark chase-due ·
    §4b set-time-invariant repair · unblock detection vs **gog calendar + `td activity` ONLY — never "recent
    Discord context"** (R5A12: an amnesiac 02:00 cron session has no verified read path to chat history) ·
    14-day staleness, §8) — it is
    **not** in a bundle; it is run directly by the 02:00 cron job, state-only, never delivering.
  - `todoist-surface` is the atomic primitive reused across bundles (write once).
- **Skill-INTEGRITY enforcement (loud — closes the silent-skip hole; NOT a boot-abort, BB8).** Hermes bundles
  **silently skip skills that aren't installed** (above), so a missing `todoist-surface` would make
  `/forzare-morning-brief` run with *no surfacing and no error* — a silent failure that violates §0/§9. The
  earlier "**boot asserts … and fails loud (abort boot)**" claim is **REMOVED as unenforceable (BB8):** the
  gateway's own startup is Hermes' launchd artifact (`ai.hermes.gateway.plist`), which forzare **must not
  patch** (no-patching-third-party-tools rule) — there is no forzare-owned hook that can abort *Hermes'* boot.
  So integrity is enforced by two forzare-owned mechanisms instead:
  - **(a) The forzare-ops watchdog's per-pass skill-INTEGRITY scan (§14 scan (f)).** Each 300s pass asserts
    **every managed file of every V1 skill dir is installed at its expected path, content-hash-matches the
    chezmoi source, AND carries its expected exec mode (KK6 — a RECURSIVE per-file manifest, not just
    `SKILL.md`).** A skill is more than its `SKILL.md`: the scan enumerates **every executable/support file
    shipped in each skill dir** — e.g. `weather/classify.py`, `followups-sweep/eligibility`,
    `calibration-log/reduce.py`, and the `forzare-capture-pipeline` stage scripts — so a corrupted or
    non-executable support file (which Hermes would silently skip or fail on) is caught, not just a missing
    `SKILL.md`. The manifest covers **the FULL V1 skill set: the bundle skills, the on-demand handles
    `forzare-next`/`forzare-today`/`forzare-capture`, the `/forzare` classifier, `forzare-capture-pipeline`,
    `calibration-log`, `waiting-reconcile` (the 02:00 cron skill, not a bundle member — JJ10), `transition`
    (the block-boundary skill — JJ10), the shared mutation helper `forzare-mutate.sh` — plus the 3 bundle
    YAMLs** — matching the plan's canonical `integrity_manifest()` exactly (plan Task A1/F1). It **alerts to
    `#forzare-errors`** (best-effort ≈5-min, W8/X9) on any missing / content-drifted / wrong-exec-mode managed
    file. This is the runtime guard against Hermes' silent-skip behavior.
  - **(b) The documented pre-start check in the go-live runbook** — the build-time SKILL-INTEGRITY GATE (plan,
    end of Phase B) runs the same path+hash assertion before delivery ever flips to live, and the runbook says
    to re-run it after any skill re-apply.
  **The gate is installed-path + content-hash, NOT a curator pin (AA11 — see below):** the forzare skills are
  repo-authored (chezmoi-installed), which the curator's managed list **excludes**, so they are never GC
  candidates and a pin would be a no-op-for-protection. Without the path+hash check a typo'd or stale-applied
  skill degrades the engine invisibly — so the watchdog scan is the standing guard, the pre-start check the
  build gate.
- **Declarative config** (`metadata.hermes.config` in each SKILL.md → stored under `skills.config` in
  `config.yaml`; entries are key/description/default/prompt). **The `work_schedule` is an EXACT per-weekday map
  + alternating-Sunday anchor (Z9/R6A5)** — validated key-by-key at build (plan B10):

  ```yaml
  work_schedule:
    days:                              # every weekday key present; null = off day
      monday:    null
      tuesday:   {start: "15:00", end: "23:00"}
      wednesday: null
      thursday:  {start: "15:00", end: "23:00"}
      friday:    null
      saturday:  {start: "15:00", end: "23:00"}
      sunday:    {start: "15:00", end: "23:00", alternating: true}   # ON/OFF per the anchor below
    alt_sunday_anchor: "2026-06-07"    # this Sunday = ON; ±14-day multiples = ON (May 31 OFF, Jun 14 OFF, …)
  gym_schedule:
    days: [monday, tuesday, wednesday, friday, saturday, sunday]     # rest day = thursday
    window_start: "06:00"
    window_end:   "09:00"              # the "Back from the gym?" backstop fires at window_end
  wake_anchor: "05:15"
  weather_thresholds: {wind_mph: 17, rain: any, temp_low_f: 50, temp_high_f: 90}
  commute_prep_minutes: 30
  commute_travel_minutes: 25
  ```

  **Cron trigger times are DOW-AWARE, DERIVED from this map (Z9/R6A5) — never a flat `block_start` read.** The
  block-boundary cron's day-of-week field is the set of weekdays that have a work block (Tue/Thu/Sat = `2,4,6`,
  **plus Sunday `0`** because Sundays are conditionally work days via the anchor), and its **time is the
  boundary FORMULA (R7A2/AA7): `block_start − commute_prep − commute_travel − 30 min` — ONE value everywhere.**
  For the 15:00 block with prep 30 + travel 25: 15:00 − 30 − 25 − 30 = **13:35**, so the boundary derives to
  `35 13 * * 0,2,4,6` — the ~30-min-until-you-leave soft pre-warning (§3a item 1), NOT the leave-time alarm
  (which is `block_start − prep − travel` = **14:05**, §3a item 3). It **never fires on a genuine off day**
  (Mon/Wed/Fri). The gym-window-end cron's DOW
  is the gym days (`1,2,3,5,6,0` — all but Thursday=`4`). **Alternating Sunday: cron cannot express "every other
  Sunday,"** so the Sunday job fires **weekly** and the `activation-prompt`/`transition` skill **no-ops on OFF
  Sundays** per `alt_sunday_anchor`. (The **peak/free windows are *derived* from these at run-time, §2/§6a — not
  a stored config value**.) **Not hardcoded** — a new job = edit `work_schedule` and **re-derive the crons**
  (plan B10 Step 4 reconcile), nothing else. Secrets (API keys) use `required_environment_variables`
  (name/prompt/help/required_for), prompted on first use, auto-injected into the sandbox.
- **No curator pin — the engine skills are NOT curator GC candidates (AA11, code-verified 2026-07-11).** The
  earlier design curator-pinned each atomic skill to stop the curator archiving it as "stale." **That pin is
  DROPPED as verified-unreachable-as-protection:** the forzare skills are **repo-authored** (chezmoi-installed
  into `~/.hermes/skills/`), and the curator's managed/GC list — `list_agent_created_skill_names()` — **includes
  a skill only if its `.usage.json` record is agent-created** (`created_by == "agent"` or `agent_created ==
  true`, `skill_usage.py`). A chezmoi-dropped skill has **no** such record, so it is **never** in the curator's
  list and can never be archived/consolidated — `hermes curator pin <name>` on it writes a `pinned: true`
  record but protects against a transition that can never target it (a no-op-for-protection). **So the gate is
  installed PATH + content HASH, not a pin** (the boot integrity check above; §19 records the verified fact).
  **Bundles are likewise not curator-managed** (the curator only walks `SKILL.md` dirs, never
  `skill-bundles/*.yaml`).

## 14. `config.yaml` stanzas + boot/runtime ordering

**⚠ Keys/values below are code-verified against the installed hermes-agent + the live `config.yaml`
(2026-07-11).** Indicative shape (real key names, not paraphrases):

```yaml
timezone: "America/Denver"          # root key — applies to cron scheduling + log timestamps (§15); live = "" (drift #1)
session_reset:                      # ROOT key (verified gateway/config.py: root session_reset → default_reset_policy,
  mode: both                        #   applies to ALL sessions incl. Discord). NOT under platforms.discord.
  at_hour: 4                        #   valid modes: daily | idle | both | none. Live = "none" (change to both).
  idle_minutes: 1440                #   04:00 daily boundary — bracketed between 23:00 EOD and the 5:15 brief.
  notify: false                     #   verified real subkey (default True) — silent reset, no user-facing ping.
kanban:
  dispatch_in_gateway: true         # default; dispatcher runs inside the gateway process
  dispatch_interval_seconds: 60     # default tick
  default_assignee: "default"       # the `default` profile (persona "Bob"; no profile named "bob" exists, §8b/§10)
  max_in_progress_per_profile: 2    # REAL key name (live = null = unbounded) — cap concurrent workers (leash RAM)
  failure_limit: 2                  # live already 2 — one retry then give up + alert (§16/§19)
  auto_decompose: false             # OFF for single-profile (auto-decompose is multi-profile fan-out); live = true (drift #2)
  auto_subscribe_on_create: false   # Z1 firewall guard — DEFAULT True (verified hermes_cli/config.py:1348).
                                     #   When a platform-bound session creates a card, the in-gateway kanban
                                     #   TOOL auto-subscribes that chat to the card's TERMINAL events
                                     #   (tools/kanban_tools.py:843,858-898) → they would leak onto the user's
                                     #   task channel (§9). forzare's capture flow uses the CLI `hermes kanban
                                     #   create` (verified subscription-free — hermes_cli/kanban.py never calls
                                     #   _maybe_auto_subscribe), so this key is belt-and-suspenders: false so
                                     #   even a stray tool-path create writes no subscription row. (drift #3)
auxiliary:
  triage_specifier: { provider: anthropic, model: claude-haiku-4-5, timeout: 20 }   # R7 — `hermes kanban specify` slot; cheap model (live = provider: auto, model: ""); timeout 20 bounds the specify LLM call (II9 — key verified config.py:1505, default 120)
cron:
  wrap_response: true               # default
  max_parallel_jobs: 1              # live = null (unbounded) — pin to 1 so rituals never interleave (U6)
  # script_timeout_seconds bounds only the optional pre-run --script (env HERMES_CRON_SCRIPT_TIMEOUT);
  # the agent turn is bounded by the cron INACTIVITY timeout HERMES_CRON_TIMEOUT (default 600s idle), §11/§16.
plugins:
  # ADDITIVE — ensure these forzare-required members are PRESENT; NEVER strip the live platform/provider
  # entries. Live enabled set (verified): anthropic-provider, chronos, disk-cleanup, herdr-agent-state,
  # image_gen/openai-codex, openai, platforms/discord, security-guidance. `platforms/discord` in particular
  # MUST stay (it is the whole delivery surface). NO bob-surface (dissolved, §12); observability/langfuse is
  # post-V1 (§18a). The ONLY forzare requirement here is a keep-OUT guard: `hermes-achievements` must never
  # ENTER plugins.enabled (gamification violates §6a no-shame). It is NOT currently enabled (verified) —
  # so this is a guard, not a removal.
  enabled: [ ..., platforms/discord, disk-cleanup, security-guidance ]   # (… = the other live members, kept)
mcp_servers:
  # (no Todoist MCP server — Bob uses the `td` CLI directly; ensure `td` is on PATH + authed)
skills:
  external_dirs: []                 # `~/.hermes/skills` is the BUILT-IN default and is scanned even when this
                                    #   list is empty (verified: live = []). external_dirs lists ADDITIONAL dirs
                                    #   only — do NOT re-add ~/.hermes/skills here.
```

`.env` (`~/.hermes/.env`): `DISCORD_BOT_TOKEN`, `DISCORD_ALLOWED_USERS`, `DISCORD_HOME_CHANNEL` (task-channel
proactive-delivery target), **`DISCORD_ERRORS_CHANNEL`** = the dedicated **`#forzare-errors`** channel. This
key is a **forzare-layer convention** — it is read by forzare's own **ops watchdog** and deliver-strings
(§16), **not** by any hermes-core config path — and is the always-loud target for system/pipeline failures
(the §9/§12 two-channel errors half, delivered via `hermes send --to discord:<channel>`, R2). The `⚠ ERROR`
home-channel fallback for an unset key is implemented **inside the ops watchdog / deliver-strings** (the only
components that consume the key), not by hermes. Plus weather/calendar creds.

**Live-config drift — MUST-FIX before go-live (R6b).** The live `~/.hermes/config.yaml` (re-audited 2026-07-11)
diverges from this spec; **fix these before flipping delivery to live:**

1. **`timezone` is EMPTY** (`timezone: ''`) — the spec requires `"America/Denver"` (§15; empty = server-local,
   which would mis-fire the 5:15 / 23:00 / 02:00 crons).
2. **`kanban.auto_decompose: true`** — the spec requires **`false`** (auto-decompose is multi-profile fan-out;
   Bob is single-profile with manual orchestration, §10).
3. **`kanban.auto_subscribe_on_create` is UNSET (defaults `True`)** — the spec requires **`false`** (Z1). The
   key defaults True (verified `hermes_cli/config.py:1348`), and on a platform-bound session the in-gateway
   kanban **tool** create path auto-subscribes the originating chat to the card's terminal events
   (`tools/kanban_tools.py:843,858-898`) — an agent-plumbing leak onto the task channel (§9). forzare's capture
   flow already routes through the CLI `hermes kanban create` (subscription-free, verified), so setting this
   `false` is a belt-and-suspenders guard: a stray tool-path create still writes **no** subscription row.
4. **`hermes-achievements` keep-out GUARD (not a removal).** `plugins.enabled` uses **allow-list** semantics,
   and `hermes-achievements` is **NOT currently in it** (verified 2026-07-11 — the earlier "it is enabled"
   claim was wrong). So the action is a **guard**: assert it never *enters* `plugins.enabled` (gamification —
   points/achievements — violates the §6a no-shame anti-patterns). Nothing to remove today; keep it out.

Two further drifts to repair while here (A17/U6): **`kanban.max_in_progress_per_profile` is `null`** (set to
`2`) and **`cron.max_parallel_jobs` is `null`** (set to `1`); and pin **`auxiliary.triage_specifier`** off its
`provider: auto` default to a cheap model **and its `timeout` to `20` (II9 — the key exists at `config.py:1505`,
default 120, and bounds the specify LLM call, `kanban_specify.py:196`)** for the §8b `specify` slot.

**Boot (deploy):** write `config.yaml` + `.env` → **fix the four live-config drifts above (R6b + Z1)** → install
every V1 skill → **run the pre-start skill-INTEGRITY check — assert every V1 skill is installed at its expected
path AND content-hash-matches the source; fail the deploy loud if any is missing or drifted (§13 silent-skip
guard; installed-path + content-hash, NOT a pin, AA11). This is a build/runbook gate, NOT a Hermes boot-abort
(BB8 — forzare cannot hook Hermes' own launchd startup); the standing runtime guard is the watchdog's §14 scan
(f)** → enable built-in plugins (no `bob-surface`, R5) → declare **the six cron ritual jobs (the exact manifest
is the plan's C2 six-job set — morning brief, end-of-day, 02:00 `@waiting` reconcile, gym-window-end,
block-boundary, monthly someday-sweep; CC14)**, each **attaching its bundle/skill via `--skill`** — a
slash-command prompt is inert on the cron path, §11/W1 — **all created `--deliver local` for the build (FF11);
go-live (§17/plan G1) flips the FOUR user-facing jobs to `discord` while `waiting-reconcile` + the
someday-sweep stay `local`** (matches §17/plan C2/EE2), then → `hermes gateway start`. **Do NOT run the deprecated standalone `kanban daemon`** alongside the gateway
dispatcher (claim races) — the gateway runs the dispatcher.

**Runtime tick:** the **gateway** is one process running platform connections + cron (60s tick, `.tick.lock`)
+ the Kanban dispatcher (60s) → **if it dies, everything stops.** So gateway liveness is **two layers**:

- **Restart — already in place:** `~/Library/LaunchAgents/ai.hermes.gateway.plist` runs the gateway with
  `RunAtLoad=true` + **`KeepAlive` = `true`** (verified live 2026-07-11 — a plain `<true/>`, changed since the
  2026-06-30 `{SuccessfulExit: false}` reading; semantically it now restarts on **any** exit, clean or crash).
  Either way the **crash-self-heal** conclusion is unchanged. Installed by `hermes gateway`, not chezmoi — so
  it isn't in the dotfiles repo.
- **Liveness + failure alerting — the piece to build is the `forzare-ops watchdog`** (one out-of-band script,
  launchd-polled with `StartInterval` **300s** (DECIDED — the W8/X9 **best-effort ≈5-min** detection target,
  not a hard ceiling: launchd skips intervals during sleep and won't re-enter a still-running pass), **zero
  LLM**), modeled on the existing
  `~/.local/bin/osquery-uptime-watchdog.sh`. It does **six** state-stamped scans each pass and routes every
  hit to `#forzare-errors`:
  - **(a) Gateway health.** `KeepAlive` catches a process *exit* but **not a wedged-but-alive gateway**, and
    never *tells* you — so the watchdog sends a one-shot probe a hung gateway can't answer:
    **`curl -fsS -m 3 http://127.0.0.1:8644/health`**, exit code **0 = up / 28 = hung / 7 = down** (verified,
    §19). The probe port is the **webhook platform's** `:8644/health` (`webhook.py:195`, DEFAULT_PORT 8644) —
    present iff the platform is enabled, which the **managed env pins**: `private_dot_env.tmpl:15` sets
    `WEBHOOK_ENABLED=true` (Phase-A prerequisite: assert post-apply). On **down / hung / restart-looping** → loud alert.
  - **(b) forzare run failures — predicate is a causal run EVENT, never status+counter (W9, corrects
    V9/R2A6).** Since its last check (a stamped watermark), it scans **`~/.hermes/cron/output/`** for failed
    ritual runs and the **Kanban DB** for genuine failures — and routes each to the errors channel. **The
    Kanban-DB half carries the SAME forzare discriminator scan (e) uses (JJ4/MM1):** only a card whose stored
    **`created_by == "forzare"` column** (`kanban_db.py:1019`, read from `hermes kanban list --json` — the
    immutable discriminator, since `specify` rewrites the title, MM1) can raise a `#forzare-errors` alarm — the private board is shared across profiles
    (§9), so a **non-forzare profile's card failing (`gave_up`/`crashed`/`timed_out`) NEVER alarms** the forzare
    errors channel. (Cron-`output/` ritual failures are already forzare-scoped by the `forzare-*` job manifest.)
    **The failure predicate is a causal run OUTCOME/EVENT, NOT a `status='blocked' AND consecutive_failures>0`
    derivation.** Verified: **`block_task` (`running → blocked`) does NOT clear `consecutive_failures`**
    (`kanban_db.py:4383` sets only status/claim fields — that claim stands). The counter *is* cleared on the
    UNBLOCK path (`unblock_task`, `kanban_db.py:4560-62`, verified — CC9 corrects the earlier "cleared only on
    success/reassign, :4561/:2645" phrasing; the "only" claim is dropped), but an **awaiting-user block never
    passes through unblock** — so a card that failed once transiently and then blocked to **await the user** still
    carries `consecutive_failures == 1` — a `blocked WITH consecutive_failures>0` predicate would
    **false-alarm on that healthy awaiting-user card** (the recovered-failure-then-user-block case). Alert
    **only** on the presence, since the watermark, of a genuine failure **event/outcome**:
    - a **`gave_up`** outcome (the `failure_limit` trip / `spawn_auto_blocked → gave_up` terminal,
      `kanban_db.py:1979`);
    - a run **`timed_out`** or **`crashed`** event.
    `blocked` is a *status*, never a failure event on its own (`VALID_STATUSES`, `kanban_db.py:101`;
    events at `kanban_watchers.py:163`), so it is **not** an alert trigger.

    **Explicitly EXCLUDED (healthy, not a failure): any `blocked` card that has emitted no failure event** — an
    **awaiting-user** card (e.g. the §8b placement question, reason = user decision), **including one carrying a
    nonzero `consecutive_failures` from an earlier recovered failure.** Awaiting-user blocks emit no
    gave_up/crashed/timed_out event, so the event-based predicate excludes them by construction — no counter
    inspection needed. Alerting on them would turn a normal "waiting on you" into a false system alarm and break
    "unread = broken" (§12.4). **This is the concrete owner of "cron/pipeline failure summaries reach the
    user":** the watchdog is the errors *router*.
    - **Durability (mirror the osquery watchdog's pattern):** alert ids are **content-stable** (a hash of
      {kind, job/card id, run timestamp}) so a re-scan never double-alerts the same failure; the watermark is
      **spooled before it advances** (write the pending alert to a spool file, *then* move the watermark), and
      each pass **drains the spool first** (drain-on-run). If `hermes send` exits non-zero (Discord
      unreachable), the **spool is retained and retried next pass** — a failed alert is never lost.
  - **(c) Delivery-only cron failures — scan `jobs.json` for new `last_delivery_error` (X8, corrects a blind
    spot in scan (b)).** A cron ritual can **succeed at the agent turn** (output saved) yet **fail to deliver**
    (Discord momentarily down) — verified `cron/jobs.py:1193` (`mark_job_run`): `last_delivery_error` is
    tracked **separately** from the agent error, precisely because "a job can succeed but fail delivery." Such
    a run has `last_status == "ok"`, so scan (b)'s failed-run scan of `cron/output/` never sees it — the
    delivery failure would be **invisible**. So the watchdog also reads `~/.hermes/cron/jobs.json` and, per job
    id, alerts on a **newly-set (or changed) `last_delivery_error`** since its watermark (content-stable id
    over `{job id, last_run_at}`, same spool). This is the one failure class the two run-outcome scans miss.
    - **Masking window — the honest bound (Y11, accept-with-framing).** `last_delivery_error` is field-tracked
      per job and **cleared only by a LATER successful delivery of the SAME job** (`mark_job_run`). Because the
      watchdog **snapshots + diffs `jobs.json` per pass** against its watermark (the `{job id, last_run_at}` it
      already keeps), the only window in which a delivery failure could be masked is a **same-job re-run within
      one 300s watchdog pass** — i.e. a manual re-trigger of the same ritual inside 5 minutes that succeeds and
      clears the error before the watchdog's next diff. The scheduled rituals fire **≥ a day apart**, so they
      cannot self-mask; only a burst of *manual* triggers of one job inside a single pass can. This is a
      **documented, accepted bounded gap** — no new machinery is added to close it (the per-pass snapshot+diff
      is the existing watermark, not new code); the wording states the bound honestly rather than claiming a
      guarantee it can't meet.
  - **(d) Ritual-ABSENCE detection — "the brief silently never ran" (AA8, the worst ADHD failure mode;
    go-live-KEYED, CC3).** The three run-outcome scans above catch a ritual that ran and *failed*; none catches a
    ritual that **never fired at all** (a deleted job, a job left `--deliver`-disabled/paused, or a run that
    produced no output). So each pass **loads the exact six-job cron manifest — the plan's C2 set:
    `forzare-morning-brief`, `forzare-eod`, `forzare-waiting-reconcile`, `forzare-gym-window-end`,
    `forzare-block-boundary`, `forzare-someday-sweep` (CC14 — this is the authoritative manifest; the §14 boot
    line points here)** and, for each job, asserts its `last_run_at` (from `jobs.json`) **and** its newest
    `~/.hermes/cron/output/<job_id>/` timestamp are within the job's **schedule-derived deadline + a 30-min grace
    (DECIDED)**. **A job that is missing from `jobs.json`, disabled/paused, or has produced no output past its
    deadline+grace ⇒ an alert — but this scan is KEYED ON `forzare/state/go-live.json` (CC3): PRE-go-live it only
    LOGS the finding (informational; the staging jobs are DELIBERATELY `--deliver local`/paused, so alerting
    would be a false alarm), and POST-go-live it ALERTS to the errors channel.** ("the morning brief has not run
    since <ts>"), same content-stable id + spool as (b)/(c). This is the one scan that closes the silent no-fire
    — the failure a prospective-memory-impaired user would never notice on their own. **Post-go-live the manifest
    also carries the plan's two drift classes (HH6, plan G1 Step 4/F1): the expected name→delivery map AND the
    prompt-DRYNESS axis — so the scan additionally ALERTS on delivery drift (a user-facing job no longer
    `--deliver discord`, or a state-only job no longer `local`) OR a job whose prompt reverts to a `DRY RUN`
    opener, either of which silently stops the job acting or delivering.**
  - **(e) Stale-triage detection (AA5 — the specify backstop).** Because the bounded `specify` attempt can fail
    or time out (§8b/BB1), a `specify` that never completed leaves a capture card stuck in `triage`. So the pass
    scans the private Kanban board for any **forzare capture card still in `status = triage` past its create
    time + a 30-min grace ⇒ an errors-channel alert** (same content-stable id + spool). This catches a wedged or
    failed specify — including a failed one-shot `--no-agent` retry — that would otherwise silently swallow a
    capture.
  - **(f) Skill-INTEGRITY scan (BB8 — the runtime integrity guard, replacing the removed boot-abort).** Because
    forzare cannot hook Hermes' own boot (no-patching rule, §13), this scan is the standing runtime guard against
    Hermes' silent-skip behavior: each pass asserts **every MANAGED FILE of every V1 skill dir is installed at
    its expected path, content-hash-matches the chezmoi source, AND carries its expected exec mode — keyed off
    the chezmoi `executable_` SOURCE attribute, not a filename suffix (KK6/NN9 — a
    RECURSIVE per-file manifest, not just `SKILL.md`)** — every skill's `SKILL.md` PLUS every executable/support
    file it ships (`weather/classify.py`, `followups-sweep/eligibility`, `calibration-log/reduce.py`, the
    `forzare-capture-pipeline` stage scripts, …). The manifest covers the bundle skills, the on-demand handles
    (`forzare-next`/`forzare-today`/`forzare-capture`), the `/forzare` classifier, `forzare-capture-pipeline`,
    `calibration-log`, **`waiting-reconcile` and `transition` (JJ10 — the 02:00-cron and block-boundary skills,
    not bundle members)**, and the shared mutation helper `forzare-mutate.sh`, plus the 3 bundle YAMLs — the
    SAME `integrity_manifest()` list the plan iterates (Task A1/F1). On any **missing / content-drifted /
    wrong-exec-mode** managed file ⇒ an errors-channel alert (same content-stable id + spool). A missing/stale
    file is silently skipped by Hermes' bundle loader, so without this scan a typo'd or half-applied skill
    degrades the engine invisibly.
  - **Alert path (out-of-band, independent of the gateway):** **`hermes send --to discord:<#forzare-errors>`**
    (R2) — no LLM, no agent loop, no running gateway for bot-token platforms — so it can report the gateway's
    own death; the relay's phone/local push stays as belt-and-suspenders. **You cannot use the thing that's
    down to report its own death** — `hermes send` talks to Discord directly with the bot token. **Binary +
    env resolution (W9):** launchd hands a minimal environment, so the watchdog resolves the **absolute
    `hermes` path at install** (a script-level `HERMES_BIN` constant, or the plist's `EnvironmentVariables`
    `PATH` including `~/.local/bin`) and **loads `DISCORD_ERRORS_CHANNEL`/`DISCORD_HOME_CHANNEL` by
    dotenv-PARSING `~/.hermes/.env` for exactly those two keys — NEVER `source`-ing it (Z4).** The managed
    `.env` carries an unquoted value with spaces that **crashes a strict shell** (`set -euo pipefail` + `.
    ~/.hermes/.env` aborts), so the watchdog extracts only the two channel ids without evaluating the file
    (`sed -n "s/^[[:space:]]*KEY=//p" ~/.hermes/.env | tail -n1`, quote-stripped) rather than trusting an
    inherited env — a `hermes: command not found` or an unset channel would silently swallow the very alert
    this watchdog exists to send. If
    `DISCORD_ERRORS_CHANNEL` is unset, the watchdog itself falls back to the home channel with a `⚠ ERROR`
    prefix (the fallback lives here, not in hermes).
  This closes the failures that would otherwise be silent — a dead/hung Bob, and any cron/pipeline give-up —
  per the §0/§9 loud-failures rule.

---

# PART III — RELIABILITY & OPERATIONS

> Governing principle: for an ADHD user, **silent failure of the boss-of-the-schedule is worse than a visible
> degraded mode** — a quiet Bob is indistinguishable from "nothing to do," and the externalized prospective
> memory just vanishes. This is the **system-failure** half of the §0 "two kinds of failure" separation, and
> it splits along the **two channels (§9):**
>
> - **Task channel** — when Bob can't *surface*, it degrades to "provide nothing, *clearly*": never a
>   guilt-wall, never a backlog-dump, never a fabricated task. Quiet here is mercy.
> - **Errors channel** — when a *dependency or pipeline* fails, it is **loud, immediate, and never
>   receptivity-gated** (`DISCORD_ERRORS_CHANNEL`, §12). Quieting a system failure is the one thing this layer
>   must never do. Quiet here is a bug.

## 15. Timezone + idempotency

- **Timezone — RESOLVED.** Set root-level `timezone: "America/Denver"` in `config.yaml` (single IANA string;
  default empty = server-local; no per-job override). The cron feature page is silent on TZ — it's documented
  only on the configuration reference. This closes the spec's long-standing open question. *(The live config
  currently has this EMPTY — R6b MUST-FIX, §14.)*
- **Idempotency contract** for every state-mutating proactive action:
  - Daily `p1`-set: guarded by the **per-day plan record** `forzare/state/plan-of-day.json` (§4c/Y13), **not**
    a p1 count. On entry read the record: a record for **today** ⇒ the day is already planned, so **resume only
    the missing `writes.*`** and never top p1 up past the recorded `selected_ids` — which distinguishes
    Bob-owned p1 from a p1 the *user* set directly in Todoist (the old "any p1 present" heuristic could not,
    §4c). A re-fire or the ±2h catch-up converges to the same day-plan idempotently. (The earlier "any p1
    present ⇒ no-op" rule is superseded — it would have mistaken a user's own p1 for "already planned.")
  - Kanban capture-pipeline kickoffs (§8b): **`hermes kanban create "fz-capture: <title>" --triage --created-by forzare
    --idempotency-key <inbox-task-id>`** (R7/W4 — `title` is a required positional, verified; the immutable
    `created_by == "forzare"` column is the forzare-card discriminator both watchdog scans filter on, DD11/MM1
    — the `fz-capture: ` title prefix is display-only, since `specify` rewrites the title; the key is the
    stage-1 Inbox task id, stable across retries) — re-firing returns the existing card, no dup; the brief is **not** a Kanban
    kickoff (its idempotency is the app-level guard above).
  - End-of-day rollover: keyed per day; safe to re-run.

## 16. Degraded-mode contract (dependencies fail visibly, never silently)

Two tiers, mapped to the two channels (§9): a dependency that **degrades but completes** is noted inline (task
channel); a dependency or job that **can't complete** escalates loud to the errors channel.

- **Weather API down/timeout** → brief still fires, weather block says "weather unavailable — assume layers"
  (degrade-and-note; no crash, no skip).
- **Google Calendar / `gog` auth expired** → surface a single actionable repair ("Calendar auth expired —
  re-auth: `gog auth …`") and proceed with the rest of the brief; never go silent.
- **`td`/Todoist unreachable** → say so once, hold surfacing, retry next tick — do not fabricate tasks. If it
  stays unreachable across retries, that's a *failure* → errors channel.
- **Liveness / non-completion — by primitive:**
  - **The brief (cron + skill bundle, §11)** is bounded by the cron **INACTIVITY** timeout
    (`HERMES_CRON_TIMEOUT`, default 600s *idle*; verified `cron/scheduler.py` — NOT `script_timeout_seconds`,
    which caps only a pre-run `--script`). An inactivity kill is a non-completion, i.e. a *failure* → logged
    to `~/.hermes/cron/output/` (§17) **and** posted loud to `#forzare-errors` (§9 two-channel; a *failed*
    turn is structurally un-silenceable, §12.2/R3 — the `[SILENT]` filter can never swallow it). *(600s of
    silence is generous for a brief that streams steps; the actual wall-clock **stale-run** alert — "the
    brief hasn't produced output today at all" — is the §14 forzare-ops watchdog's `cron/output/` scan, not
    this in-turn timeout.)*
  - **Kanban job steps (the capture pipeline, §8b)** carry **`--max-runtime 900` (DECIDED, Y7/§19)**; on
    exceed the dispatcher SIGTERMs→SIGKILLs (5s grace) and re-queues (`timed_out` event). **Re-queue restarts
    the whole card, not the step (no mid-run resume, §19)** — exactly why every stage is idempotent +
    check-before-create (§8b). A hung step never freezes the day, and a restart never duplicates a task; a card
    that exhausts its `failure_limit` after the timeout gives up (`gave_up`) → the watchdog routes it to
    `#forzare-errors` (§14).
- **Any job that exhausts its `failure_limit` and gives up** → **loud on the errors channel** (§12); the
  captured item is still safe in Inbox (§8b).
- **Native cron failure-summary delivery — a KNOWN leak, accepted with framing (V5/R2A5).** Verified
  (`cron/scheduler.py:2760-2786`): when a cron job fails, the scheduler delivers a
  `_summarize_cron_failure_for_delivery(...)` one-liner to **the job's own delivery target** — and *"failed
  jobs always deliver"* (a failed turn is structurally un-silenceable, §12.2). Since user-facing rituals
  deliver to the **home (task) channel**, their failure summaries land **there**, not (only) on
  `#forzare-errors`. This is a real leak of a system-failure notice onto the task channel, and there is **no
  suppression machinery to close it** — nor should there be, since quieting a failure is forbidden (§0/§9).
  **Ruling: accept with framing.** A system-failure notice on the task channel is **loud, not shame** — it
  names *the system* ("the morning brief job failed"), never the user, so it does not violate the no-shame
  contract (which is about *the user's* task slippage, §0). It is **system-voiced text, never user-shame.**
  And it is **not the only alarm:** the forzare-ops watchdog independently duplicates every cron/pipeline
  failure to `#forzare-errors` on a **best-effort ≈5-min** cadence (§14/X9), so the dedicated errors channel
  still carries the authoritative signal.
- **The two-channel invariant, restated precisely (W8/X9 — recorded leg-disagreement adjudication).** *Every
  system failure reaches `#forzare-errors`, on a **best-effort ≈5-min detection cadence** (NOT a hard W ≤ 5
  ceiling, X9)* — the watchdog `StartInterval` is **DECIDED at 300s** (§14/F1), which sets the *polling target*
  while the host is awake and no prior pass is still running (launchd skips intervals during sleep and
  runs them at wake; delivery is spool-retried under a Discord outage). The native cron failure summary **may
  additionally** appear on the job's own
  channel; it is **system-voiced text, never user-shame** (it names the system, not the user). **Rationale
  (why not a post-run router closing the leak at the source):** routing every cron failure through a bespoke
  post-run router would add **new critical-path machinery** (the exact bespoke code R4/R5 removed) and its own
  brief-latency/failure surface, for a leak that is already loud, correctly framed, and independently
  duplicated to the errors channel. **Adjudication:** leg A (round-2) accepted the "leak is fine, watchdog
  duplicates within ≤15 min" framing; leg B (round-3) rejected it as too slow / under-specified; the round-3
  ruling pinned a **≤5-minute** window, and **round-4 (X9) corrects that to a best-effort ≈5-min *target*** —
  a hard latency ceiling is not deliverable under launchd sleep/skip semantics, so claiming one would be a
  false invariant. The leak is still accepted with the system-voiced framing and the post-run router still
  rejected; only the latency wording weakens from "guarantee" to "best-effort target."
- **Gateway down or hung (the total-failure ceiling)** → cannot be handled *inside* forzare — the process is
  dead/wedged. **Restart is automatic** (`ai.hermes.gateway.plist` KeepAlive, §14); **detection + alerting is
  the external `forzare-ops watchdog`** (§14), which catches the hang KeepAlive misses and fires an
  **out-of-band** alert to `#forzare-errors` via **`hermes send --to discord:<#forzare-errors>`** (R2 — no LLM, no
  agent loop, no running gateway; relay phone/local as belt-and-suspenders). This is the single most important
  failure to make loud (§0/Part III: a quiet dead Bob is the worst case).

## 17. Observability + dry-run

- **Native cron audit:** every cron run is saved to `~/.hermes/cron/output/` even when delivery is `[SILENT]`
  — so "did the brief fire, what did it surface" is always inspectable.
- **V1 observability = the cron audit log (above) + `#forzare-errors` (§12).** That's the whole V1 story. The
  *behavioral* learning data is separate — §6a's owned calibration store (not an observability tool). **Langfuse
  agent tracing is post-V1 (§18a), not part of V1** — it's engineering tracing (turns/LLM-calls/tools), not
  the behavioral tuner, and isn't load-bearing.
- **Dry-run/staging path before go-live — and the honesty correction (V4).** `[SILENT]` and `--deliver
  local` suppress **DELIVERY ONLY** — they stop a message reaching Discord. They do **NOT** suppress a
  skill's **store writes**: a brief run under `--deliver local` will still `td task reschedule`, set `p1`,
  write the ledger, and create calendar blocks unless the skill itself is told to stand down. The earlier
  "without touching prod" claim was **false** and is replaced by an explicit read-only mode:
  - **Dry-run is honored by EVERY mutating skill (the read-only contract), with an OBSERVABLE artifact
    (R3A1).** The **complete writer inventory (X3/Y1/Y5):** `todoist-surface`, `calendar-write`,
    `eisenhower-plan` (writes `p1` + the `plan-of-day.json` record, Y13), the `forzare` classifier
    (schedule-override), `eod-roll` (roll + the map/journal + `decision-queue.json` fixed/stall records),
    `waiting-reconcile` (enqueues `waiting-chase` decision-queue records), `forzare-capture`,
    **`tomorrow-prep`** (writes `tomorrow-prestage.json`), **`followups-sweep` in SWEEP mode** (enqueues
    `sweep-candidate` decision-queue records), **`calibration-log`** (writes the calibration store), the
    **capture pipeline** (enqueues `triage-reraise` decision-queue records), and **`brief-assemble`'s
    prestage-CLEAR** (it consumes and then truncates `tomorrow-prestage.json`). When dry-run is active, each
    **appends each intended write, as one JSON record, to
    `forzare/state/dryrun-intents.jsonl`** — the ONE file a dry-run may write — and performs the real
    mutation NOT AT ALL: no `td` write (staged `td` also carries native `--dry-run`, Y4), no map/journal
    commit, no `calendar-write` (staged `gog` also carries `--readonly`/`--dry-run`, Y4), no
    `schedule-override.json` write, no `last-reconcile.json`/`task-lifecycle.json`/`mutation-journal.jsonl`
    write, **no `tomorrow-prestage.json` write-or-clear, no `plan-of-day.json` write, no `decision-queue.json`
    write, and no `calibration/` append**. **The one LIVE-only exception (R5A5):** the decision-queue **ack**
    (the `{id, gen, rev}` CAS that TOMBSTONES the head record, BB2) is written only by the live turn that receives the user's answer — never
    by any cron/dry-run path, so it never appears in the intents log. Each record carries `{ts, skill, op,
    target, args, run_id}`, so a staged run is verifiable by asserting the intent RECORD exists with the
    expected fields — never by inspecting the real store (which a correct dry-run leaves untouched).
  - **Enforcement is honestly PROMPT-LEVEL for v1 — the mode-check-first rule, centralized (X3).** v1 dry-run
    is a **forzare-layer prompt convention, not a hermes primitive and not a hard wrapper** — there is no
    engine-level default-deny. It is centralized as a **mode-check-first** rule inside the **shared
    mutation-helper** instructions (§4/W6, plan Task B0): every mutating skill's *first* step is "check the
    mode; if dry, intent-log only, return." Because every date/label/comment/calendar write already funnels
    through that one helper, the check lives in one place rather than being re-derived per skill. **No passage
    claims dry-run is *enforced*** — it is *honored* by the prompt contract, which is decided-before-runtime but
    not machine-guaranteed. A true **default-deny wrapper** (a code shim that refuses any real write while a
    dry-run flag is set) is booked as a **build-time hardening candidate in Phase H** — this docs-only design
    can't mandate engine code the plan doesn't own, so it is named as future hardening, not asserted as v1.
  - **Native CLI dry-run flags — REQUIRED in staged runs, UNDER the prompt mode (Y4, defense in depth).**
    Prompt-level is the primary contract, but for **external** mutations the staged runs ALSO pass the tools'
    **own** dry-run flags, so a skill that forgot the mode check still cannot mutate the outside world:
    - **`td`** — every mutation subcommand exposes **`--dry-run`** ("Preview what would happen without
      executing"): verified 2026-07-11 on `td task add / update / reschedule / complete / delete` and
      `td comment add`. Staged mutating `td` calls carry `--dry-run`.
    - **`gog`** — the global **`--readonly`** ("Block mutating API requests at runtime") and **`-n/--dry-run`**
      ("Do not make changes; print intended actions") flags both exist (verified 2026-07-11); staged
      `calendar-write` runs pass one so no 🤖-calendar event is really created.
    This is **defense in depth**, not a replacement for the prompt mode: the native flags stop **external**
    (`td`/`gog`) writes; the prompt mode + the mtime/hash gates (below) stop **owned-state** writes (the state
    files, which have no external CLI to carry a flag). The full engine-level **default-deny wrapper** across
    *both* classes remains the Phase-H hardening candidate. (B3's calendar acceptance is the exception — it runs
    a controlled **LIVE** [TEST]-keyed harness on Bob's own 🤖 calendar, R5A6, documented as a staging
    exception; the cron-staged path keeps intent-log purity.)
  - **Transport = the job/bundle INSTRUCTION variant, NOT an environment var (R3A4/W2).** A gateway-ticked
    cron job's agent turn runs inside the gateway process and does **not** inherit an ad-hoc shell
    `FORZARE_DRY_RUN=1` export — hermes strips undeclared environment from its children (verified: cron script
    subprocesses pass through `_sanitize_subprocess_env`, and the agent turn is never handed a caller-set
    var). So the dry-run directive is carried **ONLY in the job's prompt (`jobs.json`), NEVER in a bundle
    `instruction` file (OO8)**: every staged ritual job's prompt OPENS with an explicit directive — *"DRY RUN:
    record every intended write to `forzare/state/dryrun-intents.jsonl` and perform none."* Keeping it out of the
    bundle instructions is precisely what lets the go-live flip stay a **single-file (`jobs.json`) transaction**.
    The prompt-directive mechanism is the **primary** transport; a shell `FORZARE_DRY_RUN=1` export is honored
    only in the rare hand-run case where the child process does inherit it.
  - **The 23:00 eod-roll job is created DISABLED (or `--deliver local` with the dry-run instruction) until the
    go-live gate** — it must not silently reschedule real tasks during the staging window.
  - **Staging acceptance = ZERO production mutations, asserted across ALL stores (Y4/X3/AA1) — the NEGATIVE gate
    is INDEPENDENT of the intent log; intents are POSITIVE evidence only.** (positive) the intended writes are
    **present in `dryrun-intents.jsonl`** with the expected fields (proof the skill computed them); (negative)
    **every** store a mutating skill could touch is verified untouched by **before/after diffs that never consult
    the intent log** (AA1 — the earlier `--by me`-scoped, intent-target-scoped check is swept; `td activity`
    reports Bob's writes and the user's under the *same* account, so `--by me` cannot isolate a forzare leak and
    is dropped):
    - the Todoist **activity log** shows **no new forzare-authored change** on the **[TEST] fixture set** — a
      **before vs after diff, CURSOR-PAGINATED to exhaustion** (loop `--cursor` until empty, never page 1 only),
      on **both** the **`--type task`** and the separate **`--type comment`** streams (a skill that posts an
      auto-repair/if-then comment would otherwise slip past a task-only check). Scoping to the **[TEST] fixture
      fingerprint** (the tasks the harness itself seeded) — not the intent targets — keeps the check independent
      of the intent log *and* immune to the user editing their own tasks during staging;
    - the **🤖 calendar** shows no new staged event — a **[TEST]-scoped `gog calendar events`** before/after
      snapshot (calendar-write is the one external writer with no activity-log mirror);
    - the owned-layer **state + calibration stores** are unchanged by a **RECURSIVE content-hash snapshot** of
      `forzare/state/` **and** `forzare/calibration/` before vs after — enumerating **the real stores exactly**
      (`last-reconcile.json`, `task-lifecycle.json`, `mutation-journal.jsonl`, `schedule-override.json`,
      `tomorrow-prestage.json`, `plan-of-day.json`, `decision-queue.json`, `sweep-exclusion.json`, and every
      `calibration/` file) and **EXPLICITLY EXCLUDING `dryrun-intents.jsonl`** — because a dry-run *does* append
      to that one file, so including it in the negative hash would make the gate self-defeating (AA1). **The
      staging-harness RESULT files live OUTSIDE the state layer — `~/workspaces/Ivy/forzare/staging/e3-results/`
      + `staging/d1-harness-result.json` (MM7/LL6)** — so this recursive `state/` hash never sees a harness
      artifact and the never-rm / GC contracts (DD6/AA1) stand untouched; no per-file exclusion is added for them.
    A staged run is "clean" only when the positive check and every independent negative check hold.
  - So the loop is: stage with `--deliver local` **+ the dry-run instruction** → exercise the full pipeline →
    read `~/.hermes/cron/output/` + assert the intents log + assert zero real mutations → tune → then, at
    **go-live (plan G1): REMOVE the dry-run directive from EVERY job's prompt in `jobs.json` (bundles never carried it, OO8) — all six
    ritual jobs, including the two `--deliver local`-forever state-only ones (`waiting-reconcile`, the monthly
    someday-sweep), whose prompts also go live so they actually mark state (X1) — ENABLE/RESUME the
    23:00 eod-roll job, and flip delivery to `discord` for the four user-facing jobs only.** Staged cleanup
    **never** `rm`s a real state file — the rule is a **category, not a list (DD6): every file under
    `forzare/state/` and `forzare/calibration/`** (the sole exception being `dryrun-intents.jsonl`, which the
    go-live step truncates) — so the invariant never drifts as new state files are added. A correct dry-run wrote
    none of them, so there is nothing for cleanup to remove.

---

# PART IV — FORWARD PATH + OPEN

## 18. This is a Bob-only system; forward path if a second agent profile is ever added

**This surfacing engine is Bob's, full stop.** No other agent participates. (Elaine — the separate
email-triage agent in PLAN-v7 — is not part of this system. "Sierra" is a person, not an agent.)

- If a *second agent profile* is ever introduced for some other purpose, nothing here needs a redesign: cron
  jobs, skill bundles (atomic skills in a shared `skills.external_dirs`), the §9 firewall, and the delivery
  paths all stay. Kanban has **no per-profile isolation** — *data* separation is per-**board**
  (`~/.hermes/kanban/boards/<slug>/kanban.db`), so a new profile = its own board or just `--assignee` routing.
  **But a board is NOT execution isolation (W4):** the embedded dispatcher/notifier **enumerates every board
  on disk each tick** (verified `gateway/kanban_watchers.py:205-215`) and spawns any ready card whose assignee
  maps to a real profile — a "test board" is still dispatched. Execution isolation is a **non-spawnable
  assignee** (no matching profile — `has_spawnable_ready` gates on `profile_exists`, `kanban_db.py:6556`).
  Bob-only choices (single board, `default_assignee: "default"`, manual orchestration) don't paint the design
  into a corner.

## 18a. Post-V1 enhancements (deferred nice-to-haves — explicitly parked, not built)

These are **deferred, not excluded**: recorded here with rationale so they aren't forgotten, and deliberately
kept **out of V1**. Sequence is always: ship V1 first → then evaluate.

- **Per-channel delivery LEASE (post-V1 — closes the §12.3 residual interleave).** V1's single-writer story
  is procedural, not a lock (§12.3), so a live-session turn and a cron ritual *could* both emit a "next thing"
  in the same narrow window — accepted as rare and benign for v1 (both paths are receptivity-gated and each
  emits at most one thing; worst case is two short messages moments apart). If it ever proves annoying in
  practice, add a **per-channel delivery lease**: a short-held claim on the task channel so a second emitter
  in the window defers. This is exactly the bespoke machinery R4 removed, so it is **not built for v1** — it
  is booked here (and mirrored in the plan's Phase H) as the concrete hardening move.
- **Default-deny dry-run wrapper (post-V1 — hardens §17's prompt-level contract, X3).** V1 dry-run is honored
  by a **prompt convention** (the mode-check-first rule in the shared mutation helper, §17) — decided
  before runtime but not machine-guaranteed. A **default-deny wrapper** — a small code shim around the
  mutation helper that *refuses* any real `td`/calendar/state write while a dry-run flag is set, so a skill
  that forgot the mode check still can't mutate prod — would upgrade "honored" to "enforced." It is **not**
  built for v1 (this docs-only loop can't mandate engine code the plan doesn't own), and is booked here (and
  in the plan's Phase H) as the concrete hardening move.
- **Langfuse agent tracing (post-V1).** The user intends to adopt **Langfuse for agent tracing across their
  entire system**; for forzare specifically it could help **analyze how Bob services tasks** (turn /
  LLM-call / tool traces, latency, where the time goes). Hermes has **direct, native Langfuse support** (the
  built-in `observability/langfuse` plugin), and Langfuse can reportedly hook into Claude / Codex agent
  activity too — so the integration surface is plausible and worth it later.
  - **Why not V1:** not load-bearing — the cron audit log + `#forzare-errors` cover V1 observability — and it
    is **engineering tracing, not the behavioral tuner** (§6a's calibration store owns that; don't conflate
    them).
  - **What it needs first (deep research at that time):** Hermes↔Langfuse integration specifics; Langfuse's own
    capabilities in general; and crucially **whether it can trace forzare's subagent / Kanban work end-to-end**
    (the capture pipeline, §8b), not just top-level turns.
  - **Privacy gate to settle then:** traces include task content, so **self-host Langfuse** rather than
    shipping personal data to the Langfuse cloud — consistent with the owned/private-data principle (the §4c
    Marvin reasoning).
  - **Sequence:** ship V1 → adopt Langfuse system-wide → research the forzare integration → enable only if it
    earns its keep.

## 19. Open parameters / setup checks (genuine unknowns — verify at build)

- **RESOLVED (verified 2026-06-30 via `td` CLI) — "is this date fixed vs. surfacing?" (roll carve-out), with
  the V1 correction.** **The definition is LEDGER membership**, not the task fields: a task rolls **iff** it
  has a lifecycle-ledger entry (Bob wrote its date) **and** `current due == written_due` **and** it is
  date-only, today/overdue, not done (§4d/§8). The task-field checks are **secondary sanity guards** applied
  within that set — skip `due.isRecurring == true`, a `"T"` time-of-day, or a future `due.date`. **`deadline
  != null` is NOT a blanket exclusion** (V1): a deadline-bearing task with a *Bob-written lead-time surfacing
  due* is in the ledger and **rolls**; `deadline` marks "fixed" only for a task with **no** ledger entry (a
  user-dated deadline-day task). **Verified field structure** (`td task view --json`): timed due → `due.date =
  "2026-06-30T15:00:00"` (has `T`+time); date-only → `due.date = "2026-06-30"` (no `T`); recurring →
  `due.isRecurring = true`; deadline → top-level `deadline: {date, lang}` object, separate from `due`.
  **Detection is exact:** "has a time-of-day" = `"T" in due.date` (equivalently `len(due.date) > 10`).
- **cron missed-fire / catch-up — RESOLVED, and the earlier claim CORRECTED (verified 2026-07-11 against
  installed `cron/jobs.py:1456-1492`; grace math at `:475-504`).** A recurring cron job due while the gateway
  was down does **not** silently skip when it is very late. Two regimes: **(1) within the catch-up grace
  window** (`_compute_grace_seconds` = half the schedule period, clamped 120s–7200s; a *daily* job → the 2h
  cap) it replays as due; **(2) PAST the grace window** the code **fast-forwards `next_run_at` to the next
  future occurrence AND still executes once now** (`due.append(job)` after the fast-forward,
  `jobs.py:1462-1492`) — it fires exactly once at recovery, never zero times. The old "bounded ±2h, else
  **skipped**" statement was **wrong** (the ±2h is only the *catch-up/accumulate* window, not a fire/skip
  cutoff). Because a late fire can therefore land at an arbitrary time, the eod-roll keys off an **explicit
  reconciliation range** (`(last-reconcile.stored .. CEILING]`, CEILING = today at/after the 23:00 Denver
  cutoff else yesterday; §8/V3/R3A9/W5/X6), not
  the wall clock, so {on-time, ≤2h catch-up, past-grace recovery fire, defensive morning run} each reconcile a
  given day **exactly once**, a multi-day outage drains the whole gap in one pass with **one** `roll_count`
  tick per task, and a run that finds `stored ≥ CEILING` no-ops. **Build-time check (a test):** confirm the
  range/stamp guard prevents a double-roll when both a late cron fire *and* the morning brief attempt the same
  day, and that a ≥3-day outage ticks each carried task exactly once.
- **RESOLVED — Kanban mechanics corrected against upstream code (verified 2026-06-30, NousResearch/hermes-agent).**
  Several documented claims were fictional or wrong; the design follows the *code*, not the docs:
  - **No wall-clock firing.** There is **no `scheduled_at` / `--scheduled-at`** — the docs invented it. Kanban
    never fires itself on a clock; **cron is the only timer** (§10/§11).
  - **No mid-run resume.** A crash/restart re-runs the **whole card from stage 1**, not the failed step — so
    every multi-step job is **idempotent + check-before-create** (§8b dup-guard; §16).
  - **`workflow_template_id` is not a workflow primitive** — it's a half-built filter/tag column. The capture
    pipeline (§8b) uses **parent/child links + `--assignee` + manual orchestration**, never `workflow_template`.
  - **REST surface is authenticated** (docs implying an open REST port are wrong) — the private board (§9) is
    not incidentally exposed.
  - **Dispatcher runs inside the gateway** (dies with it; no standalone `kanban daemon`, §14) — so when the
    gateway is down, nothing runs; that total-failure case is **auto-restarted (KeepAlive) + alerted by the
    external watchdog** (§14/§16), *not* a silent no-op.
- **RESOLVED (verified 2026-07-03/04, installed hermes-agent) — delivery is headless-native; `bob-surface`
  plugin + `ctx.inject_message` removed (R1/R4/R5).** `ctx.inject_message` is **dead in gateway mode**
  (`hermes_cli/plugins.py:409-433` — needs an interactive CLI `_cli_ref`, returns `False` headless, returns
  "queued" even in CLI mode). Replaced by the three verified paths (§12.1): cron Discord delivery
  (`cron/scheduler.py:709-745`, `:839-870`; `gateway/run.py:18151-18161`; `gateway/delivery.py:448`), one-shot
  cron / `trigger_job` (`cron/jobs.py:774`, `:1133`), and the clarify tool → native Discord buttons
  (`tools/clarify_tool.py:20`; `plugins/platforms/discord/adapter.py:4778-4892`, `:6535-6606`). The old **"OPEN
  — `ctx.inject_message` exact return/failure semantics" item is RESOLVED: dead in gateway mode, replaced.**
- **RESOLVED (verified 2026-07-11) — `[SILENT]` suppression differs by delivery path (R3).** On the
  **gateway** (live-session) path suppression is **exact whole-response match, success-only** — a failed turn
  can never be silenced (`gateway/response_filters.py:13-52`; `gateway/delivery.py:30-40`). On the **cron**
  path it is **more lenient**: whole-response, first line, last line, **or** any `[SILENT]`-prefixed response
  (`cron/scheduler.py:244-287` — `_CRON_SILENCE_TOKENS` + `upper.startswith("[SILENT]")`). Scope the
  "exact-match only" claim to the gateway; the cron staging path (§17) relies on the lenient behavior. Build
  test matrix (§12.2): exact, prefix, first-line, last-line, failed, substantive.
- **RESOLVED (verified 2026-07-11, `cron/scheduler.py`) — the cron agent-turn bound is an INACTIVITY timeout.**
  A cron-kicked agent turn (the brief, §11) is killed after `HERMES_CRON_TIMEOUT` seconds of **no activity**
  (default **600s**), not on wall-clock — so it may run for a long time while streaming steps. Iterations are
  capped by `agent.max_turns` (default 90). `script_timeout_seconds` / `HERMES_CRON_SCRIPT_TIMEOUT`
  (`_get_script_timeout`, `cron/scheduler.py:1508-1536,1631`) bounds only an optional pre-run `--script`,
  **not** the agent turn — correcting the earlier "brief bounded by `script_timeout_seconds`" claim.
- **RESOLVED (verified 2026-07-03/04) — Discord adapter has NO inbound reaction events (R8); outbound
  ack-reactions exist free.** Only `on_ready` / `on_message` / `on_voice_state_update` are registered — there
  is **no inbound reaction event**, so **one-tap designs must use clarify buttons, never emoji reactions**
  (§12.1c). Separately, **outbound ack-reactions are free** (👀 processing → ✅/❌ done, `DISCORD_REACTIONS`
  default true) — a zero-cost "Bob heard you" cue on the task channel; use it, but never as an *input* channel.
- **RESOLVED (verified via `hermes send --help`) — out-of-band alert is `hermes send --to discord:<channel>` (R2).**
  A CLI that needs no LLM, no agent loop, and **no running gateway** for bot-token platforms — so it can report
  the gateway's own death (§14/§16). The prior "curl the Discord webhook directly" phrasing is dropped; the
  relay phone/local push stays as belt-and-suspenders.
- **NEW build check — do bundle invocations mirror to native Discord slash commands? (R5).** Hermes mirrors
  **plugin-registered** commands to native slash commands (`plugins/platforms/discord/adapter.py:3625-3733`)
  while **skills consolidate under a `/skill` group.** Confirm at build whether `/forzare-*` bundle/skill
  invocations get native `/forzare-*` autocomplete. **If not, and recognition-over-recall suffers**, build the
  documented **micro-shim plugin** that registers command **names only** (no delivery, no hooks, no lock;
  §12.5). Default: no plugin.
- **OPEN — `/forzare` activation + classification reliability.** Verified: Hermes does description-driven,
  model-judged skill activation (so phrases-in-description auto-fire works). **Still to validate at build:** (a)
  does it reliably fire on the activation phrases mid-conversation without over-firing on passing mentions
  (tune the description + the "right now" scoping); (b) classification accuracy across the signal set (gym /
  shift / energy / location) — the confirm-on-low-confidence guard (§3B) is the backstop (a clarify button on
  the live-session path, §12.1c). The `pre_gateway_dispatch` hook is no longer used for state signals.
- **RESOLVED (code-verified 2026-06-30, `kanban_db.py` / `kanban.py`) — Kanban `priority` + retry resolution.**
  `priority` is a free-form `int` (default `0`, no 1–4 scale), **higher = dispatched sooner** (`ORDER BY
  priority DESC`, `kanban_db.py:6768`). We still don't use it for user-task ordering (Todoist owns that, §9) —
  it's only an optional tiebreaker for capture cards. **Retries:** per-task `--max-retries` wins → else config
  `kanban.failure_limit` → else `DEFAULT_FAILURE_LIMIT = 2` (`kanban_db.py:6284-6292`). It's a *trip threshold*,
  off-by-one from "retries" — **`failure_limit: 2` blocks on the 2nd consecutive failure** (= 1 retry) →
  `gave_up` event → errors channel (§16). So our §14 `failure_limit: 2` = "one retry, then give up + alert."
- **RESOLVED — Todoist access is the `td` CLI, not MCP.** `td` is a command-line tool with no MCP mode; Bob
  shells out to it and learns it via the installed **`/todoist-cli` skill** (single source of `td` knowledge).
  (Earlier drafts wrongly specced an `mcp_servers.td` entry — removed.) Build need: `td` on PATH +
  authenticated in Bob's environment.
- **RESOLVED (code-verified 2026-07-11, `curator.py` / `skill_usage.py`) — NO curator pin needed; repo-authored
  skills are NOT curation candidates (AA11 supersedes the earlier "pin the atomic skills").** Verified:
  `list_agent_created_skill_names()` (the curator's managed/GC list) includes a `SKILL.md` skill **only if its
  `.usage.json` record is agent-created** (`_is_curator_managed_record` requires `created_by == "agent"` or
  `agent_created == true`). The forzare skills are **chezmoi-dropped, not agent-created**, so they carry no such
  record and are **never** in the curator's list — the curator can never archive/consolidate them.
  `hermes curator pin <name>` on one *succeeds* (writes `pinned: true`, since `is_curation_eligible` returns
  true for a non-bundled/non-hub skill) **but protects against a transition that can never target the skill** —
  a no-op-for-protection. So the earlier pin step is **dropped**; the gate is **installed PATH + content HASH**
  (§13 boot-integrity check). Bundle YAMLs are likewise not curator-managed (the curator walks only `SKILL.md`
  dirs).
- **RESOLVED (code + live-probe verified 2026-06-30; probe re-verified live 2026-07-12, OO1 — REVERTS the round-13 MM2 `:8642` decision) — gateway liveness probe.**
  The DECISION: the watchdog probes the **webhook platform's** unauthenticated **`GET http://127.0.0.1:8644/health`**
  (`gateway/platforms/webhook.py:195` `add_get("/health")`, DEFAULT_PORT 8644 at `webhook.py:73`; static JSON
  `{"status": "ok", "platform": "webhook"}`). The endpoint is **present iff the webhook platform is enabled**,
  and the **managed env pins it enabled**: `dot_hermes/private_dot_env.tmpl:15` sets `WEBHOOK_ENABLED=true`
  (verified in source; the Phase-A prerequisite is to assert this post-apply). Verified live 2026-07-12:
  `:8644/health` answers `{"status": "ok", "platform": "webhook"}` (exit 0) — the round-13 "`:8644` dead under
  `WEBHOOK_ENABLED=false`" reading was a transient/false observation, corrected here. **No new secret and no
  API-server key are added**: the probe rides the platform the managed env already enables. Watchdog: `curl -fsS -m 3
  http://127.0.0.1:8644/health`, branch on exit code — **0 = up, 28 = hung** (loop wedged: accepts TCP, no
  HTTP reply), **7 = down**. This is the hang KeepAlive + PID-checks miss (a hung gateway keeps a live PID;
  `gateway_state.json.updated_at` doesn't advance when idle). **NOT `:9119`** — that's the separate dashboard
  process, which infers "running" from the PID file and so reports a *hung* gateway as alive (useless for hang
  detection). **Alert path (R2):**
  **`hermes send --to discord:<#forzare-errors>`** + relay phone/local — independent of the gateway (§14).
  This probe is the health half of the **forzare-ops watchdog** (§14), whose script + plist are modeled on the
  existing `osquery-uptime-watchdog.sh` / `com.webdavis.osquery-uptime-watchdog.plist.tmpl` (chezmoi-managed).
- **RESOLVED (verified live 2026-07-11) — gateway plist `KeepAlive` = `true`.** `~/Library/LaunchAgents/
  ai.hermes.gateway.plist` now sets a plain `<key>KeepAlive</key><true/>` (changed from the 2026-06-30
  `{SuccessfulExit: false}` reading) → restarts on **any** exit, clean or crash. Crash-self-heal conclusion
  unchanged (§14).
- **Decided config values** (live in `forzare/` skill config, hand-editable — fixed defaults, not
  runtime-adaptive): weather thresholds wind>17 / rain / <50°F / >90°F; wake anchor 05:15; **end-of-day cron
  23:00** (fixed, §8); `@waiting` reconcile cron 02:00; stall threshold 2 (§4d); calibration α 0.15 (§6a);
  **root `session_reset` mode=both at 04:00 (R6a, §14)**; **`cron.max_parallel_jobs: 1`** +
  **`kanban.max_in_progress_per_profile: 2`** + **`kanban.auto_subscribe_on_create: false` (Z1, §9/§14 — the
  firewall guard; default `True`)** (§14); **receptivity v1 rule (V8, §6a): initiation window
  N = 30 min, withhold when trailing-24h dismissals ≥ D = 3 OR `surfacings_today` ≥ S = 8** (learned refinement
  post-V1); **task-bankruptcy trigger > 25 candidates in the sweep POOL — undated someday ∪ long-cycling dated (R2A16, §4c)**; **post-activation boost
  prior ≈ ≤~30 min, pending personal data (V10, §6/§6a)**; **commute constants `commute_prep_minutes: 30` +
  `commute_travel_minutes: 25` (X12, §3a — hand-editable; the §3a/W13 leave-time alarm fires at work-block
  start − prep − travel = start − 55 min)**; **watchdog `StartInterval: 300s` (best-effort ≈5-min detection,
  X9, §14)**; **watchdog gateway probe `:8644/health` — the webhook platform's static-JSON health route, present
  iff `WEBHOOK_ENABLED=true`, which the managed `.env` pins (`private_dot_env.tmpl:15`; OO1, §14/§16 — assert
  post-apply, no bearer key added)**;
  **capture-card `--max-runtime 900` (Y7, §8b/§16 — verified `hermes kanban create --help`)**;
  **mutation-journal retention 45 days (Y5, §4d/§8a — the calibration correlation window; the prunable
  lifecycle map has no retention window, it prunes on task terminal state)**; **stale-p1 flag age 48h (AA2,
  §4c/§8 — a user-set p1 older than this is queued once, never auto-cleared)**; **bankruptcy stale-DATED-active
  definition `roll_count ≥ 10` AND no-progress ≥ 30 days (R7A5/AA6, §4c)**; **block-boundary soft pre-warning
  time = block_start − prep − travel − 30 (R7A2/AA7, §3/§3a/§13 — 13:35 for the 15:00 block, distinct from the
  14:05 leave-time alarm)**; **watchdog ritual-absence + stale-triage grace 30 min (AA8/AA5, §14)**;
  **`task.add` API-propagation window 120s (GG4/FF3, §8a — the eventual-consistency bound the crash-heal keys off
  the journal record's `created_ts`; DECIDED, and a build task MEASURES the live `td task add`→search propagation
  and REJECTS the 120s bound if the observed round-trip is unsafe, II5)**; **because the bound is empirical, a
  past-window replay still fails closed — it fires only on TWO consecutive empty-success marker searches ≥30s
  apart, never on elapsed time alone (II5)**. (No `@q2`
  flag, and no lifecycle labels — the roll counter is the private lifecycle map, §4d/§5d.)

---

## Provenance

Hermes mechanics verified 2026-05-29 against the live docs (cron, kanban, kanban-tutorial, plugins, skills,
built-in-plugins, tools, tool-gateway, configuration, webhooks, curator) via a multi-agent
ground→design→adversarial-verify workflow; 9/11 core claims SUPPORTED verbatim, 2 PARTIALLY (bundle
field-optionality + `metadata.hermes.config` shape — both refined above, not refuted).

**2026-06-30 — upstream-code verification + gap-closure pass.** Cloned NousResearch/hermes-agent and checked
the Kanban/cron claims against source, correcting several fictional doc claims (§19: no `scheduled_at`, no
mid-run resume, `workflow_template_id` is a filter column, authenticated REST, ±2h cron catch-up). `td`
due/deadline field structure verified via the live CLI (§19). Design decisions locked this pass: the
**four-layer model** (cron/Bob/Kanban/Todoist, §9); the **§8b capture-processing pipeline** (5 staged subagent
steps, 4 inline routing cases, Inbox-as-staging, stage-3 gate, idempotent dup-guard); the **two-channel
invariant** (no-shame task channel vs always-loud errors channel, §9/§12/§16); and the **brief as
cron+skill-bundle, not Kanban** (§11). Pre-revision spec backed up to `~/workspaces/backups/`.

**2026-07-03/04 — delivery/plugin code-verification + hardening (R1–R8), folded in on the 2026-07-11 port.**
Verified against the installed hermes-agent at `~/.hermes/hermes-agent`: `ctx.inject_message` is dead in
gateway mode (R1) → delivery rebuilt on three headless-native paths (cron Discord delivery, one-shot
cron/`trigger_job`, clarify-tool native buttons, §12.1); the out-of-band alert path is `hermes send
discord:<channel>` (R2, §12.4/§14/§16); the `[SILENT]` suppression guarantee is exact-match + success-only
(R3, §12.2); the single-surface lock is replaced by procedural single-writer discipline (R4, §12.3); the
`bob-surface` plugin dissolves to zero custom code by default, with a documented micro-shim fallback (R5,
§12.5/§19); the Discord `session_reset` config + the three live-config MUST-FIX drifts (empty `timezone`,
`kanban.auto_decompose: true`, `hermes-achievements` enabled) are folded in (R6, §14); the §8b stage-2 kickoff
is `hermes kanban create --triage --idempotency-key` + `hermes kanban specify` (R7, §8b/§15); and the
consistency sweep resolved the `ctx.inject_message` OPEN item and added the no-inbound-reactions /
free-outbound-ack-reactions fact (R8, §19).

**2026-07-11 — round-1 adversarial-review hardening (re-verified against the installed reality).** Two
independent reviews + a union pass drove this wave; every command/flag/config claim below was re-checked
against `td`/`gog`/`hermes --help`, the hermes-agent source, and the live `config.yaml`. Key corrections:
"Bob" is the **persona**; the hermes profile (and every Kanban assignee) is **`default`** — no profile named
`bob` exists (§8b/§10/§14; `bob → hermes -p default` alias). The **`@rolled`/`@stalled` marker labels are
replaced by a private lifecycle ledger** (`forzare/state/task-lifecycle.json` — `roll_count`/`written_due`/
`last_escalated`), which self-heals date provenance and keeps all failure-shaped state off the user's tasks
(§4d/§5d/§7/§8/§8a; the rejected marker-label alternative is recorded inline for PR-review override). The
`session_reset` stanza is a **root** key (not `platforms.discord.*`), `mode: both`, with a verified `notify`
subkey (§14); the kanban key is **`max_in_progress_per_profile`** and `cron.max_parallel_jobs` is pinned to 1
(§14); `plugins.enabled` is treated **additively** (never strip `platforms/discord` or the providers) and the
`hermes-achievements` fix is a **keep-out guard** (it is not currently enabled) (§14). Embedded commands were
regenerated from installed help: `td … --json` output is an **envelope** (`.results[]`), label names are
**unprefixed**, stage-1 capture uses structured `td task add` (never `quickadd`, which would parse dates
pre-classification), `gog calendar calendars` (not `list`), `hermes send --to discord:<channel>`, and the
watchdog is re-scoped as the **forzare-ops watchdog** (health probe + a cron/output + kanban-DB failure scan
routed to `#forzare-errors`). The brief's real bound is the cron **inactivity** timeout (600s idle), not
`script_timeout_seconds` (§11/§16). The Sunday question is **decided** — one daily cron, content
schedule-derived (§1/§2). Pharmacology + effect-size attributions were made conservative (§5a/§6/§6a/§7).

**2026-07-11 — round-2 adversarial-review hardening (fixes to round-1's own gaps; re-verified against the
installed hermes-agent).** A second adversarial pass caught half-applied round-1 fixes and imprecise
code-claims; every corrected claim below was re-checked against installed source. Key corrections: the **roll
set is now DEFINED by the lifecycle ledger** (entry + `current due == written_due`), the four field checks
demoted to secondary sanity guards, and the deadline contradiction resolved — a Bob-written lead-time due on a
deadline task *is* in the ledger and *does* roll (§4d/§8/§19, V1). The **ledger I/O contract** is specified
(mkdir-sentinel lock, tmp+rename atomic writes, journal-then-commit order, crash-healing rule; §8a, V2). **Cron recovery
was corrected to the verified code** (`cron/jobs.py:1456-1492`): a past-grace job **fast-forwards but still
fires once**, never silently skips — so the eod-roll keys off an explicit reconciliation date, not the wall
clock (§8/§19, V3). **Staging isolation made honest:** `[SILENT]`/`--deliver local` suppress *delivery only*;
side-effect-free staging needs `FORZARE_DRY_RUN=1` honored by every mutating skill, and the 23:00 roll job is
disabled until go-live (§17/§12.4, V4). The **native cron failure-summary leak** to a job's own target is
documented as accept-with-framing + watchdog duplicate (§16, V5); the **one-thing invariant** is restated as
"at most one per response, receptivity-gated" (not a mutex), with the delivery-lease booked post-V1
(§12.3/§18a, V6). **Bundles are skill-loaders, not sequencers** (`skill_bundles.py:286-340`) — ordering now
lives in each bundle's mandatory instruction block (§11/§13, V7). Receptivity/initiation made computable
(N=30 / D=3 / S=8; §6a/§19, V8); the **watchdog Kanban scan** corrected to the verified schema — alert on
`blocked` WITH `consecutive_failures>0`, `timed_out`/`crashed`/`gave_up` events, excluding awaiting-user
blocks (§14, V9/R2A6). **Mehren** corrected: benefit ~10 min post-exercise, no effect at ~33 min, the ~1–2h
window removed (§6/§6a, V10). Plus job-id extraction rebuilt to parse `Created job:` (12-hex), apply
checkpoints moved inline per phase, `td task reschedule` noted as recurrence-preserving, d=0.99 re-attributed
to **Toli et al. 2016**, and the §8a/§8b dangling cross-refs re-pointed (R2A1/R2A2/R2A8/R2A14/R2A17/R2A18/
R2A20/R2A22/R2A24).

**2026-07-11 — round-3 adversarial-review hardening (fixes to round-2's own interference bugs; every claim
re-probed against the installed reality).** A third pass caught places where round-2 fixes contradicted each
other; each correction below was verified live. **Cron rituals must ATTACH their bundles via `--skill`**
(W1): `_build_job_prompt` (`cron/scheduler.py:1690-1889`) expands only `job.skills` and appends the prompt as
inert instruction text — a slash-command prompt never executes (§11/§14). **Dry-run made observable** (R3A1):
under the dry-run **instruction** (the transport — a gateway-ticked job inherits no shell env, R3A4/W2),
every mutating skill appends intended writes to `forzare/state/dryrun-intents.jsonl` (the one file dry-run
may write); staging assertions target the intent records, never the real store; go-live removes the
directive and resumes EOD (§17/§8a/§12.4). **The date-mutation verb is state-chosen** (W6, probed live):
`td task reschedule` errors `NO_DUE_DATE` on an undated task, so initial dating uses `td task update --due`
and reschedule is re-dating only — one centralized layer, timed/recurring never mutated, the residual blanket
`deadline != null` exclusions swept (§4/§8/§8a). **The eod-roll closes a RANGE** (R3A9/W5): days
`(stored .. CEILING]` with CEILING = today at/after the 23:00 Denver cutoff else yesterday (X6), seeded
stamp = Denver yesterday at install, a multi-day outage
drains in one pass with EXACTLY ONE `roll_count` tick per task (an outage is Bob's failure, not the user's),
and `stored ≥ CEILING` derives the duplicate-fire no-op (§8/§8a/§19). **The two-channel invariant is
restated with its window** (W8/X9 — a recorded leg-disagreement adjudication): every system failure reaches
`#forzare-errors` on a best-effort ≈5-min cadence (watchdog `StartInterval` 300s, not a hard ceiling); the native cron failure summary may
additionally hit the job's own channel as system-voiced text; the post-run router was rejected as new
critical-path machinery (§0/§12.4/§14/§16). **The watchdog predicate is event-based** (W9): `block_task`
does not clear `consecutive_failures` (`kanban_db.py:4383`), so status+counter false-alarms on a
recovered-failure-then-user-block card — alert only on `gave_up`/`crashed`/`timed_out` (§14). **Capture
pipeline concretized** (W4): pinned `forzare-capture-pipeline` skill attached per card, mandatory `specify`,
Inbox-task-id idempotency key, parent-conversation callback, and test isolation by NON-spawnable assignee —
the dispatcher enumerates every board on disk (`kanban_watchers.py:205-215`), so a board is not execution
isolation (§8b/§15/§18). Plus: `eisenhower-plan` gains the decided `replan` mode (W10); the one-per-response
invariant worded precisely with the brief as the bounded exception (W12, §0/§12.3); the §3a hard-stop alarm
owned by `calendar-write` on the 🤖 calendar (W13); calibration initiation excludes journaled forzare writes
(W7, §6a); `[SILENT]` verified by direct filter-function probes (R3A7, §12.2); apply gates take explicit
file lists — `chezmoi diff` on a directory is non-recursive, fixture-verified (W3/R3A8); `gog calendar list`
is the events alias and `cal` aliases the calendar group (R3A10); kickoff titles are required positionals
(R3A11); the §4b row renamed `Active now` (R3A12); fixture reads use `--filter`/`--all` (R3A5); the
phrasing-rotation directive is stated once and named by each consumer (R3A17).

**2026-07-11 — round-4 adversarial-review hardening (fixes to round-3's residual gaps; every command/flag
re-probed against the installed reality).** A fourth pass tightened the capture-flow ordering, the ledger's
provenance model, and the honesty of the latency + dry-run claims. **Capture kickoff re-ordered** (X2, verified
`kanban_specify.py`/`kanban_db.py:4574` + `hermes_cli/kanban.py:685`): a `--triage` card is not dispatchable,
so the PARENT runs `hermes kanban specify` **immediately after create** (it performs the `triage → todo`
transition that permits dispatch), then `hermes kanban notify-subscribe <id> --platform discord --chat-id
<home>` defines the parent-callback transport; stage 2 begins only at dispatch, never inside triage, and no
longer calls specify itself (§8b). **The ledger gains a `kind` field** (X5): `surfacing`/`leadtime` roll,
`waiting_checkback`/`user_fixed` never roll and never tick (§4d/§8), and it **widens into the forzare mutation
journal** (X11 — same store, typed `date-op`/`p1`/`label`/`comment`/`calendar`) so the §6a/W7 calibration
exclusion is actually complete; the reducer pages the `td activity` cursor and queries `--type comment`
separately (§6a). **The EOD ceiling is by invocation mode + Denver cutoff** (X6): CEILING = today at/after the
23:00 Denver cutoff, else yesterday — resolving the latent "never ≥ today" tension for the 23:00 fire itself;
the install seed is Denver yesterday (§8/§8a). **Sweep decisions became a persisted QUEUE** (X7/R4A6):
`followups-sweep` SWEEP mode enqueues ≤5 (or the >25 bankruptcy offer) to `sweep-candidates.json`, and the
brief emits only the HEAD item as its one decision (§2/§4c/§8a). **Latency stated honestly as best-effort**
(X9): the watchdog's ≈5-min cadence is a launchd polling target, not a hard ≤5-minute ceiling (§0/§14/§16).
**Dry-run honesty + completeness** (X3): the writer inventory now names `tomorrow-prep`, SWEEP-mode
`followups-sweep`, `calibration-log`, and `brief-assemble`'s prestage-clear (with their state files in the
forbidden-write + mtime lists), and enforcement is stated as prompt-level (mode-check-first in the shared
mutation helper) with a default-deny wrapper booked in Phase H — no false "enforced" claim (§17). Plus: the
if-then stall lever gets a named owner in `todoist-surface`'s `roll_count ≥ 2` branch, persisting an
agent-proposed cue to the task description (X13, §7); EOD only **marks** the stall and the brief **delivers**
it (R4A10, §2/§8); the §4c/§15 idempotency guard is the per-day plan record `plan-of-day.json` (Y13/R6A2 —
superseding the earlier "any p1 present" heuristic); the watchdog
adds a `jobs.json` `last_delivery_error` scan for delivery-only failures (X8, verified `cron/jobs.py:1193`,
§14); commute constants `commute_prep_minutes: 30` / `commute_travel_minutes: 25` are decided (X12, §3a/§19);
and the `VALID_STATUSES` citation is corrected to `kanban_db.py:101` (R4A13, §14).

**2026-07-11 — round-5 adversarial-review hardening (re-probed against the installed reality).** A fifth pass
unified the decision paths, deleted a non-existent transport, and layered the staging isolation. **ONE unified
decision queue** (`forzare/state/decision-queue.json`, Y1/R5A1): every brief-time decision source
(`waiting-chase` / `fixed-redecision` / `stall-decision` / `triage-reraise` / `sweep-candidate`) enqueues to
one head-item-only queue; the brief delivers EXACTLY the head `pending` record, which **replaces** the do-now
close (a decision and an action never both appear, §0/§2/§4c/§7/§8/§8a); `sweep-candidates.json` merged in; the
**ack is a live-only write** by the turn that receives the answer (R5A5). **The `notify-subscribe` callback
design is DELETED** (Y2, verified `hermes kanban --help`: `notify-subscribe` routes **terminal events only**,
no decision events, onto the home channel — a firewall breach with a dispatch race) — capture cards carry no
subscription; decision cards re-raise via the queue, failures reach `#forzare-errors` via the watchdog
(§8b/§9/§12). **Task bankruptcy is a REVERSIBLE UNDATE** (Y3): the batch op undates the frozen, journaled id
set back to hidden someday — never delete/complete/archive (the Todoist parent-delete cascade + the separate
2026-05-20 vault refactor, cited distinctly R6A7) — with a
bounded summary, a named confirmation, and idempotent partial-failure recovery (§4c/§19). **Native CLI dry-run
flags are layered UNDER the prompt mode** (Y4, verified: `td … --dry-run` on add/update/reschedule/complete/
delete + `td comment add --dry-run`; `gog --readonly` / `-n/--dry-run`) — defense in depth over external
writes; the mtime/hash gates extend to ALL stores incl. the comment-type activity query, a [TEST]-scoped
calendar snapshot, `schedule-override.json`, `plan-of-day.json`, and `decision-queue.json` (§17). **The
lifecycle store SPLITS** (Y5/R5A11): a prunable lifecycle MAP (`task-lifecycle.json`, pruned on terminal
state) + an append-only mutation JOURNAL (`mutation-journal.jsonl`, retained 45 days), the journal enum gaining
**`description`** so the §7/X13 if-then cue (which Todoist reports as a bare `updated`) is journaled and the W7
exclusion stays complete (§4d/§6a/§8a). **The morning-plan guard is a PER-DAY PLAN RECORD** (Y13,
`plan-of-day.json` — resume-missing-writes, distinguishes Bob-owned from user-set p1), replacing the any-p1
heuristic (§4c/§15). Plus: capture cards carry **`--max-runtime 900`** (Y7, verified `hermes kanban create
--help`); the §6 "morning peak always" framing corrected in place (R5A10); the 02:00 unblock signals are gog
calendar + Todoist activity only, no Discord (R5A12); the mutation-journal 45-day retention + max-runtime are
recorded as decided config (§19).

**2026-07-11 — round-6 adversarial-review hardening (re-probed against the installed reality).** A sixth pass
closed identity/concurrency gaps and swept the last stale guards. **Kanban subscription firewall guard**
(Z1/R6A1): the in-gateway kanban **tool** create path auto-subscribes a platform-bound chat to a card's
terminal events (verified `tools/kanban_tools.py:843,858-898`, gated by `kanban.auto_subscribe_on_create`,
default `True`, `hermes_cli/config.py:1348`) — so forzare's capture flow uses the **subscription-free CLI
`hermes kanban create`** as a hard rule and pins the key **`false`** as belt-and-suspenders; a test capture
leaves **no** subscription row (§8b/§9/§14/§19). **Decision-queue identity + concurrency contract** (Z2): each
record gains `{id (content-derived dedup key), enqueue_ts, rev}`, total order `(class-rank, enqueue_ts, id)`
with class-rank waiting-chase>fixed-redecision>stall-decision>triage-reraise>sweep-candidate (R6A10); all queue
mutations run under the map/journal lock + atomic-replace, producers dedup by `id`, and the ack is a
compare-and-set on `{id, rev}` of the record actually shown (§2/§8a). **Journal completeness + per-type
healing** (Z3): the enum gains `task.add` + `task.complete` (aligning with the intent-op vocabulary), and
pending→commit→heal is defined per `type` with a type-specific verification predicate (comment content lookup,
calendar event-key lookup, current-value compare for label/p1/description, dedup-key lookup for task.add, task
state for complete) — the map keeps its 4-field schema, operation records live only in the journal, with an
after-write crash fixture per type (§4d/§8a). **Never `source` the managed `.env`** (Z4): the live file carries
an unquoted path with spaces that crashes a strict shell, so the watchdog + checkpoints **dotenv-parse** only
`DISCORD_HOME_CHANNEL`/`DISCORD_ERRORS_CHANNEL` (§14). **The exact `work_schedule` schema is a per-weekday map +
alt-Sunday anchor** and the block-boundary/gym crons are **DOW-aware derivations** (the boundary DOW is
`0,2,4,6`, its time the R7A2 formula = `block_start − prep − travel − 30` = `35 13 * * 0,2,4,6`; never a
flat `block_start`, never firing on off days; the alt-Sunday job fires weekly and the skill no-ops on OFF
Sundays) (Z9/R6A5, §2/§13). **§15's stale "any p1 present ⇒ no-op" guard is retired** for the per-day plan
record (Z10/R6A2, §4c/§15). **Task bankruptcy is TWO-class** (Z13): stale DATED actives UNDATE, undated someday
items RETIRE onto a `sweep-exclusion.json` list (never re-proposed, reversible by deleting the entry), and the
two motivating incidents — the Todoist parent-delete cascade and the separate 2026-05-20 vault refactor — are
cited distinctly (R6A7, §4c/§8a). **Brief response-structure rule** (Z12): a non-empty queue makes the head
decision the ONLY actionable line, weather + activation rendered as non-actionable context (§2). Plus: the
02:00 unblock re-date rewrites the ledger `kind` `waiting_checkback → surfacing` so it rolls, and the §13
"Discord" unblock mention + the §8a "--type task includes comments" claim are corrected (Z14, §8/§8a/§13).

**2026-07-11 — round-7 adversarial-review hardening (re-probed against the installed reality).** A seventh pass
closed leak-gate, ownership, healing, capture-flow, and integrity gaps. **Leak gate rebuilt** (AA1/R7A1/R7A7/
R7A8): the negative staging gate is now **INDEPENDENT of the intent log** (intents = positive evidence only) —
before/after diffs of task + comment activity **cursor-paginated to exhaustion**, a [TEST]-scoped calendar
snapshot, and a **RECURSIVE** content-hash of `state/` + `calibration/` that **explicitly EXCLUDES
`dryrun-intents.jsonl`** (including it was self-defeating); the `--by me` scoping is swept (Bob writes as the
user's account) (§17). **p1 ownership** (AA2): EOD clears **only the day's `plan-of-day` `selected_ids`**, never
a user-set p1; user p1s count toward the ≤3 budget (`max(0, 3 − user_p1_count)`); a user p1 > 48h → one
`stale-p1` decision-queue item, never auto-cleared (§4b/§4c/§8/§13). **Three-way healing** (AA3): every journal
op records `{old_value, intended_value, external_marker?}` and heals absent→replay / intended→commit /
OTHER→abort+flag (never overwrites user state); `task.add` has **no native idempotency** (verified `td task add`
— dedup by the BB3/EE1 five-step state machine: pre-persisted intent + create-time marker + immediately-journaled
returned id, healed by-window, NOT a content search); the waiting-clear+redate+kind-flip is ONE composite
pending transition; the MAP stays 4-field (operation records in the journal only) (§8a). **Decision-queue
schema** (AA4): classes grow to eight (adds `q1-conflict` above waiting-chase, `stale-p1` tied with
fixed-redecision, `bankruptcy-offer` lowest); **`id` = STABLE `class:task_id`, content-INDEPENDENT** (a changed
`proposed` updates IN PLACE + `rev++`, not a duplicate); rev contract + obsolete-revision retirement (§2/§8a).
**Capture flow re-sequenced** (AA5): the PARENT owns the Inbox write + the inline placement/classification
(cases 1–4, decide-in-context) and returns after card-create; `specify` is the background job's first
supervised act (off the parent's path, retried, failure → watchdog) — **superseded by round 8 (BB1): a detached
`specify` cannot be retried or raise a failure event, so it becomes a BOUNDED synchronous attempt with a
persisted `--no-agent` cron retry + the stale-triage backstop**; the mid-flight-decision path is the
decision-queue re-raise (no card callback) (§8b). **Bankruptcy class-1 reachable** (AA6/R7A5): "stale dated
actives" = `roll_count ≥ 10` AND no-progress ≥ 30 days (the "a due that never moved" contradiction deleted —
the nightly roll re-stamps `written_due`); the sweep pool = union(oldest undated someday, long-cycling actives)
(§4c). **Boundary FORMULA** (AA7/R7A2): the block-boundary soft pre-warning = `block_start − prep − travel − 30`
= 13:35 (`35 13 * * 0,2,4,6`), one value everywhere, distinct from the 14:05 leave-time alarm (§3a/§13). **Watchdog
gains ritual-ABSENCE detection** (AA8): loads the six-job manifest and alerts on any enabled job that never ran
by its schedule-derived deadline + 30-min grace (deleted / disabled / no-output) — closing "the brief silently
never ran"; **plus the AA5 stale-triage scan** (a card stuck in `triage` > 30 min) (§14). **Curator pinning
DROPPED** (AA11): verified `skill_usage.py` excludes repo-authored (non-agent-created) skills from the curator's
managed list, so a pin protects against a transition that can never target them — the gate is **installed path +
content hash** (§13/§19). Plus: `waiting-chase` enqueued most-overdue-first with strictly increasing
`enqueue_ts` (R7A11, §8); the §11 "Today's-3 guard" remnant → the per-day plan record (R7A9); the shadow
last-reconcile is the dry-run contract (R7A10); the terminal-event probe is created WITH `--max-runtime 1
--max-retries 1` + a slow worker (kanban edit can't set runtime post-hoc — AA12); the leak-gate `hash_state`,
gate-check REPO constant, and the response-section-only imperative parse are all corrected in the plan
(R7A3/R7A4/R7A6/R7A12).

**2026-07-11 — round-8 adversarial-review hardening (residue tail; re-probed against the installed reality).** An
eighth pass closed honesty gaps in claims the code can't ground and tightened precision. **`specify` supervision
made Hermes-valid** (BB1/CC2): the ungrounded "**retried on transient failure**" and "**raises a failure
event**" claims are DELETED (verified: a parent-run/detached `specify` is not a dispatcher-claimed worker, so
Hermes neither auto-retries it nor emits a failure event for it) — `specify` becomes a **BOUNDED synchronous
attempt**; on failure/timeout the card STAYS `triage` (never `blocked`), the parent says one honest line
("capture saved; processing delayed"), and a **one-shot `--no-agent` cron job retries `hermes kanban specify`**
(persisted, its failure lands in the watchdog's failed-run scan), with the stale-triage scan as backstop (§8b).
**Queue lifecycle completed** (BB2/CC7/CC10): **aggregate ids** for the classes with no single task
(`q1-conflict:<date>`, `bankruptcy-offer:<YYYY-MM>`; per-task classes keep `class:task_id`); **promotion
participates in the total order** via a `head` flag as the primary sort key `(head DESC, class-rank, enqueue_ts,
id)`; **ack TOMBSTONES `{id, gen}`** and a re-enqueue opens `gen+1`, `rev=1`; **CAS = `{id, gen, rev}`**; and ANY
record resolved intra-day (not just the shown head) is tombstoned by the live turn (CC10) — delayed-answer,
ack-then-reenqueue, and non-head-resolution fixtures added (§2/§8a). **`task.add` five-step state machine** (BB3/EE1):
a `⟦fz:<journal-uuid>⟧` line rides the create call in `--description` (intent journaled BEFORE the API call; the
returned id journaled immediately after; marker stripped on commit-verify). Healing disambiguates by window —
journaled-id ⇒ verify-by-id; marker found ⇒ resume (robust to collision/rename/move); no marker past the
propagation window ⇒ replay ONLY on a DOUBLE empty-success search ≥30s apart (a single miss/search error ⇒ abort
+ re-confirm, II5); no marker within it (the one ambiguous window) ⇒ AT-MOST-ONCE
abort + enqueue a one-line re-confirm, never an auto-duplicate — resolving the earlier absent⇒replay /
no-marker⇒abort contradiction (a fixture per window); the journal/intent enums gain `waiting-clear`, `undate`,
`retire` (§4d/§8a/§8b).
**Boot-abort claim REMOVED** (BB8): forzare cannot hook Hermes' own launchd startup (no-patching rule), so
integrity enforcement is the **watchdog's per-pass skill-INTEGRITY scan (§14 scan (f)) covering EVERY V1 skill +
bundles + helper** plus a documented pre-start runbook check — not a Hermes boot-abort (§13/§14). **Pause-vs-
absence reconciled** (CC3): the watchdog's ritual-absence scan (d) is keyed on `forzare/state/go-live.json` —
pre-go-live it LOGS (staging jobs are deliberately paused), post-go-live it ALERTS (§14). **Staging test-override
schema authored** (CC4/CC12): `schedule-override.json`'s staging-only fields `{pinned_schedule,
synthetic_weather, FORZARE_NOW}` are an authored contract honored by eisenhower-plan/brief-assemble/eod-roll only
in staging, ignored in production, and listed staging-only in §8a. **Exactly-one gate machine-readable** (BB10):
the brief emits exactly ONE `▶ ` action line; the marker count == 1 is the primary gate for BOTH queue states,
the verb regex secondary (§2). Plus precision fixes: the **unified journal record shape** stated identically in
§4d/§8a/B0 (CC8); the §14 scan (b) citation corrected — `kanban_db.py:4560-62` is the UNBLOCK path, the "cleared
only on success/reassign" wording dropped, `block_task:4383` stands (CC9); the §14 scan (d) manifest cross-ref
points at the plan's C2 six-job set and the boot line reconciled (CC14); "yesterday's missed fixed items" →
"the just-closed day's" (CC11); "pinned" → "installed (integrity-gated)" for the capture-pipeline skill (CC5).

**2026-07-11 — round-9 adversarial-review hardening (residue closure; every command/flag re-probed against the
installed reality).** A ninth pass closed the remaining honesty and consistency residue. **`task.add` is ONE
five-step state machine** (EE1): journal-intent `{uuid, content, project}` → API-call-carrying-marker →
journal-the-returned-id-immediately → strip-marker → commit; healing enumerates every crash window —
journaled-id ⇒ verify-by-id; marker found ⇒ resume; no marker PAST the propagation window ⇒ replay (definitively
absent); no marker WITHIN it (the one ambiguous window: the API may have returned before the id-journal crash)
⇒ **AT-MOST-ONCE: abort + one one-tap re-confirm queue decision, never an auto-duplicate** (duplicates are the
worse ADHD failure) — resolving the prior absent⇒replay / no-marker⇒abort contradiction; a fixture per window
(§4d/§8a/§8b). **ONE canonical queue-record schema** (DD4): `{id, class, task_id|candidate_id|aggregate-key,
proposed, status, enqueue_ts, gen, rev, head}` with `status ∈ {pending, tombstoned}`, defined once (plan B0),
restated identically in §2/§8a; the bare-`acked` passages and divergent schema quotes swept to the
tombstone/CAS wording (§2/§8b/§17). **AA5-remnant sweep** (DD3): the four "§8b stage 2" dating/classification
labels re-pointed at stage 1 (placement + dating are the PARENT's synchronous stage-1 work, §4c/§4d/§8b);
capture cards carry the **`fz-capture: ` title-prefix discriminator** (DD11 — verified `hermes kanban create`
has no metadata field) so the watchdog stale-triage scan never alarms on non-forzare cards (§8b). **p1
ownership re-checked at clear time** (EE6): EOD clears a `selected_id` only when no intervening user priority
event exists for it (the day's activity cross-checked against Bob's journaled `p1.set`); an intervening user
event means the user re-took ownership — skip + one queue flag, never a silent wipe (§8). **stale-p1 producer
= EOD only** (DD7): §4c step 6 references the record, it does not enqueue it (§4c). **§17's never-rm rule is a
category, not a list** (DD6): every file under `forzare/state/` + `forzare/calibration/` (§17). **Backlog
figure → ~2,270, live-verified** (DD13), swept across §0/§4c/§4d.

**2026-07-11 — round-10 adversarial-review hardening (minimal-diff wave; every command/flag re-probed against the
installed reality).** A tenth pass closed structural residue. **Bankruptcy freeze located once** (FF1): the
frozen id set is a new `bankruptcy` mutation-journal type (enum += `bankruptcy` at every site); the queue's
`bankruptcy-offer` record carries NO frozen field and references the journal record by its `journal-uuid`
(canonical schema untouched, §4c/§4d/§8a). **`task.add` heal keyed off `created_ts`** (FF3/GG4): the CC8 shape
gains `created_ts`; the propagation window is DECIDED at **120s** (§19, empirically validate at build); step (1)
is a durable `pending` write, never "committed" (§8a). **Tombstone is an IN-PLACE `status` flip** (FF10/GG5):
the retained record's `status → tombstoned`, `gen` unchanged, no separate object; a re-enqueue reuses it at
`gen+1`/`rev=1`; the ambiguous-window re-confirm id is `triage-reraise:<journal-uuid>` (§2/§8a). **plan-of-day
gains `created_ts`** (FF6): the EE6 intervening-user-event check compares an activity event `ts` against it
(verified `td activity` priority-change events carry `priority`/`lastPriority` in `extraData`, §4c/§8). **§14
boot line** created `--deliver local`, go-live flips the four user-facing jobs (FF11). **§8b stale
"placement runs in the background" passage** corrected to the five-stage model — stage 1 + the bounded `specify`
are the PARENT's synchronous work (GG10). **Response gate** adds the `· ` context-prefix and a "zero non-`· `
lines after the marker" check (GG12). **Calibration exclusion is GENERIC over the journal enum incl.
`bankruptcy`** (GG13). **`hermes kanban create` help evidence corrected** — it exposes
`--body`/`--priority`/`--project`/`--workspace`/`--created-by`; the title prefix is the chosen discriminator — a
`--created-by` filter would also work (it is a stored, filterable per-card column, `kanban_db.py:1019`), but the
greppable title prefix is preferred (FF9/HH5).
The plan absorbed the matching build-side fixes (gate-check.sh ships through chezmoi + a watchdog self-guard,
GG2/FF15; phase-aware integrity manifest, GG1; bash-correct harness traps + mkdir lock, GG7/HH2; the Phase-C gateway
stop-window drops the not-yet-built watchdog, GG8; the go-live flip becomes a stopped-window transaction, GG9).

**2026-07-12 — round-11 adversarial-review hardening (HH1–HH9 · II1–II9; every command/flag/config-key re-probed
against installed reality).** **Harness lock is `mkdir`-atomic, not `flock`** (HH2/II2 — `flock` verified ABSENT
on this macOS; §8a lock sentence + plan intro/B0). **Queue schema gains `journal_ref`** (II3, nullable
mutation-journal uuid the ack consumes — populated for `bankruptcy-offer` + the ambiguous-window
`triage-reraise`; the ack matches that EXACT uuid, month search deleted; `bankruptcy` joins the per-type heal
list). **Generation rollover resets `head=false` + a fresh `enqueue_ts`** (II6). **`task.add` replay fails
closed** (II5 — past the 120s window a replay fires ONLY on TWO consecutive empty-success marker searches ≥30s
apart; a single miss/search error ⇒ abort + re-confirm; a build task measures the live create→search propagation
and rejects the bound if unsafe). **`auxiliary.triage_specifier.timeout` pinned to 20s** (II9 — the key exists,
`config.py:1505`; no `gtimeout` wrapper needed). **Ritual-absence scan (d) also alerts on the two post-go-live
drift classes** — delivery-map + prompt-dryness (HH6). **`--created-by` is a filterable per-card column**
(`kanban_db.py:1019`) — the title prefix is the *chosen* discriminator, not the only possible one (HH5). The plan
absorbed the matching build-side fixes (single-EXIT existed-before trap, HH1/II1; go-live restore fail-loud +
both services health-verified pre-flag, II2; `[TEST-STAGING]` project made real with `--project` on every
fixture, II4; response gate checks every non-marker line pre- AND post-marker, II7; RUN1/RUN2 two-run probes,
HH3/II8; morning roll-then-plan ordering command, HH7; B6 acceptance re-scoped to the measured surface, HH8;
set-e-safe retry wrapper, HH9).

**2026-07-12 — round-12 structural + adversarial-review hardening (JJ0–JJ12 · KK0–KK9; THE STRUCTURAL WAVE).** A
twelfth pass applied the process doc's own prescription for the recurring embedded-command finding class: **the
plan's staged-test bash HARNESSES are demoted from normative embedded scripts to per-task INVARIANTS + one new
build deliverable, Task E3 "forzare-staging-harness"** (a chezmoi-shipped, shellcheck+bats-gated script suite
authored at build time, each invariant id mapping to a test, writing run-scoped atomic result files the
checkpoints consume) — KK0/JJ0. This spec's design decisions are unchanged by that plan-side refactor; the
design corrections this pass are: **the state-layer lock is `fcntl.flock` (python), not a `mkdir` sentinel**
(KK1/JJ11 — `fcntl.flock` verified AVAILABLE on this macOS host via `python3 -c 'import fcntl'`; the earlier
"flock ABSENT" reading conflated the missing shell `flock(1)` binary with the syscall; the advisory lock is
crash-auto-released by the kernel, so the PID/TTL/mkdir stale-lock machinery is dropped; ONE state-layer
sentinel `task-lifecycle.lock` covers the map+journal+queue critical section, §8a). **The [TEST-STAGING]
Todoist project is a USER-CREATED prerequisite** — the build resolve-only, FATALs if absent, never auto-creates
it (KK2/JJ2, the no-unprompted-projects rule). **A producer once-guard** stops a re-ask of a tombstoned
decision whose predicate state is unchanged and whose recorded `answer` was `keep` — the `answer` field is added
to the canonical queue schema, making "stale-p1 flagged once, never nightly" a stable guarantee (KK3/JJ3, §2
step 4/§4c/§8). **Watchdog scan (b) carries the same forzare discriminator as scan (e)** so a non-forzare
profile's card failure never alarms `#forzare-errors` (KK4/JJ4, §14). **The replan bundle gains `calendar-write`**
because replan MOVES Bob's 🤖-calendar proposals inside the §5c contract (KK5/JJ5, §13). **The integrity manifest
is RECURSIVE** — every managed file of every skill dir (SKILL.md + every executable/support file:
`classify.py`, `eligibility`, `reduce.py`, …) asserted for existence + hash + exec mode, and the enumeration
gains `waiting-reconcile` + `transition` to match the plan's canonical list (KK6/JJ10, §13/§14). **Bankruptcy
healing is COMMITTED-AT-WRITE** — the frozen snapshot is one atomic append with no pending phase; recovery is
binary (absent ⇒ re-offer, complete ⇒ read the immutable cohort); the generic three-way fixture for this type is
removed (KK7/JJ9, §8a). Plus: the §8b "none is filterable" remnant is swept (`--created-by` IS a filterable
per-card column; the `fz-capture: ` title prefix stays the chosen discriminator, JJ8/HH5); the §8a heal-list +
fixtures gain the committed-at-write `bankruptcy` case and a SIGKILL-mid-critical-section flock-auto-release
fixture (JJ9/KK1); the integrity enumeration adds `waiting-reconcile` + `transition` (JJ10); lock identity is
unified to the one `fcntl.flock` sentinel (JJ11); the G1 concurrent-trigger block became a plan invariant under
E3 (JJ12/KK0). The round-11 "harness lock is `mkdir`-atomic, not `flock`" note (above) is therefore SUPERSEDED
for the state layer: the design lock is `fcntl.flock`, and the plan's own staging-harness lock is likewise a
`fcntl.flock` file via the B0 python shim (KK1).

**2026-07-12 — round-13 adversarial-review hardening (LL1–LL13 · MM1–MM10; extraction-cleanup wave, minimal
diff).** A thirteenth pass closed extraction residue. **Capture-card discriminator is now `--created-by forzare`,
not the title prefix (MM1/LL-carryover — the heaviest correction).** `specify` **provably rewrites the card
title** (`specify_triage_task` atomically updates title/body, `kanban_db.py:4574`), so a `fz-capture: `
title-prefix scan would MISS every specified card — the discriminator moves to the **stored, immutable,
filterable `created_by` column** (`kanban_db.py:1019`); create commands gain `--created-by forzare`, BOTH
watchdog scans (b)/(e) read `created_by == "forzare"` from `hermes kanban list --json` (no `--created-by` filter
flag exists — the scans jq the field), and the title prefix is demoted to display-only (§8b/§14/§15). **Gateway
liveness probe port DECIDED `:8642` (MM2)** — the webhook `:8644/health` is dead when `WEBHOOK_ENABLED=false`
(verified live), so the platform-independent API-server `:8642/health` (`api_server.py:4385`, DEFAULT_PORT 8642)
is adopted; a Phase-A `.env` gains `API_SERVER_ENABLED=1` + `API_SERVER_KEY` (the server refuses to start without
the key), and every `:8644` probe reference is swept (§16/§19/decided-config). **(SUPERSEDED round-14/OO1 — MM2
REVERTED: the `:8642` switch rested on a transient/false `WEBHOOK_ENABLED=false` reading; re-verified live
2026-07-12 the managed-env-pinned webhook `:8644/health` answers, so the probe returns to `:8644` with NO
API-server key, and the `API_SERVER_ENABLED`/`API_SERVER_KEY` additions are removed everywhere; see the round-14
recap.)** **§2 response gate** now checks
zero non-`· ` lines OTHER THAN the marker line **PRE- and POST-marker** (II7 parity with plan INV-B4-4). The plan
absorbed the matching structural fixes: E3 moved to Phase A ahead of its consumers (MM3); the integrity manifest
uses `chezmoi managed --include=files` + per-skill empty⇒FATAL (MM4/LL1/LL2); E3 ships as
`dot_local/bin/forzare-staging-harness/` with per-file `executable_` prefixes and bats under `test/` (MM5/LL3);
a machine-readable INV↔test bijection manifest with the unnumbered families numbered (MM10/LL5); intent- vs
journal-record schemas disentangled (LL4); staging harness results moved out of the state layer to
`staging/e3-results/` (MM7/LL6).

**2026-07-12 — round-14 adversarial-review hardening (NN1–NN10 · OO1–OO8; the revert + coherence wave, minimal
diff).** **Gateway liveness probe REVERTED to the webhook `:8644/health` (OO1, superseding the round-13 MM2
`:8642` decision — NN1).** The MM2 switch rested on a transient/false `WEBHOOK_ENABLED=false` observation;
re-verified live 2026-07-12, `:8644/health` answers `{"status":"ok","platform":"webhook"}` (exit 0), the managed
env `dot_hermes/private_dot_env.tmpl:15` pins `WEBHOOK_ENABLED=true`, and `:8642` refuses (exit 7). So the probe
returns to `:8644` with the platform-enabled prerequisite (asserted post-apply), and **every
`API_SERVER_ENABLED`/`API_SERVER_KEY` addition is removed** — no new bearer secret, and the gate_check `.env`
diff is made secret-safe (names/presence only). Coherence + honesty fixes: **§6a pagination fixture reworded to
the two-page cursor stub (R6A8, NN4)**; **§2's residual post-only marker sentences made PRE-and-POST (NN5)**;
**the FF6 causal note corrected — the morning's own p1-set postdates `created_ts`, so the journal cross-check
(not the cutoff) excludes it (NN6)**; **the round-6 recap's "unconditional `p1.clear`" annotated SUPERSEDED by
AA2/EE6 (NN7)**; **the "nothing messages the user before Phase G" claim scoped to the TASK channel — the
Checkpoint-A/F1 errors-channel probes are sanctioned staging traffic (NN8)**; **exec-mode assertion keyed off the
chezmoi `executable_` SOURCE attribute, not a filename suffix (NN9)**; **the §19 bankruptcy trigger reworded to
">25 candidates in the sweep POOL — undated someday ∪ long-cycling dated" (NN10)**; **§8b capture idempotency
made honest — the Inbox-task-id key dedupes best-effort (`kanban_db.py:2385-2389` documents the concurrent-insert
race); the guarantee comes from the parent being the single capture-writer serialized per Inbox task id (OO7)**;
and **the staging DRY directive scoped to `jobs.json` prompts only, never bundle instruction files, so the
go-live flip stays a single-file transaction (OO8)**. The plan absorbed the structural coherence: the E3 result
consumers unified to **{post-stage Checkpoint B2, C, D, G1}** (F consumes gate_check + hashes only) with one
shared named `e3_result_gate` reader (NN2/OO3); the four unmapped verify surfaces entered the bijection
(`b3_calendar`, `b11_retry`, `d1_harness`, `g1_matrix`; NN3); Checkpoint D's jq genuinely tightened to run_id ==
build id + ts-in-window + referenced-objects re-verified (OO4); retry-job creation resolves-by-name first
(`create_job` always appends, `cron/jobs.py:977-980`; OO5); harness teardown runs under the production
`task-lifecycle.lock` with a changed-abort (OO6); and the integrity manifest is materialized + status-checked
before its read loop (OO2).
