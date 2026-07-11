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
pipeline only), assigned to the **`default` profile** (persona "Bob" — there is no profile named `bob`;
`bob → hermes -p default` is a wrapper alias); **Todoist (via the `td` CLI v1.75.3) is the only user task
store**; the owned layer (`~/workspaces/Ivy/forzare/`) holds goals, dopamine menu, calibration, and tiny state
files (incl. the **lifecycle ledger** `state/task-lifecycle.json` — spec §4d, which replaces the old
`@rolled`/`@stalled` labels). Delivery is **headless-native, zero custom plugin by default** — cron Discord
delivery for rituals, clarify-tool native buttons for live-session asks, and
**`hermes send --to discord:<channel>`** for out-of-band failure alerts. A launchd **`forzare-ops-watchdog`**
(chezmoi-managed, modeled on the existing osquery watchdog) catches a dead/hung gateway **and** scans
`~/.hermes/cron/output/` + the kanban DB for failed runs, routing every hit to `#forzare-errors`.

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
  (`deep`/`light`/`admin`/`errand`/`waiting` — stored **unprefixed**; the `@` is filter-query notation, spec
  §4b) and all 5 filters ALREADY EXIST (verified 2026-07-11). Every data-layer task is
  **verify-then-reconcile**, never blind create. **No new labels** are created — the roll counter now lives
  in the private lifecycle ledger (`forzare/state/task-lifecycle.json`, spec §4d), not in `@rolled`/`@stalled`
  labels (those are removed from the design).
- **No unprompted Todoist projects.** Never create/rename/re-parent/archive Todoist projects without explicit
  say-so (the capture pipeline's case-4 asks the user inline; the build itself creates none).
- **Test-first for skills.** Drive each skill via a staged `[SILENT]`/`--deliver local` cron dry-run (§17)
  against disposable `[TEST]` Todoist tasks, then delete the `[TEST]` tasks (`td task delete --yes`). Never
  exercise a skill against real tasks until its dry-run is green. **Do NOT use `hermes -z … --safe-mode`** to
  "prove" `[SILENT]` — `--safe-mode` strips `skills.config`/plugins the skills need, and a `-z` one-shot does
  not traverse the delivery filter; verify suppression on the real cron/gateway path (below).
- **`[SILENT]` suppression differs by path** (spec §12.2/R3, verified 2026-07-11): the **gateway**
  (live-session) path suppresses only on an **exact whole-response** sentinel, success-only; the **cron** path
  is **lenient** — whole-response, first line, last line, or any `[SILENT]`-prefixed output. A **failed** turn
  is never silenced on either path.
- **Delivery is headless-native only** (spec §12/R1): NO `ctx.inject_message`, NO `bob-surface` plugin. Cron
  Discord delivery + clarify buttons + `hermes send --to`.
- **Fix the live-config drifts (R6b) BEFORE go-live** (Phase A): empty `timezone` and
  `kanban.auto_decompose: true` (both live-confirmed); plus `kanban.max_in_progress_per_profile: null` and
  `cron.max_parallel_jobs: null`. **`hermes-achievements` is a keep-OUT guard, not a removal** — it is NOT
  currently in `plugins.enabled` (verified 2026-07-11); assert it never enters (spec §14).
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
- Runtime-written under `state/` (created lazily by the skills, not here): `last-reconcile.json`,
  `schedule-override.json` (shift override + today's gym `activation` field, spec §8a), and the **lifecycle
  ledger** `task-lifecycle.json` (`roll_count`/`written_due`/`last_escalated` per task id, spec §4d).

- [ ] **Step 1: Create the directory tree**

```bash
mkdir -p ~/workspaces/Ivy/forzare/calibration ~/workspaces/Ivy/forzare/state
```

- [ ] **Step 2: Write `goals.md`** — port the 5 goals from the `current-goals` memory, **carrying each
  goal's Eisenhower quadrant + current sub-focus** (spec §4c build prerequisite). Structure: one `##` heading
  per goal with `**Quadrant:**` + `**Sub-focus:**` + `**Yardstick note:**` lines. Starting quadrants:

  - Podium job = **Q1** (urgent + important — the top priority spike).
  - Essential Developer = **Q2** (protect a *daily* deep block).
  - Casually Concerned = **Q2** (long-game consistency).
  - Homelab = **Q2** (cross-cutting/enabling — forzare itself is part of it).
  - Karl (karlmdavis contracting) = **Q2** (spikes to Q1 on deliverables).

  **REQUIRED — per-goal user confirmation at implementation.** The `current-goals` memory is dated 2026-05-22
  and the goals/quadrants **may be stale** (e.g. "Podium" as the Q1 spike must be re-confirmed as still the
  urgent job — this is the daily-read yardstick, so a wrong entry mis-ranks every morning). Before writing,
  ask the user **one goal at a time** (decide-in-context): "still the 5 goals? still these quadrants/
  sub-focus?" — do not silently port a year-old snapshot. End with the yardstick paragraph (the confirmed Q1
  is the urgent spike now; the others are Q2 investments needing protected regular blocks).

- [ ] **Step 3: Write `calibration/priors.md`** — the auditable population priors (spec §6a). Include:

  - **Stimulant prior (CAL Q1-1) — conservative model, specifics recorded from the user at implementation.**
    The user takes an extended-release **methylphenidate** daily + an immediate-release booster PRN. **Ask
    once at build** for the exact formulation · dose · dose time (default anchor = the 05:15 wake time,
    hand-editable) — do not hardcode a dose here. Model the daily coverage as a **RANGE with uncertainty, NOT
    a monotonic ascending curve:** onset ~1 h post-dose, a broad mid-hours plateau/peak (*from training, not
    verified:* extended-release methylphenidate labels put peak plasma ~6–10 h post-dose, then decline —
    confirm against the specific product), a **late-afternoon/evening wear-off dip**, and two caveats the
    prior must state: **peak plasma ≠ peak cognition** (effect is domain/dose-dependent) and an **afternoon
    offset/rebound** watch window. **PRN booster:** a *volunteered signal only* (never asked, INV-6) — "took a
    booster" ⇒ ~3–4 h capacity lift from report time.
  - **Chronotype / window priors:** evening-chronotype tilt; post-activation executive-function boost —
    Mehren 2019 *measured* the gain at **~33 min post-exercise** and did not track full decay, so the **~1–2 h**
    working window is the **research synthesis' own estimate**, labeled as such (not a measured Mehren value);
    the work-day window shape.
  - A header stating these are **starting priors, refined per-person by the calibration loop** (α = 0.15),
    and that Bob **never** gives medication advice or dosing nags — capacity input only.

- [ ] **Step 4: Verify**

```bash
set -o pipefail
ls -la ~/workspaces/Ivy/forzare/ ~/workspaces/Ivy/forzare/calibration ~/workspaces/Ivy/forzare/state
grep -qi 'methylphenidate' ~/workspaces/Ivy/forzare/calibration/priors.md && echo "priors OK"
grep -qi 'peak plasma' ~/workspaces/Ivy/forzare/calibration/priors.md && echo "conservative model note OK"
grep -Eqi 'Q1|Q2' ~/workspaces/Ivy/forzare/goals.md && echo "goals OK"
```

Expected: both dirs exist; `priors.md` names the stimulant prior; `goals.md` carries quadrants. **Acceptance:**
`goals.md` has all 5 goals each with a quadrant; `priors.md` has the ER-daily + IR-PRN prior + the no-advice
guard.

---

### Task A2: Fix the live-config drifts + own the kanban / session_reset / cron stanza values (R6b + spec §14)

**File:** `~/.hermes/config.yaml`, chezmoi-managed as `dot_hermes/encrypted_private_config.yaml.age`
(delivery-vehicle rule: edit the encrypted source; the KeePassXC-gated `chezmoi apply` is **user-run**).
**Back up first** (Global Rules: precious edits get a timestamped backup). All key names below are
**live-verified 2026-07-11**.

- [ ] **Step 1: Back up the live config**

```bash
cp ~/.hermes/config.yaml \
  ~/workspaces/backups/"$(date -u +%Y-%m-%dT%H-%M-%S).hermes-config-yaml.backup.yaml"
```

- [ ] **Step 2: Audit the current state**

```bash
grep -nE '^timezone:|^session_reset:|auto_decompose:|max_in_progress_per_profile:|default_assignee:|max_parallel_jobs:' ~/.hermes/config.yaml
grep -c 'hermes-achievements' ~/.hermes/config.yaml   # expect 0 (achievements is NOT enabled — verified)
sed -n '/^plugins:/,/^[a-z]/p' ~/.hermes/config.yaml | grep -E 'discord|provider'   # confirm the live members we must NOT strip
```

Expected: `timezone: ''`; live `session_reset.mode: none`; `auto_decompose: true`;
`max_in_progress_per_profile: null`; `default_assignee: ''`; `cron.max_parallel_jobs: null`;
`hermes-achievements` count **0**; `platforms/discord` + the providers present under `plugins.enabled`.

- [ ] **Step 3: Apply the fixes** (edit the encrypted source; **additive** — never delete existing
  `plugins.enabled` members):

  1. `timezone: "America/Denver"` (root key).
  2. `kanban.auto_decompose: false`.
  3. `kanban.max_in_progress_per_profile: 2` (real key name — was `null`).
  4. `kanban.default_assignee: "default"` (the `default` profile / persona "Bob" — spec §8b/§10; empty also
     resolves to `default`, but set it explicitly).
  5. `cron.max_parallel_jobs: 1` (was `null` = unbounded — pin so rituals never interleave, spec §14/U6).
  6. Root `session_reset` → `mode: both`, `at_hour: 4`, `idle_minutes: 1440`, `notify: false` (root key, NOT
     `platforms.discord.*` — verified; `notify` is a real subkey, spec §14/E1).
  7. `auxiliary.triage_specifier` → pin off `provider: auto` to a cheap model (e.g. `provider: anthropic`,
     `model: claude-haiku-4-5`) for the §8b `specify` slot.
  8. **`hermes-achievements` keep-OUT guard (no removal):** it is NOT in `plugins.enabled` today — just
     confirm it stays out (gamification violates §6a no-shame anti-patterns). Do **not** rewrite the
     `plugins.enabled` list; leave `platforms/discord` and every provider entry intact.

- [ ] **Step 4: Verify**

```bash
set -o pipefail
grep -E '^timezone: "America/Denver"' ~/.hermes/config.yaml && echo "tz OK"
grep -E 'auto_decompose: false|max_in_progress_per_profile: 2|default_assignee: "default"' ~/.hermes/config.yaml
grep -E 'max_parallel_jobs: 1' ~/.hermes/config.yaml && echo "cron cap OK"
test "$(grep -c 'hermes-achievements' ~/.hermes/config.yaml)" -eq 0 && echo "achievements absent OK"
grep -q 'platforms/discord' ~/.hermes/config.yaml && echo "discord platform preserved OK"
python3 -c 'import yaml,sys; yaml.safe_load(open(sys.argv[1])); print("yaml OK")' ~/.hermes/config.yaml
```

Expected: every asserted key present; achievements count 0; `platforms/discord` still enabled; YAML parses.
**Acceptance:** drifts resolved, kanban/cron/session_reset values set, no `plugins.enabled` member removed,
file parses. (The `session_reset` policy itself is verified via a resolved-policy read in Task E1.)

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

**Store:** Todoist via `td` v1.75.3 (outside the repo). **Verify-then-reconcile — this already exists; NO new
labels are created** (the roll counter is the lifecycle ledger, spec §4d). **`td … --json` output is an
envelope** — `td task list --json` → `{ "results": [...] }`; `td label list --json` → `{ "results": [...],
"sharedLabels": [...] }`; `td filter list --json` → `{ "results": [...] }`. Parse `.results[]`, never bare
`.[]`. Label names are **unprefixed** (`deep`, not `@deep`) — verified.

- [ ] **Step 1: Confirm `td` is on PATH + authenticated in Bob's headless environment**

```bash
set -o pipefail
command -v td && td task list --json >/dev/null 2>&1 && echo "td authed OK"
```

Expected: a path + "td authed OK". If not authed, authenticate before proceeding (Bob's environment must see
an authed `td` — spec §19).

- [ ] **Step 2: Verify the 5 surfacing labels exist (NO new labels to create)**

```bash
set -o pipefail
td label list --json | jq -r '.results[].name' | grep -E '^(deep|light|admin|errand|waiting)$' | sort \
  | tee /tmp/forzare-labels.txt
test "$(wc -l < /tmp/forzare-labels.txt)" -eq 5 && echo "5 surfacing labels OK"
```

Expected: exactly the 5 unprefixed names `admin deep errand light waiting` (verified 2026-07-11). **No
`@rolled`/`@stalled` are created** — those labels are removed from the design (spec §4d). If a label is
genuinely missing, create it with the verified verb: `td label create --name "<name>"` (NOT `td label add`,
which is not a subcommand).

- [ ] **Step 3: Verify the 5 saved filters** (exact queries from §4b — already present 2026-07-11)

```bash
set -o pipefail
td filter list --json | jq -r '.results[] | "\(.name)\t\(.query)"'
```

Expected exactly (inside filter-query syntax the labels ARE `@`-prefixed — that is Todoist query notation,
distinct from the unprefixed stored names of Step 2):
- `Today's 3` → `(today | overdue) & p1 & !@waiting`
- `Active now` → `(today | overdue) & !@waiting`
- `Follow-ups` → `@waiting & (today | overdue)`
- `Deep window` → `@deep & (today | overdue) & !@waiting`
- `Errands` → `@errand & !@waiting`

If any is missing/wrong, `td filter create`/`td filter update` it to the exact query above. **Acceptance:** 5
surfacing labels (no lifecycle labels), 5 filters with the exact §4b queries.

- [ ] **Step 4: Document the `@waiting` set-time invariant** (behavior enforced by the `todoist-surface`
  skill, Task B1) — applying `@waiting` ALWAYS sets a check-back due date + a blocker note at the same moment
  (never the bare label). This is a *skill contract*, verified in Task B1's dry-run, not a Todoist-side
  config. Record it in the `todoist-surface` SKILL.md.

---

### Task A5: Configure the DEFAULT profile's persona/directives (`SOUL.md` — chezmoi-managed)

**Persona home — INVESTIGATED (verified 2026-07-11):** the `default` profile's persona lives in
**`~/.hermes/SOUL.md`** (`hermes profile show default` → `Path: ~/.hermes`, `SOUL.md: exists`; the file opens
"You are Bob…"). It is **plaintext with no secrets** (`grep -ic 'token|api.key|secret|password' SOUL.md` = 0)
— so it is NOT inside the encrypted `.age` config, and does **not** need KeePassXC to apply. It is not yet
chezmoi-managed (`dot_hermes/` currently holds only `encrypted_private_config.yaml.age` +
`private_dot_env.tmpl`).

**File to add (in THIS repo):** `dot_hermes/private_SOUL.md` (a plaintext managed file — `private_` keeps the
applied mode 0600; no `.tmpl` needed since it carries no variables). **Because it is plaintext and
non-secret, the apply is agent-runnable** via `chezmoi apply --exclude=templates ~/.hermes/SOUL.md` — unlike
the `.age` config in Task A2, this is NOT a user-run/KeePassXC step. (Preserve the existing SOUL.md content;
this task *extends* the persona with the forzare directives, it does not overwrite Bob's character.)

- [ ] **Step 1: Snapshot the live SOUL.md** so the extension is additive, not a rewrite:

```bash
cp ~/.hermes/SOUL.md \
  ~/workspaces/backups/"$(date -u +%Y-%m-%dT%H-%M-%S).hermes-SOUL-md.backup.md"
```

- [ ] **Step 2: Add the forzare persona/directives** to `dot_hermes/private_SOUL.md` (default profile — "Bob"
  is the persona of the `default` profile, no dedicated profile). The directives encode the spec's contract:
  - **Boss-of-the-schedule** (spec premise): Bob owns and drives the day — firm and directive, never a
    passive responder.
  - **The §0 one rule:** match task-side attributes to person-side state; surface exactly ONE thing — or
    nothing; the backlog stays out of view.
  - **The no-shame contract** (spec §0/§6a/§7): the user's task slippage is normal and handled gently
    (re-shape, never scorekeep, never guilt-wall); system failures are loud on `#forzare-errors` — never
    conflate the two.
  - **Decide-in-context** (spec §8b): ask for a decision while the context is fresh, one decision at a
    time; never defer to a batch or dump option checklists.

- [ ] **Step 3: Verify by rendering the managed file against source (no `.age`, no KeePassXC)**

```bash
set -o pipefail
cd "$(git rev-parse --show-toplevel)"
# A plaintext managed file has no template vars — chezmoi `cat` shows exactly what would land:
chezmoi --source "$PWD" cat ~/.hermes/SOUL.md | grep -Eqi 'one thing|no-shame|boss|decide-in-context' \
  && echo "forzare directives present in managed SOUL.md"
chezmoi --source "$PWD" diff ~/.hermes/SOUL.md | head -60   # review the pending persona change
```

Expected: the four contract points appear in the managed content; the diff shows only the persona extension.
**Acceptance:** the directives are present; `chezmoi apply --exclude=templates ~/.hermes/SOUL.md` is the
(agent-runnable) deployment step — no secret file touched, no KeePassXC prompt.

---

## Phase B — Atomic skills (test-first; `todoist-surface` first)

> Every skill's applied target is `~/.hermes/skills/<name>/SKILL.md` (+ any helper scripts) — authored in
> the `dot_hermes/` source dir per the delivery-vehicle rule (Global Constraints) — is **curator-pinned**
> (`hermes curator pin <name>`, spec §13), and is driven test-first via a **staged cron dry-run**. `td` usage
> is learned from the installed `/todoist-cli` skill — do NOT duplicate `td` command knowledge into these
> skills (spec §10).
>
> **Staged dry-run pattern (reused below; verified flags).** `hermes cron run <id>` only *queues* a job for
> the next tick and takes **no `--deliver`**; `hermes cron tick` runs due jobs once. So the dry-run recipe is:
> create a one-shot job with `--deliver local` (NOT Discord), force it, then read the audit artifact —
> **never** `hermes -z … --safe-mode` (safe-mode strips `skills.config`/plugins, and a `-z` one-shot does not
> traverse the delivery filter, so it can't prove `[SILENT]`):
>
> ```bash
> set -o pipefail
> JID=$(hermes cron create '0 0 1 1 *' '<prompt: run the skill; if nothing to surface respond exactly [SILENT]>' \
>         --skill <skill> --deliver local --name "test-<skill>" | grep -oE 'id[: ]+[A-Za-z0-9]+' | awk '{print $NF}')
> hermes cron run "$JID" && hermes cron tick
> LATEST=$(ls -td ~/.hermes/cron/output/*/ 2>/dev/null | head -1); echo "audit: $LATEST"; cat "$LATEST"* 2>/dev/null | tail -40
> hermes cron remove "$JID"   # clean up the staging job
> ```
>
> `--deliver local` means **nothing reaches Discord**; the cron audit under `~/.hermes/cron/output/<hash>/`
> is the evidence. (Adjust the schedule/prompt per skill.)

### Task B1: `todoist-surface` — the atomic primitive (build FIRST)

**File:** `~/.hermes/skills/todoist-surface/SKILL.md`

- [ ] **Step 1: Author the skill.** It is the single reused primitive (spec §13): read the active pool via
  the saved filters (`.results[]`), **groom-on-read** (spec §4c: missing load-label ⇒ treat `@light`; missing
  duration ⇒ eligible but never capacity-fit; verb-first cleanup; next-action atomicity gate), match
  person-state → ONE task or nothing (spec §0/§4/§6), enforce the `@waiting` set-time invariant (A4 S4), read
  the **lifecycle ledger** (`forzare/state/task-lifecycle.json`, spec §4d) for `roll_count`, and — when it
  *does* mutate a label (a groomed load-label, or the `@waiting` label) — do a full-set
  read-modify-write (verified v1.75.3: `td task update <id> --labels "<full,set>"` REPLACES the set;
  `--no-labels` clears; never write a partial set). **No `@rolled`/`@stalled` writes** — stall state is the
  ledger, off the task.

- [ ] **Step 2: Curator-pin**

```bash
set -o pipefail
hermes curator pin todoist-surface && hermes curator status 2>/dev/null | grep -i todoist-surface
```

- [ ] **Step 3: Staged dry-run against `[TEST]` tasks** (structured add — no NL date parse; `.results[]`;
  `--yes` delete)

```bash
set -o pipefail
# disposable fixture (structured add, unprefixed label)
TID=$(td task add "[TEST] deep surfacing probe" --labels "deep" --due today --json | jq -r '.id')
# exercise via the staged-cron pattern (Phase B intro): --deliver local, then read the audit artifact
JID=$(hermes cron create '0 0 1 1 *' 'Run todoist-surface once; surface at most ONE task or respond exactly [SILENT].' \
        --skill todoist-surface --deliver local --name test-surface | grep -oE 'id[: ]+[A-Za-z0-9]+' | awk '{print $NF}')
hermes cron run "$JID" && hermes cron tick
cat "$(ls -td ~/.hermes/cron/output/*/ | head -1)"* 2>/dev/null | tail -40
# assert label set preserved after any grooming write (no partial clobber)
td task list --json | jq -r --arg id "$TID" '.results[]|select(.id==$id)|.labels|@csv'
# clean up
hermes cron remove "$JID"
td task list --json | jq -r '.results[]|select(.content|startswith("[TEST]"))|.id' | xargs -I{} td task delete {} --yes
```

Expected: the audit artifact shows **at most one** task (or `[SILENT]`), grooming applied, and the fixture's
label array still contains `deep` (full-set write, no clobber). Nothing reached Discord (`--deliver local`).
**Acceptance:** dry-run green; `[TEST]` tasks deleted; the cron audit shows the single decision.

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
  here, never the user's primary (spec §5c). The subcommand is **`gog calendar calendars`** (alias `cal`;
  NOT `gog calendar list`, which does not exist — verified gog 0.32.0).

```bash
set -o pipefail
gog calendar calendars -j | jq -r '.calendars[]?.summary // .[]?.summary' 2>/dev/null | grep -i '🤖\|bob' \
  || echo "create the 🤖 calendar first"
```

- [ ] **Step 2: Author `calendar-read`** — read fixed anchors (user primary + the 🤖 calendar) for today's
  free-window computation (spec §2/§4c) via `gog calendar events` (alias `ls`). **Author `calendar-write`** —
  write ONLY to the 🤖 calendar; never edit/delete user events; blocks are movable proposals except the §5c
  user-confirmed carve-out.
- [ ] **Step 3: Curator-pin + read/write dry-run**

```bash
set -o pipefail
hermes curator pin calendar-read && hermes curator pin calendar-write
# `gog auth status` exits 0 even when the API isn't actually reachable — verify with a REAL call:
gog calendar calendars -j >/dev/null 2>&1 && echo "gog API reachable (real call OK)" \
  || echo "gog auth broken — surface the re-auth repair (spec §16): gog auth add <email>"
```

**Acceptance:** `calendar-read` returns today's anchors; a `calendar-write` dry-run creates a `[TEST]` block
on the 🤖 calendar only, then deletes it; auth-expired path surfaces the one-line repair, not silence.

---

### Task B4: `eisenhower-plan`, `activation-prompt`, `brief-assemble`

**Files:** three SKILL.md under `~/.hermes/skills/`.

- [ ] **Step 1: `eisenhower-plan`** — the agent-side Eisenhower narrowing (spec §4c/§5), **one skill with a
  mode by caller** (spec §13): pool → free windows → rank Q1 → Q2 against `goals.md` → capacity/window fit →
  cap at 3. **Morning caller ⇒ WRITES the ≤3 `p1`** (idempotent — read `Today's 3` first) **and** places the
  ONE protected deep block via `calendar-write` if a deep window exists ("ANCHOR, don't fill", spec §5a).
  **EOD caller ⇒ PROPOSAL only, writes NO `p1`** (tomorrow's ≤3 + anchor are staged, confirmed next morning).
  The caller (bundle) sets the mode — this removes the old contradiction where both morning and EOD set `p1`.
  **Planning inflows live here** (A31): deadline lead-time dating + goal-matched planning-pull (bounded/
  conservative until the backlog is combed, spec §4c/U12) run inside this plan step.
- [ ] **Step 2: `activation-prompt`** — the non-negotiable morning activation line ("Breakfast first, then
  gym") + the gym-window-end backstop line ("Back from the gym?"), skipped on Thu / post-overnight recovery /
  signal-already-fired (reads the `activation` field in `schedule-override.json`, spec §8a). Rotates phrasing
  by construction (spec §7).
- [ ] **Step 3: `brief-assemble`** — compose the ordered brief (weather → calendar → ≤3 → follow-ups →
  activation → one action), each step degrading visibly on failure (spec §11/§16). **Assembly only; delivery
  is cron-native, NOT a skill** (spec §11/§12). Owns (A31): the **`fs_path` re-entry resolution** on the
  surfaced task (spec §5e) and the **dopamine-menu draws** (spec §6) woven into the one-action line.
- [ ] **Step 4: Curator-pin all three + staged dry-run** (Phase B intro pattern)

```bash
set -o pipefail
for s in eisenhower-plan activation-prompt brief-assemble; do hermes curator pin "$s"; done
```

**Acceptance:** `eisenhower-plan` in morning mode never assigns >3 p1 and is a no-op if 3 already exist today,
in EOD mode writes zero p1; `brief-assemble` yields the ordered brief and drops optional blocks under low
receptivity but always includes the anchor.

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
- [ ] **Step 2: Curator-pin + classification staged dry-run** (staged cron, `--deliver local`; NOT
  `hermes -z … --safe-mode`)

```bash
set -o pipefail
hermes curator pin forzare
JID=$(hermes cron create '0 0 1 1 *' 'The user says: "picked up a shift". Classify + act; respond exactly [SILENT] when done.' \
        --skill forzare --deliver local --name test-forzare | grep -oE 'id[: ]+[A-Za-z0-9]+' | awk '{print $NF}')
hermes cron run "$JID" && hermes cron tick
cat "$(ls -td ~/.hermes/cron/output/*/ | head -1)"* 2>/dev/null | tail -20
jq . ~/workspaces/Ivy/forzare/state/schedule-override.json   # confirm the override was written
hermes cron remove "$JID"
rm -f ~/workspaces/Ivy/forzare/state/schedule-override.json  # clean the dry-run artifact
```

**Acceptance:** each of the 4 signal classes routes correctly on clear phrasing; a low-confidence phrase
triggers the one-line confirm (button on live session); the shift signal writes a valid
`schedule-override.json`; no `pre_gateway_dispatch` hook is used (spec §3B/§12).

---

### Task B7: `eod-roll`, `waiting-reconcile`, `transition` (the lifecycle + reconcile + hand-off owners)

**Files:** three SKILL.md under `~/.hermes/skills/`. These give the roll, the 02:00 reconcile, and the
exit-ramp/hand-off logic explicit owners (spec §8/§3a/§3b; U2/A10/A14/A31).

- [ ] **Step 1: `eod-roll`** — the atomic roll skill (spec §8): compute the roll set (date-only, today/overdue,
  `deadline == null`, `due.isRecurring == false`, no time-of-day, not future — the 4 exclusions), reschedule
  each to tomorrow, **unconditional `p1`-clear on every unfinished p1** (roll-excluded included), tick
  `roll_count` in the lifecycle ledger (§4d; reset on progress), enumerate missed FIXED items for the morning
  re-decision, and stamp `forzare/state/last-reconcile.json`. Idempotent + date-stamped (safe under the ±2h
  cron catch-up and the defensive morning re-run). Used by both `/forzare-eod` and (defensively) the morning
  brief.
- [ ] **Step 2: `waiting-reconcile`** — the 02:00 owner (spec §8): mark chase-due `@waiting`; repair the §4b
  set-time invariant (dateless `@waiting` → near-term check-back + "auto-repaired" flag); unblock detection vs
  `gog` calendar / `td activity` / recent Discord; 14-day staleness sweep. **State-only — never delivers**
  (the morning `followups-sweep` delivers what it marks). Run directly by the 02:00 cron, not in a bundle.
- [ ] **Step 3: `transition`** — the §3a hyperfocus exit-ramps (soft pre-warning → one-last-thing → hard-stop
  anchor → capture re-entry) + the §3b task-transition ritual (close the loop on the outgoing task's next
  action, pre-stage the next one). Owns the deadline lead-time framing at hand-off and the exit-ramp cues
  (A31). Invoked at block boundaries + on `/forzare` transitions.
- [ ] **Step 4: Curator-pin all three + staged dry-run** (Phase B intro pattern; assert `eod-roll` is
  idempotent — a second run in the same day advances `last-reconcile.json` zero further times)

```bash
set -o pipefail
for s in eod-roll waiting-reconcile transition; do hermes curator pin "$s"; done
```

**Acceptance:** `eod-roll` rolls only the 4-exclusion-filtered set, clears every unfinished p1, and is a
no-op on a second same-day run; `waiting-reconcile` marks state and sends nothing; `transition` produces the
graduated ramp, never a hard yank.

---

### Task B8: On-demand handles — `/forzare-next`, `/forzare-today`, `/forzare-capture` (skills)

**Files:** three SKILL.md under `~/.hermes/skills/` (spec §1a; U11/A11). These are the daily on-demand pulls;
they are **skills/bundles invoked by name or plain language**, not plugin commands (spec §12).

- [ ] **Step 1: `forzare-next`** — surface the next ONE thing within the current plan (spec §4; also plain
  "what now?"). Plain-language `description` for description-driven activation. **Authorization boundary:**
  read + surface only; no destructive writes beyond a defer/mark-done the user requested.
- [ ] **Step 2: `forzare-today`** — the `Today's 3` view + what's left (spec §4b). Read-only.
- [ ] **Step 3: `forzare-capture`** — brain-dump → structured `td task add` to Inbox (NO `quickadd`, spec
  §8b stage 1), non-interrupting; hands off to the §8b background pipeline. **Authorization boundary:** writes
  only to Inbox; never auto-creates a project.
- [ ] **Step 4: Curator-pin + end-to-end test** (Discord invocation → store mutation), staged per the Phase B
  pattern

```bash
set -o pipefail
for s in forzare-next forzare-today forzare-capture; do hermes curator pin "$s"; done
# capture end-to-end: a [TEST] capture must land in Inbox as a verbatim task (no date parsed out)
JID=$(hermes cron create '0 0 1 1 *' 'Capture: "[TEST] ring the plumber Tuesday". Route via forzare-capture.' \
        --skill forzare-capture --deliver local --name test-capture | grep -oE 'id[: ]+[A-Za-z0-9]+' | awk '{print $NF}')
hermes cron run "$JID" && hermes cron tick
td task list --json | jq -r '.results[]|select(.content|startswith("[TEST]"))|"\(.content)\tdue=\(.due)"'  # due must be null (no NL parse)
hermes cron remove "$JID"
td task list --json | jq -r '.results[]|select(.content|startswith("[TEST]"))|.id' | xargs -I{} td task delete {} --yes
```

**Acceptance:** each handle activates by name and plain language; `forzare-capture` lands the raw text in
Inbox with `due == null` (verbatim, no date parsed pre-classification, spec §8b/U4); authorization boundaries
hold.

---

### Task B9: Calibration logging + reducers (spec §6a — gives G1's "calibrate from output" a producer)

**Files:** a `calibration-log` skill (append writer) + reducer scripts under `~/.hermes/skills/` and the
owned `forzare/calibration/` store (U10/A12).

- [ ] **Step 1: Versioned event schema** — one record per surfacing decision **or deliberate withhold**
  (spec §6a): `{schema_version, ts, context{day_type, tod_bucket, mins_since_activation, calendar_load,
  gap_to_next, completions_today, stalls_today, mins_since_last_surfacing, surfacings_today}, action{task_id?,
  load_class?, duration_est?, due_proximity?, roll_count?} | "provide_nothing", outcome{initiated?, latency?,
  completed|partial|rolled_again|dismissed}}`. **No energy/mood self-ratings** (INV-6).
- [ ] **Step 2: Atomic append writer** (`calibration-log` skill) — appends one record per decision; append is
  atomic (temp-write + rename or `>>` with a single formatted line) so a crash never corrupts the log.
- [ ] **Step 3: Task-outcome correlator + daily/weekly reducers** — join a surfacing record to its later
  outcome (via `td activity`), then reduce to the §6a curves (time-of-day × load initiation, activation-decay,
  receptivity, aversiveness, duration-bias, habituation).
- [ ] **Step 4: Policy read path + retention** — the engine reads the reduced curves (not raw log); define a
  retention window (e.g. keep raw N weeks, keep reductions).
- [ ] **Step 5: Deterministic fixtures** — including **provide-nothing records** (the control condition) — so
  the reducers are testable without live data.

```bash
set -o pipefail
hermes curator pin calibration-log
# reducer runs deterministically over a fixture and emits the curve files:
ls ~/workspaces/Ivy/forzare/calibration/  # priors.md + reduced-curve outputs after a fixture run
```

**Acceptance:** an appended record round-trips through the reducer to a curve file; provide-nothing records
are counted (not dropped); the engine reads reductions, never the raw log.

---

## Phase C — Bundles + cron jobs

### Task C1: The three skill bundles

**Files:** `~/.hermes/skill-bundles/{forzare-morning-brief,forzare-replan,forzare-eod}.yaml`

- [ ] **Step 1: Write the bundles** (spec §13 exact compositions — the morning bundle gains `eisenhower-plan`
  + `calendar-write` + a defensive `eod-roll`; the eod bundle gains `eod-roll`; U2/A14):
  - `forzare-morning-brief` = `eod-roll` (defensive missed-fire roll) · `weather` · `calendar-read` ·
    `todoist-surface` · `eisenhower-plan` (writes ≤3 p1) · `followups-sweep` · `activation-prompt` ·
    `calendar-write` (places the one deep anchor) · `brief-assemble`
  - `forzare-replan` = `calendar-read` · `todoist-surface` · `eisenhower-plan`
  - `forzare-eod` = `eod-roll` (roll + p1-clear + ledger ticks + last-reconcile stamp) · `todoist-surface` ·
    `daily-reflect` · `eisenhower-plan` (proposal mode, no p1) · `tomorrow-prep` · `calendar-write`
  - (`waiting-reconcile` is NOT bundled — the 02:00 cron runs the skill directly, Task C2.)
- [ ] **Step 2: Boot-time skill-existence assertion (closes the silent-skip hole, spec §13).** Add a boot
  check (a small script Bob runs at gateway start, or a documented pre-start check) that asserts every skill
  named by the bundles is installed + pinned and **fails loud** (abort boot; once up,
  `hermes send --to discord:<#forzare-errors>`) if any is missing.
- [ ] **Step 3: Verify**

```bash
set -o pipefail
for b in forzare-morning-brief forzare-replan forzare-eod; do
  echo "== $b =="; yq '.skills[]' ~/.hermes/skill-bundles/$b.yaml
done
# every named skill must resolve to an installed SKILL.md dir:
comm -23 <(yq '.skills[]' ~/.hermes/skill-bundles/*.yaml | sort -u) \
         <(ls ~/.hermes/skills | sort -u)   # expect empty
```

Expected: each bundle lists the exact skills; the `comm` diff is empty (no bundle names a missing skill).
**Acceptance:** bundles resolve fully; the boot assertion fails loud on a deliberately-unpinned skill (test it
once, then re-pin).

---

### Task C2: Cron jobs (rituals) with cron-native Discord delivery

**Store:** `~/.hermes/cron/jobs.json` via `hermes cron` (outside the repo). Delivery is
`--deliver discord[:channel_id]` (verified accepted values: `origin, local, telegram, discord, signal,
platform:chat_id`) — NOT a plugin. Timezone is Denver (Task A2). **`jobs.json` is a live-data exception** —
see Task E2's backup/rollback for it.

- [ ] **Step 1: Declare the jobs** (spec §1/§8/§19 decided times):
  - **Morning brief — ONE daily job, `15 5 * * *` (fires EVERY day; Sunday is DECIDED, spec §1/§2/U15)**
    running `/forzare-morning-brief`, `--deliver discord` (home channel). The alternating-Sunday ON/OFF
    distinction is **content**, derived from the `work_schedule` read inside the bundle (anchor Jun 7 = ON) —
    **not** a separate job and **not** a build-time question. No Mon–Sat cron; no "confirm whether Sunday
    fires."
  - **End-of-day** `0 23 * * *` running `/forzare-eod`, idempotent + date-stamped (spec §8).
  - **`@waiting` reconcile** `0 2 * * *` running the **`waiting-reconcile`** skill (Task B7) — **state-only,
    NEVER messages the user** (spec §8); `--deliver local`, it only marks state for the morning
    `followups-sweep`.
  - **Gym-window-end check** at the configured gym-window end — the "Back from the gym?" backstop, skipped on
    Thu / recovery mornings / signal-already-fired (reads `schedule-override.json` `activation`, spec §3/§8a).
  - **Block-boundary prompts** at the schedule's block edges (spec §3/§5), `--deliver discord`.
  - **Monthly someday-sweep** (spec §4c/U12) proposing ≤5 stale someday items as keep/drop/promote at the
    brief (+ the opt-in task-bankruptcy offer past the decided threshold) — `--deliver discord`.
- [ ] **Step 2: Verify (staged — do NOT deliver to Discord yet).** Note the real trigger semantics:
  `hermes cron run <job_id>` **queues** the job for the next tick and takes **NO `--deliver`**; `hermes cron
  tick` then executes due jobs. To force a dry run, either set the job's own `--deliver local` at create time,
  or run+tick:

```bash
set -o pipefail
hermes cron list
BRIEF=$(hermes cron list --json 2>/dev/null | jq -r '.[]|select(.name=="forzare-morning-brief")|.id' 2>/dev/null \
        || hermes cron list | grep -i morning-brief | grep -oE '[A-Za-z0-9]{6,}' | head -1)
hermes cron run "$BRIEF" && hermes cron tick      # execute once; the job's --deliver governs the target
LATEST=$(ls -td ~/.hermes/cron/output/*/ | head -1); echo "audit: $LATEST"; ls "$LATEST"
```

Expected: all jobs listed at the right times/TZ; the forced run writes `~/.hermes/cron/output/<hash>/` and —
while the job is set to `--deliver local` during staging — does NOT message Discord. **Acceptance:** the job
families exist (one daily brief, EOD, 02:00 reconcile, gym-check, boundaries, monthly sweep); the 02:00
reconcile has no user-facing delivery; brief/EOD staged runs are audit-logged and silent.

---

## Phase D — Capture pipeline (Kanban, private)

### Task D1: Private Kanban board + capture pipeline stages

**Store:** `~/.hermes/kanban.db` (Bob-private, firewalled from the user, spec §9). Assignee = the **`default`
profile** (persona "Bob"; no profile named `bob`); `default_assignee: "default"`, `auto_decompose: false`,
`max_in_progress_per_profile: 2`, `failure_limit: 2` (Task A2 / spec §14). **Preflight:**
`hermes profile show default` must succeed.

- [ ] **Step 1: Author the pipeline** — parent Bob does **stage 1 synchronously** (structured `td task add` to
  Inbox — NOT `quickadd`, so no date is parsed pre-classification, spec §8b/U4; instant ack, idempotent).
  Stages 2–5 run as background **default-profile** subagent Kanban work (spec §8b). Kickoff (title is a
  **required** positional arg — verified): **`hermes kanban create "<title>" --triage --idempotency-key
  <capture-id> --assignee default`** (R7 — `--triage` parks in triage, `--idempotency-key` returns the
  existing card on re-fire); concretize raw captures with **`hermes kanban specify <task_id>`** (the
  `auxiliary.triage_specifier` slot, Task A2). The 5 stages: Place → Decide-placement (task-vs-event pre-check
  + 4 routing cases) → Verify+research-decision → Research → Split, each gating the next (spec §8b).
- [ ] **Step 2: Idempotent dup-guards (forced by no-mid-run-resume, spec §8a/§8b/§19).** Every stage is
  check-before-create: stage 1 skips if the capture is already in Inbox; stage 2 skips re-routing a placed
  task and skips a duplicate 🤖-calendar event; stage 5 skips existing subtasks. A restart converges to one
  task, never a dup.
- [ ] **Step 3: Never auto-create a project** (case 4 asks inline; spec §8b). Cases 3–4 route asks through
  the **parent** Discord-bound conversation (cron/subagent turns have no session for buttons, spec §12.1c).
- [ ] **Step 4: Failures are loud** — a stage error / un-completable card → `#forzare-errors` via
  `hermes send --to discord:<#forzare-errors>` (spec §8b/§16); the captured item is safe (stage 1 persisted
  it). (The forzare-ops watchdog also scans the kanban DB for `gave_up`/`blocked` cards, Task F1.)
- [ ] **Step 5: Verify — card lifecycle + idempotency (title required; `archive`, not `delete`)**

```bash
set -o pipefail
# a titled --triage card; the idempotency key must return the SAME id on re-fire:
ID1=$(hermes kanban create "[TEST] capture probe" --triage --idempotency-key test-cap-001 --assignee default --json | jq -r '.id // .task_id')
ID2=$(hermes kanban create "[TEST] capture probe" --triage --idempotency-key test-cap-001 --assignee default --json | jq -r '.id // .task_id')
test "$ID1" = "$ID2" && echo "idempotency dedupe OK (same id $ID1)"
hermes kanban specify "$ID1" >/dev/null 2>&1 && echo "specify promoted triage→todo OK"
hermes kanban show "$ID1" | grep -iE 'triage|todo'   # lifecycle visible
# clean up: kanban has NO `delete` verb — use `archive` (then optional --rm to purge):
hermes kanban archive "$ID1"
td task list --json | jq -r '.results[]|select(.content|startswith("[TEST]"))|.id' | xargs -r -I{} td task delete {} --yes
```

Expected: the second create returns the existing card id (no dup); `specify` moves it through the triage
lifecycle; cleanup via `archive`. **Acceptance:** idempotency key dedupes; a simulated stage crash restarts
from stage 1 and still yields one task; a forced stage error lands on `#forzare-errors`. (Inbox-write
correctness for a capture is covered in Task B8's end-to-end test.)

---

## Phase E — Delivery + channels

### Task E1: `session_reset` (root) + `[SILENT]` per-path verification + clarify-button ask patterns

**File:** `~/.hermes/config.yaml` (the root `session_reset` was set in Task A2) + skill ask-patterns.

- [ ] **Step 1: Confirm the ROOT `session_reset` policy resolves for Discord (R6a, spec §14).** The stanza is
  a **root** key (verified `gateway/config.py`: root `session_reset` → `default_reset_policy`, which applies to
  the Discord session) — **NOT** `platforms.discord.session_reset` (that path is not a real schema). Set in
  Task A2 to `mode: both, at_hour: 4, idle_minutes: 1440, notify: false` — bracketed between the 23:00 EOD and
  the 5:15 brief (spec §12.3/R4). **Verify by resolving the policy, not grepping:**

```bash
set -o pipefail
~/.hermes/hermes-agent/venv/bin/python - <<'PY'
from gateway.config import load_gateway_config, Platform
p = load_gateway_config().get_reset_policy(Platform.DISCORD)
print("mode=", p.mode, "at_hour=", p.at_hour, "idle_minutes=", p.idle_minutes, "notify=", p.notify)
assert p.mode == "both" and p.at_hour == 4 and p.notify is False, "session_reset not applied"
print("session_reset OK")
PY
```

- [ ] **Step 2: Verify the `[SILENT]` contract per delivery path (spec §12.2/R3) — on the REAL cron path, no
  `hermes -z --safe-mode`.** The **cron** path is lenient (whole/first-line/last-line/`[SILENT]`-prefix); the
  **gateway** path is exact-match, success-only. Stage cron jobs with `--deliver local` and confirm the audit
  artifact reflects suppression vs delivery:

```bash
set -o pipefail
run(){ J=$(hermes cron create '0 0 1 1 *' "$1" --deliver local --name "$2" | grep -oE 'id[: ]+[A-Za-z0-9]+' | awk '{print $NF}'); hermes cron run "$J" >/dev/null && hermes cron tick >/dev/null; echo "$J"; }
J1=$(run 'Respond with exactly: [SILENT]' silent-exact)         # cron: suppressed
J2=$(run 'Respond with: [SILENT] then a task line'   silent-prefix)  # cron: ALSO suppressed (prefix) — gateway would deliver
cat "$(ls -td ~/.hermes/cron/output/*/ | head -1)"* 2>/dev/null | tail -5
for J in "$J1" "$J2"; do hermes cron remove "$J"; done
```

  Cover the full matrix at build (spec §12.2): exact, prefix, first-line, last-line, a **failed** turn (never
  silenced on either path), and a substantive turn (delivered).

- [ ] **Step 3: Standardize the clarify-button ask pattern** across skills that ask on a live session (§4
  defer, §7 stall decision, §8b cases 3–4, §3B low-confidence): max 4 choices + auto "Other"; fall back to a
  plain one-line question on cron/subagent-origin turns (spec §12.1c). **Never use emoji reactions as input**
  (no inbound reaction events, spec §19/R8) — outbound 👀→✅/❌ ack-reactions are a free "Bob heard you" cue only.

**Acceptance:** the resolved Discord reset policy is `mode=both, at_hour=4, notify=False`; on the cron path an
exact-match AND a `[SILENT]`-prefixed turn both suppress while a failed turn never does; clarify buttons render
on a live session, plain questions on cron turns.

---

### Task E2: Deployment checkpoints (user-run `chezmoi apply`) + the `jobs.json` live-data exception (U9)

**Not new files — the deploy gates that make the chezmoi-managed artifacts actually take effect.** Every
managed artifact (config `.age`, `.env`, skills, bundles, watchdog) is authored in source, then applied by a
**user-run** `chezmoi apply` at a checkpoint. Subsequent phases are **blocked until the rendered source
matches live** (a hash check), so the build never proceeds on un-applied config.

- [ ] **Checkpoint 1 — config/.env (after Phase A):** the KeePassXC-gated `chezmoi apply` of
  `encrypted_private_config.yaml.age` + `private_dot_env.tmpl` is **USER-RUN** (agents use
  `--exclude=templates`). The plaintext `private_SOUL.md` (Task A5) is agent-runnable separately.
- [ ] **Checkpoint 2 — skills + bundles (after Phase B/C1):** apply the `dot_hermes/skills/*` +
  `skill-bundles/*` sources; then the boot skill-existence assertion (C1 S2) must pass.
- [ ] **Checkpoint 3 — watchdog (after Phase F):** apply the script + plist, then `launchctl load`.
- [ ] **Gate check (block subsequent phases on rendered-vs-live):**

```bash
set -o pipefail
cd "$(git rev-parse --show-toplevel)"
chezmoi --source "$PWD" diff ~/.hermes 2>/dev/null | grep -q . \
  && { echo "PENDING chezmoi changes — run the user-run apply before proceeding"; exit 1; } \
  || echo "source == live (checkpoint clear)"
```

- [ ] **`jobs.json` = the THIRD live-data exception (declared honestly).** The two documented live-data
  exceptions are the Todoist store and the owned layer (`~/workspaces/Ivy/forzare/`). Cron jobs are created
  via `hermes cron` and live in `~/.hermes/cron/jobs.json`, which is **NOT** chezmoi-managed — so it is a
  third live-data exception. Manage it with **backup + rollback**, not a template:

```bash
set -o pipefail
# BEFORE any cron change (Task C2):
cp ~/.hermes/cron/jobs.json \
  ~/workspaces/backups/"$(date -u +%Y-%m-%dT%H-%M-%S).hermes-cron-jobs.backup.json"
# ROLLBACK if a job set goes wrong:
#   cp <the timestamped backup> ~/.hermes/cron/jobs.json   (then restart the gateway to reload)
python3 -c 'import json,sys; json.load(open(sys.argv[1])); print("jobs.json valid")' ~/.hermes/cron/jobs.json
```

**Acceptance:** each checkpoint's `chezmoi diff` is empty before the next phase starts; a `jobs.json` backup
exists before Task C2 mutates it, and the documented rollback restores it.

---

## Phase F — Watchdog + ops (chezmoi-managed, in this repo)

### Task F1: `forzare-ops-watchdog` script + launchd plist + lint wiring + docs

**Files (in the dotfiles repo, via the delivery-vehicle rule — alongside the `dot_hermes/` templates of
Tasks A2/A3/A5):**

- Create: `dot_local/bin/executable_forzare-ops-watchdog.sh`
- Create: `Library/LaunchAgents/com.webdavis.forzare-ops-watchdog.plist.tmpl`
- Modify: `scripts/lint.sh` (add the loader/script templates to the finder if templated) + add a loader
  chezmoiscript if following the osquery/atuin loader pattern.
- Modify: `CLAUDE.md` (a "forzare ops watchdog" subsection).

- [ ] **Step 1: Write the watchdog script**, modeled on `dot_local/bin/executable_osquery-uptime-watchdog.sh`
  — **zero LLM**, out-of-band, doing TWO state-stamped scans per pass (spec §14/U3):
  - **(a) Gateway health.** Probe **`curl -fsS -m 3 http://127.0.0.1:8644/health`**, branch on exit code —
    **0 = up, 28 = hung, 7 = down** (spec §19). On down / hung / restart-looping → alert.
  - **(b) forzare run failures.** Since the last stamped watermark, scan `~/.hermes/cron/output/` for failed
    ritual runs and the kanban DB for `gave_up`/`blocked` cards, and route each to the errors channel. This is
    the concrete owner of "cron/pipeline failures reach the user."
  - **Alert:** **`hermes send --to discord:<#forzare-errors>`** (R2 — no LLM, no agent loop, no running
    gateway for bot-token platforms), plus the relay's phone/local push as belt-and-suspenders; if
    `DISCORD_ERRORS_CHANNEL` is unset, fall back to the home channel with a `⚠ ERROR` prefix (the fallback
    lives HERE, not in hermes). `set -euo pipefail`, double-quoted expansions, ISO-8601 timestamps
    (`date -u +"%Y-%m-%dT%H:%M:%SZ"`). **Do NOT curl the Discord webhook directly** (R2 dropped that
    phrasing). `:8644` caveat — exists only while the webhook platform is enabled; for a platform-independent
    probe, `API_SERVER_ENABLED=1` → `/health` on `:8642` (spec §19).
- [ ] **Step 2: Write the plist**, modeled on `com.webdavis.osquery-uptime-watchdog.plist.tmpl` — launchd
  `StartInterval` ~900s (15 min), `RunAtLoad` per the osquery model, `Label`
  `com.webdavis.forzare-ops-watchdog`, stdout/stderr to `~/.local/log/hermes/forzare-ops-watchdog.log`.
  (Note: the gateway's OWN plist `ai.hermes.gateway.plist` has `KeepAlive` = `true` — restarts on any exit;
  this watchdog covers the hang KeepAlive can't detect, spec §14/§19.)
- [ ] **Step 3: Wire lint** — add the plist loader template to `find_shell_templates` in `scripts/lint.sh`
  (the `.sh` helper is auto-shellchecked by `find_shell_files`; the `.plist.tmpl` is XML → `plutil -lint`).
- [ ] **Step 4: Document** in `CLAUDE.md` — the watchdog probes `:8644/health` **and** scans cron/output +
  the kanban DB, alerts out-of-band via `hermes send --to` (never through the dead gateway), and closes the
  KeepAlive hang-detection gap.
- [ ] **Step 5: Verify (plumbing only — no real alert)**

```bash
set -o pipefail
cd "$(git rev-parse --show-toplevel)"
shellcheck dot_local/bin/executable_forzare-ops-watchdog.sh
CI=1 chezmoi --source "$PWD" execute-template --no-tty < Library/LaunchAgents/com.webdavis.forzare-ops-watchdog.plist.tmpl | plutil -lint -
# health probe returns 0 while the gateway is up:
curl -fsS -m 3 http://127.0.0.1:8644/health >/dev/null && echo "gateway health OK (exit 0)"
```

Expected: shellcheck clean; `plutil -lint` → `OK`; the live probe exits 0. **Acceptance:** the script alerts
via `hermes send --to` on a simulated down/hung code (test with a bogus port) and on a seeded `gave_up`
kanban card, and stays silent when healthy. **This task's files are committed to the repo via the normal
pre-commit flow** (`just lint-check` + `just test`), separate from the two doc commits.

---

## Phase G — Dry-run → calibrate → go-live

### Task G1: Staged dry-run + explicit go-live matrix + flip to live (the final gate)

**Files:** none new (operational — the final gate). **Go-live is Step 4 of THIS task** (there is no separate
"G4" — earlier references corrected).

- [ ] **Step 1: Run the full brief + EOD staged (`--deliver local`) across the scenario matrix**, reading
  `~/.hermes/cron/output/` each run (spec §17). Confirm the brief fires `15 5 * * *`, surfaces ≤3 sensibly,
  weather/calendar/follow-up steps degrade visibly (never silently), the EOD roll + unconditional p1-clear +
  **lifecycle-ledger** ticks behave, and the 02:00 reconcile marks (never messages).
- [ ] **Step 2: Explicit go-live matrix (replaces "several days / sensibly").** Drive each scenario and assert
  expected state + message count (U15):

  | Scenario | Expected state | Expected messages |
  |---|---|---|
  | Work day (Tue/Thu/Sat 15:00–23:00) | deep window = morning; evening = work | 1 brief |
  | Off day (Mon/Wed/Fri) | deep window = morning + evening | 1 brief |
  | ON-Sunday (alt-anchor Jun 7=ON) | work-day brief | 1 brief |
  | Recovery morning (post-overnight) | recovery/sleep window, no deep push | 1 brief, no gym nag |
  | Duplicate fire (±2h catch-up + defensive roll) | roll happens exactly once (date-stamp) | 0 extra |
  | Dependency failure (gog/td down) | degrade-and-note inline; if unrecoverable → errors channel | 1 brief (degraded) + errors msg |
  | Concurrent trigger (live turn + cron) | at most one "next thing" each (§12.3 residual accepted) | ≤2 short, no wall |

```bash
set -o pipefail
jq . ~/workspaces/Ivy/forzare/state/last-reconcile.json      # advances exactly once/day
jq . ~/workspaces/Ivy/forzare/state/task-lifecycle.json      # roll_count ticks/reset as expected
```

- [ ] **Step 3: Calibrate** — tune the duration upward-bias factor + weather thresholds + brief content from
  the observed staged output (spec §4c/§6a; the Task B9 reducers are the producer). Priors stay auditable in
  `calibration/priors.md`.
- [ ] **Step 4: Flip to live** — change the brief/EOD/boundary cron jobs' delivery from `local` to
  `--deliver discord` (home channel). The errors channel stays the forzare-ops watchdog's
  `hermes send --to discord:<#forzare-errors>` + belt-and-suspenders relay. **This is the last step; do it
  only after Steps 1–3 are green.**
- [ ] **Step 5: Post-go-live smoke**

```bash
set -o pipefail
hermes cron list                                   # jobs live, Denver TZ
curl -fsS -m 3 http://127.0.0.1:8644/health && echo "gateway up"
launchctl print "gui/$(id -u)/com.webdavis.forzare-ops-watchdog" | grep -i state
```

Expected: jobs deliver to the home channel; watchdog loaded; health probe 0. **Acceptance:** a real brief
lands in the home channel; a forced failure (and a seeded `gave_up` kanban card) lands in `#forzare-errors`;
no gamification/achievements output anywhere.

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
- [ ] **Fantastical backup-notifier (post-V1 note only, A32).** V1 already covers the §3a hard-stop rung with
  **calendar-native alarms** on the 🤖 calendar; a Fantastical mirror as a second backup notifier is a
  *post-V1 nice-to-have*, not a V1 dependency. Recorded so it isn't lost.

**Non-Goals (explicitly NOT part of these two documents):**

- **The `homelab › ai-skills` STB#3 tasks** (Useful Question Builder / Vague Ask Auditor / Definition-of-Done
  Generator) are a **separate STATUS deliverable**, out of scope here (A32). WON'T-FIX in this plan — they are
  not forzare skills and do not gate the surfacing engine.

---

## Self-Review

**Spec coverage:** owned layer (goals.md w/ quadrants + per-goal confirm, priors.md w/ conservative
methylphenidate prior, state/ incl. the lifecycle ledger, calibration/) → A1; live-config drifts (empty TZ /
auto_decompose) + kanban/cron/session_reset stanza values + achievements keep-out guard → A2; `.env` channels
+ `#forzare-errors` → A3; Todoist 5 labels (unprefixed, **no new labels** — the ledger replaces
`@rolled`/`@stalled`) + 5 filters + auth + `@waiting` invariant → A4/B1; default-profile (persona "Bob")
persona/directives in `SOUL.md` (boss-of-the-schedule, §0 one-rule, no-shame, decide-in-context) → A5;
`todoist-surface` first → B1; weather / calendar / eisenhower(dual-mode) / activation / brief-assemble /
followups / reflect / tomorrow-prep / `/forzare` classifier → B2–B6; `eod-roll` / `waiting-reconcile` /
`transition` → B7; on-demand `/forzare-next` / `-today` / `-capture` handles → B8; calibration logging +
reducers → B9; bundles (morning gains eisenhower-plan + calendar-write + defensive eod-roll; eod gains
eod-roll) + boot assertion → C1; cron rituals w/ cron-native delivery (ONE daily brief `15 5 * * *`
schedule-derived, 23:00 EOD, 02:00 `waiting-reconcile`, gym-window-end, boundaries, monthly someday-sweep;
R1a) → C2; Kanban capture pipeline (assignee `default`, titled `--triage` + `specify` + idempotency keys + 5
stages + dup-guards, R7) → D1; root `session_reset` (R6a) + `[SILENT]` per-path (R3) + clarify buttons /
no-reaction-input (R8) → E1; deployment checkpoints + jobs.json exception → E2; `forzare-ops-watchdog` (health
probe + cron/output + kanban failure scan) + plist + `hermes send --to` alert (R2/U3) → F1; staged dry-run +
explicit go-live matrix → G1; post-V1 (Langfuse self-host, `tailscale serve` webhook, Hue, Todoist webhooks,
voice, ledger, email/comms triage, Fantastical) → H; ai-skills STB#3 out of scope → H Non-Goals. Delivery is
headless-native, no plugin, no `inject_message` (R1/R4/R5) throughout; every produced artifact ships via the
chezmoi source-dir pipeline (the delivery-vehicle rule). **Known open items carried forward** (not gaps): the
2053-task backlog relevance-comb is pending, so planning-pull stays conservative (C2/B4); `/forzare-*` native
slash autocomplete depends on the §12.5 mirroring check.

**Placeholder scan:** verification commands are runnable; `<capture-id>` / lat-long / channel-ids are the
intentionally per-environment values a cold reader fills from their own setup.

**Consistency:** channel names (`DISCORD_HOME_CHANNEL` / `DISCORD_ERRORS_CHANNEL` / `#forzare-errors`), the
health probe (`curl -fsS -m 3 http://127.0.0.1:8644/health`, 0/28/7), the alert primitive (`hermes send --to
discord:<channel>`), the per-path `[SILENT]` rule, the assignee (`default`), and the unprefixed label / filter
names match the spec and each other across A–H.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-11-bob-executive-assistant.md`. Execute
task-by-task with superpowers:subagent-driven-development or superpowers:executing-plans; **go-live is Task G1
Step 4** (the final step) — everything before it runs `--deliver local`.
