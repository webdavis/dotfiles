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

- **Task-side** (Todoist): `@deep`/`@light`/`@admin`, `duration`, `deadline`, `priority` (p1 = today's
  ≤3), `@waiting` (never surfaced as do-now), `@errand`; active = has a due date.
- **Person-side** (inferred, not measured): time-of-day, calendar gaps/load, activation state, location.
- **Provide-nothing is a first-class option** (JITAI receptivity — intervene less, but clearly).

**The backlog (~2050 tasks) stays completely out of view.** Bob surfaces a single next action; the user
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
§12.4/§14.)

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
reads it and goes provide-nothing/recovery **without clearing it**; the recovery flag is consumed by the
first brief/engagement on the calendar day *after* the block ends, then normal schedule resumes. No
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
4. **Follow-ups + re-decisions (a few one-liners, max).** This is where deferred *decisions* are delivered
   — the 02:00 reconcile and the EOD sweep only *mark* them (§8): any `@waiting` past its check-back date →
   **one chase question** (most-overdue leads if several are due); yesterday's **missed fixed items**
   (deadline/timed/recurring — roll-excluded, §8) → each surfaced once as a one-line **re-decision** (do
   late / reschedule / drop — a decision, never a do-now); §8b triage items awaiting a placement answer
   re-raised here.
5. **Activation reminder:** *"Breakfast first, then gym"* (non-negotiable; the fragile morning is a JITAI
   **vulnerability state** — reinforce the routine, don't load deep work onto a collapsing morning).
6. **Ends with one action** — *"First: eat, then ride."*

**The brief always fires at its time — receptivity shapes its *content*, never withholds it.** A rough
stretch (dense dismissals → low receptivity) may lighten the ≤3 and drop optional blocks, but the daily
anchor itself is exempt from the §6a receptivity gate — a bad week is exactly when the clean reset matters
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
   as the deadline, not a nag.
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
  prevent). **Recurring tasks are never re-dated at all** (a `td` date-write destroys the recurrence rule;
  recurrence sets its own next date, and they're roll-excluded anyway) — a "not now" on a recurring task is
  within-day suppression only.
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
| **Active / now pool** | `(today \| overdue) & !@waiting` | full eligible set for momentum-mode surfacing |
| **Follow-ups** | `@waiting & (today \| overdue)` | blocked items due for a chase |
| **Deep window** | `@deep & (today \| overdue) & !@waiting` | deep-work candidates for peak windows |
| **Errands** | `@errand & !@waiting` | location-dependent tasks |

**Label-name notation (verified `td` v1.75.3).** The stored label names are **unprefixed** — `deep`,
`light`, `admin`, `errand`, `waiting` (a task's JSON `labels` array holds `["deep"]`, not `["@deep"]`). The
`@` written throughout this spec is **prose / filter-query notation only**: inside a Todoist filter query
`@waiting` is how you *reference* the label, but `td label create --name` / `td task update --labels` take
the bare name. Command examples strip the `@` accordingly.

`p1` is reserved **exclusively** for the daily ≤3 must-dos — Bob marks at most 3 tasks `p1`/day (set each
morning, cleared at day's end **unconditionally**, §8). The ≤3 is maintained by **assignment discipline, not
the query** (Todoist can't cap a filter's count). The ≤3 is a **floor, not a ceiling** — after the 3,
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
6. **Cap at 3.** If >3 Q1 deadlines collide, that's a conflict Bob surfaces for a decision (§7; INV-5: ask
   before a big reprioritization) — never a silent drop.
7. **Idempotent:** before assigning, read `Today's 3`; if 3 p1s already exist for today (brief already ran),
   do nothing (§15). *(This guard is valid only because §8's p1-clear is unconditional — every unfinished p1
   loses the flag at EOD, so "p1 present ⇒ assigned today.")*

**Someday→active promotion — the pool's designed inflow.** The active pool only *drains* (completions,
drops, future-defers) unless something dates tasks INTO it; without a designed inflow, captures externalized
to the system are functionally lost — the exact prospective-memory failure forzare exists to remove. Three
mechanisms date tasks in (each firing activation-time grooming, below):

- **Deadline lead-time (automatic).** Any task carrying a real `deadline` gets a computed **surfacing due
  date** = deadline − duration-aware padded lead time — **date-only** (so it rolls normally as
  surfacing-dated; the immovable date stays in `deadline`). Steeper ADHD temporal discounting means the
  **system** computes lead time; it never trusts future-self to notice a deadline approaching.
- **Capture dating (§8b stage 2).** Captures that state or imply timing are dated at placement (§8b).
- **Planning pull (goal-matched).** When the dated Q2 pool is thin, this morning narrowing (and EOD's
  pre-stage) pulls goal-matched candidates from the someday pool — matched against the §goals yardstick — and
  dates them. Deliberate, small (one or two), and one of the two places the someday backlog re-enters view;
  the backlog itself stays out of sight (§0). **Bounded, user-approved freshness policy:** the pull is
  conservative until the ~2050-task backlog has been relevance-combed (a separate, still-pending pass) — an
  un-combed backlog would surface stale junk, so early on the pull proposes rather than auto-dates.

**Backlog re-decision — the monthly someday-sweep (the second, deliberate re-entry path).** Left alone the
someday pool only grows, and a ~2050-item backlog is an ADHD wall by its mere existence (§0). So a **monthly**
cron proposes a **SMALL batch (≤5)** of the *oldest / most-stale* someday items at the morning brief as
one-line **keep / drop / promote** decisions — decide-in-context, one line each, **no walls and no shame** (it
is never a list-dump; ≤5 is the whole point). When the un-swept someday backlog crosses a **decided
threshold**, the sweep additionally offers an explicit, opt-in **"task bankruptcy"** batch-drop ("~N of these
haven't moved in a year — clear them all?") — a single yes clears the tail, so the backlog can never quietly
metastasize. Both are proposals; nothing is dropped without the user's word.

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
groom-on-read — not as a mass-backfill of the someday backlog. Captures: §8b stage 2 dates time-bound
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
tasks** (reschedule → due tomorrow; *excluding* deadlines/appointments/recurring/future-dated — see §8) so
overdue never piles into a guilt-wall (INV-4). But that reschedule means a task dodged for 5 days looks
identical to a brand-new task due tomorrow — overdue is *consumed* by reconciliation and can't double as the
stall count. So the stall memory must live somewhere the nightly roll doesn't wipe.

**Stored in a private lifecycle ledger, NOT on the task.** The counter lives in the owned layer as
`forzare/state/task-lifecycle.json` (§8a), a map keyed by Todoist **task id**, each entry
`{written_due, roll_count, last_escalated}`:

- **`written_due`** — the due date Bob last *wrote* on the task (a date-only agent surfacing date).
- **`roll_count`** — consecutive nights carried without progress.
- **`last_escalated`** — the date the §7 escalation last fired for this task (re-nag guard).

Rules:

- **Every agent date-write records `written_due`.** When Bob dates or re-dates a task (surfacing date, snooze,
  lead-time), it stamps the value it just wrote.
- **The roll set is the ledger entries where the task's *current* `due.date` still equals `written_due`**
  (self-healing provenance). If the two differ, **the user re-dated the task** since Bob touched it → the
  entry is **void** and the task is treated as *fixed* (user-owned date, never auto-rolled). So **user-dated
  tasks never roll**, with no label bookkeeping to get stale — the divergence *is* the signal.
- **A roll increments `roll_count`.** At `roll_count == 2` (2nd consecutive carry) Bob fires the §7
  escalation; `last_escalated` is stamped so the same stall isn't re-nagged the next night.
- **Reset on progress** (`roll_count → 0`, entry effectively cleared) on the same triggers as before —
  completion / subtask done / user comment / user-reported "touched" (§8) — and on a conscious
  beyond-tomorrow defer.
- **Prune on terminal state:** a completed or deleted task's entry is dropped (detected from the activity
  log, §8a), so the ledger stays roughly the size of the small active/rolled set, never the ~2050 backlog.

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
implementation-intention effect, Gollwitzer & Sheeran 2006; the ADHD-specific evidence is in **children** and
reports no pooled *d* — so "ADHD-specific" names the population, not that figure). Some blocking is the
highest-value thing Bob does.

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
windows derived from the `work_schedule`** (§2: morning peak always; evening peak only on OFF days — work
days own the evening), the post-lunch dip, calendar load, recent completion vs stalling. Asks only when
ambiguous (light, occasional); user may volunteer ("I'm fried"/"locked in"). Calibrates from observed
patterns over time. **No "rate your energy" gate before tasks.**

**Receptivity decision rule** (when to act vs stay quiet), not an energy gate. State maps **directly onto the
cognitive-load labels** — `@deep`/`@light`/`@admin` ARE the energy-commitment axis, so inferred state selects
which label-class is eligible (no separate energy scale exists or is needed):

- **Activated / peak** (post-gym, "locked in", recent completions, clear calendar) → **`@deep`** eligible.
  The *only* state in which deep work surfaces. **⚠ The post-gym activation boost DECAYS** — Mehren 2019
  *measured* an acute-exercise executive-function gain in ADHD adults at **~33 min post-exercise** (the study
  did not track its full decay); the **~1–2h** working window is the **research synthesis' own estimate** of
  how long the lift usefully persists, not a measured Mehren value. So "back from gym" is a *decaying* window,
  not an all-day flag. Surface the day's hardest `@deep` work into the
  hour or two right after activation, not at 4pm because the gym happened at 7am. (Corrects the earlier
  "morning peak always" framing — §6a learns the actual decay curve.)
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
   load). High recent-dismissal density ⇒ withhold.
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
  the *overall* effect and d=0.99 the self-regulation-impaired-samples figure, Gollwitzer & Sheeran 2006; the
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
win. A thin policy layer over machinery built elsewhere (§6a receptivity, §4d roll-labels, §8 nightly roll).

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
     single best-evidenced low-burden lever (d=0.65 overall / d=0.99 self-regulation-impaired, Gollwitzer &
     Sheeran; the ADHD-specific response-inhibition evidence is in children, no pooled *d*).
   Dropping it is always an offered, shame-free option. The choice is the user's. **Never** re-surface a
   stalled task with more frequency or firmer pressure — pressure adds aversiveness and feeds avoidance (§6a:
   Steel; Sirois & Pychyl). *(When a live Discord session is present, this decision is a clarify-button set —
   Break down / Pick a time / Drop — §12 R1c; at brief time it's a plain one-line question.)*

- **Self-forgiveness reduces procrastination; shame deepens it** (Round-1 Finding 9; §6a). Overdue **never
  accumulates into a wall** — in *either* class: the nightly roll (§8) sweeps surfacing-dated tasks forward,
  and missed **fixed** items (deadline/timed/recurring — roll-excluded) each get a one-line **morning
  re-decision** (§8/§2) instead of rotting. Bob never scorekeeps failures, and the count lives in the private
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
- **Roll forward ONLY the right tasks (do not blindly reschedule everything dated).** "Active = has a due
  date" (§0) includes tasks dated for a *real, fixed reason* — appointments, deadline-day tasks, things
  deliberately scheduled for a specific day, recurring tasks with a genuine date. **Rolling those would
  corrupt the schedule.** So the roll set = tasks that were **eligible to do today and weren't done, and whose
  date was "today" only as a *surfacing* date, not a fixed commitment.** Concretely, EXCLUDE from the roll
  (verified `td` JSON checks, §19): `deadline != null` · `due.isRecurring == true` (let recurrence set the
  next date) · `"T" in due.date` (has an appointment time-of-day — reliable *because* §4's hard rule makes
  agent writes date-only, so a "T" always means a **user** appointment) · `due.date` is a future day. Roll
  forward the remainder (the "I meant to get to this today" pool — date-only, today/overdue, no deadline,
  non-recurring). When uncertain whether a date is "fixed" vs "surfacing," **leave it and flag for the
  morning** rather than silently moving it (a wrongly-moved deadline is far worse than a wrongly-kept one).
- **p1-clear is unconditional — its own step, NOT scoped to the roll set.** At EOD, clear `p1` from **every**
  unfinished p1 — roll-excluded ones included (deadline/timed/recurring; clearing moves no dates, so it can't
  corrupt the schedule). `p1` is day-scoped by definition (§4b); this is what keeps §4c's idempotency guard
  valid ("p1 present ⇒ assigned today").
- **Missed FIXED items don't rot (the roll-excluded overdue path).** EOD *enumerates* yesterday's
  roll-EXCLUDED overdue (deadline ≠ null / `"T"` due / recurring) and queues each for a **one-line
  re-decision in the MORNING brief** (§2 step 4: do late / reschedule / drop — a decision, never a do-now;
  EOD itself reports no misses, per the no-scorecard rule). No new state: "overdue + fixed-shape" is derivable
  per run from the task fields (§8a's derived-state model) — the re-decision resolves it naturally. Until
  re-decided, such items are excluded from momentum-mode do-now surfacing.
- **This is the counter's primary increment site (§4d)** — the nightly path for silent non-completion: for
  each task **in the roll set**, increment its `roll_count` in the lifecycle ledger; at `roll_count == 2`
  fire the §7 escalation (stamping `last_escalated`). Rolling also re-stamps `written_due` to the new
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

**Missed-fire safety (degraded mode).** Two layers, because cron's own recovery is **bounded**. (1) **Cron
catch-up — verified (`cron/jobs.py:488-492`): a job due while the gateway was down replays on restart, but
only within a ±2h window.** So a brief outage around 23:00 self-heals. (2) **Defensive morning roll — the
backstop for outages longer than 2h:** the morning brief detects stale state (no roll/reset happened for
yesterday) and runs the roll before building today's plan. The end-of-day routine is written **idempotent +
date-stamped** (it records the date it last reconciled) so any of {on-time fire, ≤2h cron catch-up, late
morning-brief run} happens **exactly once per day** — never double-rolls. This removes the dependency on the
gateway being alive at exactly 23:00.

**`@waiting` nightly reconciliation** (idempotent cron — e.g. `0 2 * * *` Denver) — **state-only: the 02:00
run never messages the user.** Delivery of everything it finds is the **morning brief's follow-ups step (§2
step 4)**; only a genuinely time-sensitive chase may go through the normal receptivity-gated intra-day path.
What it does:

- **Mark chase-due:** any `@waiting` past its check-back date → flagged for one chase question at the brief
  (most-overdue leads).
- **Repair the §4b set-time invariant:** any `@waiting` with *no* check-back date or blocker note (the
  black-hole case) gets a near-term check-back date auto-set + an "auto-repaired — blocker unclear" flag for a
  one-line brief decision. Silent-first: repair now, ask once, never let it vanish.
- **Unblock detection:** check each blocker note against the signals Bob actually has — **gog calendar** (the
  awaited event passed?), **Todoist activity log** (the blocking task completed? a new comment?), **recent
  Discord context** (the user mentioned it landed) — and on a detected unblock, silently clear `@waiting` +
  re-date so it rejoins the normal pool. (Email is added as a signal only if Bob later gains read access.) Bob
  also clears **opportunistically mid-conversation** whenever the user or a signal reveals the unblock — the
  nightly scan is the floor, not the only path.
- **Staleness sweep:** ~14 days *since the label was applied* (from the activity log) with no movement →
  flagged for the same brief slot as a decision (still waiting? chase harder? drop?).

The agent owns the label end-to-end; the user never manages it.

**Low-friction capture:** a one-liner to Bob → a **structured** `td task add "<raw text>"` to the Todoist
**Inbox** (NOT `td task quickadd` — verified v1.75.3: `quickadd`/`qa` runs the natural-language parser that
would pull a date out of the phrase; `td task add` with a positional body stores the text verbatim and dates
only via an explicit `--due`). Stage 1 therefore **captures raw and dates nothing** — classification comes
first (§8b stage 2), dating second, so a phrase like "call the dentist Tuesday" isn't silently turned into an
appointment before Bob has decided task-vs-event. **Inbox is staging, not a store** — a brief transit lane;
the *project* is the canonical home (§8b places it there). The instant Inbox write is the capture's only
synchronous step: it acks immediately and guarantees nothing is lost even if later processing fails. Everything after — decide placement, verify, research, split — runs in the
background (§8b). **Triage** is the *exception*, not the resting state: an Inbox item becomes triage only when
the pipeline needs the user's input (an ambiguous placement or a not-yet-existing project, §8b cases 3–4).
Capture never interrupts the current surfaced task.

## 8a. State & persistence model (where everything lives)

**Bob is almost stateless by design.** An amnesiac fresh session must never trust its own memory — it
re-derives ground truth from Todoist each run. Only a few things persist, in distinct homes. This is
the authoritative map of what reads/writes what; the rest of the spec references it.

| State | Home | Read | Written | Persisted? |
|---|---|---|---|---|
| **Lifecycle ledger** (`roll_count`, `written_due`, `last_escalated` per task id, §4d) | owned layer `forzare/state/task-lifecycle.json` | EOD roll · morning brief · §4 defer · §7 escalation | agent date-writes stamp `written_due`; EOD roll / §4 tomorrow-snooze increment `roll_count`; progress/touched/complete reset; entry pruned on terminal state | **yes — one owned file, off the user's tasks** |
| **Date provenance** (`current due == written_due`?) | derived: task `due.date` (Todoist) vs ledger `written_due` | every roll-set test (§4d/§8) | n/a — *derived per run* (divergence ⇒ user re-dated ⇒ fixed) | no |
| **Progress-since-surfaced** (reset trigger) | Todoist **activity log** | `td activity --since <d> --type task --json` (completed/updated/comment events) | n/a — *derived* | no (queried) |
| **Roll-set / "is this date fixed?"** (#1) | Todoist **task fields** | `td task list --json` → `deadline` / `due.isRecurring` / due-has-time / future-date | n/a — *derived per run* | no |
| **Last-reconcile date** (idempotency) | owned layer `forzare/state/last-reconcile.json` | morning brief + end-of-day | end-of-day (+ defensive morning run) | **yes — one tiny file** |
| **Schedule override + gym activation** (§2/§3/§6) | owned layer `forzare/state/schedule-override.json` (shift block · date · recovery-morning flag · today's `activation` field) | morning brief + end-of-day + gym-window-end check + the `/forzare` skill | shift override set by `/forzare` shift signal (consumed on the day *after* the block ends; a mid-shift 5:15 brief reads without clearing); the **date-scoped `activation` field** is set when the gym-back signal fires, so the gym-window-end cron (§3, an amnesiac session) knows the signal already came | **yes — one tiny file** |
| **Fire times** (#4) | `forzare/` config + fixed 23:00 | read | hand-edited | config |
| **Calibration log** (§6a — learning) | owned layer `forzare/calibration/` | aggregate analysis (daily/weekly) | appended per surfacing decision | **yes — the learning dataset** |
| **Goals yardstick** (§4c) | owned layer `forzare/goals.md` | p1 time | hand-edited ~quarterly | yes (human-owned) |

**The two kinds of memory, kept separate:** *control state* ("what's true now" — the lifecycle ledger,
last-reconcile date, schedule override) is read every run and acted on immediately; the *calibration log*
("what tends to work for me") accumulates and is analyzed in aggregate to tune decision rules (§6a) — never
user-facing, never self-report. **Everything Bob persists now lives under `forzare/`** (lifecycle ledger,
state-stamp, calibration, goals, dopamine-menu) — nothing failure-shaped is written **on the user's Todoist
tasks** anymore (§4d). Bob holds no other *knowledge* state between sessions.

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

**Trigger + ownership.** The user captures (`/forzare-capture` or plain language). **Parent Bob** does stage
1 synchronously (instant ack, nothing-lost), then **delegates stages 2–5 to a background Kanban job** whose
stages are fresh-context **default-profile subagents** (§10). The parent returns to the user immediately; the
job runs without holding the conversation.

> **Persona vs. profile (load-bearing for every Kanban `--assignee` / `default_assignee` below).** "**Bob**"
> is the **persona** — the `SOUL.md` character and the `bob → hermes -p default` wrapper alias — running as
> the hermes-agent **`default` profile** (verified: `hermes profile list` shows `default` as the marked
> default; there is **no** profile literally named `bob`). So every assignee and `default_assignee` in this
> spec is the profile **`default`**, and Kanban's own no-assignee fallback already resolves to `default`.

**Concrete kickoff + specify (R7 — verified flags).** Stage 1's delegation parks the card with
**`hermes kanban create --triage --idempotency-key <capture-id>`** — the `--triage` card is Bob's private
work item; the idempotency key makes a re-fire (or a full parent retry) return the *existing* card, never a
duplicate (§15). When a raw capture needs fleshing into a concrete title+body spec before routing, the stage
runs **`hermes kanban specify`** — the `auxiliary.triage_specifier` slot (pinned to a cheap Haiku model in
config, §14) turns the terse one-liner ("insurance thing") into a concrete, actionable title + body. Both are
first-class `hermes kanban` verbs (verified flags + watcher behavior); no bespoke pipeline code.

**The five stages — each gates the next, so the pipeline short-circuits:**

| # | Stage | Does | Gate to next |
|---|---|---|---|
| 1 | **Place** (parent, sync) | Structured `td task add "<raw>"` to **Inbox** (staging) — **no date parsing** (never `quickadd`). Idempotent: skip if this capture is already there. | always → 2 |
| 2 | **Decide placement** (subagent) | **Pre-check: task vs calendar event** (below); optionally `hermes kanban specify` to concretize (R7) — then date time-bound captures, search the existing project hierarchy, pick the home (4 routing cases below). | placed → 3 · event → calendar, **done** · needs user input → **triage** (cases 3–4) |
| 3 | **Verify + research-decision** (subagent) | Confirm the placement is sane; decide **does this need research before it's actionable?** | research-worthy → 4 · not → **STOP (done)** |
| 4 | **Research** (subagent) | Investigate (web / vault / `/deep-research` as warranted); decide whether the result implies subtasks. | implies subtasks → 5 · not → **STOP (done)** |
| 5 | **Split** (subagent) | Rewrite as one task + concrete subtasks from the research verdict. | → done |

**Stage 3 *is* the gate (#6) — there is no separate "full-pipeline-vs-not" switch.** Most captures stop at
stage 2 or 3: an obvious, self-contained task is placed and finished; only genuinely research-worthy items
walk the whole ladder. The gate is a per-item verdict, deterministic given the item — not a global mode.

**Stage-2 pre-check — task vs calendar EVENT (before any project routing).** A capture that *is* a fixed-time
event or routine ("dentist Tue 2pm") does **not** become a Todoist task — it routes to **Bob's 🤖 calendar**
(§5c), propose-and-confirm inline like case 3. The dup-guard extends to the calendar write (check the 🤖
calendar first — a Kanban restart re-runs the card and must not duplicate the event); the stage-1 Inbox item
is completed/cleared once the event exists (staging honored, nothing lost). A user-confirmed fixed event is
**immovable load** (§5c carve-out), not a movable proposal. Only start-decision items proceed to the project
routing below.

**Stage 2 also DATES time-bound captures at placement** (this + §4c's promotion inflows replace the old
"captures are placed undated" blanket; because stage 1 stored the text verbatim, **stage 2 is the only place
a date is written** — classify first, date second): a **hard time bound**
("submit by the 15th") → `deadline` + computed date-only surfacing due (§4c lead-time rule); a **plain day**
("Saturday") → date-only due; **implied-but-vague** timing ("before prices jump") → **propose the concrete
date inline** (case-3 style) — never silently invent one; **genuinely timeless** → rests undated as someday
(§4c's planning pull is its designed way back in). Dating a capture fires activation-time grooming (§4c) right
then.

**Placement routing — four cases, all resolved INLINE (stage 2):**

1. **Explicit project** — the user named it at capture ("…to Homelab") → route there, no decision.
2. **Obvious** — exactly one project clearly fits → route, no decision.
3. **Ambiguous** — more than one plausible home → **propose-and-confirm inline** ("Captured X — put it in
   Homelab?"). One fast yes/no while context is fresh. *(Live Discord session → a clarify button, §12 R1c.)*
4. **New project needed** — nothing fits → **ask inline** ("X doesn't fit any project — make one called Y?").
   **Never auto-create a project** (firm rule).

Cases 3–4 need the user; the **primary path is decide-now** — ask while it's fresh (the decide-in-context
rule). If the user is unresponsive, the item **waits in Inbox as triage** and the Kanban card durably holds
"awaiting placement decision"; Bob re-raises it at the next natural moment (e.g. the morning brief's triage
check). Durable block-and-wait is the *fallback*, not the default. *(Because the routing subagents are
cron/Kanban-origin — no Discord-bound session — their asks route through the parent conversation, which is
Discord-bound; that is why the parent owns the inline ask, §12 R1c caveat.)*

**Dup-guard / idempotency (#5) — forced by the no-resume caveat.** Kanban restarts a crashed card from its
first stage, not the failed step (§19). So **every stage is check-before-create**: stage 1 skips if the
capture is already in Inbox (the guard that also covers a full parent-level retry); stage 2 skips re-routing
an already-placed task; stage 5 skips subtasks that already exist. A restart converges to the same single task
— never a duplicate. (Also why placement is one *move*, not create-then-move — fewer mutations to make
idempotent.)

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
- **Why it must be firewalled — and the TWO channels.** Creating a task from the gateway auto-subscribes the
  originating chat to that task's **terminal events** (completed/blocked/gave_up/crashed/timed_out). These must
  **not** leak onto the user's **task channel** (the one delivery gate, §12) — a stream of agent plumbing is
  the opposite of one-thing-or-nothing. **But "firewalled" means routed, not silenced:**
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

The brief is **cron kicking off Bob to run the `/forzare-morning-brief` skill bundle** (§13) in one agent turn
— not a Kanban parent-child graph. Two verified facts make Kanban the wrong tool here: there is **no real
workflow primitive** (`workflow_template_id` is a half-built filter/tag column, §19) and **no mid-run resume**
(a crash restarts the whole job, §19). A parent-child card graph therefore buys little durability for a short,
fast, daily sequence while adding exactly the plumbing the user must be firewalled from (§9).

- **Cron job** (~5:15, §1) runs the bundle. **Re-fire safety is app-level** (not a cron flag): the brief
  checks whether today's plan already ran — the §4c Today's-3 guard + the §8 date-stamp — before mutating, so
  a re-fire or the ±2h catch-up (§8) is a no-op. **Time bound = the cron INACTIVITY timeout**
  (`HERMES_CRON_TIMEOUT`, default 600s *idle*, verified `cron/scheduler.py`): the turn may run for a long
  wall-clock time while active and is killed only after 600s with no activity; `script_timeout_seconds` does
  **not** bound it (that caps only an optional pre-run `--script`). Iterations are capped by `agent.max_turns`
  (default 90).
- **The bundle composes the steps in order** — **defensive roll** (`eod-roll`, only if yesterday's state is
  stale, §8) → weather → calendar → active-tasks → **plan** (`eisenhower-plan`: set ≤3 `p1` + place the one
  deep anchor via `calendar-write`, §4c/§5a) → **follow-ups sweep** (§2 step 4: chases, fixed-item
  re-decisions, triage re-raises) → activation-reminder → assemble (the bundle's skills, §13), then
  **deliver** via cron's scheduled Discord path (R1a; delivery is *not* a bundle skill) — all inside the one
  run, each step degrading **visibly** on failure (§16), not as separate durable cards.
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
either path), and an ordinary substantive turn (must deliver).

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

Together these give the same "never two next things at once" guarantee the lock was meant to provide, without
a bespoke mutex — and without a plugin to host it.

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
  routine internal-job chatter only**, never for failures (§16/§17).
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
    of failure), routine status / health / heartbeat pings, "recovered/back-to-normal" notices, observability
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

## 13. Skill bundles + declarative config + curator-pin

- **Bundles** (`~/.hermes/skill-bundles/<slug>.yaml`): `skills:` (required, non-empty list) + optional
  `name`/`description`/`instruction`. `/<bundle> [instruction]` loads all listed skills at once (text after
  the command is attached as the instruction). Bundles **don't install** skills and **silently skip missing
  ones** — so the atomic skills must exist first.
  - `/forzare-morning-brief` = `eod-roll` (defensive missed-fire roll first, §8) · `weather` ·
    `calendar-read` · `todoist-surface` · `eisenhower-plan` · `followups-sweep` (§2 step 4: chases + fixed
    re-decisions + triage re-raises) · `activation-prompt` · `calendar-write` · `brief-assemble`. **The
    morning run is where the day's plan is WRITTEN** — `eisenhower-plan` sets the ≤3 `p1` (§4c) and
    `calendar-write` places the ONE protected deep anchor if a deep window exists (§5a). (These two were
    missing from the earlier composition, so the morning bundle couldn't actually build the day.)
  - `/forzare-replan` = `calendar-read` · `todoist-surface` · `eisenhower-plan` (redraw the remaining day from
    the current plan + active pool; no state-detect — that's the `/forzare` state path)
  - `/forzare-eod` = `eod-roll` (the roll + unconditional p1-clear + lifecycle ledger ticks + last-reconcile
    stamp, §8) · `todoist-surface` · `daily-reflect` · `eisenhower-plan` · `tomorrow-prep` · `calendar-write`.
    **At EOD `eisenhower-plan`/`tomorrow-prep` only PROPOSE** tomorrow's candidate ≤3 + anchor — **neither
    writes `p1`** (that is exclusively the morning run's job; this removes the earlier contradiction where both
    the morning brief and EOD appeared to set `p1`).
  - **`eisenhower-plan` is one skill with a mode by caller:** plan-time ranking used by the morning run
    (**writes** `p1` + the deep anchor) and by EOD (**proposal only**, no `p1` write). The caller (bundle)
    determines which.
  - The **02:00 `@waiting` reconcile** is owned by a dedicated **`waiting-reconcile`** skill (mark chase-due ·
    §4b set-time-invariant repair · unblock detection vs gog/activity/Discord · 14-day staleness, §8) — it is
    **not** in a bundle; it is run directly by the 02:00 cron job, state-only, never delivering.
  - `todoist-surface` is the atomic primitive reused across bundles (write once).
- **Boot-time skill-existence check (loud — closes the silent-skip hole).** Hermes bundles **silently skip
  skills that aren't installed** (above), so a missing `todoist-surface` would make `/forzare-morning-brief`
  run with *no surfacing and no error* — a silent failure that violates §0/§9. So **boot asserts every skill
  named by the 3 bundles is installed + pinned, and fails loud** (abort boot, and once the gateway is up, post
  to `#forzare-errors` via `hermes send`, R2) if any is missing. This is the only guard against Hermes'
  silent-skip behavior — without it, a typo'd or unpinned skill degrades the engine invisibly.
- **Declarative config** (`metadata.hermes.config` in each SKILL.md → stored under `skills.config` in
  `config.yaml`; entries are key/description/default/prompt): the **`work_schedule`** (per-weekday work blocks
  + alternating-Sunday anchor date — currently Tue/Thu/Sat 15:00–23:00 + alt-Sun anchored Jun 7=ON), weather
  thresholds (wind>17 / rain / <50°F / >90°F), wake anchor (5:15), **gym schedule** (days =
  Mon/Tue/Wed/Fri/Sat/Sun, rest = Thu; window — independent of `work_schedule`). (The **peak/free windows are
  *derived* from these at run-time, §2/§6a — not a stored config value**.) **Not hardcoded** — a new job = edit
  `work_schedule`, nothing else. Secrets (API keys) use `required_environment_variables`
  (name/prompt/help/required_for), prompted on first use, auto-injected into the sandbox.
- **Curator-pin the engine SKILLS** (not bundles): `hermes curator pin <name>` for `todoist-surface`,
  `weather`, `calendar-*`, and the other atomic skills — the curator never archives/consolidates pinned skills
  (and never auto-deletes; worst case is recoverable archival). Protects the engine from being
  garbage-collected as "stale." **Bundles need no pin — code-verified the curator only manages `SKILL.md`
  skills, never the `skill-bundles/*.yaml`** (pinning a bundle is a silent no-op; §19).

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
auxiliary:
  triage_specifier: { provider: anthropic, model: claude-haiku-4-5 }   # R7 — `hermes kanban specify` slot; pin a cheap model (live = provider: auto, model: "")
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
3. **`hermes-achievements` keep-out GUARD (not a removal).** `plugins.enabled` uses **allow-list** semantics,
   and `hermes-achievements` is **NOT currently in it** (verified 2026-07-11 — the earlier "it is enabled"
   claim was wrong). So the action is a **guard**: assert it never *enters* `plugins.enabled` (gamification —
   points/achievements — violates the §6a no-shame anti-patterns). Nothing to remove today; keep it out.

Two further drifts to repair while here (A17/U6): **`kanban.max_in_progress_per_profile` is `null`** (set to
`2`) and **`cron.max_parallel_jobs` is `null`** (set to `1`); and pin **`auxiliary.triage_specifier`** off its
`provider: auto` default to a cheap model for the §8b `specify` slot.

**Boot (deploy):** write `config.yaml` + `.env` → **fix the three live-config drifts above (R6b)** → install
the 3 bundles' atomic skills + pin them → **assert every skill the 3 bundles name is installed + pinned — fail
loud / abort boot if any is missing (§13 silent-skip guard)** → enable built-in plugins (no `bob-surface`,
R5) → declare cron jobs (morning/end-of-day/reconcile, each with its `deliver="discord[:…]"`, R1a) →
`hermes gateway start`. **Do NOT run the deprecated standalone `kanban daemon`** alongside the gateway
dispatcher (claim races) — the gateway runs the dispatcher.

**Runtime tick:** the **gateway** is one process running platform connections + cron (60s tick, `.tick.lock`)
+ the Kanban dispatcher (60s) → **if it dies, everything stops.** So gateway liveness is **two layers**:

- **Restart — already in place:** `~/Library/LaunchAgents/ai.hermes.gateway.plist` runs the gateway with
  `RunAtLoad=true` + **`KeepAlive` = `true`** (verified live 2026-07-11 — a plain `<true/>`, changed since the
  2026-06-30 `{SuccessfulExit: false}` reading; semantically it now restarts on **any** exit, clean or crash).
  Either way the **crash-self-heal** conclusion is unchanged. Installed by `hermes gateway`, not chezmoi — so
  it isn't in the dotfiles repo.
- **Liveness + failure alerting — the piece to build is the `forzare-ops watchdog`** (one out-of-band script,
  launchd-polled ~15 min, **zero LLM**), modeled on the existing
  `~/.local/bin/osquery-uptime-watchdog.sh`. It does **two** state-stamped scans each pass and routes every
  hit to `#forzare-errors`:
  - **(a) Gateway health.** `KeepAlive` catches a process *exit* but **not a wedged-but-alive gateway**, and
    never *tells* you — so the watchdog sends a one-shot probe a hung gateway can't answer:
    **`curl -fsS -m 3 http://127.0.0.1:8644/health`**, exit code **0 = up / 28 = hung / 7 = down** (verified,
    §19). On **down / hung / restart-looping** → loud alert.
  - **(b) forzare run failures.** Since its last check (a stamped watermark), it scans
    **`~/.hermes/cron/output/`** for failed ritual runs and the **Kanban DB** for cards in `gave_up`/`blocked`
    (the §16 `failure_limit` trips) — and routes each to the errors channel. **This is the concrete owner of
    "cron/pipeline failure summaries reach the user":** the watchdog is the errors *router*, closing the gap
    where a cron job's own delivery target would otherwise swallow the failure.
  - **Alert path (out-of-band, independent of the gateway):** **`hermes send --to discord:<#forzare-errors>`**
    (R2) — no LLM, no agent loop, no running gateway for bot-token platforms — so it can report the gateway's
    own death; the relay's phone/local push stays as belt-and-suspenders. **You cannot use the thing that's
    down to report its own death** — `hermes send` talks to Discord directly with the bot token. If
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
  - Daily `p1`-set: read `Today's 3` first; if 3 p1s already exist for today, no-op (§4c).
  - Kanban capture-pipeline kickoffs (§8b): **`hermes kanban create --triage --idempotency-key <capture-id>`**
    (R7) — re-firing returns the existing card, no dup; the brief is **not** a Kanban kickoff (its idempotency
    is the app-level guard above).
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
  - **Kanban job steps (the capture pipeline, §8b)** carry `--max-runtime`; on exceed the dispatcher
    SIGTERMs→SIGKILLs (5s grace) and re-queues (`timed_out` event). **Re-queue restarts the whole card, not
    the step (no mid-run resume, §19)** — exactly why every stage is idempotent + check-before-create (§8b). A
    hung step never freezes the day, and a restart never duplicates a task.
- **Any job that exhausts its `failure_limit` and gives up** → **loud on the errors channel** (§12); the
  captured item is still safe in Inbox (§8b).
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
- **Dry-run/staging path before go-live:** run the brief cron with a response whose *entire* output is the
  `[SILENT]` sentinel (§12.2/R3) — or `--deliver local` — so it exercises the full pipeline + writes to
  `~/.hermes/cron/output/` **without messaging the user or touching prod** — read the output, tune, then flip
  delivery to `discord`.

---

# PART IV — FORWARD PATH + OPEN

## 18. This is a Bob-only system; forward path if a second agent profile is ever added

**This surfacing engine is Bob's, full stop.** No other agent participates. (Elaine — the separate
email-triage agent in PLAN-v7 — is not part of this system. "Sierra" is a person, not an agent.)

- If a *second agent profile* is ever introduced for some other purpose, nothing here needs a redesign: cron
  jobs, skill bundles (atomic skills in a shared `skills.external_dirs`), the §9 firewall, and the delivery
  paths all stay. Kanban has **no per-profile isolation** — isolation is per-**board**
  (`~/.hermes/kanban/boards/<slug>/kanban.db`), so a new profile = its own board or just `--assignee` routing.
  Bob-only choices (single board, `default_assignee: "default"`, manual orchestration) don't paint the design into a
  corner.

## 18a. Post-V1 enhancements (deferred nice-to-haves — explicitly parked, not built)

These are **deferred, not excluded**: recorded here with rationale so they aren't forgotten, and deliberately
kept **out of V1**. Sequence is always: ship V1 first → then evaluate.

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

- **RESOLVED (verified 2026-06-30 via `td` CLI) — "is this date fixed vs. surfacing?" (#1 roll carve-out).** A
  task is **fixed (never auto-rolled)** if ANY of: `deadline != null`, `due.isRecurring == true`, **`due.date`
  contains a time** (the appointment signal), or `due.date` is a future date. Otherwise it's surfacing-dated →
  rolls. **Verified field structure** (`td task view --json`): timed due → `due.date = "2026-06-30T15:00:00"`
  (has `T`+time); date-only → `due.date = "2026-06-30"` (no `T`); recurring → `due.isRecurring = true`;
  deadline → top-level `deadline: {date, lang}` object, separate from `due`. **Detection is exact:** "has a
  time-of-day" = `"T" in due.date` (equivalently `len(due.date) > 10`). No ambiguity, no fallback needed.
- **cron missed-fire / catch-up — RESOLVED (verified 2026-06-30 against upstream `cron/jobs.py:488-492`).** A
  cron job due while the gateway was down **does replay on restart — but only within a bounded ±2h window.** So
  catch-up is the *primary* recovery for short outages, and the **defensive morning roll (§8) is the backstop
  for outages > 2h** (belt-and-suspenders, not redundant). End-of-day stays idempotent + date-stamped so
  {on-time fire, ≤2h catch-up, late morning re-run} each happen exactly once. **Build-time check (a test, not a
  design unknown):** confirm the date-stamp guard prevents a double-roll when both the late cron *and* the
  morning brief attempt it.
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
- **RESOLVED (code-verified 2026-06-30, `curator.py` / `skill_usage.py`) — `curator pin` targets SKILLS only.**
  The curator enumerates only `SKILL.md` directories; skill-bundle YAMLs (`~/.hermes/skill-bundles/`) are
  **not curator-managed at all** — it can't archive or consolidate them, so they need **no** pin. Pinning a
  bundle name is a **silent no-op** (writes a phantom usage record curation never reads). So: **pin the atomic
  skills, leave bundles alone** (§13 corrected).
- **RESOLVED (code + live-probe verified 2026-06-30) — gateway liveness probe.** The gateway serves an
  unauthenticated **`GET http://127.0.0.1:8644/health`** (the webhook platform adapter,
  `gateway/platforms/webhook.py:195,350`; static JSON, ~30ms). Watchdog: `curl -fsS -m 3
  http://127.0.0.1:8644/health`, branch on exit code — **0 = up, 28 = hung** (loop wedged: accepts TCP, no
  HTTP reply), **7 = down**. This is the hang KeepAlive + PID-checks miss (a hung gateway keeps a live PID;
  `gateway_state.json.updated_at` doesn't advance when idle). **NOT `:9119`** — that's the separate dashboard
  process, which infers "running" from the PID file and so reports a *hung* gateway as alive (useless for hang
  detection). **Caveat:** `:8644` exists only while the webhook platform is enabled (port configurable); for a
  platform-independent probe, set `API_SERVER_ENABLED=1` → `/health` on `:8642`. **Alert path (R2):**
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
  **`kanban.max_in_progress_per_profile: 2`** (§14). (No `@q2` flag, and no lifecycle labels — the roll
  counter is the private ledger, §4d/§5d.)

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
