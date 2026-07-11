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
  acceptance = zero production mutations, asserted across ALL stores (Y4/X3):** (positive) the intent RECORDS
  exist in `dryrun-intents.jsonl` with the expected fields; (negative) `td activity --by me --since <today>
  --json` (camelCase `eventType`, §19) shows no forzare task changes in the window on **both** the `--type
  task` and `--type comment` streams, a **[TEST]-scoped `gog calendar events`** snapshot is unchanged, **and**
  owned-layer state-file mtimes are unchanged for **every** file a mutating skill could touch —
  `last-reconcile.json` / `task-lifecycle.json` / **`mutation-journal.jsonl`** (Y5) / `schedule-override.json`
  / `tomorrow-prestage.json` / **`plan-of-day.json`** (Y13) / **`decision-queue.json`** (Y1) / the
  `calibration/` store. **Native CLI dry-run flags layer UNDER the prompt mode (Y4, defense in depth):** staged
  mutating `td` calls also carry **`--dry-run`** (verified on `td task add/update/reschedule/complete/delete` +
  `td comment add`) and staged `calendar-write` also carries **`gog --readonly`** or **`-n/--dry-run`** — so a
  skill that forgot the mode check still cannot mutate the outside world. The **23:00 eod-roll job is created
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
- **Seeded at install (NOT lazy):** `last-reconcile.json` = **Denver yesterday** (spec §8/R3A9/W5/X6) — so
  the first real 23:00 EOD (CEILING = today at/after the 23:00 Denver cutoff) closes exactly one day and no
  cold run mistakes a fresh install for a multi-year outage.
- Runtime-written under `state/` (created lazily by the skills, not here): `schedule-override.json` (shift
  override + today's gym `activation` field, spec §8a), the **lifecycle MAP** `task-lifecycle.json`
  (`roll_count`/`written_due`/`last_escalated`/`kind` per task id, spec §4d) **and the append-only mutation
  JOURNAL `mutation-journal.jsonl`** (typed op-history, 45-day retention — the store split, Y5/spec §4d),
  `tomorrow-prestage.json` (EOD proposal → morning brief, spec §8a/R3A13), the **unified
  `decision-queue.json`** (ALL brief-time decisions — waiting-chase/fixed-redecision/stall-decision/
  triage-reraise/sweep-candidate; Y1/R5A1, generalizes the old `sweep-candidates.json`), the **per-day
  `plan-of-day.json`** (the morning-plan idempotency record, Y13), and — **staging only** —
  `dryrun-intents.jsonl` (the ONE file a dry-run writes, spec §17/R3A1; truncated at go-live, never present in
  a live run).

- [ ] **Step 1: Create the directory tree + seed `last-reconcile.json` = Denver yesterday** (spec §8/R3A9/W5/X6
  — the bootstrap seed; BSD/macOS `TZ=America/Denver date -v-1d`):

```bash
set -o pipefail
mkdir -p ~/workspaces/Ivy/forzare/calibration ~/workspaces/Ivy/forzare/state
STAMP=~/workspaces/Ivy/forzare/state/last-reconcile.json
if [ ! -f "$STAMP" ]; then
  Y=$(TZ=America/Denver date -v-1d +%F)   # DENVER yesterday (X6) — the ceiling/stamp math is Denver-local;
                                          #   a UTC seed can be a day off across the evening TZ offset
  printf '{"reconciled_date":"%s"}\n' "$Y" > "$STAMP"
  echo "seeded last-reconcile.json = $Y (Denver yesterday)"
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

- [ ] **Step 5: Author the persistent `gate_check` script (X10 — introduced HERE, BEFORE Checkpoint A, so
  every inline checkpoint can source it; Task E2 documents its contract but no longer *defines* it).** The
  three deploy checkpoints (A/C/F) each need the shared fail-closed gate, and Checkpoint A runs at the end of
  *this* phase — so the function must exist before then, not be defined later in Phase E. Write it to the owned
  layer (persistent, no chezmoi apply needed), and have each checkpoint `source` it:

```bash
set -o pipefail
GATE=~/workspaces/Ivy/forzare/gate-check.sh
cat > "$GATE" <<'SH'
#!/usr/bin/env bash
# Shared fail-closed apply-gate (X10/W3). Usage: gate_check <file>...  — each checkpoint passes ITS OWN
# explicit FILE list; `chezmoi diff` on a DIRECTORY is NON-recursive, so never pass a dir target.
gate_check(){
  cd "$(git rev-parse --show-toplevel)" || return 1
  [ "$#" -gt 0 ] || { echo "FATAL: gate_check needs an explicit FILE list (dir diffs are non-recursive)" >&2; return 1; }
  local f rc fail=0 DIFF_OUT DIFF_ERR
  for f in "$@"; do
    DIFF_OUT=$(mktemp); DIFF_ERR=$(mktemp)
    if chezmoi --source "$PWD" diff "$f" >"$DIFF_OUT" 2>"$DIFF_ERR"; then rc=0; else rc=$?; fi
    if [ -s "$DIFF_ERR" ]; then echo "FATAL: chezmoi diff '$f' emitted stderr:" >&2; cat "$DIFF_ERR" >&2; fail=1; fi
    if [ "$rc" -ne 0 ]; then echo "FATAL: chezmoi diff '$f' exited $rc" >&2; fail=1; fi
    if [ -s "$DIFF_OUT" ]; then echo "PENDING change for '$f' — run the user-run apply before proceeding:"; cat "$DIFF_OUT"; fail=1; fi
    rm -f "$DIFF_OUT" "$DIFF_ERR"
  done
  [ "$fail" -eq 0 ] || return 1
  echo "source == live for all $# file target(s) (checkpoint CLEARED)"
}
SH
# a checkpoint uses it as:  source ~/workspaces/Ivy/forzare/gate-check.sh; gate_check <files...>
bash -n "$GATE" && echo "gate-check.sh parses OK"
```

  **Acceptance:** `gate-check.sh` exists in the owned layer, `bash -n` clean; every inline checkpoint (A/C/F)
  and Task E2 source THIS one script rather than redefining the function.

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
- **Gate (fail-closed):** `source ~/workspaces/Ivy/forzare/gate-check.sh` (the persistent script authored in
  Task A1 Step 5, X10) and run `gate_check` with this checkpoint's **explicit FILE list (W3 — `chezmoi diff`
  on a directory is NON-recursive, so never gate on a dir target):**
  `gate_check ~/.hermes/config.yaml ~/.hermes/.env ~/.hermes/SOUL.md`. It must print `checkpoint CLEARED` — a
  pending diff, a non-zero exit, or **any** stderr (undecryptable `.age` / locked template) is a **FAILURE
  that blocks Phase B**, never a silent pass.
- **CHANNEL GATE (Y10) — the two channels must resolve, be distinct, and the errors channel must actually
  deliver.** Because every failure alert (§12/§14) and every ritual delivery depend on these two ids, parse
  BOTH from the live `.env`, require each **non-empty AND distinct**, then run an **end-to-end `hermes send`
  probe to the errors channel with a verified receipt** — [TEST]-prefixed and noted as staging traffic — before
  clearing:

```bash
set -euo pipefail
set -a; . ~/.hermes/.env; set +a
: "${DISCORD_HOME_CHANNEL:?FATAL: DISCORD_HOME_CHANNEL empty}"
: "${DISCORD_ERRORS_CHANNEL:?FATAL: DISCORD_ERRORS_CHANNEL empty}"
[ "$DISCORD_HOME_CHANNEL" != "$DISCORD_ERRORS_CHANNEL" ] \
  || { echo "FATAL: home and errors channels are the SAME id — they must be distinct (Y10)" >&2; exit 1; }
echo "channels non-empty + distinct OK"
# end-to-end send probe to the ERRORS channel, receipt verified (a [TEST] line, immediately noted as staging):
SEND_ERR=$(mktemp)
if hermes send --to "discord:${DISCORD_ERRORS_CHANNEL}" \
     "[TEST] forzare Checkpoint-A channel probe — staging traffic, ignore" >/dev/null 2>"$SEND_ERR"; then
  echo "errors-channel send probe delivered OK (receipt: exit 0)"
else
  echo "FATAL: hermes send to the errors channel FAILED — the alert path is dead:" >&2; cat "$SEND_ERR" >&2; exit 1
fi
rm -f "$SEND_ERR"
```

  A dead errors-channel send here blocks Phase B — the whole failure-alerting design (§14/§16) rests on it.

---

## Phase B — Atomic skills + the shared helper (author-all → Apply Checkpoint B → pin+stage-all)

> **BUILD ORDER — the topological rule (Y6/R5A3, load-bearing).** A staged dry-run reads the skill's **LIVE**
> `~/.hermes/skills/<name>/SKILL.md`, which only exists after a `chezmoi apply`. So Phase B runs in **three
> ordered stages, not task-by-task-author-then-run:**
> 1. **Author ALL Phase-B sources first** — the **shared mutation helper FIRST (Task B0** — B1–B11 all depend
>    on it), then every skill's `SKILL.md` (B1–B10), the **`forzare-capture-pipeline` skill (Task B11**, moved
>    here from D1 per R5A4), and the schedule `skills.config` (B10) — all authored in the `dot_hermes/` source
>    dir, **no live run yet.**
> 2. **APPLY CHECKPOINT B** (new, fail-closed — at the end of the author stage) — the user-run/agent-run apply
>    of **every** Phase-B file (the skills, the capture-pipeline skill, and the `config.yaml` `skills.config`
>    from B10) + the boot skill-existence assertion + the shared gate over the explicit file list. **No staged
>    dry-run below may run until Checkpoint B is CLEARED** — that is the "no stage-run before its live copy
>    exists" rule.
> 3. **Pin + staged dry-run ALL** — only after Checkpoint B: `hermes curator pin <name>` + the per-skill staged
>    cron dry-run (each task's "curator-pin + staged dry-run" step). These are the verify steps; they are
>    explicitly gated on Checkpoint B.
>
> Each task below is written skill-by-skill for readability; its **author** steps belong to stage 1 and its
> **curator-pin + staged dry-run** steps to stage 3. (Checkpoint **C** is therefore re-scoped — it applies only
> what **Phase C** authors, the three bundles; the skills are already live from Checkpoint B, R5A3.)
>
> Every skill's applied target is `~/.hermes/skills/<name>/SKILL.md` (+ any helper scripts) — authored in
> the `dot_hermes/` source dir per the delivery-vehicle rule (Global Constraints) — is **curator-pinned**
> (`hermes curator pin <name>`, spec §13), and is driven test-first via a **staged cron dry-run**. `td` usage
> is learned from the installed `/todoist-cli` skill — do NOT duplicate `td` command knowledge into these
> skills (spec §10).
>
> **Every MUTATING skill honors the DRY-RUN instruction (V4/R3A1/R3A4/X3/Y1/Y5 — applies to each writing skill
> below).** The **complete writer inventory (X3):** `todoist-surface`, `calendar-write`, `eisenhower-plan`
> (writes `p1` + the `plan-of-day.json` record, Y13), `forzare` (schedule-override), `eod-roll` (roll + the
> lifecycle MAP/JOURNAL + `decision-queue.json` fixed/stall records, Y1), `waiting-reconcile` (enqueues
> `waiting-chase` `decision-queue.json` records), `forzare-capture`, **`tomorrow-prep`** (writes
> `tomorrow-prestage.json`), **`followups-sweep` in SWEEP mode** (enqueues `sweep-candidate`
> `decision-queue.json` records), **`calibration-log`** (appends the calibration store), the **capture
> pipeline** (enqueues `triage-reraise` `decision-queue.json` records, Y2), and **`brief-assemble`** (its
> prestage-CLEAR consumes then truncates `tomorrow-prestage.json`) — each SKILL.md must state and implement the
> read-only contract: when the dry-run directive is active, **append each intended write to
> `forzare/state/dryrun-intents.jsonl` (the ONE file dry-run may write) and perform NONE** (no `td` write, no
> MAP/JOURNAL commit, no `calendar-write`, no
> `schedule-override.json`/`tomorrow-prestage.json`/`plan-of-day.json`/`decision-queue.json`/`calibration/`
> write-or-clear). **The one LIVE-ONLY write (R5A5):** the `decision-queue.json` **ack** — marking the head
> record `acked` when the user answers — is written only by the live turn that receives the answer, **never** by
> a cron/dry-run path, so it never appears in the intents log. Each record is `{ts, skill, op, target, args,
> run_id}` — the observable evidence a staged run is asserted against (never the real store).
>
> **Intent `op` vocabulary — DEFINED ONCE here (R5A13), referenced by every jq gate below.** The
> `dryrun-intents.jsonl` `op` field is one of a fixed enum: **`task.add`**, **`task.update-labels`**,
> **`task.update-due`**, **`task.update-description`** (the §7/X13 if-then cue), **`task.complete`**,
> **`comment.add`**, **`calendar.create`** / **`calendar.update`** / **`calendar.delete`**, **`state-write`**
> (any owned-layer state file — `schedule-override.json` / `last-reconcile.json` / the lifecycle
> `task-lifecycle.json` MAP / `mutation-journal.jsonl` / `tomorrow-prestage.json` / `plan-of-day.json` /
> `decision-queue.json`), **`p1.set`** / **`p1.clear`**. Every SKILL task and every jq assertion below uses
> exactly these names. (Distinct from the mutation-JOURNAL `type` enum — `date-op`/`p1`/`label`/`comment`/
> `calendar`/`description`, spec §4d/Y5 — which classifies the append-only journal lines, not the intents.)
>
> **Enforcement is prompt-level + native CLI flags (X3/Y4):** the read-only check is centralized as the
> **mode-check-first** step of the shared mutation helper (Task B0) — every mutating skill's first move is
> "check mode; if dry, intent-log only, return" — and, as **defense in depth**, staged mutating `td` calls also
> carry **`--dry-run`** and staged `calendar-write` also carries **`gog --readonly`/`-n`** (verified 2026-07-11)
> so a forgotten mode-check still can't mutate the outside world; the full default-deny code wrapper stays
> booked in Phase H. Genuinely read-only skills (`weather`, `calendar-read`, `forzare-today`, `daily-reflect`)
> are unaffected. **Transport = the INSTRUCTION, not env (R3A4):** a gateway-ticked job never inherits a shell
> `FORZARE_DRY_RUN=1`, so every staged prompt below OPENS with `DRY RUN — record intended writes to
> forzare/state/dryrun-intents.jsonl, perform none`.
>
> **Intents-log hygiene — truncate at the START of every staged test (R4A11).** Because positive assertions
> check that an intent RECORD *exists*, a stale record from a prior run could satisfy them falsely. So every
> staged test **truncates the intents log first** — `INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl;
> : > "$INTENTS"` — before it stages the skill, and reads only records from *this* run (scope by `run_id`
> where a single test stages more than once, e.g. B7's two-run shadow-stamp probe, which truncates once at the
> start and keeps both runs' records deliberately).
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

### Task B0: The shared mutation + ledger helper — build FIRST (B1–B11 all depend on it)

**File:** a helper module under `~/.hermes/skills/` (imported by every mutating skill), authored in the
`dot_hermes/` source. **This is the FIRST Phase-B author step (Y6/V2/W6/Y5)** — the centralized date-mutation +
lifecycle-store helper that ALL date/label/comment/calendar writers call (capture dating, lead-time,
planning-pull, snooze, roll, the §7 escalation, the if-then cue). Never a scattered ad-hoc `td` call or
`json.load`/`dump`.

- [ ] **Step 1: Verb selection by task state (W6 — verified v1.75.3):**
  - **Initially dating an UNDATED non-recurring task** → **`td task update --due <YYYY-MM-DD>`**. `td task
    reschedule` **refuses** an undated task (`Error: NO_DUE_DATE — … Use "td task update --due" to set one.`),
    so it cannot do the initial dating; an undated non-recurring task has no recurrence rule or time-of-day for
    `--due` to clobber.
  - **Re-dating an EXISTING date-only non-recurring task** (snooze / roll) → **`td task reschedule <ref>
    <YYYY-MM-DD>`** (preserves recurrence + existing time-of-day). **`td task update --due` is FORBIDDEN on an
    already-dated task** (it replaces the whole due, destroying a recurrence rule).
  - **Timed or recurring tasks are NEVER date-mutated** (`due.isRecurring == true` or a `"T"` in `due.date`).
- [ ] **Step 2: The lifecycle store is SPLIT — a prunable MAP + an append-only JOURNAL (Y5, spec §4d).**
  - **MAP — `forzare/state/task-lifecycle.json`:** `{written_due, roll_count, last_escalated, kind}` per task
    id. The helper stamps **`kind`** from the writing path (X5): snooze / planning-pull promotion ⇒
    `surfacing`; deadline lead-time ⇒ `leadtime`; `@waiting` check-back ⇒ `waiting_checkback`; user-stated
    capture date ⇒ `user_fixed`. **Only `surfacing` + `leadtime` join the roll set** (B7 Step 1);
    `waiting_checkback`/`user_fixed` are journaled but never rolled/ticked. **The MAP is pruned on a task's
    terminal state.**
  - **JOURNAL — `forzare/state/mutation-journal.jsonl`** (append-only): the helper is the single writer for
    **every** Bob mutation, appending one typed line `{ts, type, target, op, args, commit_state}` — `type ∈
    {date-op, p1, label, comment, calendar, description}` (X11/R5A11 — **`description`** is the §7/X13 if-then
    cue, which Todoist reports only as a bare `updated`, so it MUST be journaled or W7's calibration exclusion
    misreads it as a user touch). **The JOURNAL is retained 45 days (the calibration correlation window, spec
    §19), then pruned — NOT pruned on task completion** (the reducer still needs the recent journal to exclude
    Bob's writes after a task is done).
- [ ] **Step 3: I/O guarantees (V2), applied to BOTH stores:** **(a)** an exclusive `flock` on a sibling lock
  around every read-modify-write; **(b)** atomic writes — temp file in the same dir → `fsync` → `os.rename`;
  **(c)** a per-entry MAP operation record `{old_due, new_due, reconcile_date}`; **(d)** the
  **journal-then-commit** write order — journal the intent (`pending`) → perform the state-chosen Todoist write
  → commit (flip `pending`, stamp new `written_due`); **(e)** the **healing rule** — on the next run, re-verify
  any `pending` entry against the live Todoist value: equals `new_due` ⇒ commit; equals `old_due` ⇒ re-apply
  then commit; equals neither ⇒ user intervened, void (§4d divergence). **journal-then-commit + healing are
  defined for EVERY journal `type`** (Y5), the date-op sequence being the worked example. Under the dry-run
  instruction the helper **appends the intended write + journal op to `forzare/state/dryrun-intents.jsonl` and
  performs neither** (R3A1); **staged external writes also carry the native `td --dry-run` flag** (Y4).
- [ ] **Step 4: Fixtures** — crash after (c)/after the write/after commit; a concurrent **EOD roll × live
  snooze** on one task id (the lock serializes; the loser re-reads and no-ops via the same-day dedupe); an
  **undated-task initial date** (asserts `td task update --due`, not `reschedule`) vs an **already-dated
  re-date** (asserts `td task reschedule`); **one fixture per `kind` (X5):** `surfacing` + `leadtime` (both
  roll-eligible), `waiting_checkback` + `user_fixed` (both roll-EXCLUDED — asserted absent from B7 Step 1's
  roll set); **and one fixture per journal `type`** — including a **`description`** write (the if-then cue)
  asserted present in the JOURNAL so W7 exclusion stays complete (Y5/R5A11).

  **Acceptance:** the helper is authored before B1; verb selection is state-chosen; the MAP and JOURNAL are
  two distinct stores with the split lifetimes (map pruned on terminal state, journal retained 45 days);
  journal-then-commit + healing hold for every `type`; the `description` fixture lands in the JOURNAL.

---

### Task B1: `todoist-surface` — the atomic primitive (build FIRST among the skills; needs Task B0)

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
  Task B0)**, which picks the verb by task state (spec §4/W6: `td task reschedule` for an already-dated task;
  `td task update --due` only when initially dating an undated task, since `reschedule` errors `NO_DUE_DATE`
  there). **Completion beats name the shared phrasing-rotation directive** (R3A17, Phase B intro).
  **Stalled-task branch — the named if-then owner (X13, spec §7/§13).** When `todoist-surface` would surface a
  task whose ledger `roll_count ≥ 2`, it emits **this decision as the one thing** instead of the task:
  decompose / if-then / drop, no-shame, single-decision (clarify buttons on a live session, a plain question
  otherwise). A chosen **if-then is agent-proposed** — Bob composes a concrete "when `<cue>`, I `<first
  action>`" and **persists it to the task's description** via the centralized mutation layer (Task B0;
  journaled as a description/comment write, X11). Research traceability: the if-then lever is d=0.65 overall
  (Gollwitzer & Sheeran 2006) / d=0.99 self-regulation-impaired (Toli et al. 2016) — spec §6a/§7.

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
# no-clobber assertion (X4/R3A3 — SLURP the whole log with `jq -s`, NEVER `tail -1`; scope to THIS run's
# run_id when any intent exists). A clean surface of an already-groomed fixture may journal ZERO intents —
# that is fine (nothing needed writing). But if there IS a label-write for the fixture there must be EXACTLY
# ONE and its FULL set must preserve every fixture label ('deep') — a partial set is the clobber bug.
RUN_ID=$(jq -rs 'map(.run_id)|last // empty' "$INTENTS")
if [ -n "$RUN_ID" ]; then
  N=$(jq -s --arg id "$TID" --arg r "$RUN_ID" \
    '[.[] | select(.run_id==$r and .op=="task.update-labels" and .target==$id)] | length' "$INTENTS")
  if [ "$N" -gt 0 ]; then
    [ "$N" -eq 1 ] || { echo "FATAL: $N label-write intents for the fixture — expected exactly one" >&2; exit 1; }
    jq -es --arg id "$TID" --arg r "$RUN_ID" \
      'first(.[] | select(.run_id==$r and .op=="task.update-labels" and .target==$id)) | .args.labels | index("deep")' \
      "$INTENTS" >/dev/null \
      || { echo "FATAL: the label-write intent dropped 'deep' (partial-set clobber)" >&2; exit 1; }
  fi
fi
echo "label-set contract OK (≤1 targeted label-write via jq -s over run_id; 'deep' preserved when present)"
# the fixture's REAL label set is untouched by a dry-run (purity) — read via --filter/--all (R3A5: default
# --limit is 300 of ~2270 tasks, a bare list can miss it):
LBLS=$(td task list --filter "search: [TEST]" --all --json | jq -r --arg id "$TID" '.results[]|select(.id==$id)|.labels|join(",")')
[ "$LBLS" = deep ] || { echo "FATAL: dry-run mutated the fixture's real labels ($LBLS)" >&2; exit 1; }
echo "dry-run purity OK (fixture's real label set untouched)"
# clean up
hermes cron remove "$JID"
td task list --filter "search: [TEST]" --all --json | jq -r '.results[]|select(.content|startswith("[TEST]"))|.id' \
  | xargs -r -I{} td task delete {} --yes
```

Expected: the audit artifact shows **at most one** task (or `[SILENT]`); any journaled label-write intent
carries the full set including `deep` (full-set write contract, no clobber — asserted, fails loud otherwise);
the fixture's real labels are untouched (dry-run purity). Nothing reached Discord (`--deliver local`).
**Acceptance:** dry-run green; `[TEST]` tasks deleted; the cron audit shows the single decision. **X13
stalled-task fixture:** seed a ledger entry with `roll_count == 2` for a `[TEST]` task and stage a surface —
the audit shows the **single** decompose/if-then/drop decision (no-shame, one thing), and choosing if-then
journals a **`task.update-description`** intent (the op-enum name, Phase B intro/R5A13) that **persists the cue
to the task's description** — mirrored in the JOURNAL as a `type: description` line (Y5/R5A11) so W7 exclusion
stays complete — asserted from the intent record, not the real store.

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
  morning plan sees a fixed **work block**, it creates a 🤖-calendar event with a **popup reminder at
  `block_start − commute_prep_minutes − commute_travel_minutes`** (back-computed from the block start) — with
  the DECIDED constants `commute_prep_minutes: 30` + `commute_travel_minutes: 25` (X12, Task B10 config), a
  15:00 block back-computes to a **14:05** popup — **idempotent by a stable event key** (a re-fired
  brief updates, never duplicates), created **at morning-plan time**. Under dry-run this alarm write is
  journaled to `dryrun-intents.jsonl`, not performed (R3A1).
- [ ] **Step 3: Curator-pin + the TWO verification paths (R5A6 — contradiction removed).** The old acceptance
  said a `calendar-write` *dry-run* "creates a `[TEST]` block, then deletes it" — a contradiction (a dry-run
  must write NOTHING). Split into two paths:
  - **Cron-staged dry-run (intent-log purity):** under the staged cron dry-run, `calendar-write` **journals**
    the intended `calendar.create`/`calendar.update`/`calendar.delete` intent to `dryrun-intents.jsonl` and
    **performs no real `gog` write** (staged `gog` also carries `--readonly`/`-n`, Y4) — asserted from the
    intent record, the 🤖 calendar untouched.
  - **Controlled LIVE harness (a documented staging exception, R5A6 — like the forced-failure script, E1).**
    The calendar create/update/cancel/recovery behaviors are exercised by a **direct one-shot LIVE
    invocation** — `hermes -p default -z "<drive calendar-write against a [TEST]-keyed event>" --skills
    calendar-write` (NOT `--safe-mode` — it strips `skills.config`) — writing a **`[TEST]`-keyed event on the
    🤖 calendar ONLY** (Bob's own lane, safe — never the user's primary). The dispatcher is never involved, so
    timing is deterministic; the test **deletes** the `[TEST]` event at the end and **verifies the deletion**.

```bash
set -o pipefail
hermes curator pin calendar-read && hermes curator pin calendar-write
# `gog auth status` exits 0 even when the API isn't actually reachable — verify with a REAL call:
gog calendar calendars -j >/dev/null 2>&1 && echo "gog API reachable (real call OK)" \
  || echo "gog auth broken — surface the re-auth repair (spec §16): gog auth add <email>"
```

**Acceptance:** `calendar-read` returns today's anchors; the cron-staged dry-run **journals** the calendar
intent and touches the 🤖 calendar not at all (intent-log purity, Y4); the auth-expired path surfaces the
one-line repair, not silence. **The leave-time alarm (W13/X12) is tested via the controlled LIVE harness
(R5A6):** create / update (leave-time moved) / cancel (block dropped) / recovery (a re-run reuses the keyed
event, no duplicate) — all against a `[TEST]`-keyed event on the 🤖 calendar, the event key making all four
idempotent, **each asserting the EXACT computed popup timestamp** (`block_start − 30 − 25`; e.g. a 15:00 block
⇒ 14:05 — X12), and the `[TEST]` event deleted + its deletion verified at the end.

---

### Task B4: `eisenhower-plan`, `activation-prompt`, `brief-assemble`

**Files:** three SKILL.md under `~/.hermes/skills/`.

- [ ] **Step 1: `eisenhower-plan`** — the agent-side Eisenhower narrowing (spec §4c/§5), **one skill with
  THREE modes by caller** (spec §13/W10): pool → free windows → rank Q1 → Q2 against `goals.md` →
  capacity/window fit → cap at 3.
  - **`morning` ⇒ WRITES the ≤3 `p1`** — **guarded by the PER-DAY PLAN RECORD, not a p1 count (Y13, spec
    §4c/§15).** On entry read **`forzare/state/plan-of-day.json`** `{date, selected_ids[], anchor,
    writes:{p1_set, anchor_placed, alarm_set}}`; if a record for **today** exists, **resume only the missing
    writes** (each `writes.*` flag is that write's done-marker) — never re-run a completed write, never top p1
    up past the recorded `selected_ids`, and **leave a `p1` the USER set directly in Todoist untouched** (only
    ids in `selected_ids` are Bob's — the old "any p1 present" heuristic couldn't tell them apart). Then place
    the ONE protected deep block **plus the §3a leave-time alarm** (W13) via `calendar-write` if a deep window /
    work block exists ("ANCHOR, don't fill", spec §5a), setting each `writes.*` flag as it lands.
  - **`eod` ⇒ PROPOSAL only, writes NO `p1`, NO calendar** (tomorrow's ≤3 + anchor staged to
    `tomorrow-prestage.json`, confirmed next morning).
  - **`replan` ⇒ redraw the REMAINING day only** (spec §13/W10): may move Bob's 🤖-calendar proposals, may
    **PROPOSE** `p1` changes but **never silently apply** one (applying needs the user's explicit yes, INV-5),
    never touches fixed anchors; partial-day (re-plan only the hours left).
  The caller (bundle) sets the mode — this removes the old contradiction where both morning and EOD set `p1`.
  **Planning inflows live here** (A31): deadline lead-time dating + goal-matched planning-pull (bounded/
  conservative until the backlog is combed, spec §4c/U12) run inside this plan step, and **both INITIALLY date
  undated tasks — so they go through the centralized date-mutation layer (Task B0), which uses
  `td task update --due` for an undated non-recurring task** (verified: `td task reschedule` errors
  `NO_DUE_DATE` on an undated task, spec §4/W6), never a scattered ad-hoc `td` call.
- [ ] **Step 2: `activation-prompt`** — the non-negotiable morning activation line ("Breakfast first, then
  gym") + the gym-window-end backstop line ("Back from the gym?"), skipped on Thu / post-overnight recovery /
  signal-already-fired (reads the `activation` field in `schedule-override.json`, spec §8a). Rotates phrasing
  by construction (spec §7).
- [ ] **Step 3: `brief-assemble`** — compose the ordered brief (weather → calendar → ≤3 → **the single
  decision-queue HEAD item** (Y1: replaces the do-now close when present) → activation → **the one thing**),
  each step degrading visibly on failure (spec §11/§16). **Assembly only; delivery is cron-native, NOT a
  skill** (spec §11/§12). Owns (A31): the **`fs_path` re-entry resolution** on the surfaced task (spec §5e) and
  the **dopamine-menu draws** (spec §6) woven into the one-thing line. It **reads the unified
  `decision-queue.json` head** and emits ONLY that record when the queue is non-empty (never a list, R5A1).
  **Names the shared phrasing-rotation directive (R3A17, Phase B intro)** for its fixed lines — it does not
  re-derive a
  rotation. The brief is the **one bounded exception** to the one-per-response rule (spec §0/W12): read-only
  context that still ends with exactly ONE action.
- [ ] **Step 4: Curator-pin all three + staged dry-run** (Phase B intro pattern)

```bash
set -o pipefail
for s in eisenhower-plan activation-prompt brief-assemble; do hermes curator pin "$s"; done
```

**Acceptance:** `eisenhower-plan` in morning mode never assigns >3 p1 and, **guarded by `plan-of-day.json`
(Y13, NOT a p1 count)**, is a no-op when today's plan record already exists — resuming only its missing writes
and leaving a user-set p1 untouched (a re-fire never tops up past the recorded `selected_ids`); in EOD mode
writes zero p1; in replan mode redraws only the remaining day, proposes (never applies) p1 changes, and never
touches a fixed anchor (W10); `brief-assemble` yields the ordered brief and drops optional blocks under low
receptivity but always includes the anchor — and **the one-per-response check counts actionable imperatives +
questions (W12):** every non-brief response totals ≤ 1; the brief's context lines are non-actionable and it
closes on exactly ONE thing — the unified decision-queue head record when non-empty (which replaces the do-now
close), else one do-now action (§0/R5A1/Y1).

---

### Task B5: `followups-sweep`, `daily-reflect`, `tomorrow-prep`

**Files:** three SKILL.md under `~/.hermes/skills/`.

- [ ] **Step 1: `followups-sweep`** — TWO modes by caller, both over the ONE unified decision queue (Y1/R5A1,
  spec §2/§4c/§8):
  - **Brief mode (default) — the §2-step-4 delivery consolidator.** Reads the unified
    **`forzare/state/decision-queue.json`** (populated by the 02:00 reconcile + EOD + the capture pipeline +
    the monthly sweep) and emits **ONLY the single HEAD `pending` record** as the brief's one decision — never
    a list. The queue holds every class: `waiting-chase` (most-overdue first), `fixed-redecision` (do
    late / reschedule / drop), `stall-decision` (any map task at `roll_count ≥ 2` that EOD marked but did not
    message — the gentle decompose/if-then/drop, R4A10, spec §7/§8), `triage-reraise`, and `sweep-candidate`
    (X7). **The head decision REPLACES the brief's do-now close** (§0/W12). **Ack is a LIVE-only write
    (R5A5):** the live turn that receives the user's answer marks the head `acked` (cleared) so the next
    surfaces next morning — followups-sweep itself, running at brief time, only READS the head.
  - **SWEEP mode (monthly, `--deliver local`, R4A6/X7) — one PRODUCER for the unified queue.** Selects the
    **≤5 oldest/most-stale** someday candidates and **enqueues them to `forzare/state/decision-queue.json`** as
    `sweep-candidate` records (`{class:"sweep-candidate", candidate_id, proposed, status:pending}`); when the
    stale-someday set **exceeds 25** it additionally enqueues the opt-in **task-bankruptcy** offer (spec
    §4c/§19). **Bankruptcy is a REVERSIBLE UNDATE (Y3):** the confirmed batch op **strips the surfacing/lead-time
    due** from a **frozen, journaled** id set so they drop to hidden someday — **never delete/complete/archive**
    (the 2026-05-20 cascade-delete incident, STATUS:76) — with a bounded summary, a confirmation that **names
    the operation** ("undate N tasks back to someday — reversible"), and **idempotent partial-failure recovery**
    (a re-run reads the frozen journaled set, completes only ids still dated). **State-only — never messages**
    (the brief-mode read is the sole delivery). This mode owns the sweep-marking logic (R4A6).
- [ ] **Step 2: `daily-reflect`** — EOD report half: completions-as-wins (no scorecard, no misses list),
  receptivity-gated (spec §8). Gain-framed, task-level (spec §7). **Completion beats name the shared
  phrasing-rotation directive** (R3A17, Phase B intro).
- [ ] **Step 3: `tomorrow-prep`** — EOD pre-stage of tomorrow's candidate anchor + ≤3 (proposal only; spec
  §5b/§8).
- [ ] **Step 4: Curator-pin + dry-run against `[TEST]` fixtures**

```bash
for s in followups-sweep daily-reflect tomorrow-prep; do hermes curator pin "$s"; done
```

**Acceptance:** `daily-reflect` never lists misses; `followups-sweep` in brief mode emits **only the single
HEAD `pending` record** of the unified `decision-queue.json` (never a list; `waiting-chase` ordered
most-overdue first) and never itself acks (ack is the live turn's job, R5A5); in SWEEP mode it enqueues
`sweep-candidate` records and, past 25 stale candidates, the **reversible-UNDATE** bankruptcy offer with a
frozen journaled id set (never a delete, Y3); `tomorrow-prep` proposes ≤3 without setting p1 (that's the
morning's job).

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

> **Prerequisite: Task B0 (the shared mutation + ledger helper) is authored FIRST (Y6).** The centralized
> date-mutation + MAP/JOURNAL helper that `eod-roll` (and B1's snooze, B4's inflows, B11's stage-2 dating) all
> call now lives in its own leading task, **Task B0** — authored before B1 so no writer references an unbuilt
> helper. `eod-roll` below uses B0's helper for every date-write (journal→date-write→commit).

- [ ] **Step 1: `eod-roll`** — the atomic roll skill (spec §8): **compute the roll set BY THE LEDGER (V1/X5)** —
  a task rolls iff it has a lifecycle-ledger entry **whose `kind ∈ {surfacing, leadtime}`** (a
  `waiting_checkback`/`user_fixed` entry is excluded by kind, X5) AND `current due == written_due` AND it is
  date-only, today/overdue, not done; the field checks (`due.isRecurring == false`, no time-of-day, not
  future) are **secondary sanity guards within** that set, **not** the definition, and **`deadline != null` is
  NOT a blanket exclusion** — a Bob-written lead-time due on a deadline task is in the ledger and rolls (§4c/§8).
  The roll re-dates **already-dated** tasks, so it uses **`td task reschedule`** (preserves recurrence/time)
  through the Step-0 centralized helper (journal→date-write→commit); the same helper uses `td task update
  --due` only where it *initially* dates an undated task (spec §4/W6). **Unconditional `p1`-clear on every
  unfinished p1** (roll-excluded included). Tick `roll_count` (§4d; reset on progress). **Enumerate missed
  FIXED items and ENQUEUE each as a `fixed-redecision` record to the unified `decision-queue.json`** (Y1, spec
  §2/§8). **Escalation is MARKED, not messaged (R4A10):** at `roll_count == 2` stamp `last_escalated` as state
  **and ENQUEUE a `stall-decision` record to `decision-queue.json`** — EOD sends nothing at 23:00; the brief's
  `followups-sweep` delivers the head (spec §2/§7/§8).
  **Key the run off an EXPLICIT reconciliation RANGE, not a single day
  (V3/R3A9/W5/X6):** the days closed = `(last-reconcile.stored .. CEILING]`, where **CEILING is set by
  invocation mode against the Denver-local 23:00 cutoff (X6): CEILING = today at/after today's 23:00 Denver
  cutoff (the on-time 23:00 EOD or a ≤2h same-night catch-up), else CEILING = yesterday (a 5:15 defensive
  morning fire or a manual `/forzare-eod` earlier in the day)** — equivalently, CEILING = today iff the current
  Denver wall-clock is at/after today's 23:00 cutoff; roll destination = **CEILING + 1**; stamp advances to
  **CEILING**. A **multi-day outage drains the whole gap in ONE pass** and **ticks each carried task's
  `roll_count` EXACTLY ONCE** for the entire gap (an outage is Bob's failure, never multi-tick stall-shame,
  spec §0/§7). **Duplicate-fire no-op (W5):** if `stored ≥ CEILING` the roll set is empty — log an
  `already-reconciled` intent and advance nothing. So {on-time fire, ≤2h catch-up, past-grace recovery fire,
  defensive morning re-run} each reconcile a given day exactly once. Used by both `/forzare-eod` and
  (defensively) the morning brief. **Recovery test matrix (W5/X6):** missing state (fresh seed = Denver
  yesterday), same-day duplicate fire (no-op), concurrent manual+cron (lock serializes, second no-ops), outage
  < 2h (catch-up), > 2h (past-grace single fire), **≥ 3-day outage (one pass closes the whole gap, one tick per
  task)**, and the **cutoff test points (X6): 22:59 (before cutoff ⇒ CEILING = yesterday) / 23:00 (at cutoff ⇒
  CEILING = today) / just-past-midnight (Denver rolled to D+1, before D+1's cutoff ⇒ still closes D) / a ≤2h
  catch-up / a manual mid-day `/forzare-eod`** — each yields the identical, once-only roll.
  **Dating fixtures (W6/X5 — one per date-writer path × kind):**
  a **user-dated** task (no ledger entry — never moves) · a **Bob lead-time** date on a deadline task
  (`kind: leadtime` — rolls) · a **capture-dated** task (§8b stage 2 — a **user-stated day** is `kind:
  user_fixed` and **never rolls**, X5; a lead-time capture is `kind: leadtime` and rolls) · a
  **planning-pull** promotion (`kind: surfacing`, initial `update --due`, then rolls) · a **`@waiting`
  check-back** date (`kind: waiting_checkback` — **never rolls, never ticks**, X5) · a **timed** task (`"T"`
  due — never mutated) · a **recurring** task (never mutated).
- [ ] **Step 2: `waiting-reconcile`** — the 02:00 owner (spec §8): **enqueue chase-due `@waiting` as
  `waiting-chase` records to `decision-queue.json`** (Y1); repair the §4b set-time invariant (dateless
  `@waiting` → near-term check-back + "auto-repaired" flag, then enqueue a `waiting-chase`); **unblock
  detection vs `gog` calendar + `td activity` ONLY — NOT "recent Discord" (R5A12)**: an amnesiac 02:00 cron
  session has no verified read path to chat history, so it relies only on the two signals it can actually read
  (opportunistic Discord-context clearing stays a *live-turn* path, spec §8); 14-day staleness sweep enqueues a
  `waiting-chase`. **State-only — never delivers** (the morning `followups-sweep` reads the head). Run directly
  by the 02:00 cron, not in a bundle.
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
: > "$INTENTS"                                        # fresh intents log for the two-run probe
# Dry-run PURITY covers EVERY real store eod-roll could write (Y5/Y1): the stamp, the lifecycle MAP, the
# append-only JOURNAL, and the decision queue (eod-roll enqueues fixed-redecision/stall-decision records).
DRY='DRY RUN — record intended writes to forzare/state/dryrun-intents.jsonl, perform none. '
declare -A MT0
for f in last-reconcile.json task-lifecycle.json mutation-journal.jsonl decision-queue.json; do
  MT0[$f]=$(stat -f %m ~/workspaces/Ivy/forzare/state/"$f" 2>/dev/null || echo none)
done
# First dry-run: journals its intended reconcile/roll to the intents log (NOT the real store).
J1=$(stage_skill '0 0 1 1 *' "${DRY}Run eod-roll once." eod-roll test-eod-1); hermes cron remove "$J1"
# Second dry-run: must observe the first's intended stamp in the intents log and log an already-reconciled no-op.
J2=$(stage_skill '0 0 1 1 *' "${DRY}Run eod-roll once." eod-roll test-eod-2); hermes cron remove "$J2"
grep -q 'already-reconciled' "$INTENTS" \
  || { echo "FATAL: 2nd consecutive dry-run did not log an already-reconciled no-op intent" >&2; exit 1; }
echo "staging idempotency OK — 2nd dry-run no-ops via the intents log"
# Dry-run PURITY: NONE of the real stores changed (a correct dry-run touches none).
for f in last-reconcile.json task-lifecycle.json mutation-journal.jsonl decision-queue.json; do
  MT1=$(stat -f %m ~/workspaces/Ivy/forzare/state/"$f" 2>/dev/null || echo none)
  [ "${MT0[$f]}" = "$MT1" ] || { echo "FATAL: dry-run mutated real $f (${MT0[$f]} -> $MT1)" >&2; exit 1; }
done
echo "dry-run purity OK — stamp + MAP + JOURNAL + decision-queue all untouched"
```

**Acceptance:** `eod-roll` rolls only the ledger-defined set (V1), clears every unfinished p1, and **enqueues
`fixed-redecision` + `stall-decision` records to `decision-queue.json`** (Y1, not messaged); the two-dry-run
probe logs an **`already-reconciled` no-op** on the second run and leaves **every** real store
(`last-reconcile.json`, the `task-lifecycle.json` MAP, `mutation-journal.jsonl`, `decision-queue.json`)
**untouched** (dry-run purity, Y5/Y1); the real double-roll guard is verified at G1 day-1; `waiting-reconcile`
enqueues `waiting-chase` records and sends nothing (unblock signals = gog + `td activity` only, no Discord,
R5A12); `transition` produces the graduated ramp, never a hard yank.

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
- [ ] **Step 3: Task-outcome correlator + daily/weekly reducers, with forzare-write EXCLUSION (W7/X11)** — join
  a surfacing record to its later outcome (via `td activity`), then reduce to the §6a curves (time-of-day ×
  load initiation, activation-decay, receptivity, aversiveness, duration-bias, habituation). **Attribution
  rule (W7):** Bob writes to Todoist as the *same* account, so `initiatorId` cannot separate his writes from
  the user's — instead the correlator **excludes every activity event that matches a journaled forzare write**,
  reading the **append-only mutation JOURNAL `forzare/state/mutation-journal.jsonl`** (Y5/X11 — split off the
  lifecycle MAP, retained 45 days; it records every typed Bob write: `date-op`/`p1`/`label`/`comment`/
  `calendar`/**`description`**). The exclusion is complete **only because the journal records every op type past
  date-ops** (incl. the §7/X13 if-then **`description`** write, which Todoist reports as a bare `updated`) — a
  label groom, auto-repair comment, or if-then cue is journaled too, so none can read as user initiation.
  **Initiation counts only
  attribution-reliable signals:** a completion, a subtask completion, or an explicit user response — never a
  raw `updated`/`comment` event the journal authored. **Pagination (X11):** the `td activity` query is
  **cursor-paginated** — loop the cursor to exhaustion (never read only page 1), and **comment** events come
  from a **separate `--type comment` query** than completed/updated, so both must be paged and both
  cross-checked against the journal.
- [ ] **Step 4: Policy read path + retention** — the engine reads the reduced curves (not the raw log). The
  **mutation JOURNAL is retained 45 days** (Y5/§19 — the calibration correlation window, so an outcome can
  still be cross-checked against Bob's writes weeks later), then pruned; the raw calibration log keeps its own
  window and the reductions are kept.
- [ ] **Step 5: Deterministic fixtures** — including **provide-nothing records** (the control condition), the
  **W7 NEGATIVE attribution fixture**, a **>100-event history** (asserts the cursor loop pages past page 1,
  X11), and a **comment-only-progress** case (a genuine user comment the journal did NOT author counts as a
  touch, X11) — so the reducers are testable without live data.

```bash
set -o pipefail
hermes curator pin calibration-log
CAL=~/workspaces/Ivy/forzare/calibration
# Scripted round-trip: a surfacing record, a provide-nothing control, the W7 negative fixture (task Y — only
# Bob-authored activity), and the X11 comment-only-progress fixture (task Z — a genuine user comment the
# journal did NOT author). The activity_probe on Z carries >100 events to exercise the cursor loop (X11).
python3 - "$CAL/fixture-events.jsonl" <<'PY'
import json, sys
rows = [
 {"schema_version":1,"ts":"2026-07-11T09:00:00Z","context":{"day_type":"off","tod_bucket":"morning"},"action":{"task_id":"X","load_class":"deep"},"outcome":{"initiated":True,"completed":True}},
 {"schema_version":1,"ts":"2026-07-11T14:00:00Z","context":{"day_type":"off","tod_bucket":"afternoon"},"action":"provide_nothing","outcome":{}},
 {"schema_version":1,"ts":"2026-07-11T10:00:00Z","context":{"day_type":"off","tod_bucket":"morning"},"action":{"task_id":"Y","load_class":"light"},"outcome":{},"activity_probe":{"task_id":"Y","events":[{"eventType":"updated","journaled_by_forzare":True},{"eventType":"comment","journaled_by_forzare":True}]}},
]
# Task Z: >100 journaled events then, on a LATER page, ONE genuine user comment (not journaled) = progress.
z_events = [{"eventType":"updated","journaled_by_forzare":True} for _ in range(120)]
z_events.append({"eventType":"comment","journaled_by_forzare":False})   # the real user touch, page 2+
rows.append({"schema_version":1,"ts":"2026-07-11T11:00:00Z","context":{"day_type":"off","tod_bucket":"morning"},"action":{"task_id":"Z","load_class":"admin"},"outcome":{},"activity_probe":{"task_id":"Z","events":z_events}})
with open(sys.argv[1],"w") as f:
    for r in rows: f.write(json.dumps(r)+"\n")
PY
# invoke the reducer authored in this task (adjust to its real entrypoint):
python3 ~/.hermes/skills/calibration-log/reduce.py "$CAL/fixture-events.jsonl" --out "$CAL/curves.test.json"
test -s "$CAL/curves.test.json" || { echo "FATAL: reducer produced no curve file" >&2; exit 1; }
# the provide-nothing CONTROL must be counted, not dropped:
jq -e '.provide_nothing_count >= 1' "$CAL/curves.test.json" \
  || { echo "FATAL: provide-nothing records dropped by the reducer" >&2; exit 1; }
# W7 NEGATIVE fixture: task Y's only activity is Bob-authored ⇒ the reducer must score initiated=false:
jq -e '.tasks.Y.initiated == false' "$CAL/curves.test.json" \
  || { echo "FATAL: Bob-authored activity was scored as user initiation (W7)" >&2; exit 1; }
# X11 comment-only-progress: task Z's genuine user comment (on a LATER page, past the 120 journaled events)
# must score initiated=true — proving the reducer paged the cursor to exhaustion, not just page 1:
jq -e '.tasks.Z.initiated == true' "$CAL/curves.test.json" \
  || { echo "FATAL: user comment past page 1 missed — reducer didn't page the cursor (X11)" >&2; exit 1; }
echo "calibration reducer round-trip OK (curve; provide-nothing counted; Bob-writes excluded; cursor paged)"
rm -f "$CAL/fixture-events.jsonl" "$CAL/curves.test.json"
```

**Acceptance:** the scripted fixture round-trips through the reducer to a non-empty curve file; the
provide-nothing control is counted (`provide_nothing_count >= 1`, not dropped); the **W7 negative fixture
scores `initiated=false`** (only Bob-authored p1/label/due/comment events ⇒ no initiation credit); the **X11
comment-only-progress fixture scores `initiated=true`** (a genuine user comment on a later page ⇒ the reducer
paged the `td activity` cursor past page 1); the engine reads reductions, never the raw log.

---

### Task B10: `work_schedule` + schedule config (`skills.config` defaults) — BLOCKS Phase C (V11/R2A10)

**This was ruled in round 1 (A13) and never landed — it is the owning task now.** The morning brief, peak/free
windows, gym backstop, and weather all read schedule/threshold config (spec §2/§6a/§13). The values are
authored as **`metadata.hermes.config`** in the owning SKILL.md (spec §13: `metadata.hermes.config` → resolved
under **`skills.config`** in `config.yaml`), shipped via the `dot_hermes/` chezmoi source, and **verified by a
rendered-live read**. **Phase C's cron-creation task (C2) is BLOCKED until this task's rendered-value verify
passes** — a schedule-derived brief with no `work_schedule` mis-fires every morning.

> **APPLY GATE for this SECOND config edit (R5A14).** B10 writes a NEW block into `config.yaml` (the
> `skills.config` stanza) — a *second* config edit after Task A2's, made AFTER Checkpoint A already applied A2's
> config. So B10's config edit gets its own named apply + fail-closed gate: **`config.yaml` is included in
> Apply Checkpoint B's file list** (the new Phase-B checkpoint, Y6), so C2 cannot force-run a schedule-derived
> brief until B10's `skills.config` is applied AND gated live. The rendered-live read (Step 3) is the value
> check; Checkpoint B is the apply gate.

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
  - **commute constants (X12, spec §3a/§19)** = `commute_prep_minutes: 30` + `commute_travel_minutes: 25`
    (hand-editable; the §3a/W13 leave-time alarm fires at `work_block_start − prep − travel`).
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
for k in ("work_schedule", "gym_schedule", "wake_anchor", "weather_thresholds",
          "commute_prep_minutes", "commute_travel_minutes"):
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
# X12: assert the DECIDED commute constants (drive the §3a leave-time alarm timestamp):
assert sc["commute_prep_minutes"] == 30,   f"commute_prep_minutes {sc['commute_prep_minutes']} != 30"
assert sc["commute_travel_minutes"] == 25, f"commute_travel_minutes {sc['commute_travel_minutes']} != 25"
print("commute constants OK: prep=30 travel=25 (leave-time alarm = block_start - 55 min)")
PY
```

**Acceptance:** `skills.config` carries `work_schedule` (with the alt-Sunday anchor `2026-06-07=ON`),
`gym_schedule` (rest=Thu), `wake_anchor=05:15`, **the four weather-threshold values asserted individually
(wind>17 / any rain / <50°F / >90°F — R3A14)**, and **`commute_prep_minutes: 30` + `commute_travel_minutes: 25`
(X12)** — each from the **resolved live config**, not the source template. Only then may Task C2 create the
schedule-derived cron jobs.

- [ ] **Step 4: The "schedule changed ⇒ re-run reconcile" rule + assertion (Y9).** Because C2 DERIVES the
  block-boundary and gym-window-end cron trigger times from this resolved `work_schedule`/`gym_schedule` at
  install (Y9, C2 Step 1), a later hand-edit of `work_schedule` would leave those cron times **stale**. So this
  task documents the **reconcile procedure**: after ANY edit to `work_schedule` or `gym_schedule`, **re-run the
  C2 boundary/gym derivation** (reconcile-by-name, Y8) to re-compute and update the affected cron trigger times
  — never leave them pinned to the old schedule. **Test:** change `work_schedule` (e.g. shift the block start
  by an hour), re-run the reconcile, and assert the boundary/gym cron trigger times moved to match the new
  derived values. Record this rule in the schedule-owner SKILL.md so a future schedule edit doesn't silently
  desync the crons.

**Acceptance (Step 4):** the schedule-change reconcile rule is documented + tested (edit schedule → re-derive
→ assert new trigger times); C2's derived boundary/gym times always track the resolved `work_schedule`.

---

### Task B11: `forzare-capture-pipeline` skill — the staged capture logic (authored in Phase B, R5A4/Y6)

**File:** `~/.hermes/skills/forzare-capture-pipeline/SKILL.md` (curator-pinned). **Authored HERE in Phase B**
(moved from D1) so it is applied at Checkpoint B alongside the other skills and every card attaches this one
pinned skill via `--skill` (W4). **Phase D (Task D1) keeps only the board CONFIG + the card
lifecycle/idempotency/harness TESTS** (R5A4) — it no longer authors the skill.

- [ ] **Step 1: Author the 5-stage pipeline logic** (spec §8b): **Place** (parent, sync `td task add` to Inbox
  — NO `quickadd`, so no date parsed pre-classification) → **Decide-placement** (task-vs-event pre-check + 4
  routing cases) → **Verify+research-decision** → **Research** → **Split**, each gating the next. Every stage-2
  date-write goes through the **centralized helper (Task B0, W6/X5)** — a user-stated day is `kind: user_fixed`
  (never rolls), a hard time bound is `deadline` + a `kind: leadtime` surfacing due (rolls).
- [ ] **Step 2: The kickoff is create → specify, BOTH by the parent — NO `notify-subscribe` (Y2, verified
  `hermes kanban --help`: `notify-subscribe` routes TERMINAL events only, onto the home channel — a firewall
  breach + dispatch race, so it is DELETED).** The parent issues, before the dispatcher can touch the card:
  1. **`hermes kanban create "<title>" --triage --idempotency-key <inbox-task-id> --assignee default
     --max-runtime 900 --skill forzare-capture-pipeline`** — titled `--triage` card (title is a required
     positional); **`--max-runtime 900` (Y7, verified `hermes kanban create --help`)**; the idempotency key is
     the stage-1 Inbox task id (a retry / no-resume restart re-derives the same card).
  2. **`hermes kanban specify <task_id>`** — concretizes via `auxiliary.triage_specifier` **and** performs the
     `triage → todo` transition that permits dispatch (X2). No third call.
- [ ] **Step 3: Idempotent dup-guards** (no mid-run resume): stage 1 skips if already in Inbox; stage 2 skips
  re-routing a placed task + a duplicate 🤖-calendar event; stage 5 skips existing subtasks — a restart
  converges to one task.
- [ ] **Step 4: Awaiting-user + failures WITHOUT a card subscription (Y2/Y1/R5A7).** When a stage needs the
  user (cases 3–4), the card **blocks awaiting-user** and the pipeline **enqueues a `triage-reraise` record to
  the unified `decision-queue.json`** (state-only, no message) — the brief delivers it as its head item and any
  live turn re-raises it opportunistically; on the answer the live turn writes it onto the card + `hermes
  kanban unblock`s it (resuming the dispatcher) and marks the queue record `acked` (R5A5). **Pipeline FAILURES**
  (crashed / timed-out / gave-up) reach `#forzare-errors` via the **forzare-ops watchdog (F1)**, never a card
  subscription — so **no user-facing message issues before Phase G go-live** (R5A7).
- [ ] **Step 5: Curator-pin** `forzare-capture-pipeline` (the live pipeline execution tests are D1's
  controlled harness; this task only authors + pins the skill, applied at Checkpoint B).

**Acceptance:** the pipeline skill is authored + pinned in Phase B (applied at Checkpoint B); the kickoff is
create + specify only (no `notify-subscribe`, Y2); cards carry `--max-runtime 900` (Y7); awaiting-user enqueues
a `triage-reraise` decision-queue record and failures route via the watchdog — no card subscription anywhere.

---

### APPLY CHECKPOINT B (inline, fail-closed — Y6/R5A3) — do this BEFORE any Phase-B staged dry-run or Phase C

**NEW checkpoint (the topological fix, Y6/R5A3).** Every Phase-B source authored above (the Task B0 helper,
B1–B10 skills, the B11 capture-pipeline skill, the B10 `skills.config`) must be **applied to live** before ANY
staged dry-run runs against it — a staged dry-run reads the LIVE `~/.hermes/skills/<name>/SKILL.md`, which
only exists after `chezmoi apply`. So the three-stage flow is: **author-all (above) → THIS Checkpoint B →
pin + staged dry-run (each task's verify step) + Phase C.** No staged run below Checkpoint B may execute until
it is CLEARED.

- **User-run/agent-run:** apply the skills + capture-pipeline skill sources (agent-runnable plaintext) + the
  KeePassXC-gated `config.yaml` re-apply for B10's `skills.config` stanza (user-run, R5A14); then the **boot
  skill-existence assertion (C1 Step 2)** must pass (every skill the bundles will name is installed + pinned).
- **Gate (fail-closed):** `source ~/workspaces/Ivy/forzare/gate-check.sh` (the persistent Phase-A script, X10)
  and run `gate_check` with this checkpoint's **explicit FILE list (W3 — `chezmoi diff` on a directory is
  NON-recursive):** **every** `~/.hermes/skills/<name>/SKILL.md` authored in Phase B (incl.
  `forzare-capture-pipeline`) **+ `~/.hermes/config.yaml`** (the B10 `skills.config` edit, R5A14). It must
  print `checkpoint CLEARED`; any pending diff / non-zero exit / stderr blocks every Phase-B staged run and
  Phase C.

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

- [ ] **Step 4: Bundle-slash-mirror check — the OWNING task for the §12.5 question (R4A14).** This decision
  was left in Self-Review "carried forward" limbo; own it here. After the bundles are applied, **perform the
  check, record the result, and decide the micro-shim yes/no** per spec §12.5:
  - **Check:** do `/forzare-*` **bundle/skill** invocations mirror to **native Discord slash commands**?
    Hermes mirrors **plugin-registered** commands (`plugins/platforms/discord/adapter.py:3625-3733`) while
    **skills consolidate under a single `/skill` group** — so the expectation is **no** native `/forzare-*`
    autocomplete by default. Confirm against the live gateway (list the registered Discord app commands; look
    for `/forzare-*`).
  - **Record:** write the observed result (mirrors: yes/no) into the build notes.
  - **Decide:** if they do NOT mirror **and** recognition-over-recall demonstrably suffers, build the
    documented **micro-shim plugin** that registers command **names only** (no delivery, no hooks, no lock,
    spec §12.5). **Default: no shim** — description-driven + plain-language activation is the baseline.

**Acceptance (Step 4):** the mirror check is run, its result recorded, and the shim decision made — not
deferred. Default outcome is "no shim."

---

### APPLY CHECKPOINT C (inline, fail-closed — R2A2/W3/R5A3) — RE-SCOPED to what Phase C authors (the bundles)

**RE-SCOPED (R5A3/Y6).** The Phase-B **skills** (incl. `forzare-capture-pipeline`) + the B10 `skills.config`
are already applied + gated at **Apply Checkpoint B**; this checkpoint therefore covers **only what Phase C
authors — the three `skill-bundles/*.yaml`** — before C2 force-runs the brief bundle and before Phase D
exercises the pipeline. **All of Task C2 and Phase D are gated on "Checkpoint C cleared"** — it sits *before*
its dependent staged command (W3).

- **User-run/agent-run:** apply the three bundle sources; then the **boot skill-existence assertion (C1 Step
  2)** must pass (every bundle-named skill installed + pinned — those skills are already live from Checkpoint
  B, so this is now a *re-confirm*, else fail loud).
- **Gate (fail-closed):** `source ~/workspaces/Ivy/forzare/gate-check.sh` (the persistent Phase-A script, X10)
  and run `gate_check` with this checkpoint's **explicit FILE list (W3 — `chezmoi diff` on a directory is
  NON-recursive):** the three `~/.hermes/skill-bundles/forzare-*.yaml`. (The skills + `config.yaml` were gated
  at Checkpoint B — not re-gated here.) It must print `checkpoint CLEARED`; any pending diff / non-zero exit /
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

**Transactional install (Y8) + schedule-DERIVED boundary/gym times (Y9).** The install is one atomic
transaction: `set -euo pipefail`, a **declared name manifest**, **reconcile by NAME** (edit an existing job of
that name, create a missing one — **never blind-create a duplicate**), and **rollback on partial failure**
(delete the ids this attempt created). The gym-window-end and block-boundary trigger times are **DERIVED from
the resolved `work_schedule`/`gym_schedule`** (B10, live) — not hardcoded — so a schedule edit re-derives them
(B10 Step 4/Y9).

```bash
set -euo pipefail
# X10: back up jobs.json HERE — immediately before the FIRST cron mutation (moved out of Task E2, which now
# only declares the live-data exception + documents the rollback). jobs.json is NOT chezmoi-managed.
cp ~/.hermes/cron/jobs.json \
  ~/workspaces/backups/"$(date -u +%Y-%m-%dT%H-%M-%S).hermes-cron-jobs.backup.json"

# Y9: DERIVE the gym-window-end + block-boundary cron specs from the RESOLVED work_schedule/gym_schedule
# (B10's live skills.config). Hardcoding them would desync on any schedule edit (Y9 reconcile rule, B10 Step 4).
# NOTE: each cron spec itself contains spaces, so python emits them PIPE-separated and bash splits on '|'
# (a plain `read a b` on space-separated specs would mis-split the fields).
IFS='|' read -r GYM_CRON BOUND_CRON < <(~/.hermes/hermes-agent/venv/bin/python - <<'PY'
import os, yaml
sc = (yaml.safe_load(open(os.path.expanduser("~/.hermes/config.yaml"))).get("skills",{}) or {}).get("config",{}) or {}
gym  = sc["gym_schedule"]; work = sc["work_schedule"]
# gym-window-end minute-of-day → the "Back from the gym?" backstop; block-boundary → the work-block START edge.
gh, gm = map(int, str(gym["window_end"]).split(":"))       # e.g. "09:00"
wh, wm = map(int, str(work["block_start"]).split(":"))     # e.g. "15:00"
print(f"{gm} {gh} * * *|{wm} {wh} * * *")                  # two 5-field Denver-local cron specs, '|'-separated
PY
)
{ [ -n "${GYM_CRON:-}" ] && [ -n "${BOUND_CRON:-}" ]; } || { echo "FATAL: could not derive gym/boundary cron from work_schedule (Y9)" >&2; exit 1; }

# jid_from_create from the Phase B intro. The DRY-RUN directive rides in the PROMPT (instruction transport,
# R3A4 — a gateway-ticked job inherits no shell env); G1 edits these jobs WITHOUT it.
DRY='DRY RUN — record intended writes to forzare/state/dryrun-intents.jsonl, perform none.'
# Y8: declared name MANIFEST (the exact six) + spec = "schedule|skill|prompt" per name.
declare -A SPEC=(
  [forzare-morning-brief]="15 5 * * *|forzare-morning-brief|$DRY"
  [forzare-eod]="0 23 * * *|forzare-eod|$DRY"
  [forzare-waiting-reconcile]="0 2 * * *|waiting-reconcile|$DRY"
  [forzare-gym-window-end]="$GYM_CRON|activation-prompt|$DRY Fire the gym-window-end backstop."
  [forzare-block-boundary]="$BOUND_CRON|transition|$DRY Fire the block-boundary prompt."
  [forzare-someday-sweep]="0 5 1 * *|followups-sweep|$DRY Run the monthly someday-sweep (state-only)."
)
MANIFEST=(forzare-morning-brief forzare-eod forzare-waiting-reconcile forzare-gym-window-end forzare-block-boundary forzare-someday-sweep)
created_ids=()
rollback(){ echo "FATAL: partial cron install — rolling back THIS attempt's created jobs" >&2; \
  for id in "${created_ids[@]:-}"; do if [ -n "$id" ]; then hermes cron remove "$id" >/dev/null 2>&1 || true; fi; done; exit 1; }
trap rollback ERR
existing_id(){ jq -r --arg n "$1" '.jobs[]|select(.name==$n)|.id' ~/.hermes/cron/jobs.json | head -1; }
for name in "${MANIFEST[@]}"; do
  IFS='|' read -r sched skill prompt <<<"${SPEC[$name]}"
  eid=$(existing_id "$name")
  if [ -n "$eid" ]; then
    # reconcile-by-name: UPDATE the existing job in place — never blind-create a duplicate (Y8)
    hermes cron edit "$eid" --schedule "$sched" --skill "$skill" --prompt "$prompt" --deliver local >/dev/null
    echo "reconciled $name ($eid)"
  else
    nid=$(hermes cron create "$sched" "$prompt" --skill "$skill" --deliver local --name "$name" | jid_from_create)
    created_ids+=("$nid"); echo "created $name ($nid)"
  fi
done
trap - ERR
# capture ids by name for Step 2's per-job reads:
for name in "${MANIFEST[@]}"; do printf '%s=%s\n' "$name" "$(existing_id "$name")"; done
BRIEF=$(existing_id forzare-morning-brief); EOD=$(existing_id forzare-eod); RECON=$(existing_id forzare-waiting-reconcile)
```

  - **Morning brief — ONE daily job, `15 5 * * *` (fires EVERY day; Sunday is DECIDED, spec §1/§2/U15)**,
    **`--skill forzare-morning-brief`** (the bundle — W1), `--deliver local` (flips to `discord` home channel
    at G1). The alternating-Sunday ON/OFF distinction is **content**, derived from the `work_schedule` read
    inside the bundle (anchor Jun 7 = ON) — **not** a separate job and **not** a build-time question.
  - **End-of-day** `0 23 * * *`, **`--skill forzare-eod`** (the bundle — W1), idempotent + **keyed off the
    EXPLICIT reconciliation range** (`(stored .. CEILING]`, CEILING = today at/after the 23:00 Denver cutoff
    else yesterday, spec §8/V3/R3A9/W5/X6), not
    the wall clock. **Created `--deliver local` + the dry-run directive (or paused via `hermes cron pause`)
    until go-live** — it must not reschedule real tasks during staging; **G1 removes the directive and
    resumes it (W2)**.
  - **`@waiting` reconcile** `0 2 * * *`, **`--skill waiting-reconcile`** (the atomic skill directly — not a
    bundle, Task B7) — **state-only, NEVER messages the user** (spec §8); `--deliver local` **permanently**.
  - **Gym-window-end check** at the **DERIVED gym-window end (Y9 — `$GYM_CRON`, from the resolved
    `gym_schedule`, not hardcoded)**, **`--skill activation-prompt`** — the "Back from the gym?" backstop,
    skipped on Thu / recovery mornings / signal-already-fired (reads `schedule-override.json` `activation`, spec
    §3/§8a); `--deliver local` until G1.
  - **Block-boundary prompts** at the **DERIVED work-block edge (Y9 — `$BOUND_CRON`, from the resolved
    `work_schedule`)**, **`--skill transition`** (spec §3/§5), `--deliver local` until G1. A schedule edit
    re-derives both times (B10 Step 4 reconcile rule).
  - **Monthly someday-sweep — one PRODUCER for the unified decision queue, delivered head-at-a-time via the
    brief (R2A20/X7/Y1).** A monthly cron (`0 5 1 * *`, brief-time on the 1st), **`--skill followups-sweep`**
    run in **SWEEP mode**, **state-only** (`--deliver local` permanently), **enqueues ≤5 oldest/most-stale
    someday candidates to the unified `forzare/state/decision-queue.json`** as `sweep-candidate` records
    (`{class:"sweep-candidate", candidate_id, proposed, status:pending}`, spec §8a/X7). The brief's
    `followups-sweep` then emits **only the single HEAD `pending` record** as its one decision each morning
    (never a batch) — **no second message**. Past the **DECIDED threshold of > 25** stale-someday candidates
    (R2A16, spec §4c/§19) it enqueues the opt-in **reversible-UNDATE** task-bankruptcy offer (Y3).
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
# R4A11: truncate the intents log at the START of this staged test so positive assertions can't be satisfied
# by a stale record from an earlier run.
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
: > "$INTENTS"
# Force one staged brief run and read the audit BY its job id:
WINDOW_START=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
hermes cron run "$BRIEF" >/dev/null && hermes cron tick >/dev/null
AUDIT=~/.hermes/cron/output/"$BRIEF"
ls "$AUDIT"/*.md >/dev/null 2>&1 || { echo "FATAL: no brief audit at $AUDIT" >&2; exit 1; }
# (b) W1/R5A8: assert per-skill OBSERVABLE EFFECTS from the INTENT LOG — NEVER a grep over the audit .md.
# Verified 2026-07-11: the cron audit records only the ## Prompt (which EMBEDS each loaded skill's instruction
# text, so a skill NAME appears there whether or not the skill did anything) + the ## Response; it records NO
# tool calls (save_job_output writes prompt+response only). So grepping skill names over the audit proves the
# bundle LOADED them, not that they RAN — the exact "prompt-text grep" R5A8 forbids. Instead, each MUTATING
# member journals its intents with a `skill` field: assert an intent-log effect per reliably-acting mutating
# member. (Read-only members — weather/calendar-read/activation-prompt — and conditional writers —
# calendar-write only on a deep-window/work day, followups-sweep brief-mode only READS, brief-assemble
# prestage-clear only if a prestage exists — produce no deterministic intent here; each is asserted in its own
# task: B2/B3/B4/B5/B7. This bundle test seeds a [TEST] fixture task that FORCES grooming + planning so the
# three always-acting writers below journal.)
[ -s "$INTENTS" ] || { echo "FATAL: staged brief logged NO intents — the loaded skills never RAN (W1/R5A8)" >&2; exit 1; }
RUN_ID=$(jq -rs 'map(.run_id)|last // empty' "$INTENTS")
for s in eod-roll todoist-surface eisenhower-plan; do
  N=$(jq -s --arg s "$s" --arg r "$RUN_ID" '[.[]|select(.run_id==$r and .skill==$s)]|length' "$INTENTS")
  [ "$N" -gt 0 ] || { echo "FATAL: no intent-log EFFECT for bundle skill '$s' — it did not run (W1/R5A8)" >&2; exit 1; }
done
echo "staged brief: each always-acting mutating bundle skill produced an intent-log effect (W1/R5A8 — effects, not a prompt grep)"
# (c) R3A2/R4A12: ZERO FORZARE mutations in the staged window — SCOPED to Bob-authored targets. A dry-run
# performs no real Todoist write, but the USER may edit Todoist freely during staging; so instead of
# FATAL-on-any-event, cross-check real activity against the dry-run INTENTS: a real mutation whose objectId
# matches a journaled intent target is a LEAK (the dry-run performed a write it should only have journaled);
# events on OTHER tasks are the user's and ignored. Verified shapes: camelCase eventType/objectId; the event
# id is float-mangled ("2.15…e+36") so never key on .id. stderr captured; ANY read error is FATAL.
[ -s "$INTENTS" ] || { echo "FATAL: staged run logged no intents — dry-run directive not honored (R3A1)" >&2; exit 1; }
TARGETS_JSON=$(jq -rs 'map(.target)|unique' "$INTENTS")   # the task ids the run INTENDED to write
ACT_ERR=$(mktemp)
LEAKS=$(td activity --since "$(date -u +%Y-%m-%d)" --type task --json 2>"$ACT_ERR" \
  | jq -r --arg w "$WINDOW_START" --argjson t "$TARGETS_JSON" \
      '[.results[] | select(.eventDate >= $w)
        | select(.eventType=="updated" or .eventType=="added" or .eventType=="deleted" or .eventType=="completed")
        | select(.objectId as $o | $t | index($o))] | length')
if [ -s "$ACT_ERR" ]; then echo "FATAL: td activity errored:" >&2; cat "$ACT_ERR" >&2; rm -f "$ACT_ERR"; exit 1; fi
rm -f "$ACT_ERR"
[ "$LEAKS" = 0 ] || { echo "FATAL: $LEAKS real mutation(s) on a Bob-intended target — dry-run leaked (R3A2/R4A12)" >&2; exit 1; }
echo "staged window: 0 Bob-authored mutations (user edits ignored), $(jq -rs length "$INTENTS") intent record(s) (gate green)"
```

Expected: all jobs listed at the right times/TZ; every user-facing job is `--deliver local` + `--skill`
during the build; the forced run writes `~/.hermes/cron/output/<job_id>/` and does NOT message Discord.
**Acceptance:** the install is **transactional (Y8)** — the exact **six-name manifest** reconciles
by name (edit existing / create missing / never blind-create; rollback deletes this attempt's created ids on
partial failure), and the gym-window-end + block-boundary times are **DERIVED from the resolved
`work_schedule`/`gym_schedule` (Y9)**, not hardcoded; each job carries its persisted `skills` value (W1); the
staged brief's **per-skill EFFECTS are asserted from the intent log — NOT a grep over the audit prompt (R5A8)**
— every always-acting mutating member (`eod-roll`/`todoist-surface`/`eisenhower-plan`) produced an intent
record keyed by `skill`+`run_id`; the 02:00 reconcile and monthly sweep have no user-facing delivery; the
staged window shows zero **Bob-authored** Todoist mutation events (scoped to the dry-run intent targets, so the
user may edit Todoist freely during staging — string-safe `eventType`/`objectId` reads, fail-loud) while the
intents log carries the computed writes (R3A1/R3A2/R4A12). The **exact six-name manifest + count** is
re-asserted at go-live (G1, Y8).

---

## Phase D — Capture pipeline (Kanban, private)

### Task D1: Private Kanban board CONFIG + card lifecycle/idempotency tests (skill authored in B11, R5A4)

**Store:** `~/.hermes/kanban.db` (Bob-private, firewalled from the user, spec §9). Assignee = the **`default`
profile** (persona "Bob"; no profile named `bob`); `default_assignee: "default"`, `auto_decompose: false`,
`max_in_progress_per_profile: 2`, `failure_limit: 2` (Task A2 / spec §14). **Preflight:**
`hermes profile show default` must succeed. **The `forzare-capture-pipeline` skill is authored + pinned in
Task B11 (Phase B, R5A4/Y6)** — D1 owns only the **board config** and the **card lifecycle / idempotency /
controlled-harness tests** below.

- [ ] **Step 1: Confirm the kickoff contract from B11 (create → specify, NO notify-subscribe — Y2).** The
  kickoff is authored in B11: parent Bob does **stage 1 synchronously** (structured `td task add` to Inbox —
  NOT `quickadd`, spec §8b/U4; instant ack, idempotent), then **two ordered `hermes kanban` calls, before the
  dispatcher can touch the card:**
  1. **`hermes kanban create "<title>" --triage --idempotency-key <inbox-task-id> --assignee default
     --max-runtime 900 --skill forzare-capture-pipeline`** (title is a **required** positional — verified; the
     idempotency key is the stage-1 Inbox TASK ID; **`--max-runtime 900`**, Y7; `--skill` pins B11's stage
     logic). A `--triage` card is **not dispatchable** — it parks in the triage column.
  2. **`hermes kanban specify <task_id>` — IMMEDIATELY, by the parent (X2).** `specify` concretizes via
     `auxiliary.triage_specifier` (Task A2) **and performs the `triage → todo` transition that PERMITS
     dispatch** (verified `specify_triage_task`, `kanban_db.py:4574`). Stage 2 (the subagent) begins only at
     dispatch, never inside triage, and never itself calls specify.
  **There is NO third call — the `notify-subscribe` callback design is DELETED (Y2, verified `hermes kanban
  --help`: `notify-subscribe` routes TERMINAL events only, onto the home channel — a firewall breach + dispatch
  race).** Awaiting-user cards re-raise via the unified decision queue (`triage-reraise`, B11 Step 4/Y1) and
  failures reach `#forzare-errors` via the watchdog (F1); no card subscription. Any stage-2 date-write goes
  through the centralized date-mutation layer (Task B0, W6). The 5 stages: Place → Decide-placement
  (task-vs-event pre-check + 4 routing cases; the card is already `todo`/`ready` here) → Verify+research-decision
  → Research → Split, each gating the next (spec §8b).
- [ ] **Step 2: Idempotent dup-guards (forced by no-mid-run-resume, spec §8a/§8b/§19).** Every stage is
  check-before-create: stage 1 skips if the capture is already in Inbox; stage 2 skips re-routing a placed
  task and skips a duplicate 🤖-calendar event; stage 5 skips existing subtasks. A restart converges to one
  task, never a dup.
- [ ] **Step 3: Never auto-create a project** (case 4 asks inline; spec §8b). Cases 3–4 route asks through
  a **live Discord-bound turn / the parent conversation** (cron/subagent turns have no session for buttons,
  spec §12.1c); the card blocks awaiting-user and enqueues a `triage-reraise` record (B11 Step 4/Y1).
- [ ] **Step 4: Failures land as CARD/EVENT state — the errors-channel ROUTE test lives in F1 (R5A7).** A
  stage error / un-completable card records a genuine failure **event** (`gave_up` / `crashed` / `timed_out`)
  and the card goes terminal; the captured item is safe (stage 1 persisted it). **D1 asserts only the
  card/event state** (a failure event is recorded, the card is terminal) — it does **NOT** drive the
  `hermes send --to discord:<#forzare-errors>` route here, because that would create a forward dependency on
  the watchdog (F1) and could emit a user-visible message before Phase G. **The end-to-end errors-channel
  route test MOVES to Task F1** (which owns the watchdog + spool), where a seeded `gave_up`/`crashed`/
  `timed_out` event is asserted to reach `#forzare-errors`. (A bare `blocked` status is healthy — an
  awaiting-user block is never an alert, W9/R4A4.)
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
records a genuine failure **event** and the card goes terminal (the errors-channel ROUTE test is F1's, R5A7;
its cron/kanban audit read keyed by the job/card id, never latest-mtime). (Inbox-write correctness for a
capture is covered in Task B8; stage-2 dated placement is asserted in Step 6.)

- [ ] **Step 6: Controlled-execution harness — exercise the stages by DIRECT `hermes -p default` invocation
  (R4A5).** The non-spawnable assignee keeps the *dispatcher* inert (Step 5), which means the stage logic never
  runs on its own — so the stage-level acceptances (dated placement, crash-restart idempotency, forced-error →
  errors-spool) need a **deterministic** driver. Run the pipeline skill **directly against the probe card**
  with a one-shot profile invocation — the dispatcher is never involved, so timing is deterministic:
  **`hermes -p default -z "<prompt: run forzare-capture-pipeline against card <ID1> from stage 2>" --skills
  forzare-capture-pipeline`**. **Do NOT pass `--safe-mode`** — it strips `skills.config`/plugins the pipeline
  needs (Global Constraints); a plain one-shot keeps the real config. Under this harness, drive and assert:
  - **Dated placement (stage 2, W6/X5):** a `[TEST]` capture with a user-stated day places a **date-only**
    due via the centralized layer with `kind: user_fixed` (a hard-time-bound capture → `deadline` + a
    `kind: leadtime` surfacing due); assert the written due + kind, then clean up.
  - **Simulated crash-restart (no mid-run resume, §8b):** invoke the harness twice against the same card (the
    second simulating a restart from stage 1) — it **converges to exactly one** Todoist task / one 🤖-calendar
    event (check-before-create dup-guards hold), never a duplicate.
  - **Forced error → CARD/EVENT state only (R5A7 — the route test is F1's).** Force a stage to error (e.g. an
    unreachable dependency) and confirm the card records a genuine failure **event** (`gave_up`/`crashed`/
    `timed_out`) and goes terminal, the captured item still safe in Inbox. **Do NOT assert the
    `#forzare-errors` delivery here** — that end-to-end route (seeded event → `hermes send` → spool) is
    asserted in **Task F1** (which owns the watchdog + spool), avoiding a forward dependency and any
    user-visible message before Phase G.

  **Acceptance (Step 6):** the direct-invocation harness drives stages 2–5 deterministically (dispatcher never
  involved); dated placement writes the correct due + `kind`; a re-run yields exactly one task/event; a forced
  error records a failure event + terminal card (the errors-channel route is F1's, R5A7) — all with the
  non-spawnable assignee keeping the live dispatcher inert.

---

### CHECKPOINT D (inline, fail-closed — Y6, post-Phase-D) — do this BEFORE Phase E/F/G

**NEW post-D checkpoint (Y6).** Phase D authors no new chezmoi file (the board config is the `config.yaml`
`kanban.*` stanza already applied + gated at Checkpoint A; the pipeline skill is B11, live from Checkpoint B),
so this is a **verify gate**, not an apply: confirm the capture pipeline is exercised end-to-end and the board
config is live before delivery/watchdog phases build on it.

- **Verify (fail-closed):** the D1 **controlled-execution harness (Step 6) passed** — dated placement writes
  the right `kind`, a re-run converges to one task/event, and a forced error records a terminal failure event
  (the errors-channel route is F1's, R5A7). AND the live `config.yaml` `kanban.*` stanza resolves as expected
  (`default_assignee: "default"`, `auto_decompose: false`, `max_in_progress_per_profile: 2`, `failure_limit:
  2`) via a resolved read:

```bash
set -o pipefail
~/.hermes/hermes-agent/venv/bin/python - <<'PY'
import os, yaml
k = (yaml.safe_load(open(os.path.expanduser("~/.hermes/config.yaml"))).get("kanban",{}) or {})
assert k.get("default_assignee") == "default", k.get("default_assignee")
assert k.get("auto_decompose") is False, k.get("auto_decompose")
assert k.get("max_in_progress_per_profile") == 2, k.get("max_in_progress_per_profile")
assert k.get("failure_limit") == 2, k.get("failure_limit")
print("kanban board config live + correct (Checkpoint D)")
PY
```

  Any harness failure or a wrong `kanban.*` value **blocks Phase E/F/G** — never a silent pass.

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

**The deploy checkpoints now live INLINE at their phase ends (R2A2), not here — and the reorder (Y6) added
two:** **Checkpoint A** (config/`.env`/`SOUL.md`) at the end of Phase A; **NEW Checkpoint B** (ALL Phase-B
skills incl. `forzare-capture-pipeline` + the B10 `config.yaml` `skills.config`) at the end of the Phase-B
author stage, BEFORE any Phase-B staged dry-run (Y6/R5A3/R5A14); **Checkpoint C** RE-SCOPED to **the three
bundles only** (the skills are live from Checkpoint B) **before Task C2** (its dependent staged command, W3);
**NEW Checkpoint D** (a post-Phase-D verify gate — the pipeline harness passed + the `kanban.*` config resolves,
Y6); **Checkpoint F** (watchdog) at the end of Phase F. Each phase's live-path verify is explicitly gated on
its checkpoint being CLEARED, so the build never proceeds on un-applied config. **This task owns only** (a) a
reference to the shared **fail-closed gate check** (its DEFINITION moved to Task A1 Step 5 — the persistent
`~/workspaces/Ivy/forzare/gate-check.sh` script — so it exists BEFORE Checkpoint A, X10) and (b) the
`jobs.json` live-data exception. The `session_reset`/config-drift resolution is owned by Task E1 (Phase E) and
Task A2 — not duplicated here.

- [ ] **The shared gate check — authored in Task A1 Step 5, referenced here (X10).** Because Checkpoint A runs
  at the *end of Phase A*, the `gate_check` function must exist before then — so its definition lives in the
  persistent Phase-A script `~/workspaces/Ivy/forzare/gate-check.sh` (Task A1 Step 5), and every checkpoint
  `source`s it. It is **SIGPIPE-safe + error-loud + FILE-TARGETED:** `chezmoi diff` on a DIRECTORY target is
  **NON-recursive** (W3, verified chezmoi v2.70.5 in an isolated fixture — a differing managed file *under* the
  directory produced NO diff on the dir target; the explicit file target showed the full diff), so the gate
  never diffs `~/.hermes` as a directory. Each checkpoint passes its **explicit FILE list** and the gate loops
  file-by-file, capturing stderr per file (warnings surfaced, never fatal-by-accident silence), fail-closed on
  any nonzero exit or pending diff. The per-checkpoint file lists:
  - **Checkpoint A:** `~/.hermes/config.yaml ~/.hermes/.env ~/.hermes/SOUL.md` (+ the Y10 channel gate)
  - **Checkpoint B (NEW, Y6):** every `~/.hermes/skills/<name>/SKILL.md` authored in Phase B (incl.
    `forzare-capture-pipeline`) + `~/.hermes/config.yaml` (the B10 `skills.config` edit, R5A14)
  - **Checkpoint C (RE-SCOPED, R5A3):** the three `~/.hermes/skill-bundles/forzare-*.yaml` only (skills gated
    at Checkpoint B)
  - **Checkpoint D (NEW, Y6):** a post-Phase-D *verify* gate (no chezmoi file) — the D1 harness passed + the
    live `kanban.*` config resolves correct
  - **Checkpoint F:** `~/.local/bin/forzare-ops-watchdog.sh ~/Library/LaunchAgents/com.webdavis.forzare-ops-watchdog.plist`

  Usage at any checkpoint: `source ~/workspaces/Ivy/forzare/gate-check.sh; gate_check <that checkpoint's files>`.

- [ ] **`jobs.json` = the THIRD live-data exception (declared honestly).** The two documented live-data
  exceptions are the Todoist store and the owned layer (`~/workspaces/Ivy/forzare/`). Cron jobs are created
  via `hermes cron` and live in `~/.hermes/cron/jobs.json`, which is **NOT** chezmoi-managed — so it is a
  third live-data exception, managed with **backup + rollback**, not a template. **The backup `cp` itself now
  runs in Task C2 Step 1** — immediately before the first cron mutation (X10, moved out of here so the backup
  is adjacent to the mutation it guards). This task documents the exception + the rollback recipe:

```bash
# The C2-side backup (for reference — it runs in Task C2 Step 1, immediately before the first cron create):
#   cp ~/.hermes/cron/jobs.json ~/workspaces/backups/"$(date -u +%Y-%m-%dT%H-%M-%S).hermes-cron-jobs.backup.json"
# ROLLBACK if a job set goes wrong:
#   cp <the timestamped backup> ~/.hermes/cron/jobs.json   (then restart the gateway to reload)
# Validate at any time:
python3 -c 'import json,sys; json.load(open(sys.argv[1])); print("jobs.json valid")' ~/.hermes/cron/jobs.json
```

**Acceptance:** each checkpoint sources the Phase-A `gate-check.sh` and its `chezmoi diff` is empty before the
next phase starts; the `jobs.json` backup is taken in Task C2 Step 1 before the first cron mutation, and the
documented rollback restores it.

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
  — **zero LLM**, out-of-band, doing THREE state-stamped scans per pass (spec §14/U3):
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
  - **(c) Delivery-only cron failures — scan `jobs.json` for new `last_delivery_error` (X8).** A cron ritual
    can **succeed at the agent turn** (output saved) yet **fail to deliver** — verified `cron/jobs.py:1193`
    (`mark_job_run`): `last_delivery_error` is tracked **separately** from the agent error ("a job can succeed
    but fail delivery"). Such a run has `last_status == "ok"`, so scan (b)'s `cron/output/` failed-run scan
    never sees it — the delivery failure is otherwise **invisible**. So the watchdog also reads
    `~/.hermes/cron/jobs.json` and, per job id, alerts on a **newly-set/changed `last_delivery_error`** since
    its watermark (content-stable id over `{job id, last_run_at}`, same spool). This is the one failure class
    the two run-outcome scans miss.
    - **Masking window — the honest bound (Y11, accept-with-framing).** `last_delivery_error` is field-tracked
      per job and **cleared only by a LATER successful delivery of the SAME job**. The watchdog already
      **snapshots + diffs `jobs.json` per pass** against its watermark, so the ONLY window in which a delivery
      failure could be masked is a **same-job re-run within one 300s pass** — i.e. a *manual* re-trigger of the
      same ritual inside 5 minutes that succeeds and clears the error before the next diff. The scheduled
      rituals fire **≥ a day apart**, so they cannot self-mask; only a burst of manual triggers of one job can.
      This is a **documented, accepted bounded gap** — **no new machinery** is added (the per-pass snapshot+diff
      is the existing watermark); the wording states the bound honestly rather than claiming a guarantee it
      can't meet.
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
  **`StartInterval` 300s (DECIDED — W8/X9: the **best-effort ≈5-min** detection target, NOT a hard ceiling;
  launchd skips intervals during sleep and won't re-enter a still-running pass, so "every system failure on
  `#forzare-errors` within 5 min" is a polling target, not a guarantee)**, `RunAtLoad` per the osquery model, `Label`
  `com.webdavis.forzare-ops-watchdog`, stdout/stderr to `~/.local/log/hermes/forzare-ops-watchdog.log`.
  (Note: the gateway's OWN plist `ai.hermes.gateway.plist` has `KeepAlive` = `true` — restarts on any exit;
  this watchdog covers the hang KeepAlive can't detect, spec §14/§19.)
- [ ] **Step 3: Wire lint** — add the plist loader template to `find_shell_templates` in `scripts/lint.sh`
  (the `.sh` helper is auto-shellchecked by `find_shell_files`; the `.plist.tmpl` is XML → `plutil -lint`).
- [ ] **Step 4: Document** in `CLAUDE.md` — the watchdog probes `:8644/health`, scans cron/output +
  the kanban DB (event-based: `gave_up`/`crashed`/`timed_out`), **and scans `jobs.json` for
  `last_delivery_error` (delivery-only failures, X8)**, alerts out-of-band via `hermes send --to` (never
  through the dead gateway), and closes the KeepAlive hang-detection gap.
- [ ] **Step 5: Verify (plumbing only — no real alert)**

```bash
set -o pipefail
cd "$(git rev-parse --show-toplevel)"
shellcheck dot_local/bin/executable_forzare-ops-watchdog.sh
CI=1 chezmoi --source "$PWD" execute-template --no-tty < Library/LaunchAgents/com.webdavis.forzare-ops-watchdog.plist.tmpl | plutil -lint -
# health probe returns 0 while the gateway is up:
curl -fsS -m 3 http://127.0.0.1:8644/health >/dev/null && echo "gateway health OK (exit 0)"
```

Expected: shellcheck clean; `plutil -lint` → `OK`; the live probe exits 0. **Acceptance (W9/X8 cases):** the
script alerts via `hermes send --to` on a simulated down/hung code (bogus port), on a seeded `gave_up`
outcome, on a `timed_out`/`crashed` run event, **and on a seeded `jobs.json` `last_delivery_error` for an
otherwise-`ok` run (X8 — the delivery-only failure the run-outcome scans miss)**; it stays **silent** for ANY `blocked` card that has emitted
no failure event — **including the recovered-failure-then-user-block fixture** (a card that fails once, is
retried successfully, then blocks awaiting the user: `status='blocked'`, `consecutive_failures == 1`, no
gave_up/crashed/timed_out event since the watermark ⇒ NO alert — this is the case the old status+counter
predicate got wrong) — and when healthy; a **second scan does NOT re-alert** the same failure (content-stable
ids); a **simulated Discord outage** (`hermes send` exit-1) **retains the spool and retries next pass** (no
lost alert); and the script finds `hermes` + the channel env **under launchd's minimal environment** (W9 —
absolute `HERMES_BIN`, sourced `.env`). **The end-to-end errors-channel ROUTE test moved here from D1 (R5A7):**
a seeded `gave_up`/`crashed`/`timed_out` capture-card event is asserted to reach `#forzare-errors` via
`hermes send` + spool — F1 owns this route because it owns the watchdog + spool, so no forward dependency and
no user-visible message issues from D1 before Phase G. **The `last_delivery_error` masking window is the
documented, accepted bounded gap (Y11)** — a same-job manual re-run inside one 300s pass; no new machinery.
**This task's files are committed to the repo via the normal pre-commit flow** (`just lint-check` + `just
test`), separate from the two doc commits.

---

### APPLY CHECKPOINT F (inline, fail-closed — R2A2) — do this BEFORE Phase G go-live

Apply the watchdog script + plist sources, then `launchctl load` the agent. **Phase G's go-live is gated on
"Checkpoint F cleared"** — the watchdog (the errors-channel router + gateway-hang detector) must be live
before delivery flips to Discord, or a post-go-live failure could go unrouted.

- **User-run/agent-run:** apply `dot_local/bin/executable_forzare-ops-watchdog.sh` +
  `Library/LaunchAgents/com.webdavis.forzare-ops-watchdog.plist.tmpl`; `launchctl load` it.
- **Gate (fail-closed) — covers this checkpoint's OWN artifacts (R3A8/W3):** `source
  ~/workspaces/Ivy/forzare/gate-check.sh` (the Phase-A script, X10) and run `gate_check` with
  the explicit file targets `~/.local/bin/forzare-ops-watchdog.sh
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
  | **EOD ceiling by cutoff (X6): 22:59 / 23:00 / just-past-midnight / catch-up / manual mid-day** | 22:59 (before cutoff) ⇒ CEILING = yesterday; 23:00 (at cutoff) ⇒ CEILING = today; just-past-midnight ⇒ still closes the prior day; a manual `/forzare-eod` follows the same cutoff rule — each a once-only close | 0 extra |
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
- [ ] **Step 4: Flip to live (W2/R3A16/X1) — the DRY-RUN strip is on ALL SIX jobs; the delivery flip is on the
  four user-facing ones only.** Two DIFFERENT axes — do not conflate them:
  1. **Remove the DRY-RUN directive from EVERY one of the six ritual jobs (X1)** — morning brief, end-of-day,
     gym-window-end, every block-boundary prompt, **the 02:00 `waiting-reconcile`, AND the monthly
     someday-sweep**. The two state-only jobs (`waiting-reconcile`, someday-sweep) keep `--deliver local`
     **forever**, but their **prompts must ALSO go live** — under the DRY-RUN directive they would only journal
     intents and never actually mark state, so a reconcile/sweep left with the directive would silently do
     nothing. `hermes cron edit` each job so its prompt no longer opens with the `DRY RUN …` line (the bundle
     instruction likewise drops any staging variant). Then truncate `forzare/state/dryrun-intents.jsonl`
     (staging evidence, not live state).
  2. **Flip delivery from `local` to `--deliver discord` (home channel) for the FOUR user-facing jobs only:**
     morning brief · end-of-day · gym-window-end · every block-boundary prompt. `waiting-reconcile` and the
     someday-sweep stay `--deliver local` (their delivery IS the brief-mode read).
  3. **Resume/enable the 23:00 eod-roll job** (`hermes cron resume` if it was paused) — it now performs REAL
     rolls keyed off the seeded `last-reconcile.json`.
  The errors channel stays the forzare-ops watchdog's `hermes send --to discord:<#forzare-errors>` +
  belt-and-suspenders relay. **This is the last step; do it only after Steps 1–3 are green.**
- [ ] **Step 5: Post-go-live smoke + DAY-1 SUPERVISED checks (R3A3/R3A7/W11)**

```bash
set -o pipefail
hermes cron list                                   # jobs live, Denver TZ
# Y8: assert the EXACT six-name manifest + count — no missing, no duplicate, no stray forzare job.
EXPECT=$(printf '%s\n' forzare-morning-brief forzare-eod forzare-waiting-reconcile forzare-gym-window-end forzare-block-boundary forzare-someday-sweep | sort)
GOT=$(jq -r '.jobs[]|select(.name|test("^forzare-"))|.name' ~/.hermes/cron/jobs.json | sort)
[ "$GOT" = "$EXPECT" ] || { echo "FATAL: forzare cron manifest mismatch (Y8):" >&2; \
  diff <(printf '%s\n' "$EXPECT") <(printf '%s\n' "$GOT") >&2; exit 1; }
N=$(printf '%s\n' "$GOT" | grep -c .); [ "$N" -eq 6 ] || { echo "FATAL: expected 6 forzare jobs, found $N (Y8)" >&2; exit 1; }
echo "exact six-name forzare manifest + count OK (Y8)"
# X1: assert EVERY forzare job's prompt is DRY-RUN-free — all six, incl. the two --deliver local state-only
# ones (waiting-reconcile, someday-sweep). A single lingering "DRY RUN" prompt means a job would only journal
# intents and never act. Read prompts from jobs.json by name; fail loud on any that still opens with DRY RUN.
DIRTY=$(jq -r '.jobs[] | select(.name | test("^forzare-")) | select((.prompt // "") | test("^\\s*DRY RUN")) | .name' \
  ~/.hermes/cron/jobs.json)
[ -z "$DIRTY" ] || { echo "FATAL: these forzare job(s) still carry a DRY-RUN prompt (X1):" >&2; \
                     printf '%s\n' "$DIRTY" >&2; exit 1; }
echo "all six forzare job prompts are DRY-RUN-free (X1)"
# Delivery axis: the four user-facing jobs deliver to discord; the two state-only jobs stay local.
jq -r '.jobs[] | select(.name|test("^forzare-")) | "\(.name)\t\(.deliver)"' ~/.hermes/cron/jobs.json
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
- [ ] **§17 Default-deny dry-run WRAPPER (X3 — hardens the prompt-level contract).** V1 dry-run is honored by
  a prompt convention (the mode-check-first rule in the shared mutation helper, Task B0). Post-V1, add a
  small **default-deny code wrapper** around that helper that *refuses* any real `td`/calendar/state write
  while a dry-run flag is set — so a skill that forgot the mode check still can't mutate prod, upgrading
  "honored" to "enforced." NOT built for v1 (this docs-only loop can't mandate engine code the plan doesn't
  own); booked here as the concrete hardening move.
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
**Inbox-task-id idempotency keys** + `--max-runtime 900` (Y7) + the pinned `forzare-capture-pipeline` skill
**authored in B11** (R5A4) + **NO `notify-subscribe` — decision-queue `triage-reraise` re-raise + watchdog
failures instead, Y2** + 5 stages + dup-guards, R7/W4; **test isolation = a non-spawnable assignee, not a
board — the dispatcher enumerates every board, W4**) → B11/D1; root `session_reset` (R6a) + `[SILENT]` per-path (**direct filter-function probes,
run green against the installed venv, R3A7**; forced-failure script under `~/.hermes/scripts/forzare-staging/`
— the verified path constraint, R3A6; delivered-vs-suppressed moved to G1 day-1, W11) + clarify buttons /
no-reaction-input (R8) → E1; **the shared fail-closed apply-gate now takes an explicit FILE list per
checkpoint — `chezmoi diff` on a directory is NON-recursive (fixture-verified), W3 — + jobs.json exception →
E2 (Checkpoint C moved BEFORE Task C2, its dependent staged command, W3; Checkpoint F covers its own
artifacts with file targets + content hashes, R3A8)**; `forzare-ops-watchdog` (health probe + cron/output +
**event-based kanban failure predicate — gave_up/crashed/timed_out only, never status+counter (`block_task`
does not clear `consecutive_failures`), W9**; `HERMES_BIN` + sourced env under launchd, W9; **`StartInterval`
300s — a BEST-EFFORT ≈5-min errors-channel detection target, NOT a hard ≤5-minute guarantee (R5A9/X9 — launchd
skips intervals during sleep and won't re-enter a still-running pass)**; durable spool; **the
`last_delivery_error` masking window is the documented accepted bounded gap, Y11**; **the D1 forced-error
errors-channel ROUTE test lives here, R5A7**) + plist + `hermes send --to` alert (R2/U3) → F1; staged dry-run (**assertions retargeted to the `dryrun-intents.jsonl` observable, R3A1/R3A3**) +
explicit go-live matrix (**+ ≥3-day outage drain with ONE `roll_count` tick per task, R3A9/W5**) + the
three-move flip (**remove the dry-run directive, flip delivery, resume EOD — on the COMPLETE user-facing job
list incl. gym-window-end, W2/R3A16**) + day-1 supervised double-roll + delivered-vs-suppressed checks
(R3A3/W11) → G1; post-V1 (**delivery-lease V6/R2A15**, Langfuse self-host, `tailscale serve` webhook, Hue,
Todoist webhooks, voice, ledger, email/comms triage, Fantastical-as-genuinely-optional W13) → H; ai-skills
STB#3 out of scope → H Non-Goals.
**Round-2 additions:** the shared **ledger I/O helper** (flock + atomic + journal-then-commit + healing, V2)
→ Task B0; the dry-run read-only contract honored by every mutating skill (V4) → Global Constraints + Phase
B intro; **eod-roll keyed off an explicit reconciliation date + past-grace single-fire recovery matrix** (V3)
→ B7; the **`work_schedule`/schedule `skills.config`** owning task (V11/R2A10, blocks C2) → B10; **job-id
parse rebuilt to `Created job:` + 12-hex, per-job audit dir** (R2A1) throughout the staged blocks.
**Round-3 additions:** dry-run made OBSERVABLE via `forzare/state/dryrun-intents.jsonl` with the INSTRUCTION
transport (env is stripped from gateway-ticked children) and staging assertions retargeted to the intent
records (R3A1/R3A2/R3A4/W2) → Global Constraints + Phase B intro + B1/B6/B7/B8/C2/G1; the **centralized
date-mutation layer with the state-chosen verb** — `td task update --due` for initial dating of an undated
task (probed live: `reschedule` errors `NO_DUE_DATE`), `td task reschedule` for re-dating, timed/recurring
never mutated — + the six W6 dating fixtures → Task B0 + B1/B4/D1; the **reconciliation RANGE with the
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
2053-task backlog relevance-comb is pending, so planning-pull stays conservative (C2/B4). *(The `/forzare-*`
native slash-autocomplete question is no longer a carried-forward item — the §12.5 mirroring check now has an
OWNING task, C1 Step 4/R4A14: perform the check, record the result, decide the micro-shim.)*

**Round-4 additions:** capture kickoff re-ordered — the PARENT runs `hermes kanban specify` immediately after
create (the `triage → todo` transition that permits dispatch) then `notify-subscribe` for the parent-callback
transport; stage 2 no longer calls specify (X2) → D1 Step 1 + spec §8b. The ledger gains a **`kind`** field
(`surfacing`/`leadtime` roll; `waiting_checkback`/`user_fixed` never roll/tick, X5) and **widens into the
mutation journal** (typed date-op/p1/label/comment/calendar so W7 exclusion is complete + cursor-paged, X11) →
Task B0 + B9 + spec §4d/§6a. The **EOD ceiling is by invocation mode + Denver cutoff** (today at/after 23:00
else yesterday, seed = Denver yesterday, X6) → A1/B7/G1. Sweep decisions are a **persisted queue**
(`sweep-candidates.json`; SWEEP-mode producer + brief head-item consumer, X7/R4A6) → B5/C2 + §8a. Watchdog
gains a **`jobs.json` `last_delivery_error` scan** (delivery-only failures, X8) and its latency is stated
**best-effort ≈5-min** (X9) → F1. Dry-run writer inventory completed (tomorrow-prep, SWEEP `followups-sweep`,
`calibration-log`, `brief-assemble` prestage-clear) with enforcement stated as prompt-level + a default-deny
wrapper booked in Phase H (X3) → Phase B intro/§17/H. The **B1 label gate slurps with `jq -s` keyed by run_id
+ exactly-one** (X4); the C2 zero-mutation gate is **scoped to Bob-authored targets** (user may edit Todoist
freely, R4A12) and its trace match is a **here-string** (X14); `gate_check` is a **persistent Phase-A script**
and the `jobs.json` backup moves to C2 (X10); the if-then stall lever gets a named owner in `todoist-surface`
(X13); G1 **strips DRY-RUN from all six jobs and asserts every prompt** (X1); commute constants
`commute_prep_minutes: 30`/`commute_travel_minutes: 25` decided with the alarm timestamp asserted (X12);
staged tests truncate the intents log at start (R4A11); the §4c idempotency guard is "any p1" not "3 p1s"
(R4A9); EOD marks the stall and the brief delivers it (R4A10); D1 gains a **direct `hermes -p default`
controlled-execution harness** (R4A5); the D1 watchdog note and the `VALID_STATUSES` citation
(`kanban_db.py:101`) corrected (R4A4/R4A13).

**Round-5 additions:** **ONE unified decision queue** (`forzare/state/decision-queue.json`, Y1/R5A1) — every
brief-time decision (`waiting-chase`/`fixed-redecision`/`stall-decision`/`triage-reraise`/`sweep-candidate`)
enqueued state-only; the brief emits EXACTLY the head `pending` record, which **replaces** the do-now close;
**ack is a LIVE-only write** by the turn that receives the answer (R5A5) → B4/B5/B7/B11/C2 + Global Constraints.
The **`notify-subscribe` callback design DELETED** (Y2, verified `hermes kanban --help`: terminal events only,
onto the home channel — a firewall breach + dispatch race) — cards re-raise via `triage-reraise`, failures via
the watchdog → B11/D1. **Task bankruptcy = a REVERSIBLE UNDATE** of a frozen journaled id set — never
delete/complete/archive, bounded summary, named confirmation, idempotent partial-failure recovery (Y3) → B5 +
§4c. **Native CLI dry-run flags UNDER the prompt mode** (Y4, verified `td … --dry-run` on add/update/
reschedule/complete/delete + `td comment add`, `gog --readonly`/`-n`) — the mtime/hash gates extended to ALL
stores incl. the `--type comment` activity query, a [TEST]-scoped calendar snapshot, `schedule-override.json`,
`plan-of-day.json`, `decision-queue.json` → Global Constraints + Phase B intro. **The lifecycle store SPLITS**
into a prunable MAP (`task-lifecycle.json`) + an append-only JOURNAL (`mutation-journal.jsonl`, 45-day
retention, `type` enum gaining `description`) so W7 exclusion stays complete (Y5/R5A11) → Task B0/B9. **Full
TOPOLOGICAL reorder** (Y6/R5A3/R5A4): the shared mutation helper is now **Task B0** (authored FIRST), the
`forzare-capture-pipeline` skill is **Task B11** (moved from D1), Phase B is **author-all → APPLY CHECKPOINT B
→ pin+stage-all**, **Checkpoint C is re-scoped to the bundles only**, and a **new post-D CHECKPOINT D** gates
the pipeline verify → B0/B11/Checkpoints B/C/D. **The morning-plan guard is a PER-DAY PLAN RECORD**
(`plan-of-day.json` — resume-missing-writes, distinguishes Bob-owned from user-set p1, Y13) replacing the
any-p1 guard, fixing the stale "3 already exist" acceptance (R5A2) → B4. Plus: capture cards carry
**`--max-runtime 900`** (Y7); the **cron install is transactional** — six-name manifest, reconcile-by-name,
rollback (Y8) → C2; boundary/gym cron times **DERIVED from the resolved `work_schedule`** with a
schedule-change reconcile rule (Y9) → B10/C2; the **Checkpoint A channel gate** parses both channels, requires
distinct, and runs an end-to-end errors-channel send probe (Y10); the **W1 bundle-trace assertion rebuilt on
per-skill intent-log EFFECTS** (never a prompt-text grep — the cron audit records prompt+response only, no tool
calls; Y12/R5A8) → C2; the **`last_delivery_error` masking window** documented as an accepted bounded gap
(Y11) and the **D1 forced-error errors-route test moved to F1** (R5A7) → F1; the B3 calendar acceptance is a
controlled **LIVE** [TEST]-keyed harness on the 🤖 calendar (R5A6); the intent `op` vocabulary is **defined
once** in the Phase B intro (R5A13); B10's second config edit is gated at Checkpoint B (R5A14); the 02:00
unblock signals are gog + `td activity` only, no Discord (R5A12); the Self-Review `≤5-minute guarantee` wording
weakened to best-effort (R5A9).

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
