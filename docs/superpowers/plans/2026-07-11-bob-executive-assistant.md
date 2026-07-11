# Bob executive-assistant implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for
> tracking.

**Goal:** Stand up **Bob as the user's executive assistant** — schedule-manager, task-manager, ADHD
(attention-deficit/hyperactivity disorder) manager — running as the **DEFAULT hermes-agent profile** (not a
dedicated one). **forzare** is the operating system for that role: the ADHD task-surfacing system this plan
builds (the system name stays "forzare" throughout). This is **the executive-assistant plan**: V1 drives
the user's day headlessly — schedule-driven morning brief, state-signal transitions, one-thing-or-nothing
surfacing, end-of-day roll, background capture pipeline, and loud system-failure alerting. Ship behind a
`[SILENT]` dry-run, calibrate, then flip to live.

**Architecture:** Cron is the clock (brief / end-of-day / reconcile / boundaries); skill bundles hold the
logic; the `/forzare` skill classifies state signals; Kanban is Bob's private background substrate (capture
pipeline only); **Todoist (via the `td` CLI) is the only user task store**; the owned layer
(`~/workspaces/Ivy/forzare/`) holds goals, dopamine menu, calibration, and tiny state files. Delivery is
**headless-native, zero custom plugin by default** — cron Discord delivery for rituals, clarify-tool native
buttons for live-session asks, and **`hermes send discord:<channel>`** for out-of-band failure alerts. A
launchd `hermes-uptime-watchdog` (chezmoi-managed, modeled on the existing osquery watchdog) catches a
dead/hung gateway.

**Tech Stack:** hermes-agent (cron / kanban / skills / skill-bundles / config.yaml), the `td` CLI (Todoist),
`gog` (Google Calendar), Open-Meteo + NWS (weather, keyless), bash, launchd, chezmoi (the delivery vehicle —
`dot_hermes/`, `dot_local/bin/`, `Library/LaunchAgents/`), the repo's `just`/`scripts/lint.sh` tooling.

**Reference spec:** `docs/superpowers/specs/2026-07-11-bob-executive-assistant-design.md`

## Global Constraints

- **Delivery vehicle — the chezmoi source-dir pipeline (stated once, applies plan-wide).** Every artifact
  this plan produces — `config.yaml` stanzas, `.env` keys, skills, bundles, the default-profile
  persona/directives, and the watchdog script + launchd plist — ships through the chezmoi source dir in
  THIS repo (`dot_hermes/`, `dot_local/bin/`, `Library/LaunchAgents/` templates — the same pattern as the
  existing osquery watchdog), never as unmanaged live-file edits. Applies that touch KeePassXC-gated
  templates (e.g. `dot_hermes/encrypted_private_config.yaml.age`, `dot_hermes/private_dot_env.tmpl`) are
  **marked user-run**: the agent edits source + verifies by rendered-template diff; the user runs the
  interactive `chezmoi apply` (agents use `--exclude=templates`). The Todoist store and the owned layer
  (`~/workspaces/Ivy/forzare/`) are the two deliberate exceptions — live data, not dotfiles.
- **Investigate before creating (Chesterton's Fence).** The Todoist 5 surfacing labels
  (`@deep`/`@light`/`@admin`/`@errand`/`@waiting`) and all 5 filters ALREADY EXIST (verified 2026-07-11).
  Every data-layer task is **verify-then-reconcile**, never blind create. Only `@rolled`/`@stalled` are new.
- **No unprompted Todoist projects.** Never create/rename/re-parent/archive Todoist projects without explicit
  say-so (the capture pipeline's case-4 asks the user inline; the build itself creates none).
- **Test-first for skills.** Drive each skill via a `[SILENT]` dry-run (§17) against disposable `[TEST]`
  Todoist tasks, then delete the `[TEST]` tasks. Never exercise a skill against real tasks until its dry-run
  is green.
- **`[SILENT]` is exact-match + success-only** (spec §12.2/R3): a dry-run turn's *entire* output must be the
  sentinel to suppress delivery; a failed turn is never silenced.
- **Delivery is headless-native only** (spec §12/R1): NO `ctx.inject_message`, NO `bob-surface` plugin. Cron
  Discord delivery + clarify buttons + `hermes send`.
- **Fix the three live-config drifts (R6b) BEFORE go-live** (Phase A): empty `timezone`,
  `kanban.auto_decompose: true`, `hermes-achievements` enabled.
- **Do not patch hermes-agent source.** Configure it through `config.yaml` / skills / bundles / cron only
  (never edit `~/.hermes/hermes-agent`).
- **Go-live is the LAST step.** Every phase before Phase G runs with delivery `[SILENT]`/`local`; nothing
  messages the user until Phase G flips it.
- The watchdog commit (Phase F) passes the repo pre-commit hook (`just lint-check` + `just test`); stage
  specific paths only.

---

## Phase A — Prerequisites + data layer

### Task A1: Create the owned-layer scaffolding under `~/workspaces/Ivy/forzare/`

**Files (all outside the repo):**

- Exists already: `~/workspaces/Ivy/forzare/dopamine-menu.md` (leave as-is).
- Create: `~/workspaces/Ivy/forzare/goals.md`
- Create: `~/workspaces/Ivy/forzare/calibration/priors.md`
- Create dirs: `~/workspaces/Ivy/forzare/calibration/`, `~/workspaces/Ivy/forzare/state/`

- [ ] **Step 1: Create the directory tree**

```bash
mkdir -p ~/workspaces/Ivy/forzare/calibration ~/workspaces/Ivy/forzare/state
```

- [ ] **Step 2: Write `goals.md`** — port the 5 goals from the `current-goals` memory, **carrying each
  goal's Eisenhower quadrant + current sub-focus** (spec §4c build prerequisite). Structure: one `##` heading
  per goal with `**Quadrant:**` + `**Sub-focus:**` + `**Yardstick note:**` lines. The quadrants are fixed:

  - Podium job = **Q1** (urgent + important — the top priority spike).
  - Essential Developer = **Q2** (protect a *daily* deep block).
  - Casually Concerned = **Q2** (long-game consistency).
  - Homelab = **Q2** (cross-cutting/enabling — forzare itself is part of it).
  - Karl (karlmdavis contracting) = **Q2** (spikes to Q1 on deliverables).

  End with the yardstick paragraph (Podium is the urgent Q1 now; the others are Q2 investments needing
  protected regular blocks).

- [ ] **Step 3: Write `calibration/priors.md`** — the auditable population priors (spec §6a). Include:

  - **Stimulant prior (CAL Q1-1):** methylphenidate **ER 54 mg daily** — formulation · dose · dose-time
    (default = the 05:15 wake anchor, hand-editable); onset ~1 h post-dose → ~12 h ascending profile with a
    late-evening wear-off dip. **PRN booster:** methylphenidate **IR 20 mg**, a *volunteered signal only*
    (never asked, INV-6) — "took a booster" ⇒ ~3–4 h capacity lift from report time.
  - **Chronotype / window priors:** evening-chronotype tilt, post-activation executive-function boost
    (decaying ~1–2 h, Mehren 2019), the work-day window shape.
  - A header stating these are **starting priors, refined per-person by the calibration loop** (α = 0.15),
    and that Bob **never** gives medication advice or dosing nags — capacity input only.

- [ ] **Step 4: Verify**

```bash
ls -la ~/workspaces/Ivy/forzare/ ~/workspaces/Ivy/forzare/calibration ~/workspaces/Ivy/forzare/state
grep -qi 'methylphenidate' ~/workspaces/Ivy/forzare/calibration/priors.md && echo "priors OK"
grep -Eqi 'Q1|Q2' ~/workspaces/Ivy/forzare/goals.md && echo "goals OK"
```

Expected: both dirs exist; `priors.md` names the stimulant prior; `goals.md` carries quadrants. **Acceptance:**
`goals.md` has all 5 goals each with a quadrant; `priors.md` has the ER-daily + IR-PRN prior + the no-advice
guard.

---

### Task A2: Fix the three live-config drifts (R6b)

**File:** `~/.hermes/config.yaml`, chezmoi-managed as `dot_hermes/encrypted_private_config.yaml.age`
(delivery-vehicle rule: edit the encrypted source; the KeePassXC-gated `chezmoi apply` is **user-run**).
**Back up first** (Global Rules: precious edits get a timestamped backup).

- [ ] **Step 1: Back up the live config**

```bash
cp ~/.hermes/config.yaml \
  ~/workspaces/backups/"$(date -u +%Y-%m-%dT%H-%M-%S).hermes-config-yaml.backup.yaml"
```

- [ ] **Step 2: Audit the current drift**

```bash
grep -nE '^timezone:|auto_decompose|hermes-achievements' ~/.hermes/config.yaml
```

Expected (the drift): `timezone:` empty, `auto_decompose: true`, `hermes-achievements` present in
`plugins.enabled`.

- [ ] **Step 3: Apply the three fixes**

  1. Set `timezone: "America/Denver"` (root key).
  2. Set `kanban.auto_decompose: false`.
  3. Remove `hermes-achievements` from `plugins.enabled` (gamification violates §6a no-shame anti-patterns).

- [ ] **Step 4: Verify**

```bash
grep -E '^timezone:|auto_decompose' ~/.hermes/config.yaml
grep -c 'hermes-achievements' ~/.hermes/config.yaml   # expect 0
python3 -c 'import yaml,sys; yaml.safe_load(open(sys.argv[1])); print("yaml OK")' ~/.hermes/config.yaml
```

Expected: `timezone: "America/Denver"`; `auto_decompose: false`; achievements count 0; YAML parses.
**Acceptance:** all three drifts resolved and the file still parses.

---

### Task A3: `.env` channels + relay wiring

**File:** `~/.hermes/.env`, chezmoi-managed as `dot_hermes/private_dot_env.tmpl` (delivery-vehicle rule:
edit the template source; the KeePassXC-gated `chezmoi apply` is **user-run**).

- [ ] **Step 1: Create the dedicated `#forzare-errors` Discord channel** (manual, in Discord) and copy its
  channel id. It carries **nothing but** forzare system/pipeline failures (spec §12.4 scope) — silent when
  healthy.

- [ ] **Step 2: Set the env keys** in `~/.hermes/.env`:
  - `DISCORD_HOME_CHANNEL` = the task channel (proactive-delivery target).
  - `DISCORD_ERRORS_CHANNEL` = the `#forzare-errors` channel id (always-loud target).

- [ ] **Step 3: Verify**

```bash
grep -E 'DISCORD_HOME_CHANNEL|DISCORD_ERRORS_CHANNEL' ~/.hermes/.env
```

Expected: both keys present and non-empty. **Acceptance:** home + errors channels resolve to distinct ids.

---

### Task A4: Todoist data layer — labels, filters, auth, `@waiting` invariant

**Store:** Todoist via `td` (outside the repo). **Verify-then-reconcile — most of this already exists.**

- [ ] **Step 1: Confirm `td` is on PATH + authenticated in Bob's headless environment**

```bash
command -v td && td task list --json >/dev/null 2>&1 && echo "td authed OK"
```

Expected: a path + "td authed OK". If not authed, authenticate before proceeding (Bob's environment must see
an authed `td` — spec §19).

- [ ] **Step 2: Verify the 5 surfacing labels exist; create the 2 lifecycle labels**

```bash
td label list --json | jq -r '.[].name' | grep -E '^@(deep|light|admin|errand|waiting)$' | sort
```

Expected: all 5 present (verified 2026-07-11). Then create the 2 new lifecycle labels (idempotent — skip if
present):

```bash
for L in rolled stalled; do
  td label list --json | jq -e --arg n "@$L" '.[]|select(.name==$n)' >/dev/null \
    || td label add "@$L"
done
td label list --json | jq -r '.[].name' | grep -E '^@(rolled|stalled)$'
```

Expected: `@rolled` + `@stalled` now exist (spec §4d — 5 → 7 labels; NO count-labels).

- [ ] **Step 3: Verify the 5 saved filters** (exact queries from §4b — already present 2026-07-11)

```bash
td filter list --json | jq -r '.[]|"\(.name)\t\(.query)"'
```

Expected exactly:
- `Today's 3` → `(today | overdue) & p1 & !@waiting`
- `Active now` → `(today | overdue) & !@waiting`
- `Follow-ups` → `@waiting & (today | overdue)`
- `Deep window` → `@deep & (today | overdue) & !@waiting`
- `Errands` → `@errand & !@waiting`

If any is missing/wrong, add/update it to the exact query above. **Acceptance:** 7 labels, 5 filters with the
exact §4b queries.

- [ ] **Step 4: Document the `@waiting` set-time invariant** (behavior enforced by the `todoist-surface`
  skill, Task B1) — applying `@waiting` ALWAYS sets a check-back due date + a blocker note at the same moment
  (never the bare label). This is a *skill contract*, verified in Task B1's dry-run, not a Todoist-side
  config. Record it in the `todoist-surface` SKILL.md.

---

### Task A5: Configure the DEFAULT profile's persona/directives (chezmoi template — docs only)

**Files (in THIS repo — the chezmoi source dir):** the `dot_hermes/` templates that carry the default
profile's persona/directives (extend `dot_hermes/encrypted_private_config.yaml.age` or add a
`dot_hermes/profiles/` template, matching however the live install stores the default profile — investigate
first, Chesterton's Fence). **This is a docs/template task: NEVER a live apply** — the KeePassXC-gated
`chezmoi apply` is user-run.

- [ ] **Step 1: Investigate where the default profile's persona lives** in the installed hermes-agent
  (config key vs a profiles file) before writing anything — mirror that exact location in the `dot_hermes/`
  source.

- [ ] **Step 2: Write the persona/directives** for the DEFAULT profile (Bob — no dedicated profile). The
  directives encode the spec's behavioral contract:
  - **Boss-of-the-schedule** (spec premise): Bob owns and drives the day — firm and directive, never a
    passive responder.
  - **The §0 one rule:** match task-side attributes to person-side state; surface exactly ONE thing — or
    nothing; the backlog stays out of view.
  - **The no-shame contract** (spec §0/§6a/§7): the user's task slippage is normal and handled gently
    (re-shape, never scorekeep, never guilt-wall); system failures are loud on `#forzare-errors` — never
    conflate the two.
  - **Decide-in-context** (spec §8b): ask for a decision while the context is fresh, one decision at a
    time; never defer to a batch or dump option checklists.

- [ ] **Step 3: Verify by rendered-template diff (never a live apply)**

```bash
cd /Users/stephen/workspaces/worktrees/dotfiles-forzare   # or the active checkout at execution time
CI=1 chezmoi --source "$PWD" execute-template --no-tty < dot_hermes/private_dot_env.tmpl >/dev/null && echo "env tmpl renders"
chezmoi --source "$PWD" diff ~/.hermes 2>/dev/null | head -40   # review the pending persona change; do NOT apply
```

Expected: the template renders; the diff shows exactly the persona/directives addition and nothing else.
**Acceptance:** the four contract points above appear in the rendered output; no live file changed (the
user runs the interactive apply).

---

## Phase B — Atomic skills (test-first; `todoist-surface` first)

> Every skill's applied target is `~/.hermes/skills/<name>/SKILL.md` (+ any helper scripts) — authored in
> the `dot_hermes/` source dir per the delivery-vehicle rule (Global Constraints) — is **curator-pinned**
> (`hermes curator pin <name>`, spec §13), and is driven test-first via a `[SILENT]` dry-run against `[TEST]`
> tasks. `td` usage is learned from the installed `/todoist-cli` skill — do NOT duplicate `td` command
> knowledge into these skills (spec §10).

### Task B1: `todoist-surface` — the atomic primitive (build FIRST)

**File:** `~/.hermes/skills/todoist-surface/SKILL.md`

- [ ] **Step 1: Author the skill.** It is the single reused primitive (spec §13): read the active pool via
  the saved filters, **groom-on-read** (spec §4c: missing load-label ⇒ treat `@light`; missing duration ⇒
  eligible but never capacity-fit; verb-first cleanup; next-action atomicity gate), match person-state → ONE
  task or nothing (spec §0/§4/§6), enforce the `@waiting` set-time invariant (A4 S4), and do the lifecycle
  label read-modify-write (spec §4d: `--labels` REPLACES the set — always write the full set).

- [ ] **Step 2: Curator-pin**

```bash
hermes curator pin todoist-surface && hermes curator status 2>/dev/null | grep -i todoist-surface
```

- [ ] **Step 3: Dry-run against `[TEST]` tasks**

```bash
# create disposable fixtures
td task add "[TEST] deep surfacing probe" --labels "@deep" --due today --json | jq -r '.id'
# exercise the skill in a [SILENT] one-shot; confirm it returns exactly ONE task or nothing
hermes -z '/skill todoist-surface — dry run, respond [SILENT]' --safe-mode 2>&1 | tail -20
# clean up
td task list --json | jq -r '.[]|select(.content|startswith("[TEST]"))|.id' | xargs -I{} td task delete {}
```

Expected: the skill surfaces at most one task, applies grooming, and never writes a partial label set.
**Acceptance:** dry-run green; `[TEST]` tasks deleted; `hermes cron/output` (if used) shows the decision.
**Verification of the full-set label write:**

```bash
td task list --json | jq -r '.[]|select(.content|startswith("[TEST]"))|.labels'   # after a roll tick, prior labels preserved
```

---

### Task B2: `weather` (Open-Meteo + NWS fallback)

**File:** `~/.hermes/skills/weather/SKILL.md`

- [ ] **Step 1: Author** — pull the day's relevant outdoor window (bike-to-gym; work-commute on work days),
  flag ONLY on the config thresholds (wind > 17 mph · any rain · < 50°F · > 90°F), quiet when clear (spec §2).
  **Source Open-Meteo (keyless); NWS as fallback** on Open-Meteo failure. Degrade-and-note on total failure
  ("weather unavailable — assume layers", spec §16) — never crash the brief.
- [ ] **Step 2: Curator-pin + dry-run**

```bash
hermes curator pin weather
curl -fsS -m 5 'https://api.open-meteo.com/v1/forecast?latitude=39.7&longitude=-105&hourly=temperature_2m,precipitation,wind_speed_10m&temperature_unit=fahrenheit&wind_speed_unit=mph' | jq '.hourly|keys'
```

Expected: Open-Meteo returns hourly temp/precip/wind; the skill flags only on threshold breach. **Acceptance:**
clear day → one-line "clear"; a threshold breach → actionable prep line; API-down → the degrade note.

---

### Task B3: `calendar-read` + `calendar-write` (gog; dedicated 🤖 calendar)

**Files:** `~/.hermes/skills/calendar-read/SKILL.md`, `~/.hermes/skills/calendar-write/SKILL.md`

- [ ] **Step 1: Confirm the dedicated 🤖 calendar exists** (create manually if not) — Bob writes **only**
  here, never the user's primary (spec §5c).

```bash
gog calendar list 2>&1 | grep -i '🤖\|bob' || echo "create the 🤖 calendar first"
```

- [ ] **Step 2: Author `calendar-read`** — read fixed anchors (user primary + the 🤖 calendar) for today's
  free-window computation (spec §2/§4c). **Author `calendar-write`** — write ONLY to the 🤖 calendar; never
  edit/delete user events; blocks are movable proposals except the §5c user-confirmed carve-out.
- [ ] **Step 3: Curator-pin + read/write dry-run**

```bash
hermes curator pin calendar-read && hermes curator pin calendar-write
gog auth status 2>&1 | tail -2   # confirm gog authed; else surface the re-auth repair (spec §16)
```

**Acceptance:** `calendar-read` returns today's anchors; a `calendar-write` dry-run creates a `[TEST]` block
on the 🤖 calendar only, then deletes it; auth-expired path surfaces the one-line repair, not silence.

---

### Task B4: `eisenhower-plan`, `activation-prompt`, `brief-assemble`

**Files:** three SKILL.md under `~/.hermes/skills/`.

- [ ] **Step 1: `eisenhower-plan`** — the agent-side Eisenhower narrowing (spec §4c/§5): pool → free windows
  → rank Q1 → Q2 against `goals.md` → capacity/window fit → cap at 3 → idempotent p1-set (read `Today's 3`
  first). "ANCHOR, don't fill": place ONE protected deep block only if a deep window exists (spec §5a).
- [ ] **Step 2: `activation-prompt`** — the non-negotiable morning activation line ("Breakfast first, then
  gym") + the gym-window-end backstop line ("Back from the gym?"), skipped on Thu / post-overnight recovery /
  signal-already-fired (spec §2/§3). Rotates phrasing by construction (spec §7).
- [ ] **Step 3: `brief-assemble`** — compose the ordered brief (weather → calendar → ≤3 → follow-ups →
  activation → one action), each step degrading visibly on failure (spec §11/§16). **Assembly only; delivery
  is cron-native, NOT a skill** (spec §11/§12).
- [ ] **Step 4: Curator-pin all three + dry-run**

```bash
for s in eisenhower-plan activation-prompt brief-assemble; do hermes curator pin "$s"; done
```

**Acceptance:** `eisenhower-plan` never assigns >3 p1 and is a no-op if 3 already exist today;
`brief-assemble` yields the 6-part ordered brief and drops optional blocks under low receptivity but always
includes the anchor.

---

### Task B5: `followups-sweep`, `daily-reflect`, `tomorrow-prep`

**Files:** three SKILL.md under `~/.hermes/skills/`.

- [ ] **Step 1: `followups-sweep`** — the §2-step-4 delivery consolidator: `@waiting` chases,
  fixed-item re-decisions (do late / reschedule / drop), triage re-raises. Reads state marked by the 02:00
  reconcile + EOD; the sweep is where deferred decisions are *delivered* (spec §2/§8).
- [ ] **Step 2: `daily-reflect`** — EOD report half: completions-as-wins (no scorecard, no misses list),
  receptivity-gated (spec §8). Gain-framed, task-level (spec §7).
- [ ] **Step 3: `tomorrow-prep`** — EOD pre-stage of tomorrow's candidate anchor + ≤3 (proposal only; spec
  §5b/§8).
- [ ] **Step 4: Curator-pin + dry-run against `[TEST]` fixtures**

```bash
for s in followups-sweep daily-reflect tomorrow-prep; do hermes curator pin "$s"; done
```

**Acceptance:** `daily-reflect` never lists misses; `followups-sweep` chases the most-overdue `@waiting`
first; `tomorrow-prep` proposes ≤3 without setting p1 (that's the morning's job).

---

### Task B6: `/forzare` classifier skill (state signals + schedule-override writes)

**File:** `~/.hermes/skills/forzare/SKILL.md`

- [ ] **Step 1: Author** — the single description-driven classifier (spec §3B): packs activation phrases in
  its `description` (auto-fire on "back from the gym" / "at work" / "I'm wiped" / "took a booster"), scoped to
  "the user reporting a change in their own availability/energy/location **right now**". Classifies into
  activation / work-shift / energy / location and dispatches (reshuffle + surface). **Persists the shift
  override** to `~/workspaces/Ivy/forzare/state/schedule-override.json` (block + date + recovery-morning flag,
  spec §2/§8a). **Low-confidence ⇒ confirm in one line** — a clarify button on a live Discord session, a plain
  question otherwise (spec §3B/§12.1c). The booster classifies through the energy branch (~3–4 h lift, logged
  in the calibration context; never asked).
- [ ] **Step 2: Curator-pin + classification dry-run**

```bash
hermes curator pin forzare
hermes -z '/forzare picked up a shift — dry run, respond [SILENT]' --safe-mode 2>&1 | tail -10
jq . ~/workspaces/Ivy/forzare/state/schedule-override.json   # confirm the override was written
rm -f ~/workspaces/Ivy/forzare/state/schedule-override.json  # clean the dry-run artifact
```

**Acceptance:** each of the 4 signal classes routes correctly on clear phrasing; a low-confidence phrase
triggers the one-line confirm (button on live session); the shift signal writes a valid
`schedule-override.json`; no `pre_gateway_dispatch` hook is used (spec §3B/§12).

---

## Phase C — Bundles + cron jobs

### Task C1: The three skill bundles

**Files:** `~/.hermes/skill-bundles/{forzare-morning-brief,forzare-replan,forzare-eod}.yaml`

- [ ] **Step 1: Write the bundles** (spec §13 exact compositions):
  - `forzare-morning-brief` = `weather` · `calendar-read` · `todoist-surface` · `followups-sweep` ·
    `activation-prompt` · `brief-assemble`
  - `forzare-replan` = `calendar-read` · `todoist-surface` · `eisenhower-plan`
  - `forzare-eod` = `todoist-surface` · `daily-reflect` · `eisenhower-plan` · `tomorrow-prep` ·
    `calendar-write`
- [ ] **Step 2: Boot-time skill-existence assertion (closes the silent-skip hole, spec §13).** Add a boot
  check (a small script Bob runs at gateway start, or a documented pre-start check) that asserts every skill
  named by the 3 bundles is installed + pinned and **fails loud** (abort boot; once up, `hermes send
  discord:<#forzare-errors>`) if any is missing.
- [ ] **Step 3: Verify**

```bash
for b in forzare-morning-brief forzare-replan forzare-eod; do
  echo "== $b =="; yq '.skills[]' ~/.hermes/skill-bundles/$b.yaml
done
# every named skill must resolve to an installed, pinned SKILL.md:
comm -23 <(yq '.skills[]' ~/.hermes/skill-bundles/*.yaml | sort -u) \
         <(ls ~/.hermes/skills | sort -u)   # expect empty
```

Expected: each bundle lists the exact skills; the `comm` diff is empty (no bundle names a missing skill).
**Acceptance:** bundles resolve fully; the boot assertion fails loud on a deliberately-unpinned skill (test it
once, then re-pin).

---

### Task C2: Cron jobs (rituals) with cron-native Discord delivery

**Store:** `~/.hermes/cron/jobs.json` via `hermes cron` (outside the repo). **All delivery is
`deliver="discord[:channel]"` (spec §12.1a/R1) — NOT a plugin.** Timezone is Denver (Task A2).

- [ ] **Step 1: Declare the jobs** (spec §1/§8/§19 decided times):
  - **Morning brief** ~05:15 **Mon–Sat** running `/forzare-morning-brief`, `deliver="discord"` (home
    channel). The **alternating-Sunday** logic lives in the `work_schedule` read (spec §2) — the brief fires
    every day but the Sunday content is schedule-derived; if a Sunday brief is wanted, add a Sunday job whose
    bundle honors the alt-Sunday anchor (Jun 7 = ON). Confirm whether Sunday should fire at build.
  - **End-of-day** **23:00 daily** running `/forzare-eod`, idempotent + date-stamped (spec §8).
  - **`@waiting` reconcile** **02:00 daily** — **state-only, NEVER messages the user** (spec §8); no
    `deliver` (or `[SILENT]`), it only marks state for the morning `followups-sweep`.
  - **Gym-window-end check** at the configured gym-window end — the "Back from the gym?" backstop, skipped on
    Thu / recovery mornings / signal-already-fired (spec §3).
  - **Block-boundary prompts** at the schedule's block edges (spec §3/§5), `deliver="discord"`.
- [ ] **Step 2: Verify (staged `[SILENT]` — do NOT deliver yet)**

```bash
hermes cron list
# dry-run the brief job with [SILENT] and confirm it wrote the audit log without delivering:
hermes cron run forzare-morning-brief --deliver local 2>&1 | tail -5
ls -t ~/.hermes/cron/output/ | head -3
```

Expected: all jobs listed at the right times/TZ; the dry-run writes `~/.hermes/cron/output/` and does NOT
message Discord. **Acceptance:** the 5 job families exist; the 02:00 reconcile has no user-facing delivery;
brief/EOD dry-runs are audit-logged and silent.

---

## Phase D — Capture pipeline (Kanban, private)

### Task D1: Private Kanban board + capture pipeline stages

**Store:** `~/.hermes/kanban.db` (Bob-private, firewalled from the user, spec §9). `default_assignee: bob`,
`auto_decompose: false`, `max_in_progress: 2`, `failure_limit: 2` (Task A2 / spec §14).

- [ ] **Step 1: Author the pipeline** — parent Bob does **stage 1 synchronously** (`td` quick-add to Inbox,
  instant ack, idempotent). Stages 2–5 run as background `bob`-subagent Kanban work (spec §8b). Kickoff:
  **`hermes kanban create --triage --idempotency-key <capture-id>`** (R7); concretize raw captures with
  **`hermes kanban specify`** (the `auxiliary.triage_specifier` Haiku slot, Task A2). The 5 stages: Place →
  Decide-placement (task-vs-event pre-check + 4 routing cases) → Verify+research-decision → Research → Split,
  each gating the next (spec §8b).
- [ ] **Step 2: Idempotent dup-guards (forced by no-mid-run-resume, spec §8a/§8b/§19).** Every stage is
  check-before-create: stage 1 skips if the capture is already in Inbox; stage 2 skips re-routing a placed
  task and skips a duplicate 🤖-calendar event; stage 5 skips existing subtasks. A restart converges to one
  task, never a dup.
- [ ] **Step 3: Never auto-create a project** (case 4 asks inline; spec §8b). Cases 3–4 route asks through
  the **parent** Discord-bound conversation (cron/subagent turns have no session for buttons, spec §12.1c).
- [ ] **Step 4: Failures are loud** — a stage error / un-completable card → `#forzare-errors` via `hermes
  send` (spec §8b/§16); the captured item is safe (stage 1 persisted it).
- [ ] **Step 5: Verify (dry-run with a `[TEST]` capture)**

```bash
hermes kanban create --triage --idempotency-key test-cap-001 2>&1 | tail -3
hermes kanban create --triage --idempotency-key test-cap-001 2>&1 | tail -3   # 2nd fire → SAME card (idempotent)
hermes kanban list 2>&1 | grep test-cap-001   # exactly one card
td task list --json | jq -r '.[]|select(.content|test("test-cap"))|.id' | xargs -r -I{} td task delete {}
hermes kanban delete <card-id>   # clean up the test card
```

Expected: the second create returns the existing card (no dup); one Inbox task; both cleaned up.
**Acceptance:** idempotency key dedupes; a simulated stage crash restarts from stage 1 and still yields one
task; a forced stage error lands on `#forzare-errors`.

---

## Phase E — Delivery + channels

### Task E1: `session_reset` + `[SILENT]` dry-run wiring + clarify-button ask patterns

**File:** `~/.hermes/config.yaml` (Task A2 file) + skill ask-patterns.

- [ ] **Step 1: Add the Discord `session_reset` stanza (R6a, spec §14)**:
  `platforms.discord.session_reset {mode: both, at_hour: 4, idle_minutes: 1440, notify: false}` — a
  daily-fresh task-channel session at 04:00, bracketed between the 23:00 EOD and the 5:15 brief. This is part
  of the procedural single-writer discipline (spec §12.3/R4).
- [ ] **Step 2: Confirm the `[SILENT]` guarantee empirically** (spec §12.2/R3): a turn whose *entire* output
  is `[SILENT]` is suppressed; a partial prefix is NOT; a failed turn is NEVER suppressed.

```bash
hermes -z 'respond with exactly: [SILENT]' --safe-mode 2>&1 | tail -3       # suppressed (no delivery)
hermes -z 'respond with: [SILENT] plus a task line' --safe-mode 2>&1 | tail -3   # DELIVERED (partial ≠ suppress)
```

- [ ] **Step 3: Standardize the clarify-button ask pattern** across skills that ask on a live session (§4
  defer, §7 stall decision, §8b cases 3–4, §3B low-confidence): max 4 choices + auto "Other"; fall back to a
  plain one-line question on cron/subagent-origin turns (spec §12.1c). **Never use emoji reactions as input**
  (no inbound reaction events, spec §19/R8) — outbound 👀→✅/❌ ack-reactions are a free "Bob heard you" cue only.
- [ ] **Step 4: Verify**

```bash
grep -A5 'session_reset' ~/.hermes/config.yaml
```

Expected: the stanza is present with `at_hour: 4`, `notify: false`. **Acceptance:** exact-match `[SILENT]`
suppresses, partial does not; clarify buttons render on a live session, plain questions on cron turns.

---

## Phase F — Watchdog + ops (chezmoi-managed, in this repo)

### Task F1: `hermes-uptime-watchdog` script + launchd plist + lint wiring + docs

**Files (in the dotfiles repo, via the delivery-vehicle rule — alongside the `dot_hermes/` templates of
Tasks A2/A3/A5):**

- Create: `dot_local/bin/executable_hermes-uptime-watchdog.sh`
- Create: `Library/LaunchAgents/com.webdavis.hermes-uptime-watchdog.plist.tmpl`
- Modify: `scripts/lint.sh` (add the loader/script templates to the finder if templated) + add a loader
  chezmoiscript if following the osquery/atuin loader pattern.
- Modify: `CLAUDE.md` (a "Hermes uptime watchdog" subsection).

- [ ] **Step 1: Write the watchdog script**, modeled on `dot_local/bin/executable_osquery-uptime-watchdog.sh`.
  Probe **`curl -fsS -m 3 http://127.0.0.1:8644/health`** and branch on exit code — **0 = up, 28 = hung, 7 =
  down** (spec §19). On down / hung / restart-looping, fire a **loud, out-of-band** alert via **`hermes send
  discord:<#forzare-errors>`** (R2 — no LLM, no agent loop, no running gateway needed for bot-token
  platforms), plus the relay's phone/local push as belt-and-suspenders. `set -euo pipefail`, double-quoted
  expansions, ISO-8601 timestamps (`date -u +"%Y-%m-%dT%H:%M:%SZ"`). **Do NOT curl the Discord webhook
  directly** (R2 dropped that phrasing). Note the `:8644` caveat — it exists only while the webhook platform
  is enabled; for a platform-independent probe, `API_SERVER_ENABLED=1` → `/health` on `:8642` (spec §19).
- [ ] **Step 2: Write the plist**, modeled on `com.webdavis.osquery-uptime-watchdog.plist.tmpl` — launchd
  `StartInterval` ~900s (15 min), `RunAtLoad` per the osquery model, `Label`
  `com.webdavis.hermes-uptime-watchdog`, stdout/stderr to `~/.local/log/hermes/uptime-watchdog.log`.
- [ ] **Step 3: Wire lint** — add the plist loader template to `find_shell_templates` in `scripts/lint.sh`
  (the `.sh` helper is auto-shellchecked by `find_shell_files`; the `.plist.tmpl` is XML → `plutil -lint`).
- [ ] **Step 4: Document** in `CLAUDE.md` — the watchdog probes `:8644/health`, alerts out-of-band via
  `hermes send` (never through the dead gateway), and closes the KeepAlive hang-detection gap.
- [ ] **Step 5: Verify (plumbing only — no real alert)**

```bash
cd /Users/stephen/workspaces/Ivy/webdavis/dotfiles
shellcheck dot_local/bin/executable_hermes-uptime-watchdog.sh
CI=1 chezmoi execute-template --no-tty < Library/LaunchAgents/com.webdavis.hermes-uptime-watchdog.plist.tmpl | plutil -lint -
# health probe returns 0 while the gateway is up:
curl -fsS -m 3 http://127.0.0.1:8644/health >/dev/null && echo "gateway health OK (exit 0)"
```

Expected: shellcheck clean; `plutil -lint` → `OK`; the live probe exits 0. **Acceptance:** the script alerts
via `hermes send` on a simulated down/hung code (test with a bogus port), and stays silent when healthy.
**This task's files are committed to the repo via the normal pre-commit flow** (`just lint-check` + `just
test`), separate from the two doc commits.

---

## Phase G — Dry-run → calibrate → go-live

### Task G1: Staged `[SILENT]` runs + threshold checks + flip to live

**Files:** none new (operational — the final gate).

- [ ] **Step 1: Run the full brief + EOD `[SILENT]` for several days** (or `--deliver local`), reading
  `~/.hermes/cron/output/` each day (spec §17). Confirm: the brief fires 05:15, surfaces ≤3 sensibly, the
  weather/calendar/follow-up steps degrade visibly (never silently), the EOD roll + p1-clear + lifecycle
  labels behave, and the 02:00 reconcile marks (never messages).
- [ ] **Step 2: Verify idempotency end-to-end** — the date-stamp guard prevents a double-roll when both a
  late cron catch-up and the defensive morning roll attempt it (spec §8/§19 build check).

```bash
jq . ~/workspaces/Ivy/forzare/state/last-reconcile.json   # advances exactly once/day
```

- [ ] **Step 3: Calibrate** — tune the duration upward-bias factor + weather thresholds + brief content from
  the observed dry-run output (spec §4c/§6a). Priors stay auditable in `calibration/priors.md`.
- [ ] **Step 4: Flip to live** — change the brief/EOD/boundary cron jobs' delivery from `[SILENT]`/`local` to
  `deliver="discord"` (home channel). Confirm the errors channel is still `hermes send` + belt-and-suspenders
  relay. **This is the last step; do it only after Steps 1–3 are green.**
- [ ] **Step 5: Post-go-live smoke**

```bash
hermes cron list                                   # jobs live, Denver TZ
curl -fsS -m 3 http://127.0.0.1:8644/health && echo "gateway up"
launchctl print "gui/$(id -u)/com.webdavis.hermes-uptime-watchdog" | grep -i state
```

Expected: jobs deliver to the home channel; watchdog loaded; health probe 0. **Acceptance:** a real brief
lands in the home channel; a forced failure lands in `#forzare-errors`; no gamification/achievements output
anywhere.

---

## Phase H — Post-V1 follow-ups (explicitly parked — NOT built in V1)

> Recorded so reviewers cover them; each ships only after V1 is live (spec §18a). These are **tasks/sections
> for the future**, deliberately out of V1 scope.

- [ ] **§18a Langfuse (self-hosted) — research first.** Adopt Langfuse system-wide, then research the forzare
  integration (can it trace subagent/Kanban work end-to-end?), **self-host** (traces carry task content —
  privacy gate), enable only if it earns its keep. Not load-bearing (cron audit log + `#forzare-errors` cover
  V1 observability). NOT the behavioral tuner (§6a owns that).
- [ ] **Phone webhook lane** — a phone Shortcut/geofence POSTs the same `/forzare` signals to
  `platforms.webhook` (spec §3C). Expose via **`tailscale serve` (NOT `tailscale funnel`)** — keep it on the
  tailnet, not the public internet.
- [ ] **Hue exit-ramp cues** — physical light cues for the §3a hyperfocus ramp-out (soft pre-warning →
  one-last-thing → hard stop).
- [ ] **Todoist real-time webhooks** — replace the poll-on-read grooming with push, if/when it earns its
  keep.
- [ ] **Voice capture** — a spoken-capture path into stage 1 of the §8b pipeline.
- [ ] **Edit-in-place ledger channel** — a self-updating Discord message/embed as a live day-ledger, distinct
  from the errors channel.
- [ ] **Email/comms triage — the natural executive-assistant expansion.** Reading, triaging, and chasing the
  user's email and other comms is the obvious next lane for Bob's executive-assistant role — **explicitly
  chosen out of V1 (spec §18: the surfacing engine is Bob-only; Elaine's email-triage lane is a separate
  system), not forgotten.** Sequence after V1: scope the read-access grant first (the §8 unblock-detection
  already anticipates email as a future signal), then design the triage lane against the same two-channel /
  no-shame invariants.

---

## Self-Review

**Spec coverage:** owned layer (goals.md w/ quadrants, priors.md w/ methylphenidate ER 54 + IR 20 PRN, state/,
calibration/) → A1; three live-config drifts (empty TZ / auto_decompose / achievements, R6b) → A2 + G4; `.env`
channels + `#forzare-errors` → A3; Todoist labels (5 existing + `@rolled`/`@stalled`) + 5 filters + auth +
`@waiting` invariant → A4/B1; default-profile persona/directives (boss-of-the-schedule, §0 one-rule,
no-shame, decide-in-context; rendered-template diff, never a live apply) → A5; `todoist-surface` first → B1;
weather / calendar / eisenhower / activation / brief-assemble / followups / reflect / tomorrow-prep /
`/forzare` classifier → B2–B6; bundles + boot assertion → C1; cron rituals w/ cron-native delivery (5:15
Mon–Sat + alt-Sunday, 23:00 EOD, 02:00 reconcile, gym-window-end, boundaries; R1a) → C2; Kanban capture
pipeline (`--triage` + `specify` + idempotency keys + 5 stages + dup-guards, R7) → D1; `session_reset` (R6a)
+ `[SILENT]` guarantee (R3) + clarify buttons / no-reaction-input (R8) → E1; `hermes-uptime-watchdog` + plist
modeled on osquery + `:8644/health` probe + `hermes send` alert (R2) → F1; dry-run → calibrate → go-live →
G1; post-V1 (Langfuse self-host, `tailscale serve` webhook, Hue, Todoist webhooks, voice, ledger, email/comms
triage) → H. Delivery is headless-native, no plugin, no `inject_message` (R1/R4/R5) throughout; every
produced artifact ships via the chezmoi source-dir pipeline (the delivery-vehicle rule, Global Constraints).
No gaps.

**Placeholder scan:** verification commands are runnable; `<card-id>` / `<capture-id>` / lat-long /
channel-ids are the intentionally per-environment values a cold reader fills from their own setup (Discord
channel ids, the specific Kanban card id from the prior command's output).

**Consistency:** channel names (`DISCORD_HOME_CHANNEL` / `DISCORD_ERRORS_CHANNEL` / `#forzare-errors`), the
health probe (`curl -fsS -m 3 http://127.0.0.1:8644/health`, 0/28/7), the alert primitive (`hermes send
discord:<channel>`), the `[SILENT]` exact-match rule, and the label/filter names match the spec and each
other across A–H.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-11-bob-executive-assistant.md`. Execute
task-by-task with superpowers:subagent-driven-development or superpowers:executing-plans; **go-live (G4) is
the final step** — everything before it runs `[SILENT]`/`local`.
