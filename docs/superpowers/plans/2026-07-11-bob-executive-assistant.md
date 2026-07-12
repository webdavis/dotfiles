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
  acceptance = zero production mutations, asserted across ALL stores (Y4/X3/AA1) — the NEGATIVE gate is
  INDEPENDENT of the intent log; intents are POSITIVE evidence only:** (positive) the intent RECORDS
  exist in `dryrun-intents.jsonl` with the expected fields; (negative) **before/after diffs that never consult
  the intent log** — a `td activity --since <today> --json` snapshot **CURSOR-PAGINATED to exhaustion** (loop
  `--cursor` until empty) on **both** the `--type task` and `--type comment` streams shows **no new
  forzare-authored change on the [TEST] fixture set** (scope to the [TEST] fingerprint, NOT `--by me` — Bob
  writes as the user's account so `--by me` cannot isolate a leak, swept; scoping to the harness-seeded [TEST]
  tasks keeps it independent of the intent log AND immune to the user editing their own tasks), a
  **[TEST]-scoped `gog calendar events`** before/after snapshot is unchanged, **and** a **RECURSIVE content-hash
  of `state/` + `calibration/`** is unchanged — enumerating the real stores (`last-reconcile.json` /
  `task-lifecycle.json` / **`mutation-journal.jsonl`** (Y5) / `schedule-override.json` / `tomorrow-prestage.json`
  / **`plan-of-day.json`** (Y13) / **`decision-queue.json`** (Y1) / `sweep-exclusion.json` / the `calibration/`
  store) and **EXPLICITLY EXCLUDING `dryrun-intents.jsonl`** (a dry-run DOES append to it, so hashing it would
  be self-defeating, AA1). **Native CLI dry-run flags layer UNDER the prompt mode (Y4, defense in depth):** staged
  mutating `td` calls also carry **`--dry-run`** (verified on `td task add/update/reschedule/complete/delete` +
  `td comment add`) and staged `calendar-write` also carries **`gog --readonly`** or **`-n/--dry-run`** — so a
  skill that forgot the mode check still cannot mutate the outside world. The **23:00 eod-roll job is created
  disabled/`local` + the dry-run instruction until go-live**; **go-live (G1) REMOVES the dry-run directive,
  RESUMES/ENABLES the eod-roll job, and never `rm`s a real state file** (a correct dry-run wrote only
  `dryrun-intents.jsonl`, which go-live truncates) (Task C2/G1, W2).
- **Delivery is headless-native only** (spec §12/R1): NO `ctx.inject_message`, NO `bob-surface` plugin. Cron
  Discord delivery + clarify buttons + `hermes send --to`.
- **Fix the live-config drifts (R6b + Z1) BEFORE go-live** (Phase A): empty `timezone`,
  `kanban.auto_decompose: true`, and `kanban.auto_subscribe_on_create` **unset → defaults `True`** (Z1 — set
  `false`, the §9 firewall guard); plus `kanban.max_in_progress_per_profile: null` and
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
  `decision-queue.json`** (ALL brief-time decisions — the eight classes q1-conflict/waiting-chase/
  fixed-redecision/stale-p1/stall-decision/triage-reraise/sweep-candidate/bankruptcy-offer; Y1/R5A1/AA4/BB2,
  generalizes the old `sweep-candidates.json`; each record `{id, class, task_id/candidate_id, proposed, status,
  enqueue_ts, gen, rev, head}` with **`id` = stable, content-INDEPENDENT — per-task classes `class:task_id`,
  AGGREGATE classes `q1-conflict:<date>` / `bankruptcy-offer:<YYYY-MM>` (BB2)**; **ack TOMBSTONES `{id, gen}`,
  a re-enqueue opens `gen+1`/`rev=1`, and promotion sets the `head` flag (the primary sort key), Z2/AA4/BB2),
  the **per-day `plan-of-day.json`** (the morning-plan idempotency record, Y13), the **`sweep-exclusion.json`**
  (bankruptcy RETIRE list for undated someday items — never re-proposed, reversible by deleting the entry; Z13),
  the **`go-live.json`** (the staging↔live boundary flag `{gone_live, ts}` — written once at G1; the watchdog's
  ritual-absence scan reads it to LOG pre-go-live / ALERT post-go-live, CC3), and — **staging only** —
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
# Shared fail-closed apply-gate (X10/W3/R7A12). Usage: gate_check <file>...  — each checkpoint passes ITS OWN
# explicit FILE list; `chezmoi diff` on a DIRECTORY is NON-recursive, so never pass a dir target.
# REPO is a PINNED, VALIDATED constant (R7A12) — the chezmoi source dir — so the gate has NO cwd dependence and
# a checkpoint needs no `cd` precondition. Validate it exists AND is the chezmoi source before any diff.
REPO="/Users/stephen/workspaces/Ivy/webdavis/dotfiles"
gate_check(){
  [ -d "$REPO" ] || { echo "FATAL: REPO $REPO does not exist (R7A12)" >&2; return 1; }
  [ "$(chezmoi source-path 2>/dev/null)" = "$REPO" ] \
    || { echo "FATAL: chezmoi source-path is not $REPO — refusing to gate against the wrong source (R7A12)" >&2; return 1; }
  [ "$#" -gt 0 ] || { echo "FATAL: gate_check needs an explicit FILE list (dir diffs are non-recursive)" >&2; return 1; }
  local f rc fail=0 DIFF_OUT DIFF_ERR
  for f in "$@"; do
    DIFF_OUT=$(mktemp); DIFF_ERR=$(mktemp)
    if chezmoi --source "$REPO" diff "$f" >"$DIFF_OUT" 2>"$DIFF_ERR"; then rc=0; else rc=$?; fi
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
grep -nE '^timezone:|^session_reset:|auto_decompose:|auto_subscribe_on_create:|max_in_progress_per_profile:|default_assignee:|max_parallel_jobs:' ~/.hermes/config.yaml
grep -c 'auto_subscribe_on_create' ~/.hermes/config.yaml   # expect 0 (UNSET → defaults True — Z1 drift)
grep -c 'hermes-achievements' ~/.hermes/config.yaml   # expect 0 (achievements is NOT enabled — verified)
sed -n '/^plugins:/,/^[a-z]/p' ~/.hermes/config.yaml | grep -E 'discord|provider'   # confirm the live members we must NOT strip
```

Expected: `timezone: ''`; live `session_reset.mode: none`; `auto_decompose: true`;
`auto_subscribe_on_create` **absent** (count 0 → defaults `True`, the Z1 drift);
`max_in_progress_per_profile: null`; `default_assignee: ''`; `cron.max_parallel_jobs: null`;
`hermes-achievements` count **0**; `platforms/discord` + the providers present under `plugins.enabled`.

- [ ] **Step 3: Apply the fixes** (edit the encrypted source; **additive** — never delete existing
  `plugins.enabled` members):

  1. `timezone: "America/Denver"` (root key).
  2. `kanban.auto_decompose: false`.
  2b. `kanban.auto_subscribe_on_create: false` (Z1 — the firewall guard; default `True`, verified
     `hermes_cli/config.py:1348`. The in-gateway kanban **tool** create path auto-subscribes a platform-bound
     chat to a card's terminal events — `tools/kanban_tools.py:843,858-898` — which would leak onto the task
     channel, §9. forzare's capture flow uses the subscription-free CLI `hermes kanban create`; this is
     belt-and-suspenders so even a stray tool-path create writes no subscription row).
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
must 'auto_subscribe_on_create: false'      'kanban auto-subscribe guard (Z1)'  # separate assert
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
# Z4: NEVER `source`/`. ~/.hermes/.env` — the managed .env carries an unquoted value with spaces that CRASHES
# a strict shell (`set -euo pipefail` + `. .env` aborts). dotenv-PARSE only the two channel keys instead —
# this extracts values without evaluating the file (verified robust for a quoted or bare numeric channel id):
dotenv_get(){ sed -n "s/^[[:space:]]*$1=//p" ~/.hermes/.env | tail -n1 \
  | sed -e 's/^"\(.*\)"$/\1/' -e "s/^'\(.*\)'\$/\1/"; }
DISCORD_HOME_CHANNEL=$(dotenv_get DISCORD_HOME_CHANNEL)
DISCORD_ERRORS_CHANNEL=$(dotenv_get DISCORD_ERRORS_CHANNEL)
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

## Phase B — Atomic skills + the shared helper (author-all → Apply Checkpoint B → stage-all → integrity gate)

> **BUILD ORDER — the topological rule (Y6/R5A3, load-bearing).** A staged dry-run reads the skill's **LIVE**
> `~/.hermes/skills/<name>/SKILL.md`, which only exists after a `chezmoi apply`. So Phase B runs in **four
> ordered stages, NO curator pinning (AA11 — repo-authored skills are not curator GC candidates; the gate is
> installed path + content hash):**
> 1. **Author ALL Phase-B sources first** — the **shared mutation helper FIRST (Task B0** — B1–B11 all depend
>    on it), then every skill's `SKILL.md` (B1–B10), the **`forzare-capture-pipeline` skill (Task B11**, moved
>    here from D1 per R5A4), the schedule `skills.config` (B10), **and the boot-check script** (moved into the
>    author stage so it exists before the SKILL-INTEGRITY GATE that invokes it) — all authored in the
>    `dot_hermes/` source dir, **no live run yet.**
> 2. **APPLY CHECKPOINT B** (fail-closed — end of the author stage) — the user-run/agent-run apply of **every**
>    Phase-B file (skills, capture-pipeline skill, `config.yaml` `skills.config`) + the shared gate over the
>    explicit file list + an **installed-dir + resolved-config check**. **No staged dry-run below may run until
>    Checkpoint B is CLEARED.**
> 3. **Staged dry-run ALL** — only after Checkpoint B: the per-skill staged cron dry-run (each task's staged
>    dry-run step). These are the verify steps. **No `hermes curator pin` — verified unreachable-as-protection
>    (AA11):** the forzare skills are chezmoi-dropped (not agent-created), so
>    `list_agent_created_skill_names()` excludes them from the curator's GC list and a pin protects against a
>    transition that can never target them.
> 4. **SKILL-INTEGRITY GATE** (fail-closed, replaces the old POST-PIN GATE — AA11) — once every skill is applied,
>    the boot skill-existence + **content-hash** assertion runs (the boot-check script): every skill the three
>    bundles will name is **installed at its expected path AND its `SKILL.md` content-hash matches the chezmoi
>    source**. Phase C is gated on it.
>
> Each task below is written skill-by-skill for readability; its **author** steps belong to stage 1 and its
> **staged dry-run** steps to stage 3. (Checkpoint **C** is therefore re-scoped — it applies only what **Phase
> C** authors, the three bundles; the skills are already live from Checkpoint B, R5A3, and integrity-checked at
> the SKILL-INTEGRITY GATE.)
>
> Every skill's applied target is `~/.hermes/skills/<name>/SKILL.md` (+ any helper scripts) — authored in
> the `dot_hermes/` source dir per the delivery-vehicle rule (Global Constraints), **installed via chezmoi (no
> pin — AA11)**, and is driven test-first via a **staged cron dry-run**. `td` usage
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
> **Shadow-state rule (R7A10 — makes a dry-run's OWN prior writes observable within the same window).** Under
> the dry-run directive, a mutating skill that would normally READ a state store it also writes **reads its own
> journaled intents in `dryrun-intents.jsonl` as SHADOW state** instead of the (deliberately untouched) real
> store — so a second consecutive dry-run of `eod-roll` sees the first run's *intended* `last-reconcile`
> advance and logs an `already-reconciled` no-op (B7 Step 4), and any within-window read-after-write is
> consistent. This one line is part of every mutating skill's authored dry-run contract (named explicitly in
> B7 Step 1's `eod-roll` contract), so the tests and the authored behavior agree.
>
> **Intent `op` vocabulary — DEFINED ONCE here (R5A13), referenced by every jq gate below.** The
> `dryrun-intents.jsonl` `op` field is one of a fixed enum: **`task.add`**, **`task.update-labels`**,
> **`task.update-due`**, **`task.update-description`** (the §7/X13 if-then cue), **`task.undate`** (bankruptcy
> UNDATE — strip the due, BB3), **`task.complete`**, **`comment.add`**, **`calendar.create`** /
> **`calendar.update`** / **`calendar.delete`**, **`state-write`**
> (any owned-layer state file — `schedule-override.json` / `last-reconcile.json` / the lifecycle
> `task-lifecycle.json` MAP / `mutation-journal.jsonl` / `tomorrow-prestage.json` / `plan-of-day.json` /
> `decision-queue.json` / `sweep-exclusion.json`), **`sweep.retire`** (bankruptcy RETIRE — append to
> `sweep-exclusion.json`, BB3), **`waiting.clear`** (the 02:00 composite unblock, BB3), **`p1.set`** /
> **`p1.clear`**. Every SKILL task and every jq assertion below uses
> exactly these names. (Distinct from the mutation-JOURNAL `type` enum — `date-op`/`p1`/`label`/`comment`/
> `calendar`/`description`/`task.add`/`task.complete`/`waiting-clear`/`undate`/`retire`, spec §4d/Y5/Z3/BB3 —
> which classifies the append-only journal lines, not the intents. Both the journal line and the intent record
> carry the **unified shape** `{ts, type/op, target, args, old_value, intended_value, external_marker?,
> reconcile_date, commit_state}`, stated identically in spec §4d/§8a and Task B0, CC8.)
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
> **Staging-harness safety — RUN-ID-scoped, trap-guarded, captured-id cleanup (BB6, load-bearing for every
> staged test below).** Every staged test that seeds `[TEST]` fixtures or backs up a real state file follows one
> discipline so a re-run, a concurrent run, or an interruption can never clobber another run's fixtures or leave
> a real state file replaced by a fixture:
> - **A per-run id.** Each test opens with `RUNID="$(date +%s)-$$"` and prefixes every fixture it creates
>   **`[TEST-$RUNID]`** (never bare `[TEST]`), so two runs never share a fixture namespace.
> - **Run-id-suffixed backups + an EXIT/INT trap that restores them ATOMICALLY.** A real state file the test
>   overwrites is backed up to a **run-id-suffixed** name (`cp "$POD" "$POD.bak.$RUNID"`, never a fixed `.bak`),
>   and a `trap` installed at the top restores it via **same-dir tmp + `mv`** (atomic) on EXIT **and** INT — so
>   an interrupted test never leaves the real store holding a fixture: `restore(){ [ -f "$POD.bak.$RUNID" ] &&
>   mv "$POD.bak.$RUNID" "$POD"; …; }; trap 'restore; cleanup' EXIT INT`.
> - **Cleanup deletes ONLY the ids the test captured — never a `search: [TEST]` prefix sweep.** The test collects
>   the ids it created into an array (`CREATED+=("$id")`) and deletes exactly those (`td task delete "$id"
>   --yes`); it **never** runs `td task list --filter "search: [TEST]" … | xargs td task delete`, which would
>   delete a concurrent run's or a real user's `[TEST]` task. **Cascade note:** `td task delete` of a parent
>   CASCADES to its subtasks (memory: Todoist parent-delete cascade) — a bankruptcy/split fixture that seeds
>   subtasks deletes the parent last and asserts the children were captured, so nothing is orphaned or
>   silently cascaded.
> - **Re-run safe.** Because fixtures are `[TEST-$RUNID]`-scoped, backups are run-id-suffixed, and cleanup is
>   captured-id-only, a second run (or a crashed-then-rerun) neither collides with nor destroys the first's
>   state.
> The individual staged blocks below are written against this convention (a block may abbreviate the trap for
> readability, but the RUN-ID prefix + captured-id cleanup + run-id-suffixed backup are required of each).
>
> **Staging test-override contract — AUTHORED, staging-only fields in `schedule-override.json` (CC4/CC12,
> mirrors spec §8a).** So the schedule-, weather-, and clock-dependent skills are testable deterministically
> without a live schedule/forecast/wall-clock, three **test-only** fields may appear in
> `forzare/state/schedule-override.json` **under the dry-run/staging directive ONLY** — each honored by the named
> skills' authored contract **only when staging is active, and IGNORED in production:**
> - **`pinned_schedule`** — a fixed `{work_block: {start,end} | null}` that **`eisenhower-plan`** and
>   **`brief-assemble`** read in place of the derived `work_schedule` (a work-day vs off-day fixture without
>   waiting for the real calendar day).
> - **`synthetic_weather`** — a fixed forecast/breach (`{breach: "rain 6am"}` or a clear blob) that **`weather`**
>   and **`brief-assemble`** read in place of a live Open-Meteo/NWS fetch.
> - **`FORZARE_NOW`** — an ISO wall-clock override that **`eod-roll`** reads for its cutoff/ceiling math (and any
>   time read), so the 22:59 / 23:00 / just-past-midnight cutoff points (spec §8/X6, B7 Step 1, G1) are driven
>   **deterministically** rather than by waiting for the real clock (CC12).
> Each field is honored **only when the staging directive is set** (the same mode gate as the dry-run
> intent-log redirect) and **ignored outside staging** — a live run never reads them. This is the authored
> contract the B4/B7/G1 fixtures below rely on; §8a lists the three as staging-only.
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
    **every** Bob mutation, appending one line in the **unified record shape (CC8 — the same fields in spec §4d,
    §8a, and here)** `{ts, type, target, op, args, old_value, intended_value, external_marker?, reconcile_date,
    commit_state}` — `type ∈ {date-op, p1, label, comment, calendar, description, task.add, task.complete,
    waiting-clear, undate, retire}` (X11/R5A11/**Z3/BB3/CC8**). **`description`** is the §7/X13 if-then cue
    (Todoist reports it as a bare `updated`, so it MUST be journaled or W7's exclusion misreads it as a user
    touch); **`task.add` + `task.complete` (Z3)** and **`waiting-clear` (the 02:00 composite unblock) + `undate`
    + `retire` (the bankruptcy ops, BB3)** are journaled too so a Bob-authored write can't read as user
    initiation. **The JOURNAL is retained 45 days (the calibration correlation window, spec
    §19), then pruned — NOT pruned on task completion** (the reducer still needs the recent journal to exclude
    Bob's writes after a task is done).
  - **QUEUE — `forzare/state/decision-queue.json`** (Z2/AA4/BB2, spec §2 step 4/§8a): the helper is ALSO the
    single writer for the unified decision queue. Each record is `{id, class, task_id/candidate_id, proposed,
    status, enqueue_ts, gen, rev, head}` — **`id` = a STABLE, content-INDEPENDENT key (BB2), NOT hashed over
    `proposed`/content:** a **per-task** class keys on the task (`class + ":" + task_id/candidate_id`); an
    **AGGREGATE** class with no single task keys on its natural period — **`q1-conflict:<collision-date>`** and
    **`bankruptcy-offer:<YYYY-MM>`**. A producer that re-enqueues an unchanged decision is a **no-op**; one that
    re-evaluates it to a **different `proposed` updates the existing record IN PLACE (same `id`) and increments
    `rev`** (a content-derived id would spawn a duplicate — the bug). **`gen`/`rev` contract (BB2):** a record
    starts `gen = 1`, `rev = 1`; every in-place `proposed`/content change or a **promotion** (setting `head =
    true` under the lock) `rev++`; the producer's re-touch **retires any obsolete revision**. **Ack TOMBSTONES
    the record (BB2 — supersedes the bare `acked` flag):** the ack writes a tombstone `{id, gen}` and retires the
    live record; a later **re-enqueue of a tombstoned `id`** opens a *fresh* record at **`gen + 1`, `rev = 1`**
    (rev resets each generation), so a decision answered today and genuinely recurring next week re-asks under
    `gen 2` rather than being suppressed forever. **Eight classes; total order `(head DESC, class-rank,
    enqueue_ts, id)`** — the **`head` flag is the PRIMARY sort key so promotion PARTICIPATES in the order** (no
    side "head slot"), class-rank **`q1-conflict > waiting-chase > fixed-redecision = stale-p1 > stall-decision >
    triage-reraise > sweep-candidate > bankruptcy-offer`** (AA4/R6A10). Producers append/dedup/update-in-place;
    the live ack is a **compare-and-set on `{id, gen, rev}`** of the record actually shown (a moved `gen`/`rev`
    fails the CAS → re-read, never tombstone a stale head), and **ANY record resolved intra-day — not just the
    shown head — is tombstoned by the live turn through the SAME CAS (CC10)**.
  - **RETIRE list — `forzare/state/sweep-exclusion.json`** (Z13, spec §4c/§8a): the helper appends an id when
    bankruptcy RETIREs an undated someday item (reversible by deleting the entry; no label, no delete, no
    re-parent).
- [ ] **Step 3: I/O guarantees (V2/Z2/AA3), applied to ALL THREE stores (MAP + JOURNAL + QUEUE):** **(a)** an
  exclusive `flock` on a sibling lock around every read-modify-write; **(b)** atomic writes — temp file in the
  same dir → `fsync` → `os.rename`; **(c)** the **operation record is a JOURNAL line** in the **unified shape**
  `{ts, type, target, op, args, old_value, intended_value, external_marker?, reconcile_date, commit_state}`
  (CC8 — identical in spec §4d/§8a) — **NOT a history array on the MAP entry (AA3 — the MAP
  keeps its 4-field schema `{written_due, roll_count, last_escalated, kind}`; the B0 contradiction is swept)**;
  **(d)** the **journal-then-commit** write order — journal the intent (`pending`) → perform the state-chosen
  write → commit (flip `pending`, stamp new value); **(e)** the **THREE-WAY healing rule (AA3)** — on the next
  run, re-verify any `pending` entry against Todoist, comparing the live value to BOTH `old_value` and
  `intended_value` (and the `external_marker` where one exists): **= `intended_value` ⇒ commit (landed); =
  `old_value` ⇒ re-apply then commit (absent); = NEITHER (OTHER/user-changed) ⇒ ABORT + FLAG — void the entry
  and surface it, NEVER overwrite the user's value** (a silent replay onto a user edit would destroy it).
  **journal-then-commit + the three-way heal are defined for EVERY journal `type`, each with a TYPE-SPECIFIC
  predicate (Z3/AA3):**
  - **`date-op`** → live `due.date` vs `old_due`/`new_due`.
  - **`comment`** → the target task has a comment with the journaled content (content + task lookup).
  - **`calendar`** → the 🤖-calendar event exists **by its stable event key** (the key IS the `external_marker`).
  - **`label` / `p1` / `description`** → the task's **current value** vs `old_value`/`intended_value`.
  - **`task.add`** → **NO native idempotency (verified: `td task add` has no dedup/idempotency flag, only
    `--dry-run`) — a PRE-PERSISTED journal intent + a healing MARKER, NOT a content search (BB3 — corrects AA3's
    content+project search, which mis-heals on a collision, rename, or project move):** at create the helper
    generates a `journal-uuid`, **journals the intent (uuid as the `external_marker`) BEFORE the `td task add`**,
    and appends a hidden `⟦fz:<journal-uuid>⟧` line to the task's `--description` (verified `td task add
    --description`). Healing **searches Todoist for a task whose description carries that marker** — landed iff
    one exists (commit; **strip the marker on commit-verify**), **absent iff none ⇒ replay**, **no marker found
    ⇒ ABORT + FLAG (never a blind content-search replay that could double-create)**. NOT an idempotency-key
    lookup (the Inbox-task-id idempotency key is Kanban's, on the *card*, not on the `td` task).
  - **`task.complete`** → the task reads completed.
  - **`waiting-clear` (composite, AA3)** → the 02:00 unblock's clear-`@waiting` + re-date + `kind` flip
    (`waiting_checkback → surfacing`) is ONE composite pending transition, healed atomically (landed iff all
    three hold; a partial re-applies the whole transition; OTHER voids+flags) — never a half-applied unblock.
  - **`undate` (bankruptcy UNDATE, BB3)** → landed iff the task's `due` is now null; absent iff still
    `old_value`; OTHER (user re-dated) voids+flags.
  - **`retire` (bankruptcy RETIRE, BB3)** → landed iff the id is present in `sweep-exclusion.json`; absent iff
    not (re-append). A state-file op — its "live value" is the exclusion list.

  The **QUEUE obeys the same `flock` + atomic-replace, and its ack is a compare-and-set on `{id, gen, rev}` that
  TOMBSTONES the record** (BB2); producers dedup by `id`, a changed `proposed` updates IN PLACE + `rev++`, a
  re-enqueue of a tombstoned `id` opens `gen+1`/`rev=1`, and promotion sets the `head` flag (the primary sort
  key), AA4/BB2. Under the
  dry-run instruction the helper **appends the intended write + journal op to
  `forzare/state/dryrun-intents.jsonl` and performs neither** (R3A1); **staged external writes also carry the
  native `td --dry-run` flag** (Y4).
- [ ] **Step 4: Fixtures** — crash after (c)/after the write/after commit; a concurrent **EOD roll × live
  snooze** on one task id (the lock serializes; the loser re-reads and no-ops via the same-day dedupe); an
  **undated-task initial date** (asserts `td task update --due`, not `reschedule`) vs an **already-dated
  re-date** (asserts `td task reschedule`); **one fixture per `kind` (X5):** `surfacing` + `leadtime` (both
  roll-eligible), `waiting_checkback` + `user_fixed` (both roll-EXCLUDED — asserted absent from B7 Step 1's
  roll set).
  - **After-write crash fixture PER JOURNAL `type` (Z3/AA3/BB3), each exercising ALL THREE heal outcomes:** one
    each for `date-op` / `comment` / `calendar` / `label` / `p1` / `description` / `task.add` / `task.complete` /
    `waiting-clear` / `undate` / `retire` — crash between the journal-`pending` and the commit, then seed the live
    value to each of (intended ⇒ **commit**), (old ⇒ **re-apply**), and (a THIRD user-changed value ⇒ **ABORT +
    FLAG, user value NOT overwritten**), asserting the type-specific predicate resolves each correctly. **The
    `task.add` fixture asserts the healing-MARKER path (BB3) with COLLISION / RENAME / MOVE cases** — a
    same-content sibling (collision: content search would false-match, the marker does not), a task renamed after
    create (content search fails, marker resolves), and a task moved to another project (project search fails,
    marker resolves) — plus a **no-marker ⇒ ABORT + FLAG** case (never a blind replay). The `waiting-clear`
    fixture asserts the **composite** transition heals atomically (a partial re-applies the whole
    clear+redate+flip); the `undate`/`retire` fixtures assert the bankruptcy ops heal (undate landed iff due
    null · retire landed iff on the exclusion list). The `description`/`task.add`/`task.complete`/`undate`/`retire`
    writes are asserted **present in the JOURNAL** so W7 exclusion stays complete (Y5/R5A11/Z3/BB3).
  - **Decision-queue concurrency fixtures (Z2/AA4/BB2):** a **producer race** (two producers enqueue the same
    `id` ⇒ exactly one record, the second a no-op); a **duplicate reconcile** (re-enqueue of an unchanged
    decision ⇒ no-op); an **IN-PLACE update** (a producer re-touches an existing `id` with a *different*
    `proposed` ⇒ the SAME record updates and `rev` increments — assert NO duplicate record and `rev == 2`); an
    **ack-vs-promotion race** (the head's `gen`/`rev` moves between the live turn's read and its ack ⇒ the CAS
    fails and the turn re-reads rather than tombstoning a stale head); an **ack-then-reenqueue** (ack tombstones
    `{id, gen 1}`; a later re-enqueue of the SAME `id` opens a fresh `gen 2`, `rev 1` record — assert it is NOT
    suppressed by the stale ack); a **delayed-answer** (a decision acked a day after it was shown ⇒ its tombstone
    prevents a re-ask, while a genuinely new occurrence re-asks under `gen 2`); an **AGGREGATE-id** case (a
    `q1-conflict` keyed `q1-conflict:<date>` and a `bankruptcy-offer` keyed `bankruptcy-offer:<YYYY-MM>` each
    dedupe on their period key, BB2); and a **non-head intra-day resolution (CC10)** — a `stall-decision` settled
    mid-day by a live turn ⇒ the same CAS tombstones it ⇒ a subsequent brief read does NOT re-surface it.

  **Acceptance:** the helper is authored before B1; verb selection is state-chosen; the MAP, JOURNAL, and QUEUE
  are three distinct stores (map pruned on terminal state, journal retained 45 days, queue under the same
  lock/atomic-replace); journal-then-commit + the **per-type healing predicate** hold for every `type`
  (incl. `task.add` marker path with collision/rename/move, `undate`, `retire`, BB3); the
  `description`/`task.add`/`task.complete`/`undate`/`retire` fixtures land in the JOURNAL; the queue concurrency
  fixtures pass (dedup-by-`id` incl. aggregate ids, `{id, gen, rev}` CAS tombstone, ack-then-reenqueue → `gen+1`,
  non-head resolution, Z2/BB2/CC10).

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
  **`todoist-surface` OWNS the receptivity gate (V8/R6A3, spec §6a).** Before surfacing, it evaluates the v1
  deterministic rule from the **configurable N/D/S** (`receptivity_initiation_window_min` = N=30,
  `receptivity_dismissal_threshold` = D=3, `receptivity_surfacing_cap` = S=8; B10 `skills.config`): **withhold
  (provide-nothing) when trailing-24h dismissals ≥ D OR `surfacings_today` ≥ S**, else proceed. This is the one
  named owner of the gate logic — no scattered per-skill receptivity checks.
  **Stalled-task branch — the named if-then owner (X13, spec §7/§13).** When `todoist-surface` would surface a
  task whose ledger `roll_count ≥ 2`, it emits **this decision as the one thing** instead of the task:
  decompose / if-then / drop, no-shame, single-decision (clarify buttons on a live session, a plain question
  otherwise). A chosen **if-then is agent-proposed** — Bob composes a concrete "when `<cue>`, I `<first
  action>`" and **persists it to the task's description** via the centralized mutation layer (Task B0;
  journaled as a description/comment write, X11). Research traceability: the if-then lever is d=0.65 overall
  (Gollwitzer & Sheeran 2006) / d=0.99 self-regulation-impaired (Toli et al. 2016) — spec §6a/§7.

- [ ] **Step 2: Skill-integrity note (NO pin — AA11).** `todoist-surface` is repo-authored (chezmoi-dropped),
  so it is not a curator GC candidate — no `hermes curator pin`. Its protection is the installed-path +
  content-hash assertion at the SKILL-INTEGRITY GATE (end of Phase B). Just confirm the dir installed:

```bash
set -o pipefail
[ -d ~/.hermes/skills/todoist-surface ] && echo "todoist-surface installed OK (no pin needed — AA11)"
```

- [ ] **Step 3: Staged dry-run against `[TEST]` tasks** (structured add — no NL date parse; `.results[]`;
  `--yes` delete)

```bash
set -o pipefail
# helpers jid_from_create / stage_skill / $DRY defined in the Phase B intro
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
# BB6 staging-harness safety: run-id-scoped fixture + a trap that deletes ONLY the captured id (no prefix sweep).
RUNID="$(date +%s)-$$"; CREATED=()
trap 'for id in "${CREATED[@]}"; do td task delete "$id" --yes >/dev/null 2>&1 || true; done' EXIT INT
# disposable fixture (structured add, unprefixed label) — the TEST script's own write, deleted by the trap
TID=$(td task add "[TEST-$RUNID] deep surfacing probe" --labels "deep" --due today --json | jq -r '.id')
[ -n "$TID" ] && [ "$TID" != null ] || { echo "FATAL: fixture task not created" >&2; exit 1; }
CREATED+=("$TID")
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
# the fixture's REAL label set is untouched by a dry-run (purity) — read via --filter/--all scoped to THIS run's
# prefix (R3A5: default --limit is 300 of ~2270 tasks, a bare list can miss it):
LBLS=$(td task list --filter "search: [TEST-$RUNID]" --all --json | jq -r --arg id "$TID" '.results[]|select(.id==$id)|.labels|join(",")')
[ "$LBLS" = deep ] || { echo "FATAL: dry-run mutated the fixture's real labels ($LBLS)" >&2; exit 1; }
echo "dry-run purity OK (fixture's real label set untouched)"
# clean up — the captured id via the EXIT/INT trap (BB6: no `search: [TEST]` prefix sweep that could delete a
# concurrent run's or a real user's [TEST] task):
hermes cron remove "$JID"
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
**Receptivity-gate staged acceptance + THRESHOLD-BOUNDARY tests (R6A3/V8, `todoist-surface` owns the gate):**
seed the calibration store with dismissal/surfacing fixtures and stage a surface for each boundary, asserting
**withhold-intent (provide-nothing) is logged** exactly at/over threshold and a surface below it:
- **Dismissals: 2 vs 3 (D=3).** 2 trailing-24h dismissals ⇒ **surface** (below D); 3 ⇒ **withhold** (a
  `provide-nothing` decision logged, no task surfaced).
- **Surfacings: 7 vs 8 (S=8).** `surfacings_today == 7` ⇒ **surface**; `== 8` ⇒ **withhold**.

Each boundary asserts the gate output (surface vs withhold) from the audit/intent, so the N/D/S constants are
exercised, not just declared.

---

### Task B2: `weather` (Open-Meteo + NWS fallback)

**File:** `~/.hermes/skills/weather/SKILL.md`

- [ ] **Step 1: Author** — pull the day's relevant outdoor window (bike-to-gym; work-commute on work days),
  flag ONLY on the config thresholds (wind > 17 mph · any rain · < 50°F · > 90°F), quiet when clear (spec §2).
  **Source Open-Meteo (keyless); NWS as fallback** on Open-Meteo failure. Degrade-and-note on total failure
  ("weather unavailable — assume layers", spec §16) — never crash the brief.
- [ ] **Step 2: Dry-run** (NO curator pin — AA11: repo-authored, not a curator GC candidate; integrity is the
  content-hash gate)

```bash
set -o pipefail
curl -fsS -m 5 'https://api.open-meteo.com/v1/forecast?latitude=39.7&longitude=-105&hourly=temperature_2m,precipitation,wind_speed_10m&temperature_unit=fahrenheit&wind_speed_unit=mph' | jq '.hourly|keys'

# R6A11: SYNTHETIC threshold-crossing harness — drive the skill's classifier off fixed JSON, so clear / breach
# / degrade are each ASSERTED deterministically (not "eyeball a live forecast"). The skill exposes a
# pure classify entrypoint reading a forecast blob + the thresholds; feed it three fixtures:
python3 ~/.hermes/skills/weather/classify.py <<'JSON'   # adjust to the skill's real classify entrypoint
{"thresholds":{"wind_mph":17,"rain":"any","temp_low_f":50,"temp_high_f":90},
 "cases":[
  {"name":"clear",  "hourly":{"wind_speed_10m":[8],"precipitation":[0.0],"temperature_2m":[62]}, "expect":"clear"},
  {"name":"breach", "hourly":{"wind_speed_10m":[22],"precipitation":[0.3],"temperature_2m":[38]},"expect":"prep"},
  {"name":"degrade","hourly":null, "source_error":"open-meteo + NWS both unreachable", "expect":"degrade"}
 ]}
JSON
# the classifier must return: clear ⇒ a one-line "clear" verdict; breach ⇒ an actionable prep line naming the
# breached factor(s); degrade ⇒ the "weather unavailable — assume layers" note (never a crash). The harness
# asserts each case's verdict equals its `expect` and EXITS NONZERO on any mismatch.
```

Expected: Open-Meteo returns hourly temp/precip/wind; the synthetic harness proves clear/breach/degrade
each classify correctly. **Acceptance (R6A11):** the three synthetic threshold-crossing cases each assert their
verdict — **clear** (wind 8 / no rain / 62°F ⇒ one-line "clear"), **breach** (wind 22 > 17, 0.3" rain, 38°F <
50°F ⇒ actionable prep line), **degrade** (both sources unreachable ⇒ "weather unavailable — assume layers",
no crash) — the harness fails loud on any mismatch; not an eyeballed live forecast.

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
# BB5: pass an explicit account (-a); do not mask a broken gog with `2>/dev/null` — surface the failure.
GOG_ACCT="${GOG_ACCT:?set GOG_ACCT to the authenticated Google account (BB5)}"
gog calendar calendars -a "$GOG_ACCT" -j | jq -r '(.calendars // .)[]?.summary' | grep -i '🤖\|bob' \
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
- [ ] **Step 3: The TWO verification paths (NO curator pin — AA11) (R5A6 — contradiction removed).** The old acceptance
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
# NO curator pin (AA11) — calendar-read/calendar-write are chezmoi-dropped, not curator GC candidates.
# `gog auth status` exits 0 even when the API isn't actually reachable — verify with a REAL, account-scoped call
# (BB5: explicit -a). This ONE `||` deliberately surfaces the re-auth REPAIR (spec §16), not a masked-to-zero
# leak gate — the leak-gate calendar snapshot (C2) is the one that must be FATAL-on-failure.
GOG_ACCT="${GOG_ACCT:?set GOG_ACCT to the authenticated Google account (BB5)}"
gog calendar calendars -a "$GOG_ACCT" -j >/dev/null 2>&1 && echo "gog API reachable (real call OK)" \
  || echo "gog auth broken — surface the re-auth repair (spec §16): gog auth add $GOG_ACCT"
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
  **The single actionable line — the queue-head decision OR the do-now close — is emitted with a leading `▶ `
  SCHEMA MARKER, and NO other line ever carries `▶ ` (BB10, spec §2):** context lines (weather, activation, the
  ≤3) render non-actionable, so the exactly-one-action gate is a mechanical `▶ `-marker count == 1 in BOTH queue
  states. **Names the shared phrasing-rotation directive (R3A17, Phase B intro)** for its fixed lines — it does
  not re-derive a rotation. The brief is the **one bounded exception** to the one-per-response rule (spec §0/W12):
  read-only context that still ends with exactly ONE `▶ ` action.
- [ ] **Step 4: Three concrete harnesses on REAL SEEDED fixtures — NO curator pin (AA11); response-section-only
  parsing + cardinality-EXACTLY-1 (R7A4/AA10); schedule-deterministic resume with an off-day variant (R7A6)**

```bash
set -o pipefail
# NO curator pin (AA11) — eisenhower-plan/activation-prompt/brief-assemble are chezmoi-dropped, integrity is the
# content-hash gate. Helper: extract ONLY the audit's ## Response section (R7A4) — the audit .md EMBEDS the
# ## Prompt, whose skill-instruction text contains imperative-shaped lines that would over-count.
resp_only(){ awk '/^## *Response/{f=1;next} /^## /{f=0} f' "$1"; }
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
POD=~/workspaces/Ivy/forzare/state/plan-of-day.json
Q=~/workspaces/Ivy/forzare/state/decision-queue.json
OVR=~/workspaces/Ivy/forzare/state/schedule-override.json
DRY='DRY RUN — record intended writes to forzare/state/dryrun-intents.jsonl, perform none. '
TODAY=$(TZ=America/Denver date +%F)
# BB6 staging-harness safety: a per-run id, run-id-suffixed backups, a trap that restores them + deletes ONLY
# the ids this run created (never a `search: [TEST]` prefix sweep).
RUNID="$(date +%s)-$$"; CREATED=()
cp "$POD" "$POD.bak.$RUNID" 2>/dev/null || true; cp "$OVR" "$OVR.bak.$RUNID" 2>/dev/null || true
restore_b4(){ [ -f "$POD.bak.$RUNID" ] && mv "$POD.bak.$RUNID" "$POD" || rm -f "$POD"; \
  [ -f "$OVR.bak.$RUNID" ] && mv "$OVR.bak.$RUNID" "$OVR" || rm -f "$OVR"; \
  [ -f "$Q.bak.$RUNID" ] && mv "$Q.bak.$RUNID" "$Q"; \
  for id in "${CREATED[@]}"; do td task delete "$id" --yes >/dev/null 2>&1 || true; done; }
trap restore_b4 EXIT INT

# (1) PLAN-OF-DAY RESUME — SCHEDULE-DETERMINISTIC via a pinned fixture schedule (R7A6). A WORK-DAY fixture
#     (a work block today, so a deep window + a leave-time alarm are due) with ONE write flag OFF: a dry re-run
#     journals EXACTLY the one missing write (the alarm) and NO p1.set. seed two real [TEST-$RUNID] tasks.
TA=$(td task add "[TEST-$RUNID] pod-a" --priority p1 --due today --json | jq -r '.id'); CREATED+=("$TA")
TB=$(td task add "[TEST-$RUNID] pod-b" --priority p1 --due today --json | jq -r '.id'); CREATED+=("$TB")
printf '{"pinned_schedule":{"work_block":{"start":"15:00","end":"23:00"}},"activation":null}\n' > "$OVR"   # WORK day
cat > "$POD" <<JSON
{"date":"$TODAY","selected_ids":["$TA","$TB"],"anchor":"$TA","writes":{"p1_set":true,"anchor_placed":true,"alarm_set":false}}
JSON
: > "$INTENTS"
JR=$(stage_skill '0 0 1 1 *' "${DRY}Run eisenhower-plan in morning mode using the pinned fixture schedule in schedule-override.json; resume today's plan-of-day." eisenhower-plan test-pod-resume); hermes cron remove "$JR"
RID=$(jq -rs 'map(.run_id)|last // empty' "$INTENTS")
NSET=$(jq -s --arg r "$RID" '[.[]|select(.run_id==$r and .op=="p1.set")]|length' "$INTENTS")
NALARM=$(jq -s --arg r "$RID" '[.[]|select(.run_id==$r and .op=="calendar.create")]|length' "$INTENTS")
[ "$NSET" = 0 ] || { echo "FATAL: resume re-ran the already-done p1.set ($NSET) — must resume ONLY missing writes (Y13/R7A6)" >&2; exit 1; }
[ "$NALARM" -ge 1 ] || { echo "FATAL: resume did not complete the one missing write (alarm_set) on a WORK day (R7A6)" >&2; exit 1; }
echo "plan-of-day resume (work-day) OK: only the missing alarm write journaled, no p1 top-up"
# OFF-DAY VARIANT (R7A6): a fixture with NO work block ⇒ the resume completes with NO alarm intent at all.
printf '{"pinned_schedule":{"work_block":null},"activation":null}\n' > "$OVR"                              # OFF day
printf '{"date":"%s","selected_ids":["%s","%s"],"anchor":"%s","writes":{"p1_set":true,"anchor_placed":true,"alarm_set":false}}\n' "$TODAY" "$TA" "$TB" "$TA" > "$POD"
: > "$INTENTS"
JRO=$(stage_skill '0 0 1 1 *' "${DRY}Run eisenhower-plan morning mode using the pinned OFF-day schedule; resume today's plan-of-day." eisenhower-plan test-pod-resume-off); hermes cron remove "$JRO"
RIDO=$(jq -rs 'map(.run_id)|last // empty' "$INTENTS")
NALARMO=$(jq -s --arg r "$RIDO" '[.[]|select(.run_id==$r and .op=="calendar.create")]|length' "$INTENTS")
[ "$NALARMO" = 0 ] || { echo "FATAL: OFF-day resume journaled a leave-time alarm ($NALARMO) — no work block, no alarm (R7A6)" >&2; exit 1; }
echo "plan-of-day resume (off-day) OK: NO alarm intent (day-of-week dependence removed, R7A6)"

# (2) >3-Q1 CONFLICT — SEED FOUR real [TEST-$RUNID] deadline-today tasks (R7A3): the plan caps at ≤3 p1.set and
#     enqueues ONE q1-conflict record (AGGREGATE id q1-conflict:<date>, BB2), NEVER a 4th p1, never a silent drop.
Q1=(); for n in 1 2 3 4; do id=$(td task add "[TEST-$RUNID] q1-$n" --due today --deadline "$TODAY" --json | jq -r '.id'); Q1+=("$id"); CREATED+=("$id"); done
: > "$INTENTS"
JC=$(stage_skill '0 0 1 1 *' "${DRY}Run eisenhower-plan morning mode against the FOUR [TEST] tasks that each carry a deadline TODAY; cap p1 at 3." eisenhower-plan test-q1-conflict); hermes cron remove "$JC"
RID=$(jq -rs 'map(.run_id)|last // empty' "$INTENTS")
NP1=$(jq -s --arg r "$RID" '[.[]|select(.run_id==$r and .op=="p1.set")]|length' "$INTENTS")
[ "$NP1" -le 3 ] || { echo "FATAL: eisenhower-plan set $NP1 p1 — must cap at 3 and surface a q1-conflict (INV-5/AA4)" >&2; exit 1; }
# structured action-field validation over verb-grep (AA10): the enqueue intent's args.class must be q1-conflict
# AND its id the AGGREGATE key q1-conflict:<date> (BB2 — no single task_id for a same-day capacity conflict).
jq -e --arg r "$RID" --arg d "$TODAY" 'select(.run_id==$r and .op=="state-write" and (.target|test("decision-queue")) and .args.class=="q1-conflict" and (.args.id=="q1-conflict:"+$d))' "$INTENTS" >/dev/null \
  || { echo "FATAL: >3-Q1 collision did not enqueue a q1-conflict record keyed q1-conflict:$TODAY (never silently drop, §4c/AA4/BB2)" >&2; exit 1; }
echo ">3-Q1 conflict OK: ≤3 p1.set + a q1-conflict:$TODAY aggregate-id record enqueued (AA4/AA10/BB2)"

# (3) EXACTLY-ONE ACTION via the `▶ ` SCHEMA MARKER (BB10) — count `▶ ` markers == 1 in BOTH queue states; the
#     verb regex stays only as SECONDARY evidence. Parse the ## Response section ONLY (R7A4). Seed uses BB2
#     fields (gen/head) + the FORZARE_NOW/synthetic_weather/pinned_schedule staging overrides (CC4).
cp "$Q" "$Q.bak.$RUNID" 2>/dev/null || true
# (3a) QUEUE NON-EMPTY ⇒ the queue head is the sole `▶ ` line.
printf '{"records":[{"id":"waiting-chase:%s","class":"waiting-chase","task_id":"%s","proposed":"chase","status":"pending","enqueue_ts":"%sT02:00:00Z","gen":1,"rev":1,"head":false}]}\n' "$TA" "$TA" "$TODAY" > "$Q"
printf '{"pinned_schedule":{"work_block":{"start":"15:00","end":"23:00"}},"activation":"pending","synthetic_weather":{"breach":"rain 6am"}}\n' > "$OVR"
: > "$INTENTS"
JI=$(stage_skill '0 0 1 1 *' "${DRY}Assemble the brief from the seeded decision-queue head + the synthetic weather breach + the pending activation in schedule-override.json. Emit the single action as a '▶ ' marker line." brief-assemble test-brief-marker-nonempty)
RESP=$(mktemp); resp_only ~/.hermes/cron/output/"$JI"/*.md > "$RESP"; hermes cron remove "$JI"
MARKS=$(grep -c '▶ ' "$RESP" || true)
[ "${MARKS:-0}" -eq 1 ] || { echo "FATAL: queue-NON-EMPTY brief has $MARKS '▶ ' marker lines — must be EXACTLY 1 (0 = head not surfaced; >1 = a wall) (BB10/Z12/W12)" >&2; exit 1; }
# secondary evidence only (BB10): the verb/question shape agrees, but the marker count is the gate.
VERB=$(grep -cE '\?[[:space:]]*$|^[[:space:]]*▶ *(First|Start|Do|Decide|Chase|Pick|Break|Drop|Reschedule|Undate|Retire):' "$RESP" || true)
echo "queue-nonempty OK: exactly 1 '▶ ' marker (verb-shape secondary = $VERB, BB10)"; rm -f "$RESP"
# (3b) QUEUE EMPTY ⇒ the do-now close is the sole `▶ ` line (the marker gate must hold here TOO, BB10).
printf '{"records":[]}\n' > "$Q"
: > "$INTENTS"
JE=$(stage_skill '0 0 1 1 *' "${DRY}Assemble the brief with an EMPTY decision queue; close on the single do-now action as a '▶ ' marker line." brief-assemble test-brief-marker-empty)
RESP=$(mktemp); resp_only ~/.hermes/cron/output/"$JE"/*.md > "$RESP"; hermes cron remove "$JE"
MARKS_E=$(grep -c '▶ ' "$RESP" || true)
[ "${MARKS_E:-0}" -eq 1 ] || { echo "FATAL: queue-EMPTY brief has $MARKS_E '▶ ' marker lines — must be EXACTLY 1 (BB10)" >&2; exit 1; }
echo "queue-empty OK: exactly 1 '▶ ' marker on the do-now close (BB10 — marker gate holds in BOTH states)"
rm -f "$RESP"
mv "$Q.bak.$RUNID" "$Q" 2>/dev/null || rm -f "$Q"
# teardown is the EXIT/INT trap (restore_b4) — captured-id deletion only, never a `search: [TEST]` sweep (BB6).
echo "B4 fixtures torn down by trap (captured ids only, BB6)"
```

**Acceptance:** NO curator pin (AA11 — integrity is the content-hash gate). `eisenhower-plan` in morning mode
never assigns >3 p1 and, **guarded by `plan-of-day.json` (Y13, NOT a p1 count)**, is a no-op when today's plan
record already exists — the **resume harness runs on a PINNED fixture schedule** (work-day: journals ONLY the
one missing alarm write, never tops p1 past `selected_ids`; **off-day variant: completes with ZERO alarm
intent** — day-of-week dependence removed, R7A6); the **>3-Q1 conflict harness seeds FOUR real [TEST]
deadline-today tasks** and proves the plan caps at ≤3 `p1.set` and enqueues **one `q1-conflict` record**
(structured `args.class` check, AA4/AA10 — never a 4th p1, never a silent drop, INV-5); in EOD mode writes zero
p1; in replan mode redraws only the remaining day, proposes (never applies) p1 changes, never touches a fixed
anchor (W10); `brief-assemble` yields the ordered brief and — **the exactly-one-action harness parses the
`## Response` section ONLY (R7A4) and counts the machine-readable `▶ ` SCHEMA MARKER (BB10) == 1 in BOTH queue
states** (non-empty: the queue head is the sole `▶ ` line; empty: the do-now close is; 0 ⇒ head/close not
surfaced ⇒ FAIL, >1 ⇒ a wall ⇒ FAIL), the verb regex kept only as secondary evidence; the q1-conflict enqueue
uses the AGGREGATE id `q1-conflict:<date>` (BB2). Fixtures are `[TEST-$RUNID]`-scoped and torn down by the
EXIT/INT trap (captured ids only, run-id-suffixed state backups restored — BB6), never a `search: [TEST]` sweep.

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
    **≤5 oldest/most-stale** candidates from the **sweep pool = union(oldest UNDATED someday items,
    long-cycling DATED actives — `roll_count ≥ 10` AND no progress ≥ 30 days, R7A5)** (skipping any id on the
    `sweep-exclusion.json` RETIRE list, Z13) and **enqueues them to `forzare/state/decision-queue.json`** as
    `sweep-candidate` records (`{id:"sweep-candidate:"+candidate_id, class:"sweep-candidate", candidate_id,
    proposed, status:pending, enqueue_ts, rev}`, Z2/AA4); when the stale set **exceeds 25** it additionally
    enqueues the opt-in **task-bankruptcy** offer as one **`bankruptcy-offer`** record (the lowest class-rank,
    AA4; spec §4c/§19). **Bankruptcy is a REVERSIBLE, TWO-CLASS clear (Y3/Z13) — NEVER delete/complete/archive.**
    The confirmed batch op applies the op that fits each item's class over a **frozen, journaled** id set:
    - **stale DATED actives** (`roll_count ≥ 10` AND no progress ≥ 30 days — R7A5; NOT "a due that never moved,"
      a contradiction since the nightly roll re-stamps `written_due`) ⇒ **UNDATE** (strip the due → back to
      hidden someday);
    - **undated someday items** (no due to strip) ⇒ **RETIRE** (append the id to `sweep-exclusion.json` so the
      monthly sweep never re-proposes it — no label, no delete, no re-parent; reversible by deleting the entry).

    With a bounded summary, a confirmation that **names the operation** ("undate N dated + retire M someday —
    reversible"), and **idempotent partial-failure recovery** (a re-run reads the frozen journaled set and
    completes only the ids not yet undated/retired). **Two distinct incidents motivate reversibility, cited
    separately (R6A7):** the **Todoist parent-delete cascade** (deleting a parent deletes its subtasks;
    STATUS:76, 3 videos — `td activity --json` recovers) and the **separate 2026-05-20 Obsidian-vault refactor**
    (a bulk rename/merge that deleted 31 files). **State-only — never messages** (the brief-mode read is the
    sole delivery). This mode owns the sweep-marking logic (R4A6).
- [ ] **Step 2: `daily-reflect`** — EOD report half: completions-as-wins (no scorecard, no misses list),
  receptivity-gated (spec §8). Gain-framed, task-level (spec §7). **Completion beats name the shared
  phrasing-rotation directive** (R3A17, Phase B intro).
- [ ] **Step 3: `tomorrow-prep`** — EOD pre-stage of tomorrow's candidate anchor + ≤3 (proposal only; spec
  §5b/§8).
- [ ] **Step 4: Head/ack + the TWO-PHASE bankruptcy over REAL SEEDED ids — NO curator pin (AA11); offer =
  ZERO clear intents, acknowledged op = UNDATE + RETIRE over the frozen seeded set (AA6/R7A5/R7A3)**

```bash
set -o pipefail
# NO curator pin (AA11). helpers from the Phase B intro.
Q=~/workspaces/Ivy/forzare/state/decision-queue.json
MAP=~/workspaces/Ivy/forzare/state/task-lifecycle.json
EXCL=~/workspaces/Ivy/forzare/state/sweep-exclusion.json
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
DRY='DRY RUN — record intended writes to forzare/state/dryrun-intents.jsonl, perform none. '
# BB6 staging-harness safety: per-run id, run-id-suffixed backups, trap that restores + deletes captured ids ONLY.
RUNID="$(date +%s)-$$"; CREATED=()
cp "$Q" "$Q.bak.$RUNID" 2>/dev/null || true; cp "$MAP" "$MAP.bak.$RUNID" 2>/dev/null || true
restore_b5(){ [ -f "$Q.bak.$RUNID" ] && mv "$Q.bak.$RUNID" "$Q" || rm -f "$Q"; \
  [ -f "$MAP.bak.$RUNID" ] && mv "$MAP.bak.$RUNID" "$MAP" || true; \
  for id in "${CREATED[@]}"; do td task delete "$id" --yes >/dev/null 2>&1 || true; done; }
trap restore_b5 EXIT INT
# (1) HEAD ordering (AA4/R6A10/BB2): seed two pending records of DIFFERENT classes with STABLE ids + gen/head;
#     brief-mode emits the class-rank HEAD (waiting-chase > sweep-candidate), a single decision, never a list.
cat > "$Q" <<'JSON'
{"records":[
 {"id":"sweep-candidate:TESTold","class":"sweep-candidate","candidate_id":"TESTold","proposed":"keep","status":"pending","enqueue_ts":"2026-07-10T05:00:00Z","gen":1,"rev":1,"head":false},
 {"id":"waiting-chase:TESTwait","class":"waiting-chase","task_id":"TESTwait","proposed":"chase","status":"pending","enqueue_ts":"2026-07-11T02:00:00Z","gen":1,"rev":1,"head":false}
]}
JSON
: > "$INTENTS"
JID=$(stage_skill '0 0 1 1 *' "${DRY}Run followups-sweep in brief mode; emit the single head decision or [SILENT]." followups-sweep test-sweep-head)
RESP=$(mktemp); awk '/^## *Response/{f=1;next} /^## /{f=0} f' ~/.hermes/cron/output/"$JID"/*.md > "$RESP"
grep -q 'TESTwait' "$RESP" && ! grep -q 'TESTold' "$RESP" \
  || { echo "FATAL: brief-mode did not emit the class-rank HEAD (waiting-chase before sweep-candidate) (AA4)" >&2; exit 1; }
echo "queue HEAD ordering OK (waiting-chase head, single decision)"; rm -f "$RESP"
# (2) ACK PURITY (CC6 — supersedes the mtime compare): brief-mode is a cron/dry path, so it must journal NO
#     ack-shaped intent (a tombstone/CAS write); assert ZERO ack intents in the intents log (mtime compare dropped).
NACK=$(jq -s '[.[]|select(.op=="state-write" and (.target|test("decision-queue")) and ((.args.tombstone!=null) or (.args.status=="acked") or (.args.op=="ack")))]|length' "$INTENTS")
[ "$NACK" = 0 ] || { echo "FATAL: brief-mode journaled $NACK ack-shaped decision-queue intent — the CAS tombstone ack is a LIVE turn ONLY (R5A5/CC6)" >&2; exit 1; }
echo "ack purity OK (brief-mode journaled ZERO ack intents, CC6)"; hermes cron remove "$JID"
# (3) BANKRUPTCY — HONEST fixture (BB7): dated actives satisfy the REAL eligibility (ledger roll_count ≥ 10 AND
#     no-progress ≥ 30d), and >25 UNDATED someday candidates. Seed 14 backdated dated actives + a MAP entry each,
#     and 26 undated someday tasks.
FORTY=$(TZ=America/Denver date -v-40d +%F)   # 40 days ago (Denver) — the backdated written_due + old activity
D_IDS=(); for n in $(seq 1 14); do
  id=$(td task add "[TEST-$RUNID] bk-dated-$n" --due "$FORTY" --json | jq -r '.id'); D_IDS+=("$id"); CREATED+=("$id"); done
U_IDS=(); for n in $(seq 1 26); do
  id=$(td task add "[TEST-$RUNID] bk-undated-$n" --json | jq -r '.id'); U_IDS+=("$id"); CREATED+=("$id"); done
# seed a lifecycle MAP entry per dated active: roll_count 12 (≥10), kind surfacing, written_due = 40d ago (so
# "no progress ≥ 30d" is genuinely true — no completion/subtask/comment activity since). This is the REAL
# eligibility the SWEEP reads, not a prompt claim (BB7).
python3 - "$MAP" "$FORTY" "${D_IDS[@]}" <<'PY'
import json,sys
mp=sys.argv[1]; wd=sys.argv[2]; ids=sys.argv[3:]
m=json.load(open(mp)) if __import__("os").path.exists(mp) else {}
for i in ids: m[i]={"written_due":wd,"roll_count":12,"last_escalated":wd,"kind":"surfacing"}
json.dump(m,open(mp,"w"))
PY
# (3a) OFFER GENERATION: enqueues ONE bankruptcy-offer (aggregate id bankruptcy-offer:<YYYY-MM>, BB2) whose args
#      carry the FROZEN, JOURNALED snapshot (every id + its class/op) — and ZERO clear intents (AA6).
: > "$INTENTS"
JB=$(stage_skill '0 0 1 1 *' "${DRY}Run followups-sweep in SWEEP mode over the seeded stale set (ledger roll_count≥10 dated + >25 undated). FREEZE + JOURNAL the exact id set with per-id op into the bankruptcy-offer record's args; enqueue the OFFER only — clear NOTHING." followups-sweep test-sweep-offer); hermes cron remove "$JB"
MONTH=$(TZ=America/Denver date +%Y-%m)
jq -e --arg m "bankruptcy-offer:$MONTH" 'select(.op=="state-write" and (.target|test("decision-queue")) and .args.class=="bankruptcy-offer" and .args.id==$m and (.args.frozen|length>25))' "$INTENTS" >/dev/null \
  || { echo "FATAL: SWEEP did not enqueue a bankruptcy-offer:$MONTH with a FROZEN snapshot of >25 ids (Y3/AA4/BB2/BB7)" >&2; exit 1; }
NCLR=$(jq -s '[.[]|select(.op=="task.undate" or .op=="task.update-due" or .op=="sweep.retire" or .op=="task.complete" or .op=="task.delete")]|length' "$INTENTS")
[ "$NCLR" = 0 ] || { echo "FATAL: OFFER generation journaled $NCLR clear intent(s) — the offer is a PROPOSAL, nothing clears until acknowledged (AA6)" >&2; exit 1; }
echo "offer-generation OK: bankruptcy-offer:$MONTH enqueued with a frozen snapshot, ZERO clear intents (AA6/BB7)"
# (3b) ACKNOWLEDGED op CONSUMES the JOURNALED FROZEN SNAPSHOT — NOT a prompt-injected id list (BB7). Persist the
#      frozen offer into the queue, then the ack prompt says ONLY "the user accepted" — the skill reads
#      args.frozen to drive undate/retire. Assert task.undate per dated + sweep.retire per undated, no destructive op.
python3 - "$Q" "$MONTH" "$FORTY" "${D_IDS[@]}" "--U--" "${U_IDS[@]}" <<'PY'
import json,sys
q=sys.argv[1]; month=sys.argv[2]; wd=sys.argv[3]; rest=sys.argv[4:]
cut=rest.index("--U--"); dated=rest[:cut]; undated=rest[cut+1:]
frozen=[{"id":i,"class":"dated","op":"undate"} for i in dated]+[{"id":i,"class":"undated","op":"retire"} for i in undated]
json.dump({"records":[{"id":f"bankruptcy-offer:{month}","class":"bankruptcy-offer","proposed":"clear","status":"pending","enqueue_ts":wd+"T05:00:00Z","gen":1,"rev":1,"head":False,"frozen":frozen}]}, open(q,"w"))
PY
: > "$INTENTS"
JA=$(stage_skill '0 0 1 1 *' "${DRY}The user ACCEPTED the bankruptcy offer. Read the FROZEN snapshot from the bankruptcy-offer record in decision-queue.json (do NOT expect an id list in this prompt) and apply each item's op: UNDATE each dated, RETIRE each undated onto sweep-exclusion.json. Never delete/complete/archive." followups-sweep test-sweep-ack); hermes cron remove "$JA"
for id in "${D_IDS[@]}"; do
  jq -e --arg id "$id" 'select(.op=="task.undate" and .target==$id)' "$INTENTS" >/dev/null \
    || { echo "FATAL: dated active $id was not UNDATEd from the journaled snapshot (BB7/AA6)" >&2; exit 1; }
done
for id in "${U_IDS[@]}"; do
  jq -e --arg id "$id" 'select(.op=="sweep.retire" and (.args.ids|index($id)))' "$INTENTS" >/dev/null \
    || { echo "FATAL: undated someday $id was not RETIREd from the journaled snapshot (BB7/AA6/Z13)" >&2; exit 1; }
done
! jq -e 'select(.op=="task.complete" or .op=="task.delete")' "$INTENTS" >/dev/null \
  || { echo "FATAL: acknowledged bankruptcy journaled a DESTRUCTIVE op — must be UNDATE/RETIRE only (Z13)" >&2; exit 1; }
echo "acknowledged bankruptcy OK (BB7/AA6/Z13): ops driven by the JOURNALED snapshot, not the prompt; 14 UNDATE + 26 RETIRE, no destructive op"
# (3c) FAILURE BETWEEN MUTATION BATCHES → idempotent retry (BB7). Mark the first 7 dated + first 13 undated as
#      already-applied (seed sweep-exclusion with those undated ids; the retry must re-read the frozen snapshot
#      and journal ops ONLY for the NOT-yet-done remainder — never re-processing a completed id).
printf '{"ids":[%s]}\n' "$(printf '"%s",' "${U_IDS[@]:0:13}" | sed 's/,$//')" > "$EXCL"
: > "$INTENTS"
JR=$(stage_skill '0 0 1 1 *' "${DRY}RESUME the interrupted bankruptcy: re-read the frozen snapshot AND sweep-exclusion.json; apply ops ONLY for ids not already retired/undated (idempotent partial-failure recovery). Never re-process a completed id." followups-sweep test-sweep-retry); hermes cron remove "$JR"
# the 13 already-retired undated ids must NOT be retired again:
for id in "${U_IDS[@]:0:13}"; do
  ! jq -e --arg id "$id" 'select(.op=="sweep.retire" and (.args.ids|index($id)))' "$INTENTS" >/dev/null \
    || { echo "FATAL: retry RE-retired an already-done id $id — not idempotent (BB7)" >&2; exit 1; }
done
# the remaining 13 undated must be retired on the retry:
for id in "${U_IDS[@]:13}"; do
  jq -e --arg id "$id" 'select(.op=="sweep.retire" and (.args.ids|index($id)))' "$INTENTS" >/dev/null \
    || { echo "FATAL: retry did not complete the remaining undated id $id (BB7)" >&2; exit 1; }
done
echo "idempotent partial-failure retry OK (BB7): already-done ids skipped, remainder completed from the frozen snapshot"
rm -f "$EXCL"
# teardown = the EXIT/INT trap (restore_b5): captured-id deletion only, never a `search: [TEST]` sweep (BB6).
echo "B5 bankruptcy fixtures torn down by trap (captured ids only, BB6)"
```

**Acceptance:** NO curator pin (AA11). `daily-reflect` never lists misses; `followups-sweep` in brief mode emits
**only the single class-rank HEAD `pending` record** of the unified `decision-queue.json` (q1-conflict >
waiting-chase > fixed-redecision = stale-p1 > stall-decision > triage-reraise > sweep-candidate >
bankruptcy-offer; `waiting-chase` most-overdue first) and **never itself acks — it journals ZERO ack-shaped
intents (CC6 — the mtime compare is dropped; the CAS tombstone ack is the live turn's job, R5A5/Z2)**. In SWEEP
mode it enqueues `sweep-candidate` records and, past 25 stale candidates, one **`bankruptcy-offer:<YYYY-MM>`**
aggregate record (BB2) — and **OFFER generation FREEZES + JOURNALS the id set into the record's args and asserts
ZERO clear intents** (AA6/BB7). The **HONEST fixture (BB7)** seeds 14 dated actives that satisfy the REAL
eligibility — a lifecycle-MAP entry with `roll_count ≥ 10` and a 40-day-old `written_due` so "no progress ≥ 30d"
is genuinely true — plus **>25 undated someday** candidates; the **acknowledged clear CONSUMES the JOURNALED
frozen snapshot (NOT a prompt-injected id list)**, journaling **`task.undate` for every dated active** and
**`sweep.retire` onto `sweep-exclusion.json` for every undated someday**, **never a delete/complete/archive**; and
a **failure-between-batches fixture proves idempotent retry** (already-done ids skipped, only the remainder
completed from the frozen snapshot). Fixtures are `[TEST-$RUNID]`-scoped and torn down by the EXIT/INT trap
(captured ids only, BB6); `tomorrow-prep` proposes ≤3 without setting p1 (that's the morning's job).

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
- [ ] **Step 2: Classification staged dry-run** (staged cron, `--deliver local`; NOT `hermes -z … --safe-mode`;
  NO curator pin — AA11)

```bash
set -o pipefail
# helpers (incl. $DRY) from the Phase B intro. NO curator pin (AA11 — repo-authored, not a curator GC candidate).
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

# R6A9: PASSING-MENTION NEGATIVE fixtures — the over-trigger guard (spec §19). A passing mention of work/gym
# in a NON-report context must NOT classify as a state change and must journal NO schedule-override write.
for neg in "my friend works at a gym" "work was busy last year, glad that's over"; do
  : > "$INTENTS"
  JN=$(stage_skill '0 0 1 1 *' "${DRY}The user says: \"$neg\". Classify; act ONLY on a genuine own-state change; respond exactly [SILENT] when done." \
          forzare "test-forzare-neg")
  # NO schedule-override / state-write intent may be journaled for a passing mention:
  if jq -e 'select(.op=="state-write" and (.target|test("schedule-override")))' "$INTENTS" >/dev/null 2>&1; then
    echo "FATAL: passing mention \"$neg\" wrongly fired a schedule-override (over-trigger, R6A9)" >&2; exit 1
  fi
  echo "passing-mention negative OK — \"$neg\" did not fire (R6A9)"
  hermes cron remove "$JN"
done
```

**Acceptance:** each of the 4 signal classes routes correctly on clear phrasing; a low-confidence phrase
triggers the one-line confirm (button on live session); the shift signal journals a valid schedule-override
INTENT (block + date + recovery flag) to `dryrun-intents.jsonl` while the real file stays absent (R3A1/W2);
**passing-mention NEGATIVES ("my friend works at a gym", "work was busy" in a recap) do NOT fire — no
schedule-override intent is journaled (R6A9, the §19 over-trigger validation requirement)**; no
`pre_gateway_dispatch` hook is used (spec §3B/§12).

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
  --due` only where it *initially* dates an undated task (spec §4/W6). **BOB-OWNED `p1`-clear — ONLY the day's
  `plan-of-day.json` `selected_ids`, NEVER a user-set p1 (AA2, spec §8).** Clear `p1` from exactly the ids Bob
  set this morning (roll-excluded ones among them included); a `p1` the USER set directly is left untouched (it
  is not in `selected_ids`). **Enqueue a `stale-p1` record** for any user-set `p1` older than 48h (never
  auto-cleared). Tick `roll_count` (§4d; reset on progress). **Enumerate missed FIXED items and ENQUEUE each as
  a `fixed-redecision` record to the unified `decision-queue.json`** (Y1, spec §2/§8). **Escalation is MARKED,
  not messaged (R4A10):** at `roll_count == 2` stamp `last_escalated` as state **and ENQUEUE a `stall-decision`
  record to `decision-queue.json`** — EOD sends nothing at 23:00; the brief's `followups-sweep` delivers the
  head (spec §2/§7/§8). **Under dry-run, `eod-roll` reads its OWN journaled intents as shadow last-reconcile
  state (R7A10, Phase B intro) — its authored contract states this** so a second consecutive dry-run observes
  the first's intended stamp advance and logs an `already-reconciled` no-op (Step 4).
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
  task)**, and the **cutoff test points (X6), each driven DETERMINISTICALLY by the staging `FORZARE_NOW`
  clock override (CC4/CC12 — the authored staging field in `schedule-override.json`, Phase B intro): 22:59
  (before cutoff ⇒ CEILING = yesterday) / 23:00 (at cutoff ⇒ CEILING = today) / just-past-midnight (Denver
  rolled to D+1, before D+1's cutoff ⇒ still closes D) / a ≤2h catch-up / a manual mid-day `/forzare-eod`** —
  each yields the identical, once-only roll (`eod-roll` reads `FORZARE_NOW` for its cutoff math under the
  staging directive, so no test waits on the real wall-clock).
  **Dating fixtures (W6/X5 — one per date-writer path × kind):**
  a **user-dated** task (no ledger entry — never moves) · a **Bob lead-time** date on a deadline task
  (`kind: leadtime` — rolls) · a **capture-dated** task (§8b stage 2 — a **user-stated day** is `kind:
  user_fixed` and **never rolls**, X5; a lead-time capture is `kind: leadtime` and rolls) · a
  **planning-pull** promotion (`kind: surfacing`, initial `update --due`, then rolls) · a **`@waiting`
  check-back** date (`kind: waiting_checkback` — **never rolls, never ticks**, X5) · a **timed** task (`"T"`
  due — never mutated) · a **recurring** task (never mutated).
- [ ] **Step 2: `waiting-reconcile`** — the 02:00 owner (spec §8): **enqueue chase-due `@waiting` as
  `waiting-chase` records to `decision-queue.json`** (Y1) — **MOST-OVERDUE-FIRST, with strictly increasing
  `enqueue_ts` in that order (R7A11)**, so the queue's FIFO-within-class total order delivers the most-overdue
  chase first without a read-time re-sort; repair the §4b set-time invariant (dateless
  `@waiting` → near-term check-back + "auto-repaired" flag, then enqueue a `waiting-chase`); **unblock
  detection vs `gog` calendar + `td activity` ONLY — NOT "recent Discord" (R5A12)**: an amnesiac 02:00 cron
  session has no verified read path to chat history, so it relies only on the two signals it can actually read
  (opportunistic Discord-context clearing stays a *live-turn* path, spec §8); the `td activity` check queries
  **BOTH** the `--type task` AND the separate **`--type comment`** streams (comments are not in the task stream,
  Z14). **On a detected unblock the re-date rewrites the ledger `kind`: `waiting_checkback → surfacing`
  (Z14)** — the entry was a chase reminder (never rolls); once unblocked it becomes a surfacing date that
  **rolls normally**, so the re-date goes through the centralized helper (Task B0) which stamps the new `kind`
  + `written_due` together. 14-day staleness sweep enqueues a `waiting-chase`. **State-only — never delivers**
  (the morning `followups-sweep` reads the head). Run directly by the 02:00 cron, not in a bundle. **Fixtures
  (Z14):** a **calendar-only unblock** (the awaited event passed on the 🤖/primary calendar ⇒ clear + re-date,
  `kind` flips to `surfacing`, entry now rolls) and a **comment-only unblock** (a genuine user comment on the
  blocking task, found on the `--type comment` stream ⇒ same clear + `kind` flip) — each asserting the
  `waiting_checkback → surfacing` transition and that the task then joins the roll set.
- [ ] **Step 3: `transition`** — the §3a hyperfocus exit-ramps (soft pre-warning → one-last-thing → hard-stop
  anchor → capture re-entry) + the §3b task-transition ritual (close the loop on the outgoing task's next
  action, pre-stage the next one). Owns the deadline lead-time framing at hand-off and the exit-ramp cues
  (A31). Invoked at block boundaries + on `/forzare` transitions. **Its stall re-engages name the shared
  phrasing-rotation directive** (R3A17, Phase B intro). The §3a hard-stop rung is the 🤖-calendar leave-time
  alarm authored in `calendar-write` (W13, Task B3).
- [ ] **Step 4: STAGING idempotency via two consecutive DRY-RUNS (R3A3) — NO curator pin (AA11).** The old
  test read the real `last-reconcile.json` stamp across two staged runs — but under the dry-run instruction a
  correct `eod-roll` **never advances the real stamp** (it writes only `dryrun-intents.jsonl`), so a
  stamp-diff proves nothing. Instead, the idempotency contract is **observable store-free from the intents
  log**: under dry-run, `eod-roll` treats `dryrun-intents.jsonl` as its shadow last-reconcile, so a **second**
  consecutive dry-run observes the first's *intended* stamp advance and logs an **`already-reconciled` no-op
  intent**. The REAL double-roll guard (the real stamp advances once, a real second run no-ops) is a **G1
  go-live day-1 supervised check** (Task G1), not a dry-run.

```bash
set -o pipefail
# NO curator pin (AA11) — eod-roll/waiting-reconcile/transition are chezmoi-dropped, not curator GC candidates.
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

**Acceptance:** `eod-roll` rolls only the ledger-defined set (V1), clears `p1` from **ONLY the day's
`plan-of-day.json` `selected_ids` — never a user-set p1 (AA2/CC1; the G1 EOD gate seeds a Bob-owned + an
unrelated user-set p1 and proves the user one SURVIVES)**, and **enqueues
`fixed-redecision` + `stall-decision` records to `decision-queue.json`** (Y1, not messaged); the two-dry-run
probe logs an **`already-reconciled` no-op** on the second run and leaves **every** real store
(`last-reconcile.json`, the `task-lifecycle.json` MAP, `mutation-journal.jsonl`, `decision-queue.json`)
**untouched** (dry-run purity, Y5/Y1); the real double-roll guard is verified at G1 day-1; `waiting-reconcile`
enqueues `waiting-chase` records and sends nothing (unblock signals = gog calendar + `td activity` on **both**
`--type task` and `--type comment`, no Discord, R5A12/Z14), and on a detected unblock rewrites the ledger
`kind` `waiting_checkback → surfacing` so the task rolls (calendar-only + comment-only unblock fixtures, Z14);
`transition` produces the graduated ramp, never a hard yank.

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
- [ ] **Step 4: End-to-end staged test** (dry-run intents, per the Phase B pattern — R3A1; NO curator pin — AA11)

```bash
set -o pipefail
# helpers (incl. $DRY) from the Phase B intro. NO curator pin (AA11 — repo-authored, not curator GC candidates).
# Pipeline state is CONTROLLED: forzare-capture's stage 1 (td task add) is synchronous; stages 2–5 are the
# separate 60s Kanban dispatcher (D1), which a cron tick does NOT run — and under the dry-run directive stage 1
# journals its Inbox write instead of performing it.

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
  raw `updated`/`comment` event the journal authored. **Pagination (X11/R6A8):** the `td activity` query is
  **cursor-paginated** — loop the cursor to exhaustion (never read only page 1), and **comment** events come
  from a **separate `--type comment` query** than completed/updated, so both must be paged and both
  cross-checked against the journal. **The reducer's activity SOURCE LAYER is stubbable (R6A8)** — a small
  fetch interface `(cursor) → (events, next_cursor)` the reducer calls in a loop — so pagination is testable
  with a **TWO-PAGE stub (a real cursor token on page 1, the progress event on page 2)** that **asserts the
  page-2 fetch occurred**, measuring pagination **without fabricating a live 100+-event history**. The live-path
  smoke stays read-only (a real `td activity` call, no assertion on volume).
- [ ] **Step 4: Policy read path + retention** — the engine reads the reduced curves (not the raw log). The
  **mutation JOURNAL is retained 45 days** (Y5/§19 — the calibration correlation window, so an outcome can
  still be cross-checked against Bob's writes weeks later), then pruned; the raw calibration log keeps its own
  window and the reductions are kept.
- [ ] **Step 5: Deterministic fixtures via the STUBBABLE source layer (R6A8)** — **provide-nothing records**
  (the control condition), the **W7 NEGATIVE attribution fixture**, and — replacing the old 120-event blob — a
  **TWO-PAGE cursor stub** (a real cursor token on page 1, the genuine user comment on page 2) that asserts the
  reducer **fetched page 2** (a `comment-only-progress` touch the journal did NOT author). The stub measures
  pagination **without fabricating a live 100+-event history**; the live-path smoke stays read-only.

```bash
set -o pipefail
# NO curator pin (AA11) — calibration-log is chezmoi-dropped, not a curator GC candidate.
CAL=~/workspaces/Ivy/forzare/calibration
# The reducer's activity SOURCE is stubbed by a fetch table keyed on cursor: page-1 (cursor=null) returns a
# next_cursor; page-2 (cursor=PAGE2) returns the genuine user comment. The stub RECORDS which cursors were
# fetched, so we assert page-2 was actually requested (R6A8 — pagination measured, not fabricated).
python3 - "$CAL/fixture-events.jsonl" "$CAL/activity-stub.json" <<'PY'
import json, sys
rows = [
 {"schema_version":1,"ts":"2026-07-11T09:00:00Z","context":{"day_type":"off","tod_bucket":"morning"},"action":{"task_id":"X","load_class":"deep"},"outcome":{"initiated":True,"completed":True}},
 {"schema_version":1,"ts":"2026-07-11T14:00:00Z","context":{"day_type":"off","tod_bucket":"afternoon"},"action":"provide_nothing","outcome":{}},
 {"schema_version":1,"ts":"2026-07-11T10:00:00Z","context":{"day_type":"off","tod_bucket":"morning"},"action":{"task_id":"Y","load_class":"light"},"outcome":{}},
 {"schema_version":1,"ts":"2026-07-11T11:00:00Z","context":{"day_type":"off","tod_bucket":"morning"},"action":{"task_id":"Z","load_class":"admin"},"outcome":{}},
]
# Two-page cursor stub the reducer calls as (task_id, cursor) -> {events, next_cursor}. Task Y: one page, only
# Bob-authored activity (W7 negative). Task Z: page 1 (only Bob-authored + a next_cursor) then page 2 (the real
# user comment) — so scoring Z initiated=true REQUIRES following the cursor to page 2.
stub = {
 "Y": {"null": {"events":[{"eventType":"updated","journaled_by_forzare":True}], "next_cursor":None}},
 "Z": {"null":  {"events":[{"eventType":"updated","journaled_by_forzare":True}], "next_cursor":"PAGE2"},
       "PAGE2": {"events":[{"eventType":"comment","journaled_by_forzare":False}], "next_cursor":None}},
}
open(sys.argv[1],"w").write("\n".join(json.dumps(r) for r in rows)+"\n")
json.dump(stub, open(sys.argv[2],"w"))
PY
# the reducer records fetched cursors to $CAL/fetched-cursors.json when driven with --activity-stub (R6A8):
python3 ~/.hermes/skills/calibration-log/reduce.py "$CAL/fixture-events.jsonl" \
  --activity-stub "$CAL/activity-stub.json" --record-cursors "$CAL/fetched-cursors.json" --out "$CAL/curves.test.json"
test -s "$CAL/curves.test.json" || { echo "FATAL: reducer produced no curve file" >&2; exit 1; }
jq -e '.provide_nothing_count >= 1' "$CAL/curves.test.json" \
  || { echo "FATAL: provide-nothing records dropped by the reducer" >&2; exit 1; }
jq -e '.tasks.Y.initiated == false' "$CAL/curves.test.json" \
  || { echo "FATAL: Bob-authored activity was scored as user initiation (W7)" >&2; exit 1; }
# R6A8: the reducer must have FETCHED page 2 (cursor PAGE2) for task Z — proving it followed the cursor:
jq -e '.Z | index("PAGE2")' "$CAL/fetched-cursors.json" \
  || { echo "FATAL: reducer never fetched page 2 (cursor PAGE2) — pagination not followed (R6A8)" >&2; exit 1; }
# and the genuine page-2 user comment scores initiated=true:
jq -e '.tasks.Z.initiated == true' "$CAL/curves.test.json" \
  || { echo "FATAL: page-2 user comment missed — reducer didn't page the cursor (R6A8/X11)" >&2; exit 1; }
echo "calibration reducer round-trip OK (curve; provide-nothing counted; Bob-writes excluded; page-2 fetch asserted)"
rm -f "$CAL/fixture-events.jsonl" "$CAL/activity-stub.json" "$CAL/fetched-cursors.json" "$CAL/curves.test.json"
```

- [ ] **Step 6: DETERMINISTIC NUMERIC fixtures per update rule + one END-TO-END recommendation shift (BB11 —
  the acceptance measures the POLICY, not just a round-trip).** Each of the four §6a rules has a known
  input→output; the reducer's pure functions are asserted against the expected number, and one end-to-end case
  proves recorded outcomes move a later recommendation by the expected amount.

```bash
set -o pipefail
CAL=~/workspaces/Ivy/forzare/calibration
python3 ~/.hermes/skills/calibration-log/reduce.py --self-check <<'PY' > "$CAL/numeric.test.json"
# feed the reducer's pure update functions fixed inputs (the reducer exposes them under --self-check):
[
 # (a) α-UPDATE (α=0.15): estimate 0.50, observed 1.0 → 0.50 + 0.15*(1.0-0.50) = 0.575 (spec §6a).
 {"rule":"alpha_update", "estimate":0.50, "observed":1.0, "expect":0.575},
 # (b) ACTIVATION-DECAY: a decaying boost — P(initiate) 0.80 at 0 min, 0.20 at 60 min → the fit is DECREASING
 #     and predicts ~0.50 at 30 min (monotone-decreasing check + midpoint within tolerance).
 {"rule":"activation_decay", "points":[[0,0.80],[60,0.20]], "at":30, "expect":0.50, "tol":0.10, "monotone":"decreasing"},
 # (c) DURATION-BIAS per load-class: estimates [30,30], observed [45,45] → bias factor 1.5 (pad-up direction).
 {"rule":"duration_bias", "load_class":"deep", "estimates":[30,30], "observed":[45,45], "expect":1.5},
 # (d) HABITUATION index: week-1 initiation-given-surfacing 0.80, week-4 0.35 → 0.35 < 0.5*0.80=0.40 ⇒ flag TRUE.
 {"rule":"habituation", "baseline_rate":0.80, "current_rate":0.35, "expect_flag":true}
]
PY
jq -e '.results | all(.pass)' "$CAL/numeric.test.json" \
  || { echo "FATAL: a numeric update-rule fixture did not match its expected output (BB11):" >&2; jq '.results' "$CAL/numeric.test.json" >&2; exit 1; }
echo "numeric update-rule fixtures OK (BB11): alpha-update=0.575, decay decreasing ~0.50@30m, duration-bias=1.5, habituation flag"

# END-TO-END: recorded outcomes shift a later RECOMMENDATION by the expected amount. Seed a history where deep
# is initiated in the MORNING and NOT in the afternoon; assert the recommended deep window flips to morning and
# the morning deep-initiation estimate is the α-updated value (not the flat prior).
python3 - "$CAL/e2e-events.jsonl" <<'PY'
import json,sys
rows=[]
for _ in range(6): rows.append({"schema_version":1,"context":{"tod_bucket":"morning"},"action":{"load_class":"deep"},"outcome":{"initiated":True}})
for _ in range(6): rows.append({"schema_version":1,"context":{"tod_bucket":"afternoon"},"action":{"load_class":"deep"},"outcome":{"initiated":False}})
open(sys.argv[1],"w").write("\n".join(json.dumps(r) for r in rows)+"\n")
PY
python3 ~/.hermes/skills/calibration-log/reduce.py "$CAL/e2e-events.jsonl" --out "$CAL/e2e-curves.json"
REC=$(jq -r '.recommendations.deep_window' "$CAL/e2e-curves.json")
[ "$REC" = morning ] || { echo "FATAL: recorded morning-deep-initiations did not shift the deep-window recommendation to morning (got $REC) (BB11)" >&2; exit 1; }
jq -e '.curves.deep.morning > .curves.deep.afternoon' "$CAL/e2e-curves.json" \
  || { echo "FATAL: morning deep-initiation estimate did not exceed afternoon after the recorded outcomes (BB11)" >&2; exit 1; }
echo "end-to-end recommendation-shift OK (BB11): recorded outcomes moved the deep-window recommendation to morning"
rm -f "$CAL/numeric.test.json" "$CAL/e2e-events.jsonl" "$CAL/e2e-curves.json"
```

**Acceptance:** the scripted fixture round-trips through the reducer to a non-empty curve file; the
provide-nothing control is counted (`provide_nothing_count >= 1`, not dropped); the **W7 negative fixture
scores `initiated=false`** (only Bob-authored events ⇒ no initiation credit); the **two-page cursor stub proves
the reducer FETCHED page 2** (the recorded-cursor list contains `PAGE2`, R6A8) and the genuine page-2 user
comment scores `initiated=true` — pagination measured with a stub, not a fabricated live history; **the four
NUMERIC update-rule fixtures (BB11) each match their expected output** (α-update `0.575`, a decreasing
activation-decay fit ~`0.50` at 30 min, duration-bias `1.5`, the habituation flag), and **one END-TO-END case
proves recorded outcomes shift a later recommendation by the expected amount** (morning deep-initiations flip the
deep-window recommendation to morning and lift the morning estimate above afternoon); the engine reads
reductions, never the raw log.

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

- [ ] **Step 1: Author the EXACT config schema + defaults** (key / description / default / prompt per entry,
  spec §13/Z9/R6A5). `work_schedule` is a **per-weekday MAP** (every weekday key present; `null` = off) **plus
  an alternating-Sunday anchor** — this exact shape is what C2's DOW-aware cron derivation and B10 Step 3's
  key-by-key verify read:

  ```yaml
  work_schedule:
    days:
      monday:    null
      tuesday:   {start: "15:00", end: "23:00"}
      wednesday: null
      thursday:  {start: "15:00", end: "23:00"}
      friday:    null
      saturday:  {start: "15:00", end: "23:00"}
      sunday:    {start: "15:00", end: "23:00", alternating: true}
    alt_sunday_anchor: "2026-06-07"      # this Sunday = ON; ±14-day multiples = ON (May 31 OFF, Jun 14 OFF, …)
  gym_schedule:
    days: [monday, tuesday, wednesday, friday, saturday, sunday]   # rest = thursday
    window_start: "06:00"
    window_end:   "09:00"
  wake_anchor: "05:15"
  weather_thresholds: {wind_mph: 17, rain: any, temp_low_f: 50, temp_high_f: 90}
  commute_prep_minutes: 30
  commute_travel_minutes: 25
  receptivity_initiation_window_min: 30   # N (V8/R6A3) — user progress within N min of surfacing = initiated
  receptivity_dismissal_threshold: 3      # D (V8/R6A3) — withhold when trailing-24h dismissals ≥ D
  receptivity_surfacing_cap: 8            # S (V8/R6A3) — withhold when surfacings_today ≥ S
  ```

  Notes: **commute constants (X12, spec §3a/§19)** drive the §3a/W13 leave-time alarm (`work_block_start − prep
  − travel`). **N/D/S are the receptivity-gate constants (V8/R6A3)** — configurable here, OWNED by
  `todoist-surface`'s gate logic (B1). **Peak/free windows are DERIVED** at run time (spec §2/§6a — NOT stored).
  **Cron trigger times are DERIVED from this map, DOW-aware (Z9, C2 Step 1)** — never a flat `block_start`.
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
          "commute_prep_minutes", "commute_travel_minutes",
          "receptivity_initiation_window_min", "receptivity_dismissal_threshold",
          "receptivity_surfacing_cap"):
    need(k)
# R6A3: assert the DECIDED receptivity constants N/D/S:
assert sc["receptivity_initiation_window_min"] == 30, f"N {sc['receptivity_initiation_window_min']} != 30"
assert sc["receptivity_dismissal_threshold"] == 3,    f"D {sc['receptivity_dismissal_threshold']} != 3"
assert sc["receptivity_surfacing_cap"] == 8,          f"S {sc['receptivity_surfacing_cap']} != 8"
print("receptivity constants OK: N=30, D=3, S=8 (R6A3)")
# Z9/R6A5: validate the work_schedule per-weekday MAP key-by-key with DECIDED values.
ws = sc["work_schedule"]
days = ws["days"]
DOW = ("monday","tuesday","wednesday","thursday","friday","saturday","sunday")
for d in DOW:
    assert d in days, f"work_schedule.days missing weekday key: {d}"   # every weekday present
work_days = {"tuesday","thursday","saturday","sunday"}
for d in DOW:
    blk = days[d]
    if d in work_days:
        assert blk and blk.get("start")=="15:00" and blk.get("end")=="23:00", f"{d} block != 15:00-23:00 ({blk})"
    else:
        assert blk is None, f"{d} must be an OFF day (null), got {blk}"
assert days["sunday"].get("alternating") is True, "sunday must carry alternating: true"
assert ws.get("alt_sunday_anchor") == "2026-06-07", f"alt_sunday_anchor {ws.get('alt_sunday_anchor')} != 2026-06-07"
print("work_schedule per-weekday map OK: Tue/Thu/Sat/Sun 15:00-23:00, Mon/Wed/Fri OFF, Sun alternating, anchor 2026-06-07")
# gym_schedule: rest day = Thursday (absent), the other six present.
gym = sc["gym_schedule"]; gdays = set(gym["days"])
assert gdays == {"monday","tuesday","wednesday","friday","saturday","sunday"}, f"gym days {gdays} (rest must be Thu)"
assert "thursday" not in gdays, "thursday must be the gym REST day (absent from gym_schedule.days)"
assert sc["wake_anchor"] == "05:15", f"wake_anchor {sc['wake_anchor']} != 05:15"
print("gym_schedule OK (rest=Thu); wake_anchor=05:15")
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

**Acceptance:** `skills.config` carries the **EXACT `work_schedule` per-weekday map validated key-by-key
(Z9/R6A5)** — every weekday key present, Tue/Thu/Sat/Sun `15:00–23:00`, Mon/Wed/Fri `null` (off),
`sunday.alternating: true`, `alt_sunday_anchor: 2026-06-07` — plus `gym_schedule` (rest=Thu, key-by-key),
`wake_anchor=05:15`, **the four weather-threshold values asserted individually (wind>17 / any rain / <50°F /
>90°F — R3A14)**, and **`commute_prep_minutes: 30` + `commute_travel_minutes: 25` (X12)** — each from the
**resolved live config**, not the source template. Only then may Task C2 create the schedule-derived cron jobs
(whose block-boundary/gym DOW fields are DERIVED from this map — Z9, C2 Step 1: boundary `* * 0,2,4,6` never
fires Mon/Wed/Fri; gym excludes Thu; the alt-Sunday job fires weekly and the skill no-ops on OFF Sundays).

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

**File:** `~/.hermes/skills/forzare-capture-pipeline/SKILL.md` (installed via chezmoi — NO curator pin, AA11).
**Authored HERE in Phase B** (moved from D1) so it is applied at Checkpoint B alongside the other skills and
every card attaches this one installed skill via `--skill` (W4). **Phase D (Task D1) keeps only the board CONFIG
+ the card lifecycle/idempotency/harness TESTS** (R5A4) — it no longer authors the skill.

- [ ] **Step 1: Author the pipeline logic — PLACEMENT is the PARENT's, `specify` is the background job's first
  act (AA5, spec §8b):** stage 1 (PARENT, sync) is the Inbox `td task add` — NO `quickadd`, no date parsed
  pre-classification — **PLUS the task-vs-event pre-check + the 4 routing cases (decide-in-context) + dating**;
  then the background job runs **specify** (concretize + `triage → todo`) → **Verify+research-decision** →
  **Research** → **Split**, each gating the next. Every placement date-write goes through the **centralized
  helper (Task B0, W6/X5)** — a user-stated day is `kind: user_fixed` (never rolls), a hard time bound is
  `deadline` + a `kind: leadtime` surfacing due (rolls).
- [ ] **Step 2: The kickoff is CREATE (parent) + a BOUNDED `specify` attempt (parent, supervised by a persisted
  cron retry) — via the SUBSCRIPTION-FREE CLI, NO `notify-subscribe` (Z1/Y2/BB1).** **CLI transport is a HARD
  RULE (Z1):** create every card through the CLI **`hermes kanban create`**, NEVER the in-gateway kanban *tool* —
  the CLI create path is verified subscription-free (`hermes_cli/kanban.py` never calls `_maybe_auto_subscribe`),
  whereas the tool path auto-subscribes the chat to the card's terminal events
  (`tools/kanban_tools.py:843,858-898`, gated by `kanban.auto_subscribe_on_create`, default `True`). The config
  guard `auto_subscribe_on_create: false` (A2) is belt-and-suspenders. (`notify-subscribe` is separately
  DELETED — verified `hermes kanban --help`: it routes TERMINAL events only, onto the home channel — a firewall
  breach + dispatch race, Y2.)
  1. **`hermes kanban create "<title>" --triage --idempotency-key <inbox-task-id> --assignee default
     --max-runtime 900 --skill forzare-capture-pipeline`** — titled `--triage` card (title required positional);
     **`--max-runtime 900` (Y7)**; idempotency key = the stage-1 Inbox task id (a retry / no-resume restart
     re-derives the same card). Stage 1's Inbox write already gave the instant nothing-lost ack.
  2. **`hermes kanban specify <task_id>` — a BOUNDED synchronous attempt, supervised by a persisted cron retry
     (BB1 — NOT a detached fire-and-forget; corrects the AA5 framing that claimed a supervision Hermes cannot
     give a non-dispatched call).** The parent runs `specify` with a short bound (a cheap Haiku
     `auxiliary.triage_specifier` call, so the happy path completes in ~a second): it concretizes **and** performs
     the `triage → todo` transition that permits dispatch (verified `specify_triage_task` requires `triage`
     status, `kanban_db.py:4574`; `auto_decompose: false` means nothing else auto-specifies it). **On success**
     the card is released and the parent returns. **On failure/timeout** the card **STAYS in `triage`** (**never
     `blocked`** — a parent-run `specify` is not a dispatcher-claimed worker, so it emits no
     `gave_up`/`crashed`/`timed_out` event and Hermes cannot auto-retry it; the earlier "**retried on transient
     failure**" / "**raises a failure event**" claims are **ungrounded and DELETED**), the parent says **one
     honest line, "capture saved; processing delayed"**, and schedules a **ONE-SHOT `--no-agent` cron job that
     retries `hermes kanban specify <id>`** (verified `hermes cron create --no-agent --script`; the script runs
     `hermes kanban specify <id>` and its non-zero exit lands in the watchdog's failed-run scan, F1 (b)). The
     **forzare-ops watchdog STALE-TRIAGE scan (F1/AA5) alerts on a card in `triage` > 30 min** as the final
     backstop. No third call, no card subscription.
- [ ] **Step 3: Idempotent dup-guards** (no mid-run resume): stage 1 skips if already in Inbox + skips
  re-routing a placed task/duplicate 🤖-calendar event; stage 5 skips existing subtasks — a restart converges to
  one task.
- [ ] **Step 4: Awaiting-user + failures WITHOUT a card subscription (Y2/Y1/R5A7).** When a stage needs the
  user (cases 3–4), the card **blocks awaiting-user** and the pipeline **enqueues a `triage-reraise` record to
  the unified `decision-queue.json`** (state-only, no message) — the brief delivers it as its head item and any
  live turn re-raises it opportunistically; on the answer the live turn writes it onto the card + `hermes
  kanban unblock`s it (resuming the dispatcher) and marks the queue record `acked` (R5A5). **Pipeline FAILURES**
  (crashed / timed-out / gave-up) reach `#forzare-errors` via the **forzare-ops watchdog (F1)**, never a card
  subscription — so **no user-facing message issues before Phase G go-live** (R5A7).
- [ ] **Step 5: Install (NO curator pin — AA11)** `forzare-capture-pipeline` (the live pipeline execution tests
  are D1's controlled harness; this task only authors the skill, applied at Checkpoint B; integrity is the
  content-hash gate).

**Acceptance:** the pipeline skill is authored + installed in Phase B (applied at Checkpoint B, NO pin — AA11);
placement is the PARENT's inline decision; `specify` is a **BOUNDED synchronous attempt** (BB1) whose failure
leaves the card in `triage` + a "capture saved; processing delayed" line + a persisted one-shot `--no-agent`
cron retry (its failure → F1's failed-run scan), backstopped by the F1 stale-triage scan — the ungrounded
"retried / raises a failure event" claims deleted; no `notify-subscribe` (Y2); cards carry `--max-runtime 900`
(Y7); awaiting-user enqueues a `triage-reraise` decision-queue record and failures route via the watchdog — no
card subscription anywhere.

---

### APPLY CHECKPOINT B (inline, fail-closed — Y6/R5A3) — do this BEFORE any Phase-B staged dry-run or Phase C

**NEW checkpoint (the topological fix, Y6/R5A3), NO PINNING (AA11).** Every Phase-B source authored above
(the Task B0 helper, B1–B10 skills, the B11 capture-pipeline skill, the B10 `skills.config`) must be **applied
to live** before ANY staged dry-run runs against it — a staged dry-run reads the LIVE
`~/.hermes/skills/<name>/SKILL.md`, which only exists after `chezmoi apply`. **Checkpoint B verifies APPLIED
FILES + RESOLVED CONFIG.** So the four-stage flow is: **author-all (above) → THIS Checkpoint B (files + config)
→ staged dry-run ALL (each task's verify step, NO pinning) → SKILL-INTEGRITY GATE (installed path +
content-hash) → Phase C.** No staged run below Checkpoint B may execute until it is CLEARED.

- **User-run/agent-run:** apply the skills + capture-pipeline skill sources (agent-runnable plaintext) + the
  KeePassXC-gated `config.yaml` re-apply for B10's `skills.config` stanza (user-run, R5A14).
- **Gate (fail-closed) — files + config:** `source ~/workspaces/Ivy/forzare/gate-check.sh` (the persistent
  Phase-A script, X10) and run `gate_check` with this checkpoint's **explicit FILE list (W3 — `chezmoi diff` on
  a directory is NON-recursive):** **every** `~/.hermes/skills/<name>/SKILL.md` authored in Phase B (incl.
  `forzare-capture-pipeline`) **+ `~/.hermes/config.yaml`** (the B10 `skills.config` edit, R5A14). Then assert
  every named skill **dir exists live**:
  `for s in <names>; do [ -d ~/.hermes/skills/"$s" ] || { echo "FATAL: $s not applied" >&2; exit 1; }; done`,
  and the B10 `skills.config` **resolves** (the B10 Step 3 rendered-live read). It must print `checkpoint
  CLEARED`; any pending diff / non-zero exit / stderr / missing dir blocks every Phase-B staged run and Phase C.

---

### SKILL-INTEGRITY GATE (inline, fail-closed — AA11, replaces the old POST-PIN GATE) — do this AFTER the staged dry-runs, BEFORE Phase C

**Installed-path + content-hash, NO pins (AA11).** The forzare skills are chezmoi-dropped, so they are NOT
curator GC candidates (verified `skill_usage.py` excludes non-agent-created skills from the curator's managed
list) — the old "pin then assert pinned" loop is dropped. This gate runs the **boot skill-existence +
content-hash assertion** (the boot-check script, authored in the Phase-B author stage; documented at C1 Step 2):
**every skill the three bundles will name is installed at its expected path AND its `SKILL.md` content-hash
matches the chezmoi source**, and **fails loud** if any is missing or drifted. This must pass before Phase C
authors the bundles and before any bundle-level staged run.

```bash
set -o pipefail
cd "$(git rev-parse --show-toplevel)"   # the chezmoi source tree (worktree), for `chezmoi cat` of the source
# every bundle-named skill must be INSTALLED at its path AND content-hash-match the chezmoi source (AA11 —
# repo-authored skills are not curator GC candidates, so no pin; the silent-skip guard is installed+hash):
for s in todoist-surface weather calendar-read calendar-write eisenhower-plan activation-prompt brief-assemble \
         followups-sweep daily-reflect tomorrow-prep eod-roll waiting-reconcile transition forzare-capture-pipeline; do
  LIVE=~/.hermes/skills/"$s"/SKILL.md
  [ -f "$LIVE" ] || { echo "FATAL: skill '$s' is not installed at $LIVE (silent-skip guard, spec §13)" >&2; exit 1; }
  SRC_H=$(chezmoi --source "$PWD" cat "$LIVE" | shasum -a 256 | cut -d' ' -f1)
  LIVE_H=$(shasum -a 256 "$LIVE" | cut -d' ' -f1)
  [ "$SRC_H" = "$LIVE_H" ] || { echo "FATAL: skill '$s' content-hash drift (src $SRC_H != live $LIVE_H) — re-apply (AA11)" >&2; exit 1; }
done
echo "SKILL-INTEGRITY GATE CLEARED — every bundle-named skill installed + content-hash-matched (AA11, no pin)"
```

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
  - `forzare-eod` = `eod-roll` (roll + **Bob-owned p1-clear** (plan-record `selected_ids` only, never a user
    p1, AA2) + ledger ticks + last-reconcile stamp) · `todoist-surface` · `daily-reflect` · `eisenhower-plan`
    (proposal mode, no p1) · `tomorrow-prep`. **NO `calendar-write`
    (R2A8)** — EOD writes no calendar; `tomorrow-prep` only records the candidate anchor to
    `forzare/state/tomorrow-prestage.json` (spec §8a); the morning run is the sole calendar writer.
  - (`waiting-reconcile` is NOT bundled — the 02:00 cron runs the skill directly, Task C2.)
- [ ] **Step 2: Skill-INTEGRITY assertion — a BUILD/RUNBOOK gate + the watchdog scan, NOT a Hermes boot-abort
  (closes the silent-skip hole, spec §13; installed path + content-hash, NO pin — AA11/BB8).** The path+hash
  check asserts every skill named by the bundles is **installed at its expected path AND its `SKILL.md`
  content-hash matches the chezmoi source** and **fails loud** if any is missing or drifted. **The earlier
  "abort boot" framing is REMOVED (BB8):** forzare **cannot** hook Hermes' own launchd startup
  (`ai.hermes.gateway.plist`) — patching a third-party tool's artifact is forbidden — so there is no
  forzare-owned way to abort *Hermes'* boot. Enforcement is instead **two forzare-owned mechanisms:** (a) the
  **build/pre-start gate** — the boot-check script runs at the **SKILL-INTEGRITY GATE** (end of Phase B) and is
  the documented pre-start runbook check re-run after any skill re-apply; and (b) the **standing runtime guard —
  the forzare-ops watchdog's per-pass skill-INTEGRITY scan (F1 scan (f)) covering EVERY V1 skill + the 3 bundle
  YAMLs + the shared helper**, which alerts to `#forzare-errors` (`hermes send --to`) on any missing/drifted
  skill. **AUTHORING of the boot-check script MOVES to the Phase-B author stage** — it must exist before the
  SKILL-INTEGRITY GATE that invokes it, written alongside the skills (like `gate-check.sh`). No pinning (repo
  skills are not curator GC candidates, AA11).
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
**Acceptance:** bundles resolve fully; the boot INTEGRITY assertion fails loud on a deliberately-missing or
content-drifted skill (test it once by moving/editing a skill dir, then restore) — no pins involved (AA11).

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

- **User-run/agent-run:** apply the three bundle sources. (Every bundle-named skill was already asserted
  installed + **content-hash-matched** at the **SKILL-INTEGRITY GATE** at the end of Phase B, AA11 — so this
  checkpoint does not re-run that assertion; it gates only the newly-authored bundles.)
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

**Transactional install (Y8 + Z7 + AA9 + BB4) + schedule-DERIVED boundary/gym times (Y9).** **The whole install
runs in a GATEWAY-STOPPED window (BB4):** because it is **pre-go-live by definition** (every job is created
`--deliver local`, and delivery only flips at G1), and the gateway's 60s tick reloads `jobs.json` on every tick,
the install stops the gateway first so a tick can never observe or reload a half-written `jobs.json` mid-reconcile
— **a user-run `launchctl unload …/ai.hermes.gateway.plist` (or `hermes gateway stop`) before, and a `launchctl
load` / `hermes gateway start` after** (the same user-run stop/start already in the checkpoint pattern; the
watchdog's KeepAlive is also unloaded for the window). The install is otherwise one atomic transaction:
`set -euo pipefail`, a **declared name manifest**, **(1) pre-validate the manifest** (reject a duplicate name in
the manifest or an ambiguous already-duplicated live name — AA9), **reconcile by NAME** (edit an existing job of
that name, create a missing one — **never blind-create a duplicate**; each job attaches ONE bundle/skill via a
single `--skill`, and a multi-skill job would **repeat `--skill` per skill** — verified help, AA9), **(2)
ROLLBACK is an ATOMIC restore of the VALIDATED `jobs.json` backup via same-dir TMP + `mv` (BB4 — a bare `cp`
could leave a torn file if anything reads it mid-restore; the atomic rename never does) + a BYTE-COMPARE (AA9)**,
and **(3) the ERR trap stays ARMED through ALL postconditions** — the post-install exact-manifest assert is
INSIDE the transaction, so a post-install mismatch ALSO triggers the atomic restore; the trap is disarmed only
after the assert passes. **Ordering note (BB4):** stop gateway → back up jobs.json → reconcile → post-assert →
disarm trap → restart gateway (which reloads the final `jobs.json`). The gym-window-end and block-boundary
trigger times are **DERIVED from the resolved `work_schedule`/`gym_schedule`, DOW-aware** (B10, live) — the
boundary time is the **R7A2 formula** `block_start − prep − travel − 30` = 13:35 — not hardcoded — so a schedule
edit re-derives them (B10 Step 4/Y9/Z9/R7A2).

```bash
set -euo pipefail
# X10: back up jobs.json HERE — immediately before the FIRST cron mutation (moved out of Task E2, which now
# only declares the live-data exception + documents the rollback). jobs.json is NOT chezmoi-managed.
# AA9: capture the VALIDATED backup path so rollback is an ATOMIC whole-file restore (not per-record replay).
JOBS=~/.hermes/cron/jobs.json
JOBS_BAK=~/workspaces/backups/"$(date -u +%Y-%m-%dT%H-%M-%S).hermes-cron-jobs.backup.json"
cp "$JOBS" "$JOBS_BAK"
python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$JOBS_BAK" \
  && echo "jobs.json backup validated at $JOBS_BAK (AA9)" \
  || { echo "FATAL: jobs.json backup is not valid JSON — refusing to proceed (AA9)" >&2; exit 1; }

# Y9/Z9/R6A5: DERIVE the gym-window-end + block-boundary cron specs from the RESOLVED per-weekday
# work_schedule/gym_schedule (B10's live skills.config) — DOW-AWARE, never a flat block_start read, so a
# boundary NEVER fires on a genuine off day. Hardcoding would desync on any schedule edit (Y9 reconcile, B10 S4).
# NOTE: each cron spec itself contains spaces, so python emits them PIPE-separated and bash splits on '|'.
IFS='|' read -r GYM_CRON BOUND_CRON < <(~/.hermes/hermes-agent/venv/bin/python - <<'PY'
import os, yaml
sc = (yaml.safe_load(open(os.path.expanduser("~/.hermes/config.yaml"))).get("skills",{}) or {}).get("config",{}) or {}
gym  = sc["gym_schedule"]; work = sc["work_schedule"]
# cron day-of-week numbers: Sun=0 Mon=1 … Sat=6
DOWNUM = {"sunday":0,"monday":1,"tuesday":2,"wednesday":3,"thursday":4,"friday":5,"saturday":6}
days = work["days"]
# BLOCK-BOUNDARY DOW = the weekdays that HAVE a work block (Tue/Thu/Sat + Sunday, which is conditionally a
# work day via the alt-Sunday anchor — the skill no-ops on OFF Sundays). Never Mon/Wed/Fri (genuine off days).
work_dow = sorted({DOWNUM[d] for d, blk in days.items() if blk})
# all work blocks share one start time in v1 — assert that, then take it (uniform start, one boundary cron).
starts = {blk["start"] for blk in days.values() if blk}
assert len(starts) == 1, f"non-uniform work-block starts {starts} — a per-start boundary cron is a future concern"
wh, wm = map(int, starts.pop().split(":"))
bound_dow = ",".join(str(n) for n in work_dow)             # e.g. "0,2,4,6" (Sun/Tue/Thu/Sat)
# BLOCK-BOUNDARY TIME = the R7A2 FORMULA: block_start - commute_prep - commute_travel - 30 (the ~30-min-until-
# you-leave soft pre-warning, spec §3/§3a — NOT the leave-time alarm, which is block_start - prep - travel).
prep   = int(sc["commute_prep_minutes"]); travel = int(sc["commute_travel_minutes"])
bmin   = wh*60 + wm - prep - travel - 30                    # 15:00 - 30 - 25 - 30 = 815 min
assert bmin >= 0, f"boundary minutes negative ({bmin}) — block start too early for the offsets"
bh, bm = divmod(bmin, 60)                                   # 13, 35
# GYM-WINDOW-END DOW = the gym days (all but Thursday); backstop fires at window_end.
gym_dow = sorted({DOWNUM[d] for d in gym["days"]})
gh, gm = map(int, str(gym["window_end"]).split(":"))       # e.g. "09:00"
gym_dowc = ",".join(str(n) for n in gym_dow)               # e.g. "0,1,2,3,5,6"
print(f"{gm} {gh} * * {gym_dowc}|{bm} {bh} * * {bound_dow}")   # two DOW-AWARE Denver-local cron specs, '|'-sep
PY
)
{ [ -n "${GYM_CRON:-}" ] && [ -n "${BOUND_CRON:-}" ]; } || { echo "FATAL: could not derive gym/boundary cron from work_schedule (Y9/Z9)" >&2; exit 1; }
# R7A2/Z9 sanity: the boundary TIME must be the formula (15:00 block ⇒ 13:35) and its DOW must EXCLUDE the off
# days (Mon=1/Wed=3/Fri=5) and INCLUDE Sun=0 (alt-Sunday fires weekly; the skill no-ops on OFF Sundays).
[ "$BOUND_CRON" = "35 13 * * 0,2,4,6" ] \
  && echo "boundary cron OK: $BOUND_CRON (block_start 15:00 − prep 30 − travel 25 − 30 = 13:35; R7A2)" \
  || { echo "FATAL: boundary cron is '$BOUND_CRON', expected '35 13 * * 0,2,4,6' (R7A2 formula)" >&2; exit 1; }

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
existing_id(){ jq -r --arg n "$1" '.jobs[]|select(.name==$n)|.id' ~/.hermes/cron/jobs.json | head -1; }

# Z7 (1): PRE-VALIDATE the manifest before any mutation — reject a duplicate NAME in our own manifest, and
# reject a duplicate name ALREADY in jobs.json (two live jobs of one name = ambiguous reconcile).
DUP=$(printf '%s\n' "${MANIFEST[@]}" | sort | uniq -d)
[ -z "$DUP" ] || { echo "FATAL: duplicate name in the manifest: $DUP (Z7)" >&2; exit 1; }
for name in "${MANIFEST[@]}"; do
  C=$(jq -r --arg n "$name" '[.jobs[]|select(.name==$n)]|length' ~/.hermes/cron/jobs.json)
  [ "$C" -le 1 ] || { echo "FATAL: $C live jobs already named '$name' — ambiguous reconcile (Z7)" >&2; exit 1; }
done
echo "manifest pre-validated OK (no dup names, no ambiguous live names) (Z7)"

# AA9: rollback = ATOMIC restore of the VALIDATED jobs.json backup, then a BYTE-COMPARE to prove it landed —
# simpler and more robust than a per-record `hermes cron edit` replay (which is itself fallible and could pass a
# comma-joined --skill as one skill; --skill is REPEATED per skill, verified `hermes cron create/edit --help`).
# BB4: the gateway is STOPPED for this window (pre-go-live), so no tick reloads a half-written jobs.json; it is
# restarted after the trap disarms. Restore is a SAME-DIR TMP + `mv` (atomic rename) + BYTE-COMPARE — never a
# bare `cp` (which could leave a torn file if anything read it mid-restore); the rename swaps the whole file.
rollback(){ echo "FATAL: partial cron install — ATOMIC-restoring jobs.json from the validated backup (AA9/BB4)" >&2; \
  TMP="$(dirname "$JOBS")/.jobs.json.restore.$$"; \
  cp "$JOBS_BAK" "$TMP" && mv "$TMP" "$JOBS" \
    || { echo "FATAL: atomic restore failed — jobs.json may be inconsistent, restore $JOBS_BAK by hand" >&2; exit 1; }; \
  if cmp -s "$JOBS_BAK" "$JOBS"; then echo "jobs.json byte-identical to the pre-install backup — rollback verified (AA9)" >&2; \
  else echo "FATAL: post-restore byte-compare MISMATCH — restore $JOBS_BAK by hand" >&2; fi; \
  exit 1; }
trap rollback ERR

# reconcile by NAME (edit existing / create missing / never blind-create a duplicate). Each job attaches ONE
# bundle/skill via a single --skill (a multi-skill job would REPEAT --skill per skill — verified help, AA9).
for name in "${MANIFEST[@]}"; do
  IFS='|' read -r sched skill prompt <<<"${SPEC[$name]}"
  eid=$(existing_id "$name")
  if [ -n "$eid" ]; then
    hermes cron edit "$eid" --schedule "$sched" --skill "$skill" --prompt "$prompt" --deliver local >/dev/null
    echo "reconciled $name ($eid)"
  else
    nid=$(hermes cron create "$sched" "$prompt" --skill "$skill" --deliver local --name "$name" | jid_from_create)
    echo "created $name ($nid)"
  fi
done

# AA9: the trap stays ARMED through ALL postconditions — the exact-manifest assert below is inside the
# transaction, so a post-install mismatch ALSO triggers the atomic rollback (trap disarmed only after it passes).
# Z7 (4): POST-INSTALL assert the EXACT manifest — every name present exactly once, no stray forzare job.
GOT=$(jq -r '.jobs[]|select(.name|test("^forzare-"))|.name' ~/.hermes/cron/jobs.json | sort)
WANT=$(printf '%s\n' "${MANIFEST[@]}" | sort)
# AA9: on mismatch, CALL rollback (atomic restore) rather than a bare exit — the postcondition is inside the txn.
[ "$GOT" = "$WANT" ] || { echo "FATAL: post-install manifest mismatch (Z7):" >&2; diff <(printf '%s\n' "$WANT") <(printf '%s\n' "$GOT") >&2; rollback; }
echo "post-install exact-manifest OK (Z7): 6 forzare jobs, no dup, no stray"
trap - ERR   # AA9: postconditions passed — NOW disarm the transaction trap
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
  tick` then executes due jobs. It **seeds the [TEST] fixture it claims** (an ungroomed dated task, R6A4) so the
  always-acting writers have real work. Three assertions per job: **(a) the persisted job carries the expected
  `skills` value (W1)**, **(b) the staged trace shows the attached skills' activity via the intent log (W1 —
  the intent log is POSITIVE evidence only)**, **(c) the staged window performed ZERO real mutations — the
  NEGATIVE gate is INDEPENDENT of the intent log (AA1):** before/after diffs of task activity + the **separate
  `--type comment`** stream, **CURSOR-PAGINATED to exhaustion (R7A8)** and scoped to the **[TEST] fingerprint**
  (the harness-seeded tasks, NOT the intent targets, NOT `--by me` — so the user may edit real tasks freely) +
  a **[TEST]-scoped 🤖-calendar** count + a **RECURSIVE state+calibration content-hash that EXCLUDES
  `dryrun-intents.jsonl`** (a dry-run DOES append to it, so hashing it would be self-defeating — AA1). Asserted
  with the verified `td activity` shapes (camelCase `eventType`/`objectId`; the event `id` is float-mangled
  scientific notation, so NEVER compare on `.id` — key on `objectId`), with a fail-LOUD nonzero-exit negative
  branch:

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
# R6A4: SEED the fixture the gate claims — an UNGROOMED DATED task (no load-label, no duration) so the staged
# brief's groom-on-read + eisenhower-plan have real work to do (the three always-acting writers journal). BB6:
# run-id-prefixed + a trap that deletes ONLY the captured id (no prefix sweep).
RUNID="$(date +%s)-$$"
FIX=$(td task add "[TEST-$RUNID] ungroomed dated fixture" --due today --json | jq -r '.id')
[ -n "$FIX" ] && [ "$FIX" != null ] || { echo "FATAL: could not seed the bundle fixture (R6A4)" >&2; exit 1; }
trap 'td task delete "$FIX" --yes >/dev/null 2>&1 || true' EXIT INT
# AA1/R7A1/R7A8: the NEGATIVE gate is INDEPENDENT of the intent log — before/after diffs of task + comment
# activity (CURSOR-PAGINATED to exhaustion), a [TEST]-scoped calendar count, and a RECURSIVE state+calibration
# hash that EXCLUDES dryrun-intents.jsonl. Scope activity to the [TEST] FINGERPRINT (the harness-seeded tasks,
# NOT the intent targets and NOT `--by me`): the tester owns [TEST] tasks so the user can edit real tasks freely.
STATE=~/workspaces/Ivy/forzare/state
CAL=~/workspaces/Ivy/forzare/calibration
# hash_state: RECURSE over state/ AND calibration/, EXPLICITLY EXCLUDING dryrun-intents.jsonl — a dry-run DOES
# append to that one file, so hashing it would make this gate self-defeating (AA1).
hash_state(){ find "$STATE" "$CAL" -type f ! -name 'dryrun-intents.jsonl' -print0 2>/dev/null \
  | sort -z | xargs -0 -r shasum -a 256; }
# td_activity_paged: loop --cursor until exhausted (never page 1 only, R7A8). Emits every event as one JSON line.
td_activity_paged(){ local typ="$1" cur="" page nc; while :; do
    page=$(td activity --since "$(date -u +%Y-%m-%d)" --type "$typ" --json ${cur:+--cursor "$cur"}) || return 1
    printf '%s' "$page" | jq -c '.results[]?'
    nc=$(printf '%s' "$page" | jq -r '.nextCursor // .cursor // empty'); [ -n "$nc" ] || break; cur="$nc"
  done; }
# the [TEST] fingerprint set (objectIds the harness owns) — a task's activity here can only be a forzare leak:
TEST_IDS=$(td task list --filter "search: [TEST-$RUNID]" --all --json | jq -r '[.results[].id]')
# count [TEST]-scoped events across a paginated stream:
count_test_events(){ td_activity_paged "$1" | jq -s --argjson t "$TEST_IDS" '[.[]|select(.objectId as $o|$t|index($o))]|length'; }
TASK_BEFORE=$(count_test_events task); COMMENT_BEFORE=$(count_test_events comment)
STATE_BEFORE=$(hash_state)
# BB5: gog probes ALWAYS pass an explicit account (-a) + the 🤖 calendar id, and command/parse failure is FATAL
# (never `|| echo 0`, which would mask a broken gog into a false "clean" gate). Resolve both up front, fail loud.
GOG_ACCT="${GOG_ACCT:?set GOG_ACCT to the authenticated Google account (BB5)}"
BOT_CAL=$(gog calendar calendars -a "$GOG_ACCT" -j | jq -r '(.calendars // .)[] | select((.summary//"")|test("🤖|[Bb]ob")) | .id' | head -1) \
  || { echo "FATAL: gog calendar calendars failed — cannot resolve the 🤖 calendar id (BB5)" >&2; exit 1; }
[ -n "$BOT_CAL" ] || { echo "FATAL: no 🤖 calendar found for $GOG_ACCT (BB5)" >&2; exit 1; }
# cal_snapshot: the [TEST]-scoped event {id, updated} pairs on the 🤖 calendar — a SET compare on IDs + the
# `updated` timestamp (not a count), so an in-place edit of an existing [TEST] event is caught too. FATAL on
# any command/parse failure (BB5 — no ||-to-zero).
cal_snapshot(){ gog calendar events -a "$GOG_ACCT" "$BOT_CAL" -j \
    | jq -S '[.events[]? | select((.summary//"")|test("\\[TEST\\]")) | {id, updated}] | sort_by(.id)' \
    || { echo "FATAL: gog calendar events snapshot failed (BB5)" >&2; exit 1; }; }
CAL_BEFORE=$(cal_snapshot)
# Force one staged brief run and read the audit BY its job id:
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
# (c) ZERO FORZARE mutations — the NEGATIVE gate is INDEPENDENT of the intent log (AA1). The intent log is
# POSITIVE evidence only ((b) above). A dry-run performs no real write, so ANY new activity on a [TEST] task —
# which the tester owns and the user never edits — is a leak. Verified shapes: camelCase eventType/objectId;
# the event id is float-mangled ("2.15…e+36") so never key on .id. Cursor-paginated to exhaustion (R7A8).
# (c1) TASK-activity: no NEW [TEST]-scoped event (before vs after, paginated):
TASK_AFTER=$(count_test_events task)
[ "$TASK_AFTER" = "$TASK_BEFORE" ] || { echo "FATAL: [TEST]-scoped TASK activity grew ($TASK_BEFORE -> $TASK_AFTER) — dry-run leaked (AA1)" >&2; exit 1; }
# (c2) COMMENT-activity is a SEPARATE paginated stream (§6a) — an auto-repair/if-then comment would slip past a
# task-only check:
COMMENT_AFTER=$(count_test_events comment)
[ "$COMMENT_AFTER" = "$COMMENT_BEFORE" ] || { echo "FATAL: [TEST]-scoped COMMENT activity grew ($COMMENT_BEFORE -> $COMMENT_AFTER) — dry-run leaked (AA1)" >&2; exit 1; }
# (c3) [TEST]-scoped 🤖-CALENDAR unchanged — SET compare on event IDs + `updated` fields, not a count (BB5;
# no activity-log mirror for calendar). A new or in-place-edited [TEST] event changes the snapshot:
CAL_AFTER=$(cal_snapshot)
[ "$CAL_AFTER" = "$CAL_BEFORE" ] || { echo "FATAL: 🤖-calendar [TEST] events changed under dry-run — leaked (AA1/BB5):" >&2; \
  diff <(printf '%s\n' "$CAL_BEFORE") <(printf '%s\n' "$CAL_AFTER") >&2; exit 1; }
# (c4) RECURSIVE state+calibration hash unchanged, dryrun-intents.jsonl EXCLUDED (AA1):
STATE_AFTER=$(hash_state)
[ "$STATE_BEFORE" = "$STATE_AFTER" ] || { echo "FATAL: a state/calibration file changed under dry-run — leaked (AA1):" >&2; \
  diff <(printf '%s\n' "$STATE_BEFORE") <(printf '%s\n' "$STATE_AFTER") >&2; exit 1; }
echo "staged window: 0 Bob-authored mutations across [TEST]-scoped task + comment (paginated) + calendar + recursive state/calibration hash (AA1 independent of the intent log; intents = positive evidence only), $(jq -rs length "$INTENTS") intent record(s) (gate green)"
# cleanup the seeded fixture (--yes):
td task delete "$FIX" --yes
```

Expected: all jobs listed at the right times/TZ; every user-facing job is `--deliver local` + `--skill`
during the build; the forced run writes `~/.hermes/cron/output/<job_id>/` and does NOT message Discord.
**Acceptance:** the install is a **true transaction (Y8 + Z7 + AA9)** — the exact **six-name manifest** is
**pre-validated** (no dup name, no ambiguous live name), reconciled by name (edit existing / create missing /
never blind-create; `--skill` repeated per skill), with **rollback = an ATOMIC restore of the validated
`jobs.json` backup + a byte-compare** and the **ERR trap armed through ALL postconditions** (a post-install
manifest mismatch triggers the same atomic restore; trap disarmed only after the assert passes), and a
**post-install exact-manifest assert**; and the
gym-window-end + block-boundary times are **DERIVED from the resolved
`work_schedule`/`gym_schedule`, DOW-AWARE (Y9/Z9/R6A5)** — never a flat `block_start`: the boundary cron is the
**R7A2 formula** `block_start − prep − travel − 30` = **`35 13 * * 0,2,4,6`** (Sun/Tue/Thu/Sat), so it **never
fires on an off day (Mon/Wed/Fri)** and is distinct from the 14:05 leave-time alarm; the gym cron excludes
Thursday; the alternating Sunday is a **weekly** fire the skill no-ops on OFF Sundays (cron can't express
"every other Sunday"). Each job carries its persisted `skills` value (W1); the staged brief's **per-skill
EFFECTS are asserted from the intent log — NOT a grep over the audit prompt (R5A8)** — every always-acting
mutating member (`eod-roll`/`todoist-surface`/`eisenhower-plan`) produced an intent record keyed by
`skill`+`run_id`; the 02:00 reconcile and monthly sweep have no user-facing delivery; the staged window shows
zero **Bob-authored** mutations across **before/after diffs INDEPENDENT of the intent log (AA1)** — the
`--type task` stream and the separate `--type comment` stream **cursor-paginated to exhaustion (R7A8)**, both
scoped to the **[TEST] fingerprint** (not the intent targets, not `--by me` — so the user may edit Todoist
freely during staging), the [TEST]-scoped 🤖-calendar count, and a **RECURSIVE state+calibration hash that
EXCLUDES `dryrun-intents.jsonl`** (string-safe `eventType`/`objectId` reads, fail-loud), with the **seeded
[TEST] ungroomed fixture** giving the always-acting writers real work, while the intents log carries the
computed writes as **positive evidence only** (R3A1/R3A2/AA1). The **exact six-name manifest + count** is
re-asserted at go-live (G1, Y8).

---

## Phase D — Capture pipeline (Kanban, private)

### Task D1: Private Kanban board CONFIG + card lifecycle/idempotency tests (skill authored in B11, R5A4)

**Store:** `~/.hermes/kanban.db` (Bob-private, firewalled from the user, spec §9). Assignee = the **`default`
profile** (persona "Bob"; no profile named `bob`); `default_assignee: "default"`, `auto_decompose: false`,
`max_in_progress_per_profile: 2`, `failure_limit: 2` (Task A2 / spec §14). **Preflight:**
`hermes profile show default` must succeed. **The `forzare-capture-pipeline` skill is authored + installed
(integrity-gated, NO curator pin — AA11) in Task B11 (Phase B, R5A4/Y6; CC5)** — D1 owns only the **board
config** and the **card lifecycle / idempotency / controlled-harness tests** below.

- [ ] **Step 1: Confirm the kickoff contract from B11 (create + a BOUNDED `specify`; NO notify-subscribe —
  Y2/BB1).** The kickoff is authored in B11: parent Bob does, synchronously, **stage 1 (the
  `td task add` to Inbox — NOT `quickadd`, spec §8b/U4; instant ack, idempotent) AND the placement/classification
  decisions (task-vs-event pre-check + the 4 routing cases, decide-in-context — placement moved to the PARENT,
  AA5)**, then:
  1. **`hermes kanban create "<title>" --triage --idempotency-key <inbox-task-id> --assignee default
     --max-runtime 900 --skill forzare-capture-pipeline`** (title required positional — verified; idempotency
     key = the stage-1 Inbox TASK ID; **`--max-runtime 900`**, Y7; `--skill` attaches B11's stage logic). A
     `--triage` card is **not dispatchable**; stage 1's Inbox write already gave the instant nothing-lost ack.
  2. **`hermes kanban specify <task_id>` — a BOUNDED synchronous attempt, supervised by a persisted cron retry
     (BB1, NOT a detached fire-and-forget).** `specify` concretizes via `auxiliary.triage_specifier` (Task A2)
     **and performs the `triage → todo` transition that PERMITS dispatch** (verified `specify_triage_task`
     requires `triage` status, `kanban_db.py:4574`; `auto_decompose: false` means nothing else auto-specifies it,
     A2). **On failure/timeout** the card **stays `triage`** (never `blocked`), the parent says "capture saved;
     processing delayed", and a **one-shot `--no-agent` cron job retries `hermes kanban specify <id>`** (its
     non-zero exit → F1's failed-run scan). The **watchdog STALE-TRIAGE scan (F1/AA5) alerts on a card > 30 min
     in `triage`** as the backstop. The earlier "**raises a failure event**" claim is **DELETED** — a parent-run
     `specify` is not a dispatcher-claimed worker, so it emits no failure event (BB1).
  **There is NO third call — the `notify-subscribe` callback design is DELETED (Y2, verified `hermes kanban
  --help`: it routes TERMINAL events only, onto the home channel — a firewall breach + dispatch race).**
  Awaiting-user cards re-raise via the unified decision queue (`triage-reraise`, B11 Step 4/Y1) and failures
  reach `#forzare-errors` via the watchdog (F1); no card subscription. Any placement date-write goes through the
  centralized date-mutation layer (Task B0, W6). The 5 stages: **Place + decide-placement (PARENT, sync)** →
  Specify (background, first act) → Verify+research-decision → Research → Split, each gating the next (spec §8b).
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
RUNID="$(date +%s)-$$"   # BB6: run-id-scope the card + key so concurrent/re-runs never collide
# a titled --triage card (title is a REQUIRED positional, R3A11); the idempotency key must return the SAME id on re-fire:
ID1=$(hermes kanban create "[TEST-$RUNID] capture probe" --triage --idempotency-key "test-cap-$RUNID" --assignee "$NOOP" --json | jq -r '.id // .task_id')
ID2=$(hermes kanban create "[TEST-$RUNID] capture probe" --triage --idempotency-key "test-cap-$RUNID" --assignee "$NOOP" --json | jq -r '.id // .task_id')
[ -n "$ID1" ] && [ "$ID1" = "$ID2" ] || { echo "FATAL: idempotency did not dedupe ($ID1 vs $ID2)" >&2; exit 1; }
echo "idempotency dedupe OK (same id $ID1)"
# read the STATUS field from JSON (not a text grep): a fresh --triage card is status=triage
ST=$(hermes kanban show "$ID1" --json | jq -r '.status')
[ "$ST" = triage ] || { echo "FATAL: expected status=triage, got $ST" >&2; exit 1; }
echo "triage status OK; assignee=$NOOP is non-spawnable, so the live dispatcher leaves it inert"
# Z1: the CLI `hermes kanban create` is subscription-free — the notify-subs table must have NO row for the
# card (belt-and-suspenders: config `auto_subscribe_on_create: false`, A2, means even the tool path wouldn't).
SUBS=$(hermes kanban notify-list "$ID1" --json | jq 'length')
[ "$SUBS" = 0 ] || { echo "FATAL: $SUBS subscription row(s) for the CLI-created card — firewall leak (Z1)" >&2; exit 1; }
echo "no-subscription-row OK (CLI create is subscription-free, Z1)"
# Z6: DO NOT archive the card here — Step 6's controlled harness drives it. The archive is the LAST step of
# Step 6 (a card archived now would leave the harness nothing to run). Only THIS run's [TEST-$RUNID] Todoist
# tasks are cleaned here (BB6: run-id-scoped, never a bare `search: [TEST]` sweep); the card $ID1 is kept
# unarchived through Step 6.
td task list --filter "search: [TEST-$RUNID]" --all --json | jq -r '.results[]|select(.content|startswith("[TEST-'"$RUNID"']"))|.id' \
  | xargs -r -I{} td task delete {} --yes
```

Expected: the second create returns the existing card id (no dup); the `status` field reads `triage`; the
CLI-created card has **no `kanban_notify_subs` row** (Z1 — the CLI is subscription-free); the
non-spawnable assignee keeps the live dispatcher from ever running the probe card. **Acceptance:** idempotency
key dedupes; the no-subscription-row check passes (Z1); a simulated stage crash restarts from stage 1 and still yields one task; a forced stage error
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
  - **STAGE-LOGIC tests stay on the direct harness (Z6):** dated placement + crash-restart idempotency above are
    pure stage logic — a direct `-z` invocation drives them deterministically.
  - **TERMINAL-EVENT test runs through the REAL DISPATCHER on a PURPOSE-BUILT card (AA12) — a direct `-z`
    invocation is NOT a claimed worker, so it CANNOT produce a genuine `timed_out`/`crashed`/`gave_up` event**
    (verified: those events are emitted only for dispatcher-claimed workers — `enforce_max_runtime` /
    `detect_crashed_workers`, `hermes_cli/kanban_db.py:5766+`/`6027+`). **`hermes kanban edit` CANNOT set the
    runtime cap post-hoc — verified `hermes kanban edit --help` exposes only `--result`/`--summary`/`--metadata`
    (AA12)** — so the runtime cap MUST be set at create time, and the earlier "reassign the probe card then force
    `--max-runtime`" instruction is **deleted** (it relied on setting runtime after creation, which is
    impossible). Instead create a **dedicated terminal-event probe card WITH `--max-runtime 1 --max-retries 1`,
    assigned to a REAL profile (`default`)** + a **deterministic slow worker** (a skill/prompt that sleeps past
    1s — any real agent turn already exceeds a 1-second cap), so ONE dispatcher tick claims + spawns it, it
    **times out (`timed_out`) → with `--max-retries 1` the first failure trips the breaker (`gave_up`) → the card
    goes `blocked`/terminal**; then **archive it**. Assert the recorded terminal **event** = `timed_out`, the
    outcome = `gave_up`, and the status = `blocked`; the captured item is still safe in Inbox. **Do NOT assert
    the `#forzare-errors` delivery here** — that end-to-end route (seeded event → `hermes send` → spool) is
    asserted in **Task F1** (which owns the watchdog + spool), avoiding a forward dependency and any user-visible
    message before Phase G.
  - **RECORD a machine-checked result file (Z6/R6A6).** The harness writes its verdicts to
    `~/workspaces/Ivy/forzare/state/d1-harness-result.json` — `{dated_placement_kind, restart_task_count,
    restart_event_count, terminal_event, terminal_status, all_pass}` — so **Checkpoint D consumes a machine
    result, not prose.**
  - **Archive the card LAST (Z6):** `hermes kanban archive "$ID1"` is the final line of Step 6, after every
    assertion has run against the still-live card.

  **Acceptance (Step 6):** the direct harness drives the stage-LOGIC tests deterministically (dated placement
  writes the correct due + `kind`; a re-run yields exactly one task/event); the **terminal-EVENT test creates a
  dedicated probe card WITH `--max-runtime 1 --max-retries 1` + a deterministic slow worker (AA12 — kanban edit
  can't set runtime post-hoc)** and asserts the real-dispatcher chain **`timed_out` event → `gave_up` outcome →
  `blocked` status → archive**; a **machine-checked `d1-harness-result.json`** is written (Checkpoint D reads
  it); and the probe card is
  **archived as the last step**, kept unarchived through every assertion (Z6).

---

### CHECKPOINT D (inline, fail-closed — Y6, post-Phase-D) — do this BEFORE Phase E/F/G

**NEW post-D checkpoint (Y6).** Phase D authors no new chezmoi file (the board config is the `config.yaml`
`kanban.*` stanza already applied + gated at Checkpoint A; the pipeline skill is B11, live from Checkpoint B),
so this is a **verify gate**, not an apply: confirm the capture pipeline is exercised end-to-end and the board
config is live before delivery/watchdog phases build on it.

- **Verify (fail-closed):** Checkpoint D **consumes the machine-checked D1 result file, NOT prose (Z6/R6A6)** —
  it reads `~/workspaces/Ivy/forzare/state/d1-harness-result.json` and asserts `all_pass == true` (dated
  placement wrote the right `kind`, the re-run converged to one task/event, and the **dispatcher-driven**
  terminal-event test recorded a genuine `timed_out`/`crashed`/`gave_up` — the errors-channel route is F1's,
  R5A7). AND the live `config.yaml` `kanban.*` stanza resolves as expected (`default_assignee: "default"`,
  `auto_decompose: false`, `auto_subscribe_on_create: false`, `max_in_progress_per_profile: 2`, `failure_limit:
  2`) via a resolved read:

```bash
set -o pipefail
# Z6: the D1 harness result is a MACHINE artifact, not a prose "it passed":
RES=~/workspaces/Ivy/forzare/state/d1-harness-result.json
jq -e '.all_pass == true and (.terminal_event|IN("timed_out","crashed","gave_up"))' "$RES" \
  || { echo "FATAL: D1 harness result not all_pass or missing a real terminal event (Z6)" >&2; exit 1; }
echo "D1 harness result OK (machine-checked, Z6): $(jq -c . "$RES")"
~/.hermes/hermes-agent/venv/bin/python - <<'PY'
import os, yaml
k = (yaml.safe_load(open(os.path.expanduser("~/.hermes/config.yaml"))).get("kanban",{}) or {})
assert k.get("default_assignee") == "default", k.get("default_assignee")
assert k.get("auto_decompose") is False, k.get("auto_decompose")
assert k.get("auto_subscribe_on_create") is False, k.get("auto_subscribe_on_create")  # Z1 firewall guard
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
  — **zero LLM**, out-of-band, doing SIX state-stamped scans per pass (spec §14/U3):
  - **(a) Gateway health.** Probe **`curl -fsS -m 3 http://127.0.0.1:8644/health`**, branch on exit code —
    **0 = up, 28 = hung, 7 = down** (spec §19). On down / hung / restart-looping → alert.
  - **(b) forzare run failures — the predicate is a causal run EVENT, never status+counter (W9, corrects
    V9/R2A6).** Since the last stamped watermark, scan `~/.hermes/cron/output/` for failed ritual runs and the
    kanban DB for **genuine** failures, routing each to the errors channel. Alert **ONLY** on failure
    **events/outcomes since the watermark**: a **`gave_up`** outcome (the `failure_limit` trip); a
    **`timed_out`** or **`crashed`** run event. **NEVER derive failure from `status='blocked' AND
    consecutive_failures > 0`** — verified: **`block_task` does NOT clear `consecutive_failures`**
    (`kanban_db.py:4383` sets only status/claim fields — that claim stands). The counter *is* cleared on the
    UNBLOCK path (`unblock_task`, `kanban_db.py:4560-62`, verified — CC9 corrects the earlier "cleared only on
    success/reassign" wording; the "only" claim is dropped), but an awaiting-user block never passes through
    unblock — so a healthy **awaiting-user** block after one recovered transient failure still carries
    `counter == 1` and the
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
  - **(d) Ritual-ABSENCE detection — "the brief silently never ran" (AA8; go-live-KEYED, CC3).** Scans (b)/(c)
    catch a ritual that ran and failed; NONE catches a ritual that **never fired**. So the pass **loads the exact
    six-job C2 manifest** (`forzare-morning-brief`, `forzare-eod`, `forzare-waiting-reconcile`,
    `forzare-gym-window-end`, `forzare-block-boundary`, `forzare-someday-sweep`; CC14 — the authoritative manifest
    the spec §14 boot line points at) and, per job, asserts its `jobs.json` `last_run_at` **and** its newest
    `~/.hermes/cron/output/<job_id>/` timestamp are within the job's **schedule-derived deadline + a 30-min grace
    (DECIDED)**. **A missing/disabled/paused/no-output job past deadline+grace is a finding — but the scan is
    KEYED ON `forzare/state/go-live.json` (CC3): PRE-go-live it only LOGS the finding (staging jobs are
    DELIBERATELY `--deliver local`/paused, so alerting would be a false alarm), POST-go-live it ALERTS** ("
    forzare-morning-brief has not run since <ts>"), same content-stable id + spool. This closes the silent
    no-fire — the failure a prospective-memory-impaired user would never notice — without false-alarming during
    the staging window.
  - **(e) Stale-triage detection — the `specify` backstop (AA5/BB1).** Because the bounded `specify` attempt can
    fail or time out (§8b/B11/BB1), a `specify` that never completed leaves a capture card stuck in `triage`. So
    the pass reads the private Kanban board for any **forzare capture card in `status = triage` past its create
    time + 30-min grace ⇒ an errors-channel alert** (same content-stable id + spool) — catching a wedged/failed
    specify (including a failed one-shot `--no-agent` retry) that would otherwise silently swallow a capture.
  - **(f) Skill-INTEGRITY scan — the runtime guard replacing the removed boot-abort (BB8).** Because forzare
    cannot hook Hermes' own boot (no-patching rule, spec §13), this is the standing runtime guard against Hermes'
    silent-skip behavior: each pass asserts **every V1 skill is installed at its expected path AND its `SKILL.md`
    content-hash matches the chezmoi source** — the bundle skills, the on-demand handles
    (`forzare-next`/`forzare-today`/`forzare-capture`), the `/forzare` classifier, `forzare-capture-pipeline`,
    `calibration-log`, and the shared mutation helper, plus the 3 bundle YAMLs — and on any **missing or
    content-drifted** skill ⇒ an errors-channel alert (same content-stable id + spool). Without it a typo'd or
    half-applied skill degrades the engine invisibly, because Hermes' bundle loader silently skips it.
  - **Alert:** **`hermes send --to discord:<#forzare-errors>`** (R2 — no LLM, no agent loop, no running
    gateway for bot-token platforms), plus the relay's phone/local push as belt-and-suspenders; if
    `DISCORD_ERRORS_CHANNEL` is unset, fall back to the home channel with a `⚠ ERROR` prefix (the fallback
    lives HERE, not in hermes). **Robustness under launchd's minimal env (W9):** resolve the **absolute
    `hermes` binary path at install** — a script-level `HERMES_BIN` constant or the plist's
    `EnvironmentVariables` `PATH` including `~/.local/bin` — and **load the channel env by dotenv-PARSING
    `~/.hermes/.env` for exactly `DISCORD_ERRORS_CHANNEL`/`DISCORD_HOME_CHANNEL` — NEVER `source`-ing it (Z4).**
    The managed `.env` carries an unquoted value with spaces that **crashes a strict shell** (`set -euo
    pipefail` + `. ~/.hermes/.env` aborts before the alert can send), so extract only the two keys without
    evaluating the file:
    `dotenv_get(){ sed -n "s/^[[:space:]]*$1=//p" ~/.hermes/.env | tail -n1 | sed -e 's/^"\(.*\)"$/\1/'; }`.
    An inherited-PATH `hermes: command not found` or an unset channel would silently swallow the alert. `set
    -euo pipefail`, double-quoted
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
  the kanban DB (event-based: `gave_up`/`crashed`/`timed_out`), scans `jobs.json` for `last_delivery_error`
  (delivery-only failures, X8), **loads the six-job C2 manifest for ritual-ABSENCE (a job that never ran by its
  deadline+30-min grace, AA8; go-live-KEYED — LOG pre-go-live / ALERT post-go-live, CC3)**, **scans the board
  for STALE TRIAGE cards (> 30 min, the specify backstop, AA5/BB1)**, **and runs the skill-INTEGRITY scan (every
  V1 skill + bundle YAML + helper: installed-path + content-hash, the runtime guard replacing the removed
  boot-abort, BB8)**, alerts out-of-band via `hermes send --to` (never through the dead gateway), and closes the
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

Expected: shellcheck clean; `plutil -lint` → `OK`; the live probe exits 0. **Acceptance (W9/X8/AA8/AA5/BB8/CC3
cases):**
the script alerts via `hermes send --to` on a simulated down/hung code (bogus port), on a seeded `gave_up`
outcome, on a `timed_out`/`crashed` run event, **and on a seeded `jobs.json` `last_delivery_error` for an
otherwise-`ok` run (X8 — the delivery-only failure the run-outcome scans miss)**; **on ritual ABSENCE (AA8),
GO-LIVE-KEYED (CC3) — with `go-live.json` present/`gone_live:true`, fixtures fire an alert: a DELETED job
(missing from the manifest), a DISABLED/paused job, and a job whose newest `cron/output/` timestamp is past its
schedule-derived deadline + 30-min grace (a NO-OUTPUT run); but with `go-live.json` ABSENT/`false` (staging) the
SAME paused-job fixture LOGS only, NO alert** (paused-while-staging ⇒ no alert; paused-after-go-live ⇒ alert);
**on a STALE-TRIAGE card (AA5/BB1) — a seeded capture card left in `triage` past its create time + 30 min fires
an alert**; **on skill-INTEGRITY drift (BB8) — a fixture that moves or content-edits one V1 skill's `SKILL.md`
fires an alert (path-missing and hash-mismatch cases), then restore**; it stays **silent** for ANY `blocked`
card that has emitted
no failure event — **including the recovered-failure-then-user-block fixture** (a card that fails once, is
retried successfully, then blocks awaiting the user: `status='blocked'`, `consecutive_failures == 1`, no
gave_up/crashed/timed_out event since the watermark ⇒ NO alert — this is the case the old status+counter
predicate got wrong) — and when healthy; a **second scan does NOT re-alert** the same failure (content-stable
ids); a **simulated Discord outage** (`hermes send` exit-1) **retains the spool and retries next pass** (no
lost alert); and — **the launchd-minimal-environment test (Z4/W9)** — the script, run with a **scrubbed env**
(`env -i PATH=/usr/bin:/bin HOME="$HOME" bash forzare-ops-watchdog.sh …`, mimicking launchd's stripped
environment), still **resolves `hermes`** (absolute `HERMES_BIN`) and **dotenv-PARSES both channel ids without
crashing** — proving it never `source`s the `.env` (a `. ~/.hermes/.env` under `set -euo pipefail` would abort
on the managed file's unquoted value with spaces, swallowing the alert). **The end-to-end errors-channel ROUTE
test moved here from D1 (R5A7):**
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
  EOD roll + **Bob-owned p1-clear (AA2)** + **lifecycle-ledger** ticks behave, and the 02:00 reconcile marks
  (never messages). **Assert the actual step ORDER / mutation boundaries from the INTENTS LOG + trace
  (V7/R3A1):** the **EOD** run journals **zero `p1.set` and zero `calendar.*` intents** in `dryrun-intents.jsonl`
  (EOD sets no p1 and writes no calendar, spec §8/R2A8) **and its `p1.clear` intents target ONLY the day's
  `plan-of-day` `selected_ids` — never a user-set p1 (AA2).** So the gate seeds TWO [TEST] p1s: a **Bob-owned**
  one (listed in `plan-of-day.selected_ids`) and a **user-set** one (NOT listed), then asserts `p1.set == 0`,
  `p1.clear` present for the Bob-owned id, and **`p1.clear` ABSENT for the user-set id** (the old "clear every
  unfinished p1 unconditionally" rule would have wrongly cleared it). A correct dry-run left the real store
  untouched, so the intents log — not `td activity` or the 🤖 calendar — is where a boundary violation would
  show. The **morning** run's roll intents precede its `p1.set` intents in the log's record order (the defensive
  `eod-roll` before any `eisenhower-plan` p1 write); a bundle whose instruction failed to sequence would journal
  a `p1.set` before the roll intents — that fails this gate.

```bash
set -o pipefail
INTENTS=~/workspaces/Ivy/forzare/state/dryrun-intents.jsonl
POD=~/workspaces/Ivy/forzare/state/plan-of-day.json
# EOD gate (AA2): zero p1.set + zero calendar.*; p1.clear targets ONLY plan-of-day selected_ids (Bob-owned),
# NEVER a user-set p1. Seed a BOB-owned p1 (in selected_ids) + a USER-set p1 (not in selected_ids).
DRY='DRY RUN — record intended writes to forzare/state/dryrun-intents.jsonl, perform none. '
TODAY=$(TZ=America/Denver date +%F)
# BB6: run-id-scoped fixtures + run-id-suffixed POD backup + a trap that restores it and deletes captured ids.
RUNID="$(date +%s)-$$"; CREATED=()
cp "$POD" "$POD.bak.$RUNID" 2>/dev/null || true
trap '[ -f "$POD.bak.$RUNID" ] && mv "$POD.bak.$RUNID" "$POD" || rm -f "$POD"; for id in "${CREATED[@]}"; do td task delete "$id" --yes >/dev/null 2>&1 || true; done' EXIT INT
BOB=$(td task add "[TEST-$RUNID] eod bob-p1" --priority p1 --due today --json | jq -r '.id'); CREATED+=("$BOB")
USR=$(td task add "[TEST-$RUNID] eod user-p1" --priority p1 --due today --json | jq -r '.id'); CREATED+=("$USR")
printf '{"date":"%s","selected_ids":["%s"],"anchor":"%s","writes":{"p1_set":true,"anchor_placed":true,"alarm_set":true}}\n' "$TODAY" "$BOB" "$BOB" > "$POD"
: > "$INTENTS"
JE=$(stage_skill '0 0 1 1 *' "${DRY}Run the forzare-eod bundle once; clear ONLY the day's plan-of-day selected_ids." forzare-eod test-eod-gate); hermes cron remove "$JE"
RID=$(jq -rs 'map(.run_id)|last // empty' "$INTENTS")
NSET=$(jq -s --arg r "$RID" '[.[]|select(.run_id==$r and .op=="p1.set")]|length' "$INTENTS")
NCAL=$(jq -s --arg r "$RID" '[.[]|select(.run_id==$r and (.op|startswith("calendar.")))]|length' "$INTENTS")
CLR_BOB=$(jq -s --arg r "$RID" --arg id "$BOB" '[.[]|select(.run_id==$r and .op=="p1.clear" and .target==$id)]|length' "$INTENTS")
CLR_USR=$(jq -s --arg r "$RID" --arg id "$USR" '[.[]|select(.run_id==$r and .op=="p1.clear" and .target==$id)]|length' "$INTENTS")
[ "$NSET" = 0 ] || { echo "FATAL: EOD journaled $NSET p1.set intent(s) — EOD sets no p1 (Z10)" >&2; exit 1; }
[ "$NCAL" = 0 ] || { echo "FATAL: EOD journaled $NCAL calendar intent(s) — EOD writes no calendar (R2A8)" >&2; exit 1; }
[ "$CLR_BOB" -gt 0 ] || { echo "FATAL: EOD did not clear the Bob-owned p1 in selected_ids (AA2)" >&2; exit 1; }
[ "$CLR_USR" = 0 ] || { echo "FATAL: EOD cleared a USER-set p1 ($CLR_USR) — must clear ONLY selected_ids (AA2)" >&2; exit 1; }
echo "EOD gate OK (AA2): 0 p1.set, 0 calendar, Bob-owned p1 cleared, USER-set p1 UNTOUCHED"
# POD restore + captured-id deletion happen in the EXIT/INT trap (BB6).
```
- [ ] **Step 2: Explicit go-live matrix (replaces "several days / sensibly").** Drive each scenario and assert
  expected state + message count (U15):

  | Scenario | Expected state | Expected messages |
  |---|---|---|
  | Work day (Tue/Thu/Sat 15:00–23:00) | deep window = morning; evening = work | 1 brief |
  | Off day (Mon/Wed/Fri) | deep window = morning + evening | 1 brief |
  | ON-Sunday (alt-anchor Jun 7=ON) | work-day brief | 1 brief |
  | Recovery morning (post-overnight) | recovery/sleep window, no deep push | 1 brief, no gym nag |
  | Recovery fire — ≤2h catch-up, >2h past-grace single fire (V3) | the day closes exactly once (range + stamp) | 0 extra |
  | **EOD ceiling by cutoff (X6), driven by `FORZARE_NOW` (CC4/CC12): 22:59 / 23:00 / just-past-midnight / catch-up / manual mid-day** | 22:59 (before cutoff) ⇒ CEILING = yesterday; 23:00 (at cutoff) ⇒ CEILING = today; just-past-midnight ⇒ still closes the prior day; a manual `/forzare-eod` follows the same cutoff rule — each a once-only close (clock set via the staging `FORZARE_NOW` override, not the real wall-clock) | 0 extra |
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

# CONCURRENT-TRIGGER row (R6A11): a cron ritual and a live one-shot fired in the SAME window must EACH emit at
# most ONE surfaced item (§12.3/W12 residual accepted — not a mutex, but each path is one-thing-bounded). Drive
# both simultaneously and assert each surface is a single item. Both staged --deliver local / dry-run.
: > "$INTENTS"
JCT=$(hermes cron create '0 0 1 1 *' "${DRY}Surface the next ONE thing or [SILENT]." --skill todoist-surface --deliver local --name test-concurrent | jid_from_create)
# fire the cron job and a simultaneous one-shot in the background, then wait:
( hermes cron run "$JCT" >/dev/null && hermes cron tick >/dev/null ) &
hermes -p default -z "${DRY}Surface the next ONE thing or [SILENT]." --skills todoist-surface >/tmp/forzare-oneshot.out 2>&1 &
wait
CRON_AUDIT=~/.hermes/cron/output/"$JCT"
# each surface must be a SINGLE item — parse the cron audit's ## Response section ONLY (R7A4/AA10; the audit
# .md EMBEDS the ## Prompt whose skill-instruction text would over-count). The one-shot stdout has no such
# embedding, but strip any prompt echo defensively by counting only actionable lines.
CRESP=$(mktemp); awk '/^## *Response/{f=1;next} /^## /{f=0} f' "$CRON_AUDIT"/*.md > "$CRESP"
CN=$(grep -cE '\?[[:space:]]*$|^[[:space:]]*(First|Start|Do|Next|Surface):' "$CRESP" || true)
ON=$(grep -cE '\?[[:space:]]*$|^[[:space:]]*(First|Start|Do|Next|Surface):' /tmp/forzare-oneshot.out || true)
rm -f "$CRESP"
[ "${CN:-0}" -le 1 ] && [ "${ON:-0}" -le 1 ] \
  || { echo "FATAL: concurrent trigger surfaced a WALL (cron=$CN, one-shot=$ON) — each must be ≤1 (§12.3/W12/AA10)" >&2; exit 1; }
echo "concurrent-trigger OK: cron + one-shot each surfaced ≤1 item (residual interleave accepted, not a wall)"
hermes cron remove "$JCT"; rm -f /tmp/forzare-oneshot.out
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
  4. **Write the go-live flag (CC3):** `printf '{"gone_live":true,"ts":"%s"}\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
     > ~/workspaces/Ivy/forzare/state/go-live.json` — this **arms the watchdog's ritual-absence scan (F1 (d))
     to ALERT** (before this flag it only LOGS, so the staging window's deliberately-paused jobs never
     false-alarm).
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
R1a) → C2; Kanban capture pipeline (assignee `default`, **PARENT owns placement; `specify` is a BOUNDED
synchronous attempt supervised by a persisted `--no-agent` cron retry + the stale-triage backstop — BB1
supersedes the AA5 "detached fire-and-forget", and the ungrounded "retried / raises a failure event" claims are
deleted** +
**Inbox-task-id idempotency keys** + `--max-runtime 900` (Y7) + the **installed (no pin — AA11)**
`forzare-capture-pipeline` skill **authored in B11** (R5A4) + **NO `notify-subscribe` — decision-queue
`triage-reraise` re-raise + watchdog failures/stale-triage instead, Y2/AA5** + 5 stages + dup-guards, R7/W4;
**test isolation = a non-spawnable assignee, not a board — the dispatcher enumerates every board, W4**;
**terminal-event probe created WITH `--max-runtime 1 --max-retries 1`, AA12**) → B11/D1; root `session_reset` (R6a) + `[SILENT]` per-path (**direct filter-function probes,
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
staged tests truncate the intents log at start (R4A11); the §4c/§15 idempotency guard is the per-day plan
record `plan-of-day.json` (Y13/R6A2, superseding the "any p1 present" heuristic)
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

**Round-6 additions:** **Kanban subscription firewall guard** (Z1/R6A1): `kanban.auto_subscribe_on_create:
false` added to A2 (audit + apply + verify), the CLI-transport hard rule in B11/D1, and a **no-subscription-row**
assertion (`hermes kanban notify-list` empty) in D1 — the CLI create path is verified subscription-free
(`hermes_cli/kanban.py`), the tool path gated by the config key (default `True`, `config.py:1348`) → A2/B11/D1
+ Checkpoint D. **Decision-queue identity + concurrency** (Z2): records gain `{id, enqueue_ts, rev}`, total
order `(class-rank, enqueue_ts, id)` with class-rank waiting-chase>fixed-redecision>stall-decision>triage-reraise>
sweep-candidate (R6A10), queue mutations under the map/journal lock + atomic-replace, dedup-by-`id`, CAS ack —
the B0 helper is the single queue writer, with producer-race / duplicate-reconcile / ack-vs-promotion fixtures
→ B0/A1/B5. **Journal completeness + per-type healing** (Z3): the type enum gains `task.add` + `task.complete`
(aligning with the intent-op enum), and pending→commit→heal is defined per type with a type-specific predicate
(comment content, calendar event-key, current-value compare, dedup-key lookup, task state), one after-write
crash fixture per type → B0. **Never `source` the managed `.env`** (Z4): Checkpoint A's channel gate and F1's
watchdog **dotenv-PARSE** the two channel keys (grep/sed, no evaluation — the live file's unquoted spaces crash
a strict shell), + a **launchd-minimal-environment test** in F1 → Checkpoint A/F1. **Checkpoint B
de-circularized** (Z5): it verifies applied files + resolved config ONLY (not pins); a new **POST-PIN GATE**
asserts pins after stage-3 pinning; the boot-check script authoring moves to the Phase-B author stage →
Phase-B flow/Checkpoint B/C1. **D1 dispatcher-driven terminal events** (Z6/R6A6): the probe card is kept
unarchived through Step 6 (archive is the LAST step); stage-LOGIC tests stay on the direct harness but
**terminal-EVENT tests run through the real dispatcher** (non-spawnable→spawnable→failure→revert, since events
only come from claimed workers, `kanban_db.py:5766+/6027+`); Checkpoint D consumes a **machine-checked
`d1-harness-result.json`**, not prose → D1/Checkpoint D. **Truly transactional cron install** (Z7): pre-validate
the manifest (reject dup names), snapshot every to-be-edited record, rollback restores edits AND deletes
creates, post-install exact-manifest assert → C2. **Staged leak gate on INDEPENDENT snapshots** (Z8/R6A4): the
C2 bundle-effects gate **seeds its [TEST] fixture** and diffs task activity + a separate `--type comment` stream
+ a [TEST]-scoped calendar list + hashes of ALL owned-state files, intent log as positive evidence only →
C2. **Exact `work_schedule` schema + DOW-aware crons** (Z9/R6A5): B10 validates the per-weekday map key-by-key;
C2 derives DOW-aware crons (`0 14 * * 0,2,4,6` at round 6 — **superseded by round 7's R7A2 formula
`35 13 * * 0,2,4,6` = 13:35, CC13**; never a flat `block_start`, never firing on off days; alt-Sunday
fires weekly, the skill no-ops on OFF Sundays) → B10/C2. **§15 p1-guard swept + G1 EOD gate** (Z10/R6A2): the
stale "any p1 present" recap replaced by the plan-of-day record; G1 asserts **zero `p1.set` but REQUIRED
`p1.clear`** intents (unconditional clear) → G1. **Named receptivity-gate owner + boundary tests** (Z11/R6A3):
N/D/S are B10 `skills.config`, `todoist-surface` owns the gate, with 2-vs-3-dismissal / 7-vs-8-surfacing
boundary tests → B1/B10; the B9 correlator gets a **two-page cursor stub** asserting the page-2 fetch
(R6A8). **Brief response-structure fixture** (Z12): B4's imperative-count harness asserts ≤1 actionable line
with a queue head + weather + activation present → B4. **Bankruptcy two-class** (Z13): B5 UNDATEs dated /
RETIREs undated onto `sweep-exclusion.json`, with a before/after set-membership test, never a destructive op →
B5/A1/B0. **Sweeps cleanup** (Z14): the 02:00 unblock re-date rewrites `kind` `waiting_checkback → surfacing`
with calendar-only + comment-only fixtures → B7. **Incident citations split** (R6A7): the Todoist parent-delete
cascade and the separate 2026-05-20 vault refactor cited distinctly → B5. **Passing-mention negatives** (R6A9):
B6 asserts "my friend works at a gym" / "work was busy" do NOT fire → B6. **Unfalsifiable acceptances get
harnesses** (R6A11): B2 synthetic weather threshold-crossing, B4 plan-of-day-resume + >3-Q1-conflict +
imperative-count, B5 head/ack/bankruptcy queue fixtures, G1 concurrent-trigger scripted (cron + one-shot, each
≤1 item) → B2/B4/B5/G1.

**Round-7 additions.** **Leak gate rebuilt** (AA1/R7A1/R7A7/R7A8): the C2 negative gate is now INDEPENDENT of
the intent log — `hash_state` RECURSES over `state/` + `calibration/` and **EXCLUDES `dryrun-intents.jsonl`**
(hashing it was self-defeating), the task + comment activity diffs are **cursor-paginated to exhaustion** and
scoped to the **[TEST] fingerprint** (the `--by me` scoping swept, in Global Constraints + C2), intents =
positive evidence only → Global Constraints/C2. **p1 ownership** (AA2): the EOD p1-clear targets ONLY
`plan-of-day.selected_ids` (never a user-set p1), user p1s count toward the ≤3 budget, a user p1 > 48h → a
`stale-p1` queue item; G1's EOD gate seeds a Bob-owned + a user-set p1 and asserts the user one is UNTOUCHED →
B7/G1 + §4b/§4c/§8/§13. **Three-way healing** (AA3): B0 records `{old_value, intended_value, external_marker?}`
and heals absent→replay / intended→commit / OTHER→abort+flag (never overwrites user state); `task.add` has NO
idempotency (dedup by content+project search); the waiting-clear+redate+flip is ONE composite transition; the
MAP stays 4-field (op records in the journal) → B0. **Decision-queue schema** (AA4): eight classes (adds
`q1-conflict`/`stale-p1`/`bankruptcy-offer`), **`id` = stable `class:task_id`, content-INDEPENDENT** (a changed
`proposed` updates IN PLACE + `rev++`), rev contract + obsolete-revision retirement → B0/A1/B4/B5. **Capture
flow re-sequenced** (AA5): placement (cases 1–4) moves to the PARENT (decide-in-context); `specify` is the
background job's first supervised act (fired detached, off the parent's path) — **superseded by round 8 (BB1):
a detached `specify` cannot be retried or raise a failure event, so it becomes a BOUNDED synchronous attempt +
a persisted `--no-agent` cron retry + the stale-triage backstop**; the watchdog gains a stale-triage scan →
B11/D1/F1. **Bankruptcy class-1 reachable** (AA6/R7A5): stale dated actives
= `roll_count ≥ 10` AND no-progress ≥ 30 days (the "never moved" contradiction deleted); B5 seeds a >25 mixed
[TEST] set, the OFFER asserts ZERO clear intents, the acknowledged op UNDATEs each dated + RETIREs each undated
over the frozen seeded ids → B5. **Boundary FORMULA** (AA7/R7A2): C2 derives the boundary cron from
`block_start − prep − travel − 30` = `35 13 * * 0,2,4,6` (13:35, distinct from the 14:05 leave-time alarm) →
C2. **Watchdog ritual-ABSENCE** (AA8): loads the six-job manifest and alerts on any enabled job that never ran
by its deadline+30-min grace (deleted / disabled / no-output fixtures) → F1. **Cron installer hardened** (AA9):
rollback = atomic restore of the validated `jobs.json` backup + byte-compare, the ERR trap armed through ALL
postconditions, `--skill` repeated per skill, dup names rejected pre-mutation → C2. **Seeded fixtures +
response-section parsing + cardinality-EXACTLY-1** (AA10/R7A3/R7A4/R7A6): B4/B5/G1 seed real tasks/queue
records/synthetic weather, parse the `## Response` section only, and assert exactly-1 when the queue is nonempty;
B4's resume is schedule-deterministic with an off-day variant → B4/B5/G1. **Curator pinning DROPPED** (AA11):
every `hermes curator pin` removed, the POST-PIN GATE → SKILL-INTEGRITY GATE (installed path + content hash) —
verified `skill_usage.py` excludes repo-authored skills from the curator's managed list → Phase B flow /
Checkpoints B/C / C1 / all B-tasks. **Terminal-event probe** (AA12): created WITH `--max-runtime 1 --max-retries
1` + a slow worker, asserting `timed_out → gave_up → blocked → archive` (kanban edit can't set runtime post-hoc)
→ D1. Plus: `waiting-chase` enqueued most-overdue-first with strictly increasing `enqueue_ts` (R7A11, spec §8);
the §11 "Today's-3 guard" remnant → the plan record (R7A9); the shadow-last-reconcile rule is B7's authored
contract + the Phase-B dry-run contract (R7A10); `gate-check.sh` pins a validated `REPO` constant, no cwd
dependence (R7A12).

**Round-8 additions.** **`specify` supervision made Hermes-valid** (BB1/CC2): the ungrounded "retried on transient
failure" / "raises a failure event" claims DELETED (a parent-run `specify` is not a dispatcher-claimed worker) —
it becomes a BOUNDED synchronous attempt; on failure/timeout the card stays `triage`, the parent says "capture
saved; processing delayed", and a one-shot `--no-agent` cron job retries `hermes kanban specify` (its failure →
F1's failed-run scan), backstopped by the stale-triage scan → B11/D1. **Queue lifecycle completed**
(BB2/CC7/CC10): AGGREGATE ids (`q1-conflict:<date>`, `bankruptcy-offer:<YYYY-MM>`; per-task classes keep
`class:task_id`), promotion participates in the order via the `head` flag `(head DESC, class-rank, enqueue_ts,
id)`, ack TOMBSTONES `{id, gen}` + re-enqueue opens `gen+1`/`rev=1`, CAS = `{id, gen, rev}`, and ANY intra-day
resolution is tombstoned by the live turn (CC10) — delayed-answer / ack-then-reenqueue / non-head fixtures →
B0/A1/B4/B5. **`task.add` healing MARKER** (BB3): a `⟦fz:<journal-uuid>⟧` line appended to the description at
create (journaled before the API call, stripped on commit-verify); heal by marker search (collision/rename/move
fixtures), no marker ⇒ abort+flag; journal/intent enums gain `waiting-clear`/`undate`/`retire` → B0/Phase-B intro.
**Gateway-stopped cron install + atomic tmp/rename restore** (BB4): the C2 install runs in a gateway-stopped
window and its rollback is a same-dir tmp + `mv` (+ byte-compare) → C2. **gog probes hardened** (BB5): explicit
`-a <account>` + the 🤖 calendar id, command/parse failure is FATAL (no `|| echo 0`), the leak-gate calendar
compare is on event IDs + `updated` fields, not counts → B3/C2. **Trap-guarded run-id-scoped harness safety**
(BB6): every staged test uses `[TEST-$RUNID]` fixtures, run-id-suffixed backups restored by an EXIT/INT trap, and
captured-id cleanup (never a `search: [TEST]` sweep; cascade note) → Phase-B intro + B1/B4/B5/C2/D1/G1.
**Bankruptcy fixture honest** (BB7): dated actives satisfy the REAL eligibility (seeded lifecycle-MAP `roll_count
≥ 10` + a 40-day-old `written_due`) with >25 undated candidates, the acknowledged op CONSUMES the JOURNALED frozen
snapshot (no prompt id list), and a failure-between-batches fixture proves idempotent retry → B5. **Boot-abort
claim REMOVED** (BB8): forzare cannot hook Hermes' launchd boot, so integrity is the watchdog's per-pass
skill-INTEGRITY scan (F1 (f), EVERY V1 skill + bundles + helper) + the documented pre-start check → C1/F1. **Exactly-one gate
machine-readable** (BB10): the brief emits one `▶ ` marker line; the B4 harness counts markers == 1 for BOTH
queue states, the verb regex secondary → B4. **Calibration acceptance measures the policy** (BB11): deterministic
numeric fixtures (α-update 0.575, decreasing decay, duration-bias 1.5, habituation flag) + one end-to-end
recommendation shift → B9. **Pause-vs-absence reconciled** (CC3): the F1 absence scan (d) is keyed on
`go-live.json` (LOG pre-go-live / ALERT post-go-live; G1 writes the flag) → F1/G1/A1. **Staging test-override
schema authored** (CC4/CC12): `schedule-override.json`'s staging-only `{pinned_schedule, synthetic_weather,
FORZARE_NOW}` fields are an authored contract, cutoff tests driven by `FORZARE_NOW` → Phase-B intro/B7/G1. Plus
precision fixes: the unified journal/intent record shape stated once (CC8) → B0/Phase-B intro; the F1 scan (b)
citation corrected (`4560-62` is the unblock path, "only" dropped, `4383` stands, CC9) → F1; the ack-purity check
asserts NO ack-shaped intent, mtime compare dropped (CC6) → B5; "pinned" → "installed (integrity-gated)" for the
capture-pipeline skill (CC5) → D1; the round-6 boundary value annotated superseded (CC13) → changelog; the F1
absence-scan manifest cross-refs the six-job C2 set (CC14) → F1.

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
