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
- **Suppression is DELIVERY-only — the dry-run INSTRUCTION is the read-only contract (spec §17/V4, R3A1/R3A4).**
  `[SILENT]` and `--deliver local` stop a *message*, NOT a skill's *store writes*: a staged brief still writes
  its date-mutations, sets `p1`, writes the ledger, and creates calendar blocks unless it also sees the dry-run
  directive. **Transport is the job/bundle INSTRUCTION, not an env var (R3A4):** a gateway-ticked job's agent
  turn does **not** inherit a shell `FORZARE_DRY_RUN=1` export (hermes strips undeclared env from children —
  verified `_sanitize_subprocess_env` for script subprocesses; the agent turn is never handed a caller-set
  var), so every staged ritual's **prompt OPENS with** `DRY RUN — record intended writes to
  forzare/state/dryrun-intents.jsonl, perform none`. Under dry-run, EVERY mutating skill **appends each
  intended write to `forzare/state/dryrun-intents.jsonl` (the ONE file dry-run may write) and performs none**
  (R3A1 — the intent record `{ts,skill,op,target,args,run_id}` is the observable evidence). **Staging
  acceptance = zero production mutations, asserted two ways:** (positive) the intent RECORDS exist in
  `dryrun-intents.jsonl` with the expected fields; (negative) `td activity --by me --since <today> --json`
  (camelCase `eventType`, §19) shows no forzare task changes in the window **and** owned-layer state-file
  mtimes (`last-reconcile.json` / `task-lifecycle.json`) are unchanged. The **23:00 eod-roll job is created
  disabled/`local` + the dry-run instruction until go-live**; **go-live (G1) REMOVES the dry-run directive,
  RESUMES/ENABLES the eod-roll job, and never `rm`s a real state file** (a correct dry-run wrote only
  `dryrun-intents.jsonl`, which go-live truncates) (Task C2/G1, W2).
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
- **Seeded at install (NOT lazy):** `last-reconcile.json` = **yesterday** (spec §8/R3A9/W5) — so the first
  real 23:00 EOD closes exactly one day and no cold run mistakes a fresh install for a multi-year outage.
- Runtime-written under `state/` (created lazily by the skills, not here): `schedule-override.json` (shift
  override + today's gym `activation` field, spec §8a), the **lifecycle ledger** `task-lifecycle.json`
  (`roll_count`/`written_due`/`last_escalated` per task id, spec §4d), `tomorrow-prestage.json` (EOD proposal
  → morning brief, spec §8a/R3A13), and — **staging only** — `dryrun-intents.jsonl` (the ONE file a dry-run
  writes, spec §17/R3A1; truncated at go-live, never present in a live run).

- [ ] **Step 1: Create the directory tree + seed `last-reconcile.json` = yesterday** (spec §8/R3A9/W5 — the
  bootstrap seed; BSD/macOS `date -v-1d`):

```bash
set -o pipefail
mkdir -p ~/workspaces/Ivy/forzare/calibration ~/workspaces/Ivy/forzare/state
STAMP=~/workspaces/Ivy/forzare/state/last-reconcile.json
if [ ! -f "$STAMP" ]; then
  Y=$(date -u -v-1d +%Y-%m-%d)   # yesterday, Denver-agnostic seed (a date-only anchor)
  printf '{"reconciled_date":"%s"}\n' "$Y" > "$STAMP"
  echo "seeded last-reconcile.json = $Y"
else
  echo "last-reconcile.json exists — leaving as-is (never re-seed a live stamp)"
fi
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
    Mehren 2019 observed the benefit at **~10 min post-exercise** and found **no effect at the ~33-min
    measure** (authors note its limited duration), so the old **~1–2 h** window is **REMOVED** (V10); seed a
    **conservative ≤~30 min boost prior, explicitly labeled *pending personal data*** (the calibration loop
    learns the real duration); the work-day window shape.
  - A header stating these are **starting priors, refined per-person by the calibration loop** (α = 0.15),
    and that Bob **never** gives medication advice or dosing nags — capacity input only.

- [ ] **Step 4: Verify**

```bash
set -o pipefail
G=~/workspaces/Ivy/forzare/goals.md; P=~/workspaces/Ivy/forzare/calibration/priors.md
ls -la ~/workspaces/Ivy/forzare/ ~/workspaces/Ivy/forzare/calibration ~/workspaces/Ivy/forzare/state
grep -qi 'methylphenidate' "$P" || { echo "FATAL: priors.md missing the stimulant prior" >&2; exit 1; }
grep -qi 'peak plasma' "$P"     || { echo "FATAL: priors.md missing the conservative-model note" >&2; exit 1; }
# goals.md must have exactly 5 goal blocks (## headings) AND a Quadrant line in each:
HEADS=$(grep -cE '^## ' "$G")
QUADS=$(grep -ciE '^\*\*Quadrant:\*\*' "$G")
test "$HEADS" -eq 5 || { echo "FATAL: expected 5 goal headings, found $HEADS" >&2; exit 1; }
test "$QUADS" -eq 5 || { echo "FATAL: expected 5 Quadrant lines, found $QUADS" >&2; exit 1; }
echo "goals.md: 5 blocks, 5 quadrants OK; priors.md OK"
```

Expected: both dirs exist; `priors.md` names the stimulant prior + conservative-model note; `goals.md` has
**exactly 5** `##` goal blocks **each with a `**Quadrant:**` line**. **Acceptance:** all five assertions pass
(any shortfall fails loud); `priors.md` has the ER-daily + IR-PRN prior + the no-advice guard.

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
C=~/.hermes/config.yaml
must(){ grep -Eq "$1" "$C" || { echo "FATAL: missing /$1/ in config.yaml" >&2; exit 1; }; echo "OK: $2"; }
must '^timezone: "America/Denver"'          'timezone'
must 'auto_decompose: false'                'auto_decompose'          # separate assert
must 'max_in_progress_per_profile: 2'       'kanban in-progress cap'  # separate assert
must 'default_assignee: "default"'          'default_assignee'        # separate assert
must 'max_parallel_jobs: 1'                 'cron parallel cap'
test "$(grep -c 'hermes-achievements' "$C")" -eq 0 || { echo "FATAL: hermes-achievements present" >&2; exit 1; }
echo "OK: achievements absent"
grep -q 'platforms/discord' "$C" || { echo "FATAL: platforms/discord stripped" >&2; exit 1; }; echo "OK: discord preserved"
python3 -c 'import yaml,sys; yaml.safe_load(open(sys.argv[1])); print("OK: yaml parses")' "$C"
```

Expected: each key asserted **separately** (fails loud on the specific missing one); achievements count 0;
`platforms/discord` still enabled; YAML parses.
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
"You are Bob…"). It is **plaintext with no secrets** (`grep -Eic 'token|api[._-]?key|secret|password' SOUL.md` = 0)
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
MANAGED=$(chezmoi --source "$PWD" cat ~/.hermes/SOUL.md)
printf '%s' "$MANAGED" | grep -Eqi 'one thing|no-shame|boss|decide-in-context' \
  && echo "forzare directives present in managed SOUL.md" \
  || { echo "FATAL: forzare directives missing from managed SOUL.md" >&2; exit 1; }
# It must remain secret-free (real alternation; assert exactly 0 matches):
SECRETS=$(printf '%s' "$MANAGED" | grep -Eic 'token|api[._-]?key|secret|password')
test "$SECRETS" -eq 0 || { echo "FATAL: $SECRETS secret-shaped strings in SOUL.md — must be 0" >&2; exit 1; }
echo "SOUL.md secret-free (0 matches) OK"
chezmoi --source "$PWD" diff ~/.hermes/SOUL.md | head -60   # review the pending persona change
```

Expected: the four contract points appear; the secrets grep is 0; the diff shows only the persona extension.
**Acceptance:** the directives are present; `chezmoi apply --exclude=templates ~/.hermes/SOUL.md` is the
(agent-runnable) deployment step — no secret file touched, no KeePassXC prompt.

---

### APPLY CHECKPOINT A (inline, fail-closed — R2A2) — do this BEFORE any Phase B work

The config/`.env`/`SOUL.md` sources authored in Phase A must be **applied to live** before Phase B builds
anything against them. Phase B's staged skill dry-runs read the live `config.yaml` (timezone, `skills.config`,
`session_reset`) and the live `.env` channels, so they are **explicitly gated on "Checkpoint A cleared."**

- **User-run:** the KeePassXC-gated `chezmoi apply` of `encrypted_private_config.yaml.age` +
  `private_dot_env.tmpl` (agents use `--exclude=templates`); the plaintext `private_SOUL.md` (A5) is
  agent-runnable separately.
- **Gate (fail-closed):** run the **E2 gate check** (Task E2) with this checkpoint's **explicit FILE list
  (W3 — `chezmoi diff` on a directory is NON-recursive, so never gate on a dir target):**
  `gate_check ~/.hermes/config.yaml ~/.hermes/.env ~/.hermes/SOUL.md`. It must print `checkpoint CLEARED` — a
  pending diff, a non-zero exit, or **any** stderr (undecryptable `.age` / locked template) is a **FAILURE
  that blocks Phase B**, never a silent pass.

---

## Phase B — Atomic skills (test-first; `todoist-surface` first)

> Every skill's applied target is `~/.hermes/skills/<name>/SKILL.md` (+ any helper scripts) — authored in
> the `dot_hermes/` source dir per the delivery-vehicle rule (Global Constraints) — is **curator-pinned**
> (`hermes curator pin <name>`, spec §13), and is driven test-first via a **staged cron dry-run**. `td` usage
> is learned from the installed `/todoist-cli` skill — do NOT duplicate `td` command knowledge into these
> skills (spec §10).
>
> **Every MUTATING skill honors the DRY-RUN instruction (V4/R3A1/R3A4 — applies to each writing skill below).**
> `todoist-surface`, `calendar-write`, `eisenhower-plan`, `forzare` (schedule-override), `eod-roll`,
> `waiting-reconcile`, `forzare-capture` — each SKILL.md must state and implement the read-only contract: when
> the dry-run directive is active, **append each intended write to `forzare/state/dryrun-intents.jsonl` (the
> ONE file dry-run may write) and perform NONE** (no `td` write, no ledger commit, no `calendar-write`, no
> real state-file write). Each record is `{ts, skill, op, target, args, run_id}` — the observable evidence a
> staged run is asserted against (never the real store). Read-only skills (`weather`, `calendar-read`,
> `forzare-today`, `brief-assemble`, `daily-reflect`) are unaffected. **Transport = the INSTRUCTION, not env
> (R3A4):** a gateway-ticked job never inherits a shell `FORZARE_DRY_RUN=1`, so every staged prompt below OPENS
> with `DRY RUN — record intended writes to forzare/state/dryrun-intents.jsonl, perform none`.
>
> **Shared phrasing-rotation directive (R3A17 — one owner, named by every recurring-prompt skill).** Every
> recurring user-facing prompt shape rotates its phrasing/format by construction (spec §7): a single
> deterministic design-time rotation over listed form/framing axes, never the same form twice running, with a
> FIXED semantic contract (same options, no-shame frame, one thing only). This ONE directive binds
> **`brief-assemble`** (the brief's fixed lines), the **surfacing skill's** nudges + **`transition`**'s stall
> re-engages, and the **completion beats** in `todoist-surface`/`daily-reflect`. Each of those skill tasks
> below **names this shared directive** rather than re-deriving a rotation; it is not per-skill improvisation.
>
> **Staged dry-run pattern (reused below; verified flags + verified job-id parse).** `hermes cron run <id>`
> only *queues* a job for the next tick and takes **no `--deliver`**; `hermes cron tick` runs due jobs once.
> So the dry-run recipe is: create a one-shot job with `--deliver local` (NOT Discord), force it, then read
> the audit artifact — **never** `hermes -z … --safe-mode` (safe-mode strips `skills.config`/plugins, and a
> `-z` one-shot does not traverse the delivery filter, so it can't prove `[SILENT]`). **Gateway-vs-manual
> tick race (R3A4):** the live gateway's own 60s tick may execute the queued job before the manual
> `hermes cron tick` does — that is fine, because every read below is **keyed by the job id**
> (`~/.hermes/cron/output/<job_id>/`), so either ticker produces the same per-job audit; never read
> newest-mtime.
>
> **Job-id extraction — the ONLY correct parse (verified `hermes_cli/cron.py:285`).** `hermes cron create`
> prints exactly `Created job: <job_id>`, where `job_id = uuid.uuid4().hex[:12]` (12 lowercase hex —
> `cron/jobs.py:854`); color is stripped when the output is piped (non-TTY). There is **no `hermes cron list
> --json`** (`cron_list` takes no `--json` — verified), so never parse job ids that way. The audit for a run
> lives at **`~/.hermes/cron/output/<job_id>/<timestamp>.md`** — keyed by the job id (`save_job_output`,
> `cron/jobs.py:1548`), so read it **by job id**, never by newest-mtime dir. Define these two helpers once
> and reuse them in every staged block below:
>
> ```bash
> set -o pipefail
> # The dry-run directive every mutating staged prompt OPENS with (R3A1/R3A4 — instruction transport):
> DRY='DRY RUN — record intended writes to forzare/state/dryrun-intents.jsonl, perform none. '
> # Parse "Created job: <12-hex>" from cron-create stdout; fail loud if absent.
> jid_from_create(){ local j; j=$(sed -n 's/.*Created job: *//p' | grep -oiE '[0-9a-f]{12}' | head -1); \
>   [ -n "$j" ] || { echo "FATAL: no 'Created job:' id parsed" >&2; return 1; }; printf '%s\n' "$j"; }
> # Stage one skill: create --deliver local, force-run, return the job id.
> stage_skill(){ # $1=schedule $2=prompt $3=skill $4=name
>   local jid; jid=$(hermes cron create "$1" "$2" --skill "$3" --deliver local --name "$4" | jid_from_create) \
>     || return 1
>   hermes cron run "$jid" >/dev/null && hermes cron tick >/dev/null || { echo "FATAL: run/tick failed" >&2; return 1; }
>   printf '%s\n' "$jid"; }
> ```
>
> Then, per skill:
>
> ```bash
> set -o pipefail
> JID=$(stage_skill '0 0 1 1 *' "${DRY}Run <skill> once; surface at most ONE task or respond exactly [SILENT]." <skill> test-<skill>)
> AUDIT=~/.hermes/cron/output/"$JID"
> ls "$AUDIT"/*.md >/dev/null 2>&1 || { echo "FATAL: no audit artifact at $AUDIT" >&2; exit 1; }
> cat "$AUDIT"/*.md | tail -40
> hermes cron remove "$JID"   # clean up the staging job
> ```
>
> `--deliver local` means **nothing reaches Discord**; the per-job cron audit is the evidence. **Store writes
> are NOT suppressed by `--deliver local`** — a mutating skill still writes unless it also sees the **dry-run
> instruction** carried in the prompt (spec §17/V4/R3A4; Global Constraints), which redirects each intended
> write to `forzare/state/dryrun-intents.jsonl`. (Adjust schedule/prompt per skill; always keep the `$DRY`
> prefix on a mutating skill.)

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
  ledger, off the task. **Any defer/snooze date-write goes through the centralized date-mutation layer (Task
  B7 Step 0)**, which picks the verb by task state (spec §4/W6: `td task reschedule` for an already-dated task;
  `td task update --due` only when initially dating an undated task, since `reschedule` errors `NO_DUE_DATE`
  there). **Completion beats name the shared phrasing-rotation directive** (R3A17, Phase B intro).

- [ ] **Step 2: Curator-pin**

```bash
set -o pipefail
hermes curator pin todoist-surface && hermes curator status 2>/dev/null | grep -i todoist-surface
```

- [ ] **Step 3: Staged dry-run against `[TEST]` tasks** (structured add — no NL date parse; `.results[]`;
  `--yes` delete)

```bash
set -o pipefail
# helpers jid_from_create / stage_skill / $DRY defined in the Phase B intro
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
# disposable fixture (structured add, unprefixed label) — the TEST script's own write, deleted below
TID=$(td task add "[TEST] deep surfacing probe" --labels "deep" --due today --json | jq -r '.id')
[ -n "$TID" ] && [ "$TID" != null ] || { echo "FATAL: fixture task not created" >&2; exit 1; }
: > "$INTENTS"
# exercise via the staged-cron pattern UNDER DRY-RUN (the groom-on-read touches the whole active pool —
# without the directive a staged run would groom REAL tasks); read the audit BY JOB ID (not newest-mtime dir)
JID=$(stage_skill '0 0 1 1 *' "${DRY}Run todoist-surface once; surface at most ONE task or respond exactly [SILENT]." \
        todoist-surface test-surface)
AUDIT=~/.hermes/cron/output/"$JID"
ls "$AUDIT"/*.md >/dev/null 2>&1 || { echo "FATAL: no audit artifact at $AUDIT" >&2; exit 1; }
cat "$AUDIT"/*.md | tail -40
# no-clobber assertion, retargeted to the INTENT record (R3A1): any journaled label-write intent for the
# fixture must carry the FULL set (deep preserved alongside whatever grooming adds) — a partial set is the
# clobber bug. No intent for the fixture is also fine (nothing needed grooming).
BAD=$(jq -r --arg id "$TID" \
  '[select(.op=="task.update-labels" and .target==$id) | select(.args.labels | index("deep") | not)] | length' \
  "$INTENTS" | tail -1)
[ "${BAD:-0}" = 0 ] || { echo "FATAL: a label-write intent for the fixture dropped 'deep' (partial-set clobber)" >&2; exit 1; }
# the fixture's REAL label set is untouched by a dry-run (purity) — read via --filter/--all (R3A5: default
# --limit is 300 of ~2270 tasks, a bare list can miss it):
LBLS=$(td task list --filter "search: [TEST]" --all --json | jq -r --arg id "$TID" '.results[]|select(.id==$id)|.labels|join(",")')
[ "$LBLS" = deep ] || { echo "FATAL: dry-run mutated the fixture's real labels ($LBLS)" >&2; exit 1; }
echo "label-set contract OK (full-set intents; real store untouched)"
# clean up
hermes cron remove "$JID"
td task list --filter "search: [TEST]" --all --json | jq -r '.results[]|select(.content|startswith("[TEST]"))|.id' \
  | xargs -r -I{} td task delete {} --yes
```

Expected: the audit artifact shows **at most one** task (or `[SILENT]`); any journaled label-write intent
carries the full set including `deep` (full-set write contract, no clobber — asserted, fails loud otherwise);
the fixture's real labels are untouched (dry-run purity). Nothing reached Discord (`--deliver local`).
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
  here, never the user's primary (spec §5c). List calendars with **`gog calendar calendars`** (verified gog
  0.32.0). **Two aliasing traps (R3A10):** (1) `cal` aliases the calendar **GROUP** (so `gog cal calendars`),
  NOT the `calendars` subcommand — `calendars` has no short alias; (2) **`gog calendar list` DOES exist, but
  it is the `events` alias** (`events (list,ls)`) — it lists **EVENTS, not calendars**, so never use `list`
  to enumerate calendars. Event reads use `gog calendar events` (alias `ls`); the singular `gog calendar
  event` (alias `get`/`show`) fetches one event.

```bash
set -o pipefail
gog calendar calendars -j | jq -r '.calendars[]?.summary // .[]?.summary' 2>/dev/null | grep -i '🤖\|bob' \
  || echo "create the 🤖 calendar first"
```

- [ ] **Step 2: Author `calendar-read`** — read fixed anchors (user primary + the 🤖 calendar) for today's
  free-window computation (spec §2/§4c) via `gog calendar events` (alias `ls`). **Author `calendar-write`** —
  write ONLY to the 🤖 calendar; never edit/delete user events; blocks are movable proposals except the §5c
  user-confirmed carve-out. **`calendar-write` also OWNS the §3a hard-stop leave-time alarm (W13):** when the
  morning plan sees a fixed **work block**, it creates a 🤖-calendar event with a **popup reminder at leave-time
  minus prep+travel** (back-computed from the block start), **idempotent by a stable event key** (a re-fired
  brief updates, never duplicates), created **at morning-plan time**. Under dry-run this alarm write is
  journaled to `dryrun-intents.jsonl`, not performed (R3A1).
- [ ] **Step 3: Curator-pin + read/write dry-run**

```bash
set -o pipefail
hermes curator pin calendar-read && hermes curator pin calendar-write
# `gog auth status` exits 0 even when the API isn't actually reachable — verify with a REAL call:
gog calendar calendars -j >/dev/null 2>&1 && echo "gog API reachable (real call OK)" \
  || echo "gog auth broken — surface the re-auth repair (spec §16): gog auth add <email>"
```

**Acceptance:** `calendar-read` returns today's anchors; a `calendar-write` dry-run creates a `[TEST]` block
on the 🤖 calendar only, then deletes it; auth-expired path surfaces the one-line repair, not silence. **The
leave-time alarm (W13) is tested create / update (leave-time moved) / cancel (block dropped) / recovery (a
re-run reuses the keyed event, no duplicate)** — the event key makes all four idempotent.

---

### Task B4: `eisenhower-plan`, `activation-prompt`, `brief-assemble`

**Files:** three SKILL.md under `~/.hermes/skills/`.

- [ ] **Step 1: `eisenhower-plan`** — the agent-side Eisenhower narrowing (spec §4c/§5), **one skill with
  THREE modes by caller** (spec §13/W10): pool → free windows → rank Q1 → Q2 against `goals.md` →
  capacity/window fit → cap at 3.
  - **`morning` ⇒ WRITES the ≤3 `p1`** (idempotent — read `Today's 3` first) **and** places the ONE protected
    deep block **plus the §3a leave-time alarm** (W13) via `calendar-write` if a deep window / work block
    exists ("ANCHOR, don't fill", spec §5a).
  - **`eod` ⇒ PROPOSAL only, writes NO `p1`, NO calendar** (tomorrow's ≤3 + anchor staged to
    `tomorrow-prestage.json`, confirmed next morning).
  - **`replan` ⇒ redraw the REMAINING day only** (spec §13/W10): may move Bob's 🤖-calendar proposals, may
    **PROPOSE** `p1` changes but **never silently apply** one (applying needs the user's explicit yes, INV-5),
    never touches fixed anchors; partial-day (re-plan only the hours left).
  The caller (bundle) sets the mode — this removes the old contradiction where both morning and EOD set `p1`.
  **Planning inflows live here** (A31): deadline lead-time dating + goal-matched planning-pull (bounded/
  conservative until the backlog is combed, spec §4c/U12) run inside this plan step, and **both INITIALLY date
  undated tasks — so they go through the centralized date-mutation layer (Task B7 Step 0), which uses
  `td task update --due` for an undated non-recurring task** (verified: `td task reschedule` errors
  `NO_DUE_DATE` on an undated task, spec §4/W6), never a scattered ad-hoc `td` call.
- [ ] **Step 2: `activation-prompt`** — the non-negotiable morning activation line ("Breakfast first, then
  gym") + the gym-window-end backstop line ("Back from the gym?"), skipped on Thu / post-overnight recovery /
  signal-already-fired (reads the `activation` field in `schedule-override.json`, spec §8a). Rotates phrasing
  by construction (spec §7).
- [ ] **Step 3: `brief-assemble`** — compose the ordered brief (weather → calendar → ≤3 → follow-ups →
  activation → one action), each step degrading visibly on failure (spec §11/§16). **Assembly only; delivery
  is cron-native, NOT a skill** (spec §11/§12). Owns (A31): the **`fs_path` re-entry resolution** on the
  surfaced task (spec §5e) and the **dopamine-menu draws** (spec §6) woven into the one-action line. **Names
  the shared phrasing-rotation directive (R3A17, Phase B intro)** for its fixed lines — it does not re-derive a
  rotation. The brief is the **one bounded exception** to the one-per-response rule (spec §0/W12): read-only
  context that still ends with exactly ONE action.
- [ ] **Step 4: Curator-pin all three + staged dry-run** (Phase B intro pattern)

```bash
set -o pipefail
for s in eisenhower-plan activation-prompt brief-assemble; do hermes curator pin "$s"; done
```

**Acceptance:** `eisenhower-plan` in morning mode never assigns >3 p1 and is a no-op if 3 already exist today;
in EOD mode writes zero p1; in replan mode redraws only the remaining day, proposes (never applies) p1 changes,
and never touches a fixed anchor (W10); `brief-assemble` yields the ordered brief and drops optional blocks
under low receptivity but always includes the anchor — and **the one-per-response check counts actionable
imperatives + questions (W12):** every non-brief response totals ≤ 1; the brief's context lines are
non-actionable and it closes on exactly one action.

---

### Task B5: `followups-sweep`, `daily-reflect`, `tomorrow-prep`

**Files:** three SKILL.md under `~/.hermes/skills/`.

- [ ] **Step 1: `followups-sweep`** — the §2-step-4 delivery consolidator: `@waiting` chases,
  fixed-item re-decisions (do late / reschedule / drop), triage re-raises. Reads state marked by the 02:00
  reconcile + EOD; the sweep is where deferred decisions are *delivered* (spec §2/§8).
- [ ] **Step 2: `daily-reflect`** — EOD report half: completions-as-wins (no scorecard, no misses list),
  receptivity-gated (spec §8). Gain-framed, task-level (spec §7). **Completion beats name the shared
  phrasing-rotation directive** (R3A17, Phase B intro).
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
# helpers (incl. $DRY) from the Phase B intro
hermes curator pin forzare
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
: > "$INTENTS"
JID=$(stage_skill '0 0 1 1 *' "${DRY}The user says: \"picked up a shift\". Classify + act; respond exactly [SILENT] when done." \
        forzare test-forzare)
AUDIT=~/.hermes/cron/output/"$JID"
ls "$AUDIT"/*.md >/dev/null 2>&1 || { echo "FATAL: no audit artifact at $AUDIT" >&2; exit 1; }
cat "$AUDIT"/*.md | tail -20
# R3A1: assert the INTENT RECORD, never the real store — a dry-run classifier journals the intended
# schedule-override write (op + target + a well-formed block/date payload) instead of performing it:
jq -e 'select(.op=="state-write" and (.target|test("schedule-override")) and .args.block and .args.date)' \
    "$INTENTS" >/dev/null \
  || { echo "FATAL: no well-formed schedule-override intent in dryrun-intents.jsonl (R3A1)" >&2; exit 1; }
# W2: the REAL schedule-override.json must NOT exist (dry-run wrote nothing) — and staged cleanup NEVER
# rm's the real file (a correct dry-run leaves nothing to clean; an rm here could destroy a live override):
[ ! -f ~/workspaces/Ivy/forzare/state/schedule-override.json ] \
  || { echo "FATAL: dry-run wrote the REAL schedule-override.json" >&2; exit 1; }
hermes cron remove "$JID"
```

**Acceptance:** each of the 4 signal classes routes correctly on clear phrasing; a low-confidence phrase
triggers the one-line confirm (button on live session); the shift signal journals a valid schedule-override
INTENT (block + date + recovery flag) to `dryrun-intents.jsonl` while the real file stays absent (R3A1/W2);
no `pre_gateway_dispatch` hook is used (spec §3B/§12).

---

### Task B7: `eod-roll`, `waiting-reconcile`, `transition` (the lifecycle + reconcile + hand-off owners)

**Files:** three SKILL.md under `~/.hermes/skills/`. These give the roll, the 02:00 reconcile, and the
exit-ramp/hand-off logic explicit owners (spec §8/§3a/§3b; U2/A10/A14/A31).

- [ ] **Step 0: the centralized date-mutation + ledger helper (V2/W6 — build this FIRST; ALL date-writers use
  it: capture dating, lead-time, planning-pull, snooze, roll, and the §7 escalation).** One helper module for
  `forzare/state/task-lifecycle.json` **and every agent date-write**, never a scattered ad-hoc `td` call or
  `json.load`/`dump`:
  - **Verb selection by task state (W6 — the load-bearing rule, verified v1.75.3):**
    - **Initially dating an UNDATED non-recurring task** → **`td task update --due <YYYY-MM-DD>`**. `td task
      reschedule` **refuses** an undated task (`Error: NO_DUE_DATE — … Use "td task update --due" to set
      one.`), so it cannot do the initial dating; on an undated non-recurring task there is no recurrence rule
      or time-of-day for `--due` to clobber.
    - **Re-dating an EXISTING date-only non-recurring task** (snooze / roll) → **`td task reschedule <ref>
      <YYYY-MM-DD>`** (preserves recurrence + existing time-of-day). **`td task update --due` is FORBIDDEN on
      an already-dated task** (it replaces the whole due, destroying a recurrence rule).
    - **Timed or recurring tasks are NEVER date-mutated** (`due.isRecurring == true` or a `"T"` in `due.date`).
  - **Ledger I/O guarantees:** **(a)** an exclusive `flock` on a sibling `task-lifecycle.lock` around every
    read-modify-write; **(b)** atomic writes — temp file in the same dir → `fsync` → `os.rename`; **(c)** a
    per-entry operation record `{old_due, new_due, reconcile_date}`; **(d)** the **journal-then-commit** write
    order — journal the intent (`pending`) → perform the state-chosen Todoist date-write → commit (flip
    `pending`, stamp new `written_due`); **(e)** the **healing rule** — on the next run, re-verify any
    `pending` entry against the live Todoist `due.date`: equals `new_due` ⇒ commit; equals `old_due` ⇒
    re-apply then commit; equals neither ⇒ user intervened, void the entry (§4d divergence). Under the dry-run
    instruction the helper **appends the intended date-write + ledger op to `forzare/state/dryrun-intents.jsonl`
    and performs neither** (R3A1). **Fixtures:** crash after (c)/after the date-write/after commit; a concurrent
    **EOD roll × live snooze** on one task id (the lock serializes; the loser re-reads and no-ops via the
    same-day dedupe); an **undated-task initial date** (asserts `td task update --due`, not `reschedule`) vs an
    **already-dated re-date** (asserts `td task reschedule`).
- [ ] **Step 1: `eod-roll`** — the atomic roll skill (spec §8): **compute the roll set BY THE LEDGER (V1)** —
  a task rolls iff it has a lifecycle-ledger entry AND `current due == written_due` AND it is date-only,
  today/overdue, not done; the field checks (`due.isRecurring == false`, no time-of-day, not future) are
  **secondary sanity guards within** that set, **not** the definition, and **`deadline != null` is NOT a
  blanket exclusion** — a Bob-written lead-time due on a deadline task is in the ledger and rolls (§4c/§8).
  The roll re-dates **already-dated** tasks, so it uses **`td task reschedule`** (preserves recurrence/time)
  through the Step-0 centralized helper (journal→date-write→commit); the same helper uses `td task update
  --due` only where it *initially* dates an undated task (spec §4/W6). **Unconditional `p1`-clear on every
  unfinished p1** (roll-excluded included). Tick `roll_count` (§4d; reset on progress). Enumerate missed FIXED
  items for the morning re-decision. **Key the run off an EXPLICIT reconciliation RANGE, not a single day
  (V3/R3A9/W5):** the days closed = `(last-reconcile.stored .. CEILING]`, where **CEILING is never ≥ today**
  (W5 eligibility ceiling — today is still in progress); roll destination = **CEILING + 1**; stamp advances to
  **CEILING**. A **multi-day outage drains the whole gap in ONE pass** and **ticks each carried task's
  `roll_count` EXACTLY ONCE** for the entire gap (an outage is Bob's failure, never multi-tick stall-shame,
  spec §0/§7). **Duplicate-fire no-op (W5):** if `stored ≥ CEILING` the roll set is empty — log an
  `already-reconciled` intent and advance nothing. So {on-time fire, ≤2h catch-up, past-grace recovery fire,
  defensive morning re-run} each reconcile a given day exactly once. Used by both `/forzare-eod` and
  (defensively) the morning brief. **Recovery test matrix (W5):** missing state (fresh seed = yesterday),
  same-day duplicate fire (no-op), concurrent manual+cron (lock serializes, second no-ops), outage < 2h
  (catch-up), > 2h (past-grace single fire), **≥ 3-day outage (one pass closes the whole gap, one tick per
  task)** — each yields the identical, once-only roll. **Dating fixtures (W6 — one per date-writer path):**
  a **user-dated** task (no ledger entry — never moves) · a **Bob lead-time** date on a deadline task (ledger
  entry — rolls) · a **capture-dated** task (§8b stage 2 — initial `update --due`, then rolls) · a
  **planning-pull** promotion (initial `update --due`, then rolls) · a **timed** task (`"T"` due — never
  mutated) · a **recurring** task (never mutated).
- [ ] **Step 2: `waiting-reconcile`** — the 02:00 owner (spec §8): mark chase-due `@waiting`; repair the §4b
  set-time invariant (dateless `@waiting` → near-term check-back + "auto-repaired" flag); unblock detection vs
  `gog` calendar / `td activity` / recent Discord; 14-day staleness sweep. **State-only — never delivers**
  (the morning `followups-sweep` delivers what it marks). Run directly by the 02:00 cron, not in a bundle.
- [ ] **Step 3: `transition`** — the §3a hyperfocus exit-ramps (soft pre-warning → one-last-thing → hard-stop
  anchor → capture re-entry) + the §3b task-transition ritual (close the loop on the outgoing task's next
  action, pre-stage the next one). Owns the deadline lead-time framing at hand-off and the exit-ramp cues
  (A31). Invoked at block boundaries + on `/forzare` transitions. **Its stall re-engages name the shared
  phrasing-rotation directive** (R3A17, Phase B intro). The §3a hard-stop rung is the 🤖-calendar leave-time
  alarm authored in `calendar-write` (W13, Task B3).
- [ ] **Step 4: Curator-pin all three + STAGING idempotency via two consecutive DRY-RUNS (R3A3).** The old
  test read the real `last-reconcile.json` stamp across two staged runs — but under the dry-run instruction a
  correct `eod-roll` **never advances the real stamp** (it writes only `dryrun-intents.jsonl`), so a
  stamp-diff proves nothing. Instead, the idempotency contract is **observable store-free from the intents
  log**: under dry-run, `eod-roll` treats `dryrun-intents.jsonl` as its shadow last-reconcile, so a **second**
  consecutive dry-run observes the first's *intended* stamp advance and logs an **`already-reconciled` no-op
  intent**. The REAL double-roll guard (the real stamp advances once, a real second run no-ops) is a **G1
  go-live day-1 supervised check** (Task G1), not a dry-run.

```bash
set -o pipefail
for s in eod-roll waiting-reconcile transition; do hermes curator pin "$s"; done
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
STAMP=~/workspaces/Ivy/forzare/state/last-reconcile.json
LEDGER=~/workspaces/Ivy/forzare/state/task-lifecycle.json
: > "$INTENTS"                                        # fresh intents log for the two-run probe
STAMP_MT0=$(stat -f %m "$STAMP" 2>/dev/null || echo none)
LEDG_MT0=$(stat -f %m "$LEDGER" 2>/dev/null || echo none)
DRY='DRY RUN — record intended writes to forzare/state/dryrun-intents.jsonl, perform none. '
# First dry-run: journals its intended reconcile/roll to the intents log (NOT the real store).
J1=$(stage_skill '0 0 1 1 *' "${DRY}Run eod-roll once." eod-roll test-eod-1); hermes cron remove "$J1"
# Second dry-run: must observe the first's intended stamp in the intents log and log an already-reconciled no-op.
J2=$(stage_skill '0 0 1 1 *' "${DRY}Run eod-roll once." eod-roll test-eod-2); hermes cron remove "$J2"
grep -q 'already-reconciled' "$INTENTS" \
  || { echo "FATAL: 2nd consecutive dry-run did not log an already-reconciled no-op intent" >&2; exit 1; }
echo "staging idempotency OK — 2nd dry-run no-ops via the intents log"
# Dry-run PURITY: neither the real stamp nor the ledger changed (a correct dry-run touches neither).
STAMP_MT1=$(stat -f %m "$STAMP" 2>/dev/null || echo none)
LEDG_MT1=$(stat -f %m "$LEDGER" 2>/dev/null || echo none)
[ "$STAMP_MT0" = "$STAMP_MT1" ] && [ "$LEDG_MT0" = "$LEDG_MT1" ] \
  || { echo "FATAL: dry-run mutated a real state file (stamp $STAMP_MT0->$STAMP_MT1, ledger $LEDG_MT0->$LEDG_MT1)" >&2; exit 1; }
echo "dry-run purity OK — real stamp + ledger untouched"
```

**Acceptance:** `eod-roll` rolls only the ledger-defined set (V1), clears every unfinished p1; the two-dry-run
probe logs an **`already-reconciled` no-op** on the second run and leaves the **real** `last-reconcile.json`
and `task-lifecycle.json` **untouched** (dry-run purity); the real double-roll guard is verified at G1 day-1;
`waiting-reconcile` marks state and sends nothing; `transition` produces the graduated ramp, never a hard yank.

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
- [ ] **Step 4: Curator-pin + end-to-end staged test** (dry-run intents, per the Phase B pattern — R3A1)

```bash
set -o pipefail
# helpers (incl. $DRY) from the Phase B intro. Pipeline state is CONTROLLED: forzare-capture's stage 1
# (td task add) is synchronous; stages 2–5 are the separate 60s Kanban dispatcher (D1), which a cron tick
# does NOT run — and under the dry-run directive stage 1 journals its Inbox write instead of performing it.
for s in forzare-next forzare-today forzare-capture; do hermes curator pin "$s"; done

# R3A1: staged capture runs are DRY-RUNS — assertions target the INTENT RECORD in dryrun-intents.jsonl,
# never the real store. R3A5: the due check distinguishes MISSING (no due key journaled — correct: stage 1
# never parses a date) from NULL (a due key explicitly journaled as null) from a VALUE (a parsed date — the
# bug). Any real-store read below uses `--filter "search: [TEST]"` + `--all` (default --limit is 300 and the
# store holds ~2270 tasks — a bare list can silently MISS the fixture).
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
# due state of a journaled task.add intent: MISSING | NULL | the parsed value (R3A5's three-way read):
intent_due(){ jq -r --arg c "$1" \
  'select(.op=="task.add" and .args.content==$c)
   | if (.args|has("due"))|not then "MISSING" elif .args.due==null then "NULL" else (.args.due|tostring) end' \
  "$INTENTS"; }

# (1) TIMELESS capture (no date word at all): the intent must exist and carry NO due key at all
: > "$INTENTS"
JID=$(stage_skill '0 0 1 1 *' "${DRY}Capture: \"[TEST] alphabetize the spice rack\". Route via forzare-capture." \
        forzare-capture test-capture-timeless)
D=$(intent_due "[TEST] alphabetize the spice rack")
[ -n "$D" ] || { echo "FATAL: no task.add intent journaled for the timeless capture (R3A1)" >&2; exit 1; }
[ "$D" = MISSING ] || { echo "FATAL: timeless capture journaled a due key ($D) — stage 1 must store verbatim" >&2; exit 1; }
echo "timeless capture intent OK (no due key)"; hermes cron remove "$JID"

# (2) DATE-WORD capture: the intent must ALSO carry no due key — proving stage 1 never NL-parses (no
#     quickadd). The stage-2 dated PLACEMENT (the pipeline actually choosing a date) is asserted in Task D1.
: > "$INTENTS"
JID=$(stage_skill '0 0 1 1 *' "${DRY}Capture: \"[TEST] ring the plumber Tuesday\". Route via forzare-capture." \
        forzare-capture test-capture-dateword)
D2=$(intent_due "[TEST] ring the plumber Tuesday")
[ -n "$D2" ] || { echo "FATAL: no task.add intent journaled for the date-word capture (R3A1)" >&2; exit 1; }
[ "$D2" = MISSING ] || { echo "FATAL: date-word capture journaled a due ($D2) — NL parse leaked into stage 1" >&2; exit 1; }
echo "date-word capture intent OK (no due key — no quickadd NL parse)"; hermes cron remove "$JID"

# DRY-RUN PURITY: neither capture may exist in the REAL store (--filter + --all so nothing hides past the
# default page); a hit means the dry-run leaked a real write:
LEAKED=$(td task list --filter "search: [TEST]" --all --json \
  | jq -r '[.results[]|select(.content|startswith("[TEST]"))]|length')
[ "$LEAKED" = 0 ] || { echo "FATAL: $LEAKED [TEST] task(s) in the real store — dry-run leaked (R3A1)" >&2; exit 1; }
echo "dry-run purity OK (no [TEST] task in the real store)"
```

**Acceptance:** each handle activates by name and plain language; `forzare-capture` journals **both** the
timeless and the date-word captures as verbatim `task.add` intents with **no due key** (no date parsed
pre-classification, spec §8b/U4; MISSING-vs-NULL-vs-value distinguished explicitly, R3A5), while the real
store stays clean (dry-run purity, R3A1); authorization boundaries hold. Stage-2 dated placement is asserted
in D1; the live Inbox write is observed at G1 day-1.

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
- [ ] **Step 3: Task-outcome correlator + daily/weekly reducers, with forzare-write EXCLUSION (W7)** — join a
  surfacing record to its later outcome (via `td activity`), then reduce to the §6a curves (time-of-day ×
  load initiation, activation-decay, receptivity, aversiveness, duration-bias, habituation). **Attribution
  rule (W7):** Bob writes to Todoist as the *same* account, so `initiatorId` cannot separate his writes from
  the user's — instead the correlator **excludes every activity event that matches a journaled forzare write**
  (the centralized date-mutation layer + ledger already record each p1-set / label groom / lead-time / roll /
  auto-repair comment — reuse that journal, add no new store). **Initiation counts only
  attribution-reliable signals:** a completion, a subtask completion, or an explicit user response — never a
  raw `updated`/`comment` event the journal authored.
- [ ] **Step 4: Policy read path + retention** — the engine reads the reduced curves (not raw log); define a
  retention window (e.g. keep raw N weeks, keep reductions).
- [ ] **Step 5: Deterministic fixtures** — including **provide-nothing records** (the control condition)
  **and the W7 NEGATIVE attribution fixture** — so the reducers are testable without live data.

```bash
set -o pipefail
hermes curator pin calibration-log
CAL=~/workspaces/Ivy/forzare/calibration
# Scripted round-trip: a surfacing record, a provide-nothing control, AND the W7 negative fixture — a
# surfaced task whose only post-surfacing activity is Bob-authored (journaled p1/label/due/comment writes):
cat > "$CAL/fixture-events.jsonl" <<'JSON'
{"schema_version":1,"ts":"2026-07-11T09:00:00Z","context":{"day_type":"off","tod_bucket":"morning"},"action":{"task_id":"X","load_class":"deep"},"outcome":{"initiated":true,"completed":true}}
{"schema_version":1,"ts":"2026-07-11T14:00:00Z","context":{"day_type":"off","tod_bucket":"afternoon"},"action":"provide_nothing","outcome":{}}
{"schema_version":1,"ts":"2026-07-11T10:00:00Z","context":{"day_type":"off","tod_bucket":"morning"},"action":{"task_id":"Y","load_class":"light"},"outcome":{},"activity_probe":{"task_id":"Y","events":[{"eventType":"updated","journaled_by_forzare":true},{"eventType":"comment","journaled_by_forzare":true}]}}
JSON
# invoke the reducer authored in this task (adjust to its real entrypoint):
python3 ~/.hermes/skills/calibration-log/reduce.py "$CAL/fixture-events.jsonl" --out "$CAL/curves.test.json"
test -s "$CAL/curves.test.json" || { echo "FATAL: reducer produced no curve file" >&2; exit 1; }
# the provide-nothing CONTROL must be counted, not dropped:
jq -e '.provide_nothing_count >= 1' "$CAL/curves.test.json" \
  || { echo "FATAL: provide-nothing records dropped by the reducer" >&2; exit 1; }
# W7 NEGATIVE fixture: task Y's only activity is Bob-authored ⇒ the reducer must score initiated=false:
jq -e '.tasks.Y.initiated == false' "$CAL/curves.test.json" \
  || { echo "FATAL: Bob-authored activity was scored as user initiation (W7)" >&2; exit 1; }
echo "calibration reducer round-trip OK (curve emitted; provide-nothing counted; Bob-writes excluded)"
rm -f "$CAL/fixture-events.jsonl" "$CAL/curves.test.json"
```

**Acceptance:** the scripted fixture round-trips through the reducer to a non-empty curve file; the
provide-nothing control is counted (`provide_nothing_count >= 1`, not dropped); the **W7 negative fixture
scores `initiated=false`** (only Bob-authored p1/label/due/comment events ⇒ no initiation credit); the engine
reads reductions, never the raw log.

---

### Task B10: `work_schedule` + schedule config (`skills.config` defaults) — BLOCKS Phase C (V11/R2A10)

**This was ruled in round 1 (A13) and never landed — it is the owning task now.** The morning brief, peak/free
windows, gym backstop, and weather all read schedule/threshold config (spec §2/§6a/§13). The values are
authored as **`metadata.hermes.config`** in the owning SKILL.md (spec §13: `metadata.hermes.config` → resolved
under **`skills.config`** in `config.yaml`), shipped via the `dot_hermes/` chezmoi source, and **verified by a
rendered-live read**. **Phase C's cron-creation task (C2) is BLOCKED until this task's rendered-value verify
passes** — a schedule-derived brief with no `work_schedule` mis-fires every morning.

**File:** the owning skill's `SKILL.md` under `dot_hermes/skills/…/` (the schedule readers — `eisenhower-plan`
/ `activation-prompt` / `weather`), whose `metadata.hermes.config` block declares every parameter with a
default.

- [ ] **Step 1: Author the config schema + defaults** (key / description / default / prompt per entry, spec
  §13):
  - **`work_schedule`** — per-weekday work blocks (currently **Tue/Thu/Sat 15:00–23:00**) **+ the
    alternating-Sunday rule with anchor date `2026-06-07 = ON`** (so May 31 OFF, Jun 14 OFF, Jun 21 ON, …).
  - **`gym_schedule`** — days = **Mon/Tue/Wed/Fri/Sat/Sun**, rest = **Thu**; the gym window (independent of
    `work_schedule`).
  - **`wake_anchor`** = **05:15**.
  - **weather thresholds** = wind > 17 mph / any rain / < 50°F / > 90°F.
  - (Peak/free windows are **derived** at run time from these, spec §2/§6a — NOT stored.)
- [ ] **Step 2: Ship via chezmoi** — the SKILL.md is authored in `dot_hermes/skills/…`; the resolved
  `skills.config` lands in the chezmoi-managed `config.yaml` (`.age`) per the delivery-vehicle rule.
- [ ] **Step 3: Verify by a RENDERED-LIVE read** (not a source grep — assert the resolved value each
  parameter actually takes):

```bash
set -o pipefail
~/.hermes/hermes-agent/venv/bin/python - <<'PY'
import os, yaml
cfg = yaml.safe_load(open(os.path.expanduser("~/.hermes/config.yaml")))
sc = (cfg.get("skills", {}) or {}).get("config", {}) or {}
def need(k):
    assert k in sc and sc[k] not in (None, "", {}), f"skills.config missing/empty: {k}"
    print("OK:", k, "=", sc[k])
for k in ("work_schedule", "gym_schedule", "wake_anchor", "weather_thresholds"):
    need(k)
ws = sc["work_schedule"]
assert "anchor" in str(ws) or "2026-06-07" in str(ws), "work_schedule missing the alt-Sunday anchor"
print("rendered work_schedule anchor present OK")
# R3A14: assert the CLAIMED weather thresholds, value by value (wind > 17 mph / any rain / < 50°F / > 90°F):
wt = sc["weather_thresholds"]
assert wt.get("wind_mph") == 17,   f"wind threshold {wt.get('wind_mph')} != 17"
assert wt.get("rain") in (True, "any"), f"rain trigger {wt.get('rain')} not 'any'"
assert wt.get("temp_low_f") == 50, f"low-temp threshold {wt.get('temp_low_f')} != 50"
assert wt.get("temp_high_f") == 90, f"high-temp threshold {wt.get('temp_high_f')} != 90"
print("weather thresholds OK: wind>17 / any rain / <50F / >90F")
PY
```

**Acceptance:** `skills.config` carries `work_schedule` (with the alt-Sunday anchor `2026-06-07=ON`),
`gym_schedule` (rest=Thu), `wake_anchor=05:15`, and **the four weather-threshold values asserted individually
(wind>17 / any rain / <50°F / >90°F — R3A14)** — each from the **resolved live config**, not the source
template. Only then may Task C2 create the schedule-derived cron jobs.

---

## Phase C — Bundles + cron jobs

### Task C1: The three skill bundles

**Files:** `~/.hermes/skill-bundles/{forzare-morning-brief,forzare-replan,forzare-eod}.yaml`

- [ ] **Step 1: Write the bundles** (spec §13 exact compositions — the morning bundle gains `eisenhower-plan`
  + `calendar-write` + a defensive `eod-roll`; the eod bundle gains `eod-roll`; U2/A14). **Each bundle carries
  a MANDATORY `instruction:` block (V7)** — the loader does NOT sequence
  (`agent/skill_bundles.py:286-340` loads all skills at once), so the `instruction` is the only thing that
  orders steps and states mutation boundaries. Each must set: **mode** (`morning`|`eod`), the **ordered step
  list**, the **mutation boundaries** (EOD = "writes NO `p1`, NO calendar"; morning = "run the defensive
  `eod-roll` + roll-set check BEFORE any `p1` write"), and **failure handling** (degrade visibly, never
  silent).
  - `forzare-morning-brief` = `eod-roll` (defensive missed-fire roll) · `weather` · `calendar-read` ·
    `todoist-surface` · `eisenhower-plan` (writes ≤3 p1) · `followups-sweep` · `activation-prompt` ·
    `calendar-write` (places the one deep anchor) · `brief-assemble`
  - `forzare-replan` = `calendar-read` · `todoist-surface` · `eisenhower-plan`
  - `forzare-eod` = `eod-roll` (roll + p1-clear + ledger ticks + last-reconcile stamp) · `todoist-surface` ·
    `daily-reflect` · `eisenhower-plan` (proposal mode, no p1) · `tomorrow-prep`. **NO `calendar-write`
    (R2A8)** — EOD writes no calendar; `tomorrow-prep` only records the candidate anchor to
    `forzare/state/tomorrow-prestage.json` (spec §8a); the morning run is the sole calendar writer.
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
  # V7: each bundle MUST carry a non-empty instruction block (the sequencer)
  INSTR=$(yq -r '.instruction // ""' ~/.hermes/skill-bundles/$b.yaml)
  [ -n "$INSTR" ] || { echo "FATAL: $b has no instruction: block (V7 — nothing sequences the steps)" >&2; exit 1; }
done
# the eod bundle must NOT list calendar-write (R2A8). Capture-then-match (R3A15): never pipe yq into
# `grep -q`, which exits at first match and can SIGPIPE the producer under pipefail.
EOD_SKILLS=$(yq '.skills[]' ~/.hermes/skill-bundles/forzare-eod.yaml)
if printf '%s\n' "$EOD_SKILLS" | grep -x calendar-write >/dev/null; then
  echo "FATAL: forzare-eod must not include calendar-write (R2A8)" >&2; exit 1
fi
echo "eod bundle calendar-write-free OK"
# every named skill must resolve to an installed SKILL.md dir — REAL assertion, not an eyeball (R3A15):
MISSING=$(comm -23 <(yq '.skills[]' ~/.hermes/skill-bundles/*.yaml | sort -u) \
                   <(ls ~/.hermes/skills | sort -u))
[ -z "$MISSING" ] || { echo "FATAL: bundle(s) name skills with no installed SKILL.md dir:" >&2; \
                       printf '%s\n' "$MISSING" >&2; exit 1; }
echo "all bundle-named skills resolve to installed dirs"
```

Expected: each bundle lists the exact skills, carries a non-empty `instruction:`, and the eod bundle has no
`calendar-write`; the `comm` check **asserts** emptiness and fails loud with the missing names otherwise.
**Acceptance:** bundles resolve fully; the boot assertion fails loud on a deliberately-unpinned skill (test it
once, then re-pin).

---

### APPLY CHECKPOINT C (inline, fail-closed — R2A2/W3) — placed BEFORE Task C2, whose staged runs depend on it

The `dot_hermes/skills/*` + `skill-bundles/*` sources (Phases B/B10/C1) must be **applied to live** before C2
force-runs the brief bundle and before the capture pipeline (Phase D) exercises the skills. **All of Task C2
and Phase D are gated on "Checkpoint C cleared"** — this checkpoint sits *before* its dependent staged
command (W3), not after it.

- **User-run/agent-run:** apply the skills + bundles sources; then the **boot skill-existence assertion (C1
  Step 2)** must pass (every bundle-named skill installed + pinned, else fail loud).
- **Gate (fail-closed):** run the **E2 gate check** (Task E2) with this checkpoint's **explicit FILE list
  (W3 — `chezmoi diff` on a directory is NON-recursive):** every `~/.hermes/skills/<name>/SKILL.md` (all
  Phase-B skills incl. `forzare-capture-pipeline`) + the three `~/.hermes/skill-bundles/forzare-*.yaml` + the
  B10 schedule-config owner's SKILL.md. It must print `checkpoint CLEARED`; any pending diff / non-zero exit /
  stderr blocks C2 and Phase D.

---

### Task C2: Cron jobs (rituals) with cron-native Discord delivery

**Store:** `~/.hermes/cron/jobs.json` via `hermes cron` (outside the repo). Delivery is
`--deliver discord[:channel_id]` (verified accepted values: `origin, local, telegram, discord, signal,
platform:chat_id`) — NOT a plugin. Timezone is Denver (Task A2). **`jobs.json` is a live-data exception** —
see Task E2's backup/rollback for it.

- [ ] **Step 1: Declare the jobs — every job ATTACHES its bundle/skill via `--skill` (W1, load-bearing).**
  **Verified (`cron/scheduler.py:1690-1889`, `_build_job_prompt`):** the cron path expands **`job.skills`**
  (the `--skill` list — a bundle slug expands its member skills' full content), and the free-text `prompt` is
  appended only as inert *"instruction alongside the skill invocation"* text. **A slash-command in the prompt
  (`'/forzare-morning-brief'`) is NEVER executed on the cron path** — such a job would run with no skill
  loaded. So every ritual is created `--skill <bundle-or-skill>`, with the prompt carrying only the staging
  **DRY-RUN directive** (R3A4) + any run-specific instruction. **EVERY user-facing job is created
  `--deliver local` for the whole build (V5/R2A4)** — Task G1 Step 4 is the *only* place delivery flips to
  `discord` (and removes the dry-run directive, W2).

```bash
set -o pipefail
# jid_from_create from the Phase B intro. The DRY-RUN directive rides in the PROMPT (instruction transport,
# R3A4 — a gateway-ticked job inherits no shell env); G1 recreates/edits these jobs WITHOUT it.
DRY='DRY RUN — record intended writes to forzare/state/dryrun-intents.jsonl, perform none.'
BRIEF=$(hermes cron create '15 5 * * *'  "$DRY" --skill forzare-morning-brief --deliver local --name forzare-morning-brief | jid_from_create)
EOD=$(hermes cron create   '0 23 * * *'  "$DRY" --skill forzare-eod           --deliver local --name forzare-eod           | jid_from_create)
RECON=$(hermes cron create '0 2 * * *'   "$DRY" --skill waiting-reconcile     --deliver local --name forzare-waiting-reconcile | jid_from_create)
GYM=$(hermes cron create   '0 9 * * *'   "$DRY Fire the gym-window-end backstop." --skill activation-prompt --deliver local --name forzare-gym-window-end | jid_from_create)   # time = configured gym-window end
BOUND=$(hermes cron create '0 14 * * *'  "$DRY Fire the block-boundary prompt." --skill transition --deliver local --name forzare-block-boundary | jid_from_create)            # one per schedule block edge
SWEEP=$(hermes cron create '0 5 1 * *'   "$DRY Run the monthly someday-sweep (state-only)." --skill followups-sweep --deliver local --name forzare-someday-sweep | jid_from_create)
echo "created: brief=$BRIEF eod=$EOD recon=$RECON gym=$GYM boundary=$BOUND sweep=$SWEEP"
```

  - **Morning brief — ONE daily job, `15 5 * * *` (fires EVERY day; Sunday is DECIDED, spec §1/§2/U15)**,
    **`--skill forzare-morning-brief`** (the bundle — W1), `--deliver local` (flips to `discord` home channel
    at G1). The alternating-Sunday ON/OFF distinction is **content**, derived from the `work_schedule` read
    inside the bundle (anchor Jun 7 = ON) — **not** a separate job and **not** a build-time question.
  - **End-of-day** `0 23 * * *`, **`--skill forzare-eod`** (the bundle — W1), idempotent + **keyed off the
    EXPLICIT reconciliation range** (`(stored .. CEILING]`, CEILING never ≥ today, spec §8/V3/R3A9/W5), not
    the wall clock. **Created `--deliver local` + the dry-run directive (or paused via `hermes cron pause`)
    until go-live** — it must not reschedule real tasks during staging; **G1 removes the directive and
    resumes it (W2)**.
  - **`@waiting` reconcile** `0 2 * * *`, **`--skill waiting-reconcile`** (the atomic skill directly — not a
    bundle, Task B7) — **state-only, NEVER messages the user** (spec §8); `--deliver local` **permanently**.
  - **Gym-window-end check** at the configured gym-window end, **`--skill activation-prompt`** — the "Back
    from the gym?" backstop, skipped on Thu / recovery mornings / signal-already-fired (reads
    `schedule-override.json` `activation`, spec §3/§8a); `--deliver local` until G1.
  - **Block-boundary prompts** at the schedule's block edges, **`--skill transition`** (spec §3/§5),
    `--deliver local` until G1.
  - **Monthly someday-sweep — DELIVERED VIA THE MORNING BRIEF, not a separate proactive job (R2A20).** A
    monthly cron (`0 5 1 * *`, brief-time on the 1st), **`--skill followups-sweep`**, runs the sweep
    **state-only** (`--deliver local` permanently), marking ≤5 oldest/most-stale someday candidates in a state
    file that the brief's `followups-sweep` consumes that morning — so there is **no second message**. Past
    the **DECIDED threshold of > 25** stale-someday candidates (R2A16, spec §4c/§19) it also marks the opt-in
    task-bankruptcy offer for the same brief slot.
- [ ] **Step 2: Verify (staged — do NOT deliver to Discord yet).** Note the real trigger semantics:
  `hermes cron run <job_id>` **queues** the job for the next tick and takes **NO `--deliver`**; `hermes cron
  tick` then executes due jobs. Three assertions per job: **(a) the persisted job carries the expected
  `skills` value (W1)**, **(b) the staged trace shows the attached skills' activity (W1)**, **(c) the staged
  window performed ZERO real mutations (R3A2)** — asserted with the verified `td activity` shapes (camelCase
  `eventType`/`objectId`; the event `id` is float-mangled scientific notation, so NEVER compare on `.id` —
  key on `objectId`+`eventDate`), time-window bounded, cross-checked against the dryrun-intents log, with a
  fail-LOUD nonzero-exit negative branch:

```bash
set -o pipefail
# There is NO `hermes cron list --json` (verified: cron_list takes no --json). Job ids were captured at
# create time (Step 1). (a) W1: the PERSISTED job must carry the skills list — read jobs.json by job id:
for pair in "$BRIEF:forzare-morning-brief" "$EOD:forzare-eod" "$RECON:waiting-reconcile"; do
  jid=${pair%%:*}; want=${pair#*:}
  got=$(jq -r --arg id "$jid" '.jobs[]|select(.id==$id)|.skills|if type=="array" then join(",") else tostring end' ~/.hermes/cron/jobs.json)
  [ "$got" = "$want" ] || { echo "FATAL: job $jid persisted skills=$got, want $want (W1)" >&2; exit 1; }
done
echo "persisted --skill values OK (W1)"
# Force one staged brief run and read the audit BY its job id:
WINDOW_START=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
hermes cron run "$BRIEF" >/dev/null && hermes cron tick >/dev/null
AUDIT=~/.hermes/cron/output/"$BRIEF"
ls "$AUDIT"/*.md >/dev/null 2>&1 || { echo "FATAL: no brief audit at $AUDIT" >&2; exit 1; }
# (b) W1: the trace must show the bundle's member skills actually engaged (each name appears in the audit):
TRACE=$(cat "$AUDIT"/*.md)
for s in eod-roll weather calendar-read todoist-surface eisenhower-plan followups-sweep activation-prompt brief-assemble; do
  printf '%s' "$TRACE" | grep -q "$s" || { echo "FATAL: staged trace shows no activity for bundle skill '$s' (W1)" >&2; exit 1; }
done
echo "staged trace shows every bundle skill (W1)"
# (c) R3A2: ZERO real mutations in the staged window — verified shapes: camelCase eventType/objectId; the
# event id is float-mangled ("2.15…e+36") so never key on .id. Window-bound with --since (today) and filter
# on eventDate >= WINDOW_START. stderr is captured, not discarded; ANY mutation event or read error is FATAL.
ACT_ERR=$(mktemp)
MUTS=$(td activity --since "$(date -u +%Y-%m-%d)" --type task --json 2>"$ACT_ERR" \
  | jq -r --arg w "$WINDOW_START" \
      '[.results[] | select(.eventDate >= $w) | select(.eventType=="updated" or .eventType=="added" or .eventType=="deleted" or .eventType=="completed")] | length')
if [ -s "$ACT_ERR" ]; then echo "FATAL: td activity errored:" >&2; cat "$ACT_ERR" >&2; rm -f "$ACT_ERR"; exit 1; fi
rm -f "$ACT_ERR"
[ "$MUTS" = 0 ] || { echo "FATAL: $MUTS Todoist mutation event(s) during the staged window — dry-run leaked (R3A2)" >&2; exit 1; }
# cross-check: the run DID compute work — its intended writes are in the intents log (R3A1):
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
[ -s "$INTENTS" ] || { echo "FATAL: staged run logged no intents — dry-run directive not honored (R3A1)" >&2; exit 1; }
echo "staged window: 0 real mutations, $(wc -l < "$INTENTS" | tr -d ' ') intent record(s) (R3A2 gate green)"
```

Expected: all jobs listed at the right times/TZ; every user-facing job is `--deliver local` + `--skill`
during the build; the forced run writes `~/.hermes/cron/output/<job_id>/` and does NOT message Discord.
**Acceptance:** the job families exist (one daily brief, EOD, 02:00 reconcile, gym-check, boundaries, monthly
sweep-via-brief), each with its persisted `skills` value asserted (W1); the staged brief trace shows every
bundle skill's activity (W1); the 02:00 reconcile and monthly sweep have no user-facing delivery; the staged
window shows zero Todoist mutation events (string-safe `eventType`/`objectId` reads, fail-loud) while the
intents log carries the computed writes (R3A1/R3A2).

---

## Phase D — Capture pipeline (Kanban, private)

### Task D1: Private Kanban board + capture pipeline stages

**Store:** `~/.hermes/kanban.db` (Bob-private, firewalled from the user, spec §9). Assignee = the **`default`
profile** (persona "Bob"; no profile named `bob`); `default_assignee: "default"`, `auto_decompose: false`,
`max_in_progress_per_profile: 2`, `failure_limit: 2` (Task A2 / spec §14). **Preflight:**
`hermes profile show default` must succeed.

- [ ] **Step 1: Author the pipeline** — parent Bob does **stage 1 synchronously** (structured `td task add` to
  Inbox — NOT `quickadd`, so no date is parsed pre-classification, spec §8b/U4; instant ack, idempotent).
  Stages 2–5 run as background **default-profile** subagent Kanban work (spec §8b/W4). Author a dedicated
  **curator-pinned `forzare-capture-pipeline` skill** holding the stage logic, and attach it to every card.
  Kickoff (title is a **required** positional arg — verified): **`hermes kanban create "<title>" --triage
  --idempotency-key <inbox-task-id> --assignee default --skill forzare-capture-pipeline`** (R7/W4 —
  `--triage` parks in triage; the **idempotency key is the stage-1 Inbox TASK ID**, so a parent retry or a
  no-resume card restart re-derives the same key and gets the same card; `--skill` pins the stage logic to
  the card). **`hermes kanban specify <task_id>` is MANDATORY in stage 2** (the `auxiliary.triage_specifier`
  slot, Task A2) — every capture is concretized before routing, never routed as a raw fragment. **User
  decisions (cases 3–4) call back through the parent** Discord-bound conversation (the named callback, spec
  §8b/W4). Any stage-2 date-write goes through the centralized date-mutation layer (B7 Step 0, W6). The 5
  stages: Place → Decide-placement (task-vs-event pre-check + 4 routing cases) → Verify+research-decision →
  Research → Split, each gating the next (spec §8b).
- [ ] **Step 2: Idempotent dup-guards (forced by no-mid-run-resume, spec §8a/§8b/§19).** Every stage is
  check-before-create: stage 1 skips if the capture is already in Inbox; stage 2 skips re-routing a placed
  task and skips a duplicate 🤖-calendar event; stage 5 skips existing subtasks. A restart converges to one
  task, never a dup.
- [ ] **Step 3: Never auto-create a project** (case 4 asks inline; spec §8b). Cases 3–4 route asks through
  the **parent** Discord-bound conversation (cron/subagent turns have no session for buttons, spec §12.1c).
- [ ] **Step 4: Failures are loud** — a stage error / un-completable card → `#forzare-errors` via
  `hermes send --to discord:<#forzare-errors>` (spec §8b/§16); the captured item is safe (stage 1 persisted
  it). (The forzare-ops watchdog also scans the kanban DB for `gave_up`/`blocked` cards, Task F1.)
- [ ] **Step 5: Verify — card lifecycle + idempotency, isolated by a NON-SPAWNABLE test assignee (W4,
  corrects R2A25/R2A21).** **A dedicated `--board forzare-test` is NOT isolation from the dispatcher** — the
  embedded dispatcher/notifier **enumerates every board on disk each tick** (verified
  `gateway/kanban_watchers.py:205-215`, `list_boards(include_archived=False)`), so it would pick up and run
  cards on `forzare-test` too. The **real** isolation is a **non-spawnable test assignee**: a card is spawned
  only when `status='ready'` **and its assignee maps to a real Hermes profile** (verified `has_spawnable_ready`
  → `profile_exists`, `kanban_db.py:6556`). Profiles are `{default, butters, concerned, elaine, nicodemus}`
  (verified `hermes profile list`), so **`--assignee forzare-noop-test`** (no such profile) is never spawned —
  even after `specify` auto-promotes the `--triage` card off `triage` (the specifier promotes to `todo`, so a
  real-profile assignee WOULD then dispatch; the non-spawnable assignee is what keeps it inert). `--triage`
  alone is not enough. (Also verified: **`kanban.max_in_progress < 1` is IGNORED**, not a pause —
  `kanban_watchers.py:749-752` — so never rely on `max_in_progress: 0`.) Read the **`status` field via
  `--json`** (verified `kanban show --json` emits `status`), not a text grep.

```bash
set -o pipefail
NOOP=forzare-noop-test   # NOT a real profile — the dispatcher can never spawn it (W4 isolation)
# a titled --triage card (title is a REQUIRED positional, R3A11); the idempotency key must return the SAME id on re-fire:
ID1=$(hermes kanban create "[TEST] capture probe" --triage --idempotency-key test-cap-001 --assignee "$NOOP" --json | jq -r '.id // .task_id')
ID2=$(hermes kanban create "[TEST] capture probe" --triage --idempotency-key test-cap-001 --assignee "$NOOP" --json | jq -r '.id // .task_id')
[ -n "$ID1" ] && [ "$ID1" = "$ID2" ] || { echo "FATAL: idempotency did not dedupe ($ID1 vs $ID2)" >&2; exit 1; }
echo "idempotency dedupe OK (same id $ID1)"
# read the STATUS field from JSON (not a text grep): a fresh --triage card is status=triage
ST=$(hermes kanban show "$ID1" --json | jq -r '.status')
[ "$ST" = triage ] || { echo "FATAL: expected status=triage, got $ST" >&2; exit 1; }
echo "triage status OK; assignee=$NOOP is non-spawnable, so the live dispatcher leaves it inert"
# clean up: kanban has NO `delete` verb — use `archive`:
hermes kanban archive "$ID1"
td task list --filter "search: [TEST]" --all --json | jq -r '.results[]|select(.content|startswith("[TEST]"))|.id' \
  | xargs -r -I{} td task delete {} --yes
```

Expected: the second create returns the existing card id (no dup); the `status` field reads `triage`; the
non-spawnable assignee keeps the live dispatcher from ever running the probe card. **Acceptance:** idempotency
key dedupes; a simulated stage crash restarts from stage 1 and still yields one task; a forced stage error
lands on `#forzare-errors` (its cron/kanban audit read keyed by the job/card id, never latest-mtime).
(Inbox-write correctness for a capture is covered in Task B8; stage-2 dated placement is asserted with the
non-spawnable assignee here.)

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

- [ ] **Step 2: Verify the `[SILENT]` contract per delivery path — DIRECT FILTER PROBES (R3A7/W11, replaces
  the staged agent-echo jobs).** The old test asked a staged agent to *echo* the sentinel — a flaky dependency
  on the model following instructions. The suppression decision is a **pure function** in the installed
  source, so probe **both paths separately** by importing the real filter functions (import viability
  verified 2026-07-11: `cron.scheduler._is_cron_silence_response` + `_CRON_SILENCE_TOKENS`;
  `gateway.response_filters.is_intentional_silence_response` + `is_intentional_silence_agent_result` — the
  exact probes below were run green against the installed hermes-agent venv):

```bash
set -o pipefail
~/.hermes/hermes-agent/venv/bin/python - <<'PY'
# CRON path (lenient): whole-response, first-line, last-line, [SILENT]-prefix all suppress; mid-sentence delivers.
from cron.scheduler import _is_cron_silence_response, _CRON_SILENCE_TOKENS
assert "[SILENT]" in _CRON_SILENCE_TOKENS
assert _is_cron_silence_response("[SILENT]"),               "cron: exact must suppress"
assert _is_cron_silence_response("[SILENT] no changes"),    "cron: prefix must suppress"
assert _is_cron_silence_response("[SILENT]\nnote"),         "cron: first-line must suppress"
assert _is_cron_silence_response("2 filtered\n\n[SILENT]"), "cron: last-line must suppress"
assert not _is_cron_silence_response("I considered [SILENT] but here is the report"), "cron: mid-sentence must DELIVER"
# GATEWAY path (strict): exact whole-response only, success-only.
from gateway.response_filters import is_intentional_silence_response, is_intentional_silence_agent_result
assert is_intentional_silence_response("[SILENT]"),              "gateway: exact must suppress"
assert not is_intentional_silence_response("[SILENT] no changes"), "gateway: prefix must DELIVER"
# FAILED turn is structurally un-silenceable on the gateway path:
assert not is_intentional_silence_agent_result({"failed": True},  "[SILENT]"), "gateway: failed turn must never be silenced"
assert is_intentional_silence_agent_result({"failed": False}, "[SILENT]"),     "gateway: successful exact silence suppresses"
print("SILENT contract probes: cron(lenient) + gateway(exact, success-only) all green")
PY

# CRON-path FAILED-turn end-to-end check (the one piece a pure-function probe can't cover): a failed run
# still writes its audit + delivers its failure summary. Deterministic via a --no-agent script exiting 1.
# The script MUST live under ~/.hermes/scripts/ — verified cron/scheduler.py:1571-1590 BLOCKS any path that
# resolves outside it ("Blocked: script path resolves outside the scripts directory") — so use a dedicated
# forzare-staging/ subdir, created + removed by the test (a documented staging-only live-write exception,
# like the staged cron jobs themselves — R3A6):
mkdir -p ~/.hermes/scripts/forzare-staging
printf '#!/usr/bin/env bash\nexit 1\n' > ~/.hermes/scripts/forzare-staging/forzare-fail.sh
chmod +x ~/.hermes/scripts/forzare-staging/forzare-fail.sh
JC=$(hermes cron create '0 0 1 1 *' 'n/a' --no-agent --script forzare-staging/forzare-fail.sh --deliver local --name silent-fail | jid_from_create)
hermes cron run "$JC" >/dev/null && hermes cron tick >/dev/null
ls ~/.hermes/cron/output/"$JC"/*.md >/dev/null 2>&1 \
  && echo "failure audit recorded for $JC (a failed run is un-silenceable — delivers its failure summary)" \
  || { echo "FATAL: no failure audit for $JC" >&2; exit 1; }
hermes cron remove "$JC"
rm -f ~/.hermes/scripts/forzare-staging/forzare-fail.sh && rmdir ~/.hermes/scripts/forzare-staging
```

  The **delivered-vs-suppressed live observation** (an actual Discord message appearing / not appearing) is
  **deliberately moved to G1 day-1** (R3A7/W11) — optionally against a throwaway test channel
  (`--deliver discord:<test-channel-id>`) before the home channel — because it needs live delivery, which
  staging forbids. No assertion here depends on an agent *echoing* a sentinel.

- [ ] **Step 3: Standardize the clarify-button ask pattern** across skills that ask on a live session (§4
  defer, §7 stall decision, §8b cases 3–4, §3B low-confidence): max 4 choices + auto "Other"; fall back to a
  plain one-line question on cron/subagent-origin turns (spec §12.1c). **Never use emoji reactions as input**
  (no inbound reaction events, spec §19/R8) — outbound 👀→✅/❌ ack-reactions are a free "Bob heard you" cue only.

**Acceptance:** the resolved Discord reset policy is `mode=both, at_hour=4, notify=False`; the direct filter
probes pass for both paths (cron lenient: exact/prefix/first-line/last-line suppress, mid-sentence delivers;
gateway strict: exact-only, failed-turn never silenced); the forced-failure cron run writes its audit and
delivers its failure summary; the staging script dir `~/.hermes/scripts/forzare-staging/` is removed after
the test; clarify buttons render on a live session, plain questions on cron turns; delivered-vs-suppressed is
observed live at G1 day-1 (W11).

---

### Task E2: The shared apply-checkpoint GATE + the `jobs.json` live-data exception (U9)

**The three deploy checkpoints now live INLINE at their phase ends (R2A2), not here:** **Checkpoint A** (config
/`.env`/`SOUL.md`) at the end of Phase A; **Checkpoint C** (skills + bundles) **before Task C2** (its
dependent staged command, W3) and Phase D; **Checkpoint F** (watchdog) at the end of Phase F. Each phase's live-path verify is explicitly gated on its
checkpoint being CLEARED, so the build never proceeds on un-applied config. **This task owns only** (a) the
shared **fail-closed gate check** below (invoked by every inline checkpoint) and (b) the `jobs.json` live-data
exception. The `session_reset`/config-drift resolution is owned by Task E1 (Phase E) and Task A2 — not
duplicated here.

- [ ] **The shared gate check (SIGPIPE-safe + error-loud + FILE-TARGETED; used by Checkpoints A / C / F).**
  **`chezmoi diff` on a DIRECTORY target is NON-recursive (W3, verified chezmoi v2.70.5 in an isolated
  fixture: a differing managed file *under* the directory produced NO diff output on the dir target; the
  explicit file target showed the full diff).** So the gate never diffs `~/.hermes` as a directory — each
  checkpoint passes its **explicit FILE list**, and the gate loops file-by-file (file-only recursive
  comparison), capturing stderr per file (warnings surfaced, never fatal-by-accident silence), fail-closed on
  any nonzero exit or pending diff:

```bash
set -o pipefail
# Usage: gate_check <file>...   — each checkpoint passes ITS OWN explicit file list (W3):
#   Checkpoint A: ~/.hermes/config.yaml ~/.hermes/.env ~/.hermes/SOUL.md
#   Checkpoint C: ~/.hermes/skills/*/SKILL.md (each Phase-B skill) + ~/.hermes/skill-bundles/forzare-*.yaml
#   Checkpoint F: ~/.local/bin/forzare-ops-watchdog.sh ~/Library/LaunchAgents/com.webdavis.forzare-ops-watchdog.plist
gate_check(){
  cd "$(git rev-parse --show-toplevel)" || return 1
  [ "$#" -gt 0 ] || { echo "FATAL: gate_check needs an explicit FILE list (dir diffs are non-recursive)" >&2; return 1; }
  local f rc fail=0 DIFF_OUT DIFF_ERR
  for f in "$@"; do
    DIFF_OUT=$(mktemp); DIFF_ERR=$(mktemp)
    if chezmoi --source "$PWD" diff "$f" >"$DIFF_OUT" 2>"$DIFF_ERR"; then rc=0; else rc=$?; fi
    # An undecryptable .age / KeePassXC-locked template surfaces on stderr — a checkpoint FAILURE, never a
    # silent "no diff":
    if [ -s "$DIFF_ERR" ]; then
      echo "FATAL: chezmoi diff '$f' emitted stderr:" >&2; cat "$DIFF_ERR" >&2; fail=1
    fi
    if [ "$rc" -ne 0 ]; then echo "FATAL: chezmoi diff '$f' exited $rc" >&2; fail=1; fi
    if [ -s "$DIFF_OUT" ]; then
      echo "PENDING change for '$f' — run the user-run apply before proceeding:"; cat "$DIFF_OUT"; fail=1
    fi
    rm -f "$DIFF_OUT" "$DIFF_ERR"
  done
  [ "$fail" -eq 0 ] || return 1
  echo "source == live for all $# file target(s) (checkpoint CLEARED)"
}
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
  - **(b) forzare run failures — the predicate is a causal run EVENT, never status+counter (W9, corrects
    V9/R2A6).** Since the last stamped watermark, scan `~/.hermes/cron/output/` for failed ritual runs and the
    kanban DB for **genuine** failures, routing each to the errors channel. Alert **ONLY** on failure
    **events/outcomes since the watermark**: a **`gave_up`** outcome (the `failure_limit` trip); a
    **`timed_out`** or **`crashed`** run event. **NEVER derive failure from `status='blocked' AND
    consecutive_failures > 0`** — verified: **`block_task` does NOT clear `consecutive_failures`**
    (`kanban_db.py:4383` sets only status/claim fields; the counter clears only on success/reassign), so a
    healthy **awaiting-user** block after one recovered transient failure still carries `counter == 1` and the
    status+counter predicate would false-alarm on it. Awaiting-user blocks emit no failure event, so the
    event-based predicate excludes them by construction — "unread `#forzare-errors` = broken" holds.
    - **Durability (mirror `executable_osquery-uptime-watchdog.sh`):** **content-stable alert ids** (hash of
      {kind, id, run-ts}) so a re-scan never double-alerts; **spool the pending alert BEFORE advancing the
      watermark**, and **drain the spool first each pass**; if `hermes send` exits non-zero (Discord down),
      **retain the spool and retry next pass** — a failed alert is never lost.
  - **Alert:** **`hermes send --to discord:<#forzare-errors>`** (R2 — no LLM, no agent loop, no running
    gateway for bot-token platforms), plus the relay's phone/local push as belt-and-suspenders; if
    `DISCORD_ERRORS_CHANNEL` is unset, fall back to the home channel with a `⚠ ERROR` prefix (the fallback
    lives HERE, not in hermes). **Robustness under launchd's minimal env (W9):** resolve the **absolute
    `hermes` binary path at install** — a script-level `HERMES_BIN` constant or the plist's
    `EnvironmentVariables` `PATH` including `~/.local/bin` — and **load the channel env explicitly** (source
    `~/.hermes/.env` for `DISCORD_ERRORS_CHANNEL`/`DISCORD_HOME_CHANNEL`); an inherited-PATH `hermes: command
    not found` or an unset channel would silently swallow the alert. `set -euo pipefail`, double-quoted
    expansions, ISO-8601 timestamps (`date -u +"%Y-%m-%dT%H:%M:%SZ"`). **Do NOT curl the Discord webhook
    directly** (R2 dropped that phrasing). `:8644` caveat — exists only while the webhook platform is enabled;
    for a platform-independent probe, `API_SERVER_ENABLED=1` → `/health` on `:8642` (spec §19).
- [ ] **Step 2: Write the plist**, modeled on `com.webdavis.osquery-uptime-watchdog.plist.tmpl` — launchd
  **`StartInterval` 300s (5 min, DECIDED — W8: it is what makes the two-channel invariant's "every system
  failure on `#forzare-errors` within W ≤ 5 minutes" true)**, `RunAtLoad` per the osquery model, `Label`
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

Expected: shellcheck clean; `plutil -lint` → `OK`; the live probe exits 0. **Acceptance (W9 cases):** the
script alerts via `hermes send --to` on a simulated down/hung code (bogus port), on a seeded `gave_up`
outcome, and on a `timed_out`/`crashed` run event; it stays **silent** for ANY `blocked` card that has emitted
no failure event — **including the recovered-failure-then-user-block fixture** (a card that fails once, is
retried successfully, then blocks awaiting the user: `status='blocked'`, `consecutive_failures == 1`, no
gave_up/crashed/timed_out event since the watermark ⇒ NO alert — this is the case the old status+counter
predicate got wrong) — and when healthy; a **second scan does NOT re-alert** the same failure (content-stable
ids); a **simulated Discord outage** (`hermes send` exit-1) **retains the spool and retries next pass** (no
lost alert); and the script finds `hermes` + the channel env **under launchd's minimal environment** (W9 —
absolute `HERMES_BIN`, sourced `.env`). **This task's files are committed to the repo via the normal
pre-commit flow** (`just lint-check` + `just test`), separate from the two doc commits.

---

### APPLY CHECKPOINT F (inline, fail-closed — R2A2) — do this BEFORE Phase G go-live

Apply the watchdog script + plist sources, then `launchctl load` the agent. **Phase G's go-live is gated on
"Checkpoint F cleared"** — the watchdog (the errors-channel router + gateway-hang detector) must be live
before delivery flips to Discord, or a post-go-live failure could go unrouted.

- **User-run/agent-run:** apply `dot_local/bin/executable_forzare-ops-watchdog.sh` +
  `Library/LaunchAgents/com.webdavis.forzare-ops-watchdog.plist.tmpl`; `launchctl load` it.
- **Gate (fail-closed) — covers this checkpoint's OWN artifacts (R3A8/W3):** run the **E2 gate check** with
  the explicit file targets `gate_check ~/.local/bin/forzare-ops-watchdog.sh
  ~/Library/LaunchAgents/com.webdavis.forzare-ops-watchdog.plist` (never a directory — dir diffs are
  non-recursive, W3); then a **content-hash compare** of both applied artifacts against their rendered source
  (belt-and-suspenders over the diff — catches a stale apply the template renderer would mask):

```bash
set -o pipefail
cd "$(git rev-parse --show-toplevel)"
for t in ~/.local/bin/forzare-ops-watchdog.sh; do
  SRC_H=$(chezmoi --source "$PWD" cat "$t" | shasum -a 256 | cut -d' ' -f1)
  LIVE_H=$(shasum -a 256 "$t" | cut -d' ' -f1)
  [ "$SRC_H" = "$LIVE_H" ] || { echo "FATAL: content hash mismatch for $t (src $SRC_H != live $LIVE_H)" >&2; exit 1; }
done
PLIST_SRC_H=$(CI=1 chezmoi --source "$PWD" execute-template --no-tty \
  < Library/LaunchAgents/com.webdavis.forzare-ops-watchdog.plist.tmpl | shasum -a 256 | cut -d' ' -f1)
PLIST_LIVE_H=$(shasum -a 256 ~/Library/LaunchAgents/com.webdavis.forzare-ops-watchdog.plist | cut -d' ' -f1)
[ "$PLIST_SRC_H" = "$PLIST_LIVE_H" ] || { echo "FATAL: plist content hash mismatch ($PLIST_SRC_H != $PLIST_LIVE_H)" >&2; exit 1; }
echo "content hashes OK"
launchctl print "gui/$(id -u)/com.webdavis.forzare-ops-watchdog" | grep -i state
```

  `checkpoint CLEARED` + matching hashes + a loaded agent are all required; any pending diff / stderr /
  hash mismatch / unloaded agent blocks go-live.

---

## Phase G — Dry-run → calibrate → go-live

### Task G1: Staged dry-run + explicit go-live matrix + flip to live (the final gate)

**Files:** none new (operational — the final gate). **Go-live is Step 4 of THIS task** (there is no separate
"G4" — earlier references corrected).

- [ ] **Step 1: Run the full brief + EOD staged (`--deliver local` + the DRY-RUN prompt directive) across the
  scenario matrix**, reading `~/.hermes/cron/output/` each run by job id (spec §17). Confirm the brief fires
  `15 5 * * *`, surfaces ≤3 sensibly, weather/calendar/follow-up steps degrade visibly (never silently), the
  EOD roll + unconditional p1-clear + **lifecycle-ledger** ticks behave, and the 02:00 reconcile marks (never
  messages). **Assert the actual step ORDER / mutation boundaries from the INTENTS LOG + trace (V7/R3A1):**
  the **EOD** run journals **zero `p1` and zero calendar intents** in `dryrun-intents.jsonl` (a correct
  dry-run left the real store untouched, so the intents log — not `td activity` or the 🤖 calendar — is where
  a boundary violation would show); the **morning** run's roll intents precede its `p1` intents in the log's
  record order (the defensive `eod-roll` before any `eisenhower-plan` p1 write). A bundle whose instruction
  failed to sequence would journal a p1 intent before the roll intents — that fails this gate.
- [ ] **Step 2: Explicit go-live matrix (replaces "several days / sensibly").** Drive each scenario and assert
  expected state + message count (U15):

  | Scenario | Expected state | Expected messages |
  |---|---|---|
  | Work day (Tue/Thu/Sat 15:00–23:00) | deep window = morning; evening = work | 1 brief |
  | Off day (Mon/Wed/Fri) | deep window = morning + evening | 1 brief |
  | ON-Sunday (alt-anchor Jun 7=ON) | work-day brief | 1 brief |
  | Recovery morning (post-overnight) | recovery/sleep window, no deep push | 1 brief, no gym nag |
  | Recovery fire — ≤2h catch-up, >2h past-grace single fire (V3) | the day closes exactly once (range + stamp) | 0 extra |
  | **≥3-day outage drain (R3A9/W5)** | ONE pass closes the whole gap `(stored .. yesterday]`; tasks roll to today; **`roll_count` ticks ONCE per task for the entire gap** (no multi-tick shame); stamp = yesterday | 0 extra |
  | Duplicate fire / already-reconciled (W5) | `stored ≥ CEILING` ⇒ no-op (an `already-reconciled` record; no dates, no counters, no stamp) | 0 extra |
  | Dependency failure (gog/td down) | degrade-and-note inline; if unrecoverable → errors channel | 1 brief (degraded) + errors msg |
  | Concurrent trigger (live turn + cron) | at most one DO-NOW action or one requested decision each (§12.3/W12 residual accepted) | ≤2 short, no wall |

```bash
set -o pipefail
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
STAMP=~/workspaces/Ivy/forzare/state/last-reconcile.json
# STAGED duplicate-fire scenario (R3A3: observable via the intents log — a dry-run never advances the real
# stamp, so comparing the real stamp across two dry-runs proves nothing):
: > "$INTENTS"
STAMP_MT0=$(stat -f %m "$STAMP")
DRY='DRY RUN — record intended writes to forzare/state/dryrun-intents.jsonl, perform none. '
J1=$(stage_skill '0 0 1 1 *' "${DRY}Run eod-roll once." eod-roll test-dupfire-1); hermes cron remove "$J1"
J2=$(stage_skill '0 0 1 1 *' "${DRY}Run eod-roll once (duplicate/defensive fire)." eod-roll test-dupfire-2); hermes cron remove "$J2"
grep -q 'already-reconciled' "$INTENTS" \
  || { echo "FATAL: duplicate staged fire logged no already-reconciled no-op intent (R3A3)" >&2; exit 1; }
[ "$(stat -f %m "$STAMP")" = "$STAMP_MT0" ] || { echo "FATAL: a dry-run advanced the REAL stamp" >&2; exit 1; }
echo "staged duplicate-fire idempotency OK (intents-log no-op; real stamp untouched)"
```

- [ ] **Step 3: Calibrate** — tune the duration upward-bias factor + weather thresholds + brief content from
  the observed staged output (spec §4c/§6a; the Task B9 reducers are the producer). Priors stay auditable in
  `calibration/priors.md`.
- [ ] **Step 4: Flip to live (W2/R3A16) — three moves, on the COMPLETE user-facing job list.** The user-facing
  jobs are exactly: **morning brief · end-of-day · gym-window-end check · every block-boundary prompt** (the
  02:00 `waiting-reconcile` and the monthly someday-sweep stay `--deliver local` permanently — state-only).
  For EACH user-facing job:
  1. **Remove the DRY-RUN directive** from its prompt (`hermes cron edit` — the prompt no longer opens with
     the `DRY RUN …` line; the bundle instruction likewise drops any staging variant), and truncate
     `forzare/state/dryrun-intents.jsonl` (staging evidence, not live state).
  2. **Flip delivery** from `local` to `--deliver discord` (home channel).
  3. **Resume/enable the 23:00 eod-roll job** (`hermes cron resume` if it was paused) — it now performs REAL
     rolls keyed off the seeded `last-reconcile.json`.
  The errors channel stays the forzare-ops watchdog's `hermes send --to discord:<#forzare-errors>` +
  belt-and-suspenders relay. **This is the last step; do it only after Steps 1–3 are green.**
- [ ] **Step 5: Post-go-live smoke + DAY-1 SUPERVISED checks (R3A3/R3A7/W11)**

```bash
set -o pipefail
hermes cron list                                   # jobs live, Denver TZ; no prompt begins "DRY RUN"
curl -fsS -m 3 http://127.0.0.1:8644/health && echo "gateway up"
launchctl print "gui/$(id -u)/com.webdavis.forzare-ops-watchdog" | grep -i state
```

  **Day-1 supervised (the two checks staging cannot perform):**
  - **REAL double-roll guard (R3A3):** after the first live 23:00 EOD, note `last-reconcile.json`'s stamp;
    manually run `/forzare-eod` once — the stamp must NOT advance and no task may move a second time (the
    live counterpart of Step 2's intents-log probe).
  - **Delivered-vs-suppressed live observation (R3A7/W11):** watch the first live brief actually LAND in the
    home channel, and confirm the 02:00 reconcile + monthly sweep deliver NOTHING (optionally point one job at
    a throwaway test channel first: `--deliver discord:<test-channel-id>`).

Expected: jobs deliver to the home channel; watchdog loaded; health probe 0. **Acceptance:** a real brief
lands in the home channel; the day-1 double-roll and delivered-vs-suppressed checks pass; a forced failure
(and a seeded `gave_up` kanban card) lands in `#forzare-errors`; no gamification/achievements output anywhere.

---

## Phase H — Post-V1 follow-ups (explicitly parked — NOT built in V1)

> Recorded so reviewers cover them; each ships only after V1 is live (spec §18a). These are **tasks/sections
> for the future**, deliberately out of V1 scope.

- [ ] **§18a Per-channel delivery LEASE (V6/R2A15).** Close the §12.3 residual live-turn × cron interleave: a
  short-held claim on the task channel so a second emitter in the window defers. Explicitly NOT built for V1
  (it is the bespoke machinery R4 removed) — the residual is accepted as rare/benign (both paths
  receptivity-gated, each emits at most one thing). Add only if double-fire proves annoying in practice.
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
- [ ] **Fantastical backup-notifier (post-V1 note only, A32/W13 — genuinely optional).** V1 covers the §3a
  hard-stop rung with the **`calendar-write`-owned leave-time alarm** on the 🤖 calendar (event + popup
  reminder at leave-time minus prep, idempotent by event key, created at morning-plan — Task B3, W13); a
  Fantastical mirror as a second backup notifier is a *post-V1 nice-to-have*, never a V1 dependency. Recorded
  so it isn't lost.

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
`todoist-surface` first → B1; weather / calendar / eisenhower(**three modes incl. `replan`, W10**) /
activation / brief-assemble / followups / reflect / tomorrow-prep / `/forzare` classifier → B2–B6; `eod-roll`
/ `waiting-reconcile` / `transition` → B7; on-demand `/forzare-next` / `-today` / `-capture` handles → B8;
calibration logging + reducers → B9; bundles (morning gains eisenhower-plan + calendar-write + defensive
eod-roll; eod gains eod-roll, **no calendar-write, R2A8**; each carries a **mandatory `instruction:`
sequencer, V7**) + boot assertion → C1; cron rituals (**every job ATTACHES its bundle/skill via `--skill` —
a slash-command prompt is inert on the cron path, W1**; ONE daily brief `15 5 * * *` schedule-derived, 23:00
EOD, 02:00 `waiting-reconcile`, gym-window-end, boundaries, **monthly someday-sweep VIA the brief not a second
message, R2A20**; **all user-facing jobs `--deliver local` + the dry-run prompt directive until G1, V5/R3A4**;
R1a) → C2; Kanban capture pipeline (assignee `default`, titled `--triage` + **mandatory `specify`** +
**Inbox-task-id idempotency keys** + the pinned `forzare-capture-pipeline` skill + parent callback + 5 stages
+ dup-guards, R7/W4; **test isolation = a non-spawnable assignee, not a board — the dispatcher enumerates
every board, W4**) → D1; root `session_reset` (R6a) + `[SILENT]` per-path (**direct filter-function probes,
run green against the installed venv, R3A7**; forced-failure script under `~/.hermes/scripts/forzare-staging/`
— the verified path constraint, R3A6; delivered-vs-suppressed moved to G1 day-1, W11) + clarify buttons /
no-reaction-input (R8) → E1; **the shared fail-closed apply-gate now takes an explicit FILE list per
checkpoint — `chezmoi diff` on a directory is NON-recursive (fixture-verified), W3 — + jobs.json exception →
E2 (Checkpoint C moved BEFORE Task C2, its dependent staged command, W3; Checkpoint F covers its own
artifacts with file targets + content hashes, R3A8)**; `forzare-ops-watchdog` (health probe + cron/output +
**event-based kanban failure predicate — gave_up/crashed/timed_out only, never status+counter (`block_task`
does not clear `consecutive_failures`), W9**; `HERMES_BIN` + sourced env under launchd, W9; **`StartInterval`
300s — the W8 ≤5-minute errors-channel guarantee**; durable spool) + plist + `hermes send --to` alert (R2/U3)
→ F1; staged dry-run (**assertions retargeted to the `dryrun-intents.jsonl` observable, R3A1/R3A3**) +
explicit go-live matrix (**+ ≥3-day outage drain with ONE `roll_count` tick per task, R3A9/W5**) + the
three-move flip (**remove the dry-run directive, flip delivery, resume EOD — on the COMPLETE user-facing job
list incl. gym-window-end, W2/R3A16**) + day-1 supervised double-roll + delivered-vs-suppressed checks
(R3A3/W11) → G1; post-V1 (**delivery-lease V6/R2A15**, Langfuse self-host, `tailscale serve` webhook, Hue,
Todoist webhooks, voice, ledger, email/comms triage, Fantastical-as-genuinely-optional W13) → H; ai-skills
STB#3 out of scope → H Non-Goals.
**Round-2 additions:** the shared **ledger I/O helper** (flock + atomic + journal-then-commit + healing, V2)
→ B7 Step 0; the dry-run read-only contract honored by every mutating skill (V4) → Global Constraints + Phase
B intro; **eod-roll keyed off an explicit reconciliation date + past-grace single-fire recovery matrix** (V3)
→ B7; the **`work_schedule`/schedule `skills.config`** owning task (V11/R2A10, blocks C2) → B10; **job-id
parse rebuilt to `Created job:` + 12-hex, per-job audit dir** (R2A1) throughout the staged blocks.
**Round-3 additions:** dry-run made OBSERVABLE via `forzare/state/dryrun-intents.jsonl` with the INSTRUCTION
transport (env is stripped from gateway-ticked children) and staging assertions retargeted to the intent
records (R3A1/R3A2/R3A4/W2) → Global Constraints + Phase B intro + B1/B6/B7/B8/C2/G1; the **centralized
date-mutation layer with the state-chosen verb** — `td task update --due` for initial dating of an undated
task (probed live: `reschedule` errors `NO_DUE_DATE`), `td task reschedule` for re-dating, timed/recurring
never mutated — + the six W6 dating fixtures → B7 Step 0 + B1/B4/D1; the **reconciliation RANGE with the
never-≥-today ceiling, one-pass outage drain, single tick per task, seeded stamp** (R3A9/W5) → A1/B7/G1;
`--filter "search: [TEST]"` + `--all` on every fixture read (default page = 300 of ~2270, R3A5) → B1/B8/D1;
calibration attribution excludes journaled forzare writes + the negative fixture (W7) → B9; the two-channel
invariant restated with the ≤5-minute window as a recorded adjudication (W8) → F1 + spec §0/§12.4/§16; B10
asserts the four weather thresholds it claims (R3A14); C1's yq checks are capture-then-match with a real
`comm` assertion (R3A15); the leave-time hard-stop alarm owned by `calendar-write` (W13) → B3/B4/H; the
shared phrasing-rotation directive named by every recurring-prompt skill (R3A17) → Phase B intro +
B1/B4/B5/B7; `gog calendar list` is the EVENTS alias and `cal` aliases the calendar group (R3A10) → B3;
kickoff titles are required positionals (R3A11) → D1 + spec §8b/§15; `Active now` row named (R3A12) + the
A1 state-file list gains `tomorrow-prestage.json` + `dryrun-intents.jsonl` (R3A13/R3A1). Delivery is
headless-native, no plugin, no `inject_message` (R1/R4/R5) throughout; every produced artifact ships via the
chezmoi source-dir pipeline (the delivery-vehicle rule). **Known open items carried forward** (not gaps): the
2053-task backlog relevance-comb is pending, so planning-pull stays conservative (C2/B4); `/forzare-*` native
slash autocomplete depends on the §12.5 mirroring check.

**Placeholder scan:** verification commands are runnable; `<inbox-task-id>` / lat-long / channel-ids are the
intentionally per-environment values a cold reader fills from their own setup.

**Consistency:** channel names (`DISCORD_HOME_CHANNEL` / `DISCORD_ERRORS_CHANNEL` / `#forzare-errors`), the
health probe (`curl -fsS -m 3 http://127.0.0.1:8644/health`, 0/28/7), the alert primitive (`hermes send --to
discord:<channel>`), the per-path `[SILENT]` rule, the assignee (`default`), and the unprefixed label / filter
names match the spec and each other across A–H.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-11-bob-executive-assistant.md`. Execute
task-by-task with superpowers:subagent-driven-development or superpowers:executing-plans; **go-live is Task G1
Step 4** (the final step) — everything before it runs `--deliver local`.
