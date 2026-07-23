> **⛔ SUPERSEDED 2026-06-10 by `specs/2026-06-10-osquery-alerting-master-spec-v1.md`.** Kept as history; do not use as the source of truth.

# osquery Alerting — Reshape Design (v2)

**Date:** 2026-06-09
**Supersedes the *approach* of:** `specs/2026-06-03-osquery-alerter-ingestion-model-design.md` +
`plans/2026-06-04-osquery-alerter-hardening.md` (detector-by-detector "fix the noisy alerter"). Those
remain as history; decisions #1/#2 fold in below.
**Grounded in:** two research passes (`research/2026-06-08-macos-persistence-monitoring-design-research.md`
+ the detection-engineering / Fleet-Kolide-Uptycs / Wazuh-FIM-EDR-Velociraptor sweeps) and the external
build-guidance file (`research/2026-06-09-osquery-alerting-build-guidance.md`).

## North star

**One calm, trustworthy Discord channel.** The failure loop: noise → ADHD overwhelm → ignored alerts →
quiet anxiety. So the channel must stay rare enough that **silence is trustworthy** and Stephen can stop
watching it. Precision is weighted far above recall: a recurring false positive trains him to ignore the
channel, and that ignore-response then spreads to the real alerts.

## Core architecture — two tiers, one gate

- **PAGE tier** — a small set of high-confidence, rare, "almost-certainly-bad **and** actionable"
  detections. Posts to Discord immediately (no batching — a fired alert must leave the box before any
  tampering).
- **LOG-ONLY tier** — everything else osquery collects. Stays on disk, queryable; **never** pushed.

Principle (osquery's own docs + Fleet): a healthy host returns **zero** page-tier rows; any page = review.
Meaning is *not* encoded in deterministic rules beyond these specific signatures — the noisy/ambiguous
middle is deferred (log-only), not ruled into submission.

- **Delivery:** osquery → results log → alerter → **Hermes webhook → Discord** (fixed sink, unchanged).
  Every alert tagged with **hostname**.
- **Collection:** scheduled **differential** queries (hourly), not raw evented alerting. First-run
  "everything added" baseline is **discarded** (`counter == 0`). [decisions #1]

## PAGE tier (the only things that ping you)

Each runs hourly/differential, is calibrated against the host baseline before it may page, ships
self-contained + actionable copy, and is allowlistable in one action.

**Core (from build-guidance):**

1. **New launchd persistence — existence-based on the differential `launchd` LABEL.** A new label appears
   → system LaunchDaemons page ALWAYS; user LaunchAgents page unless the label is in the per-host label
   allowlist (your agents' labels allowlisted during calibration — see Dresden note). The trust/writer-path
   multi-signal gate was reverted 2026-06-10 as bypassable (a `bash …/evil.sh` agent and a Developer-ID
   binary both evade it). Enrich with **es_launchd_writes** → "written by `<process>`" and the
   signing/quarantine verdict — as **text only, never a page suppressor**. [decisions #1 + 2026-06-10 re-review]
2. **New SSH `authorized_key`** — via **real-time `file_events`** on `authorized_keys` (page on
   CREATED/UPDATED, never DELETED) so a sub-hour add-then-revert can't slip a polled gap. The
   `authorized_keys` **table** is the enrichment lookup (username / algorithm / path only; **never** the
   key). [2026-06-10 re-review restored decision #4.]
3. **New admin / new user account** (admin group, gid 80; optionally any new user).

**Added — already detected here, high-signal:**
4\. **A protection turned OFF** — firewall / SIP / Gatekeeper / FileVault (+ screen-lock, sharing).
[security-policy-regression pack]
5\. **New setuid-root binary** (`suid_bin_unexpected`).

**Agent attack-surface (your primary access path → highest stakes):**
6\. **Agent binary swapped / launchd `program` rewritten** — `com.webdavis.paseo-daemon`,
`ai.hermes.gateway`, `com.claude.code`, `ai.openclaw.*`. Page on **program-path or signer change**
(updates change the hash legitimately → don't page on hash alone).
7\. **Agent network-exposure change** — an agent moving from `127.0.0.1` to a routable bind, or a
*different* process on a known agent port. **Targeted** `listening_ports`/`process_open_sockets` on agent
ports only (not the generic listener firehose).
8\. **Agent auth/secret file modified** — `PASEO_PASSWORD` source, API-key/token files, agent configs.
Targeted FIM; alert says "X modified" only, never the contents.

## LOG-ONLY (collected, queryable, never paged)

Installed apps, Homebrew packages, browser extensions, generic listening ports, recent logins/sessions,
raw file_events.

**Borderline — start log-only, promote to page only if proven quiet:** new kernel/system extension,
sudoers / sshd_config changes, launchd overrides, cron/startup items.

## Calibration (per host — this is what makes it quiet)

1. Validate each query with `osqueryi --json` on the host.
2. **Discard the first differential run** (baseline, not alerts).
3. Label one week: each fired row → *real* or *mine*; "mine" → allowlist.
4. **One-action allowlist** — a Discord reaction or a single command, **never** edit-config-and-redeploy.
5. Calibrated = a week of only-real-or-nothing.

## Heartbeat

Once daily at a fixed time → "✅ pipeline healthy, nothing to report." Makes silence mean **safe**, not
broken; its **absence** is the alarm. Non-optional here: Dresden is headless/remote — no local popup ever
reaches you.

## Scope — single host (Dresden)

**This implementation targets only Dresden** (locked 2026-06-10), provisioned via this chezmoi/dotfiles
repo. Alerts still carry a `host` tag (payload hygiene + future-proofing). **Multi-host is NOT built here:**
when Hermes moves to the homelab NUC, this implementation migrates **out of dotfiles** into homelab
automation (Ansible/K8s), where multi-host fan-in (per-host alert URLs, Hermes bound off `127.0.0.1`, a
firewall ACL) is built. The detection design here is the durable part; deployment relocates later.

## Dresden specifics

Server ~97% of the time, accessed remotely via agents (Claude Code mobile, Paseo, Hermes). Consequences:
the **agents are the main legitimate activity** that trips detectors (they install LaunchAgents → baseline
them in calibration); **remote sessions are constant** (→ logins log-only) but a **new SSH key is rare**
(→ page); **no screen access** → Discord + heartbeat is the only window.

## Deferred (not now)

- **The LLM judge (Mouse)** — only needed once a noisy tier exists. Keep it **out of the page-tier path**
  (an attacker can prompt-inject telemetry to make a *gating* LLM suppress its own alert). Refined
  invariant: the LLM may never gate the page tier; it may only ever gate the *noisy* tier, where a miss
  is low-stakes.
- **Wazuh migration** — revisit for fleet-scale management *after* the page tier proves calm.
- Beaconing / new-listening-port detection (normal churn on a dev box).
- **Homelab migration → multi-host.** When Hermes moves to the NUC, this implementation leaves dotfiles for
  homelab automation (Ansible/K8s); per-host alert URLs, Hermes bound off `127.0.0.1`, and a firewall ACL
  are built there — not in this repo.
- **Cross-host machine-death detection.** A host cannot detect its own death locally (a powered-off /
  network-isolated / launchd-wiped host emits nothing, indistinguishable from healthy-and-quiet). It needs
  an **off-host** consumer that pages when a host's daily ✅ heartbeat goes overdue — folds into the
  homelab / Wazuh layer once all hosts post to the one channel. Not built now.

## What we discarded from the build-guidance file

- "One host only / dresden" → **correct for this repo** (single-host Dresden); multi-host is a future
  homelab migration out of dotfiles, not a fleet rollout from here. (Reversed the earlier "it's a fleet"
  read — 2026-06-10.)
- `/var/log/osquery` → real path is `~/.local/log/osquery`; config lives in `/var/osquery` + packs +
  flags (chezmoi-templated), not the inline stanza.
- Its 3-only detection set → **added** protections-OFF, suid, and the agent surface (all already
  detectable here, all high-signal).
- "Hermes webhook unconfirmed" → confirmed: Hermes is the fixed delivery hop, unchanged.

## Phased build

- **Phase 0 — verify & baseline (must come first):** confirm each page-tier query on Dresden
  (`osqueryi --json`); confirm the Hermes payload contract; enumerate the agent daemons, their ports +
  expected bind addresses, binary paths + signers, and auth-file paths — and baseline them.
- **Phase 1 — page tier + gate:** wire the page-tier queries; alerter keeps only `added` rows for the
  page set, applies the allowlist, posts to Hermes with hostname + actionable copy; everything else
  stays log-only.
- **Phase 2 — calibration loop:** one-action allowlist (Discord reaction or command); discard-first-run;
  one-week label per host.
- **Phase 3 — heartbeat** via chezmoi (single host; no fleet rollout — see Scope + the homelab-migration deferral).

(The detailed per-task plan is written after Phase 0 — the SQL, paths, and Hermes contract must be
confirmed first, or the plan would be guesswork.)

______________________________________________________________________

## Phase 0 — confirmed facts (2026-06-09)

**Delivery (Hermes contract — from `dot_local/bin/executable_osquery-alert-dispatch.sh`):**

- Page → POST `http://127.0.0.1:8644/webhooks/osquery-priority` (the one channel Stephen watches).
- Body: `{"event_type":"osquery.alert","host":"<hostname>","alert":{"title":…,"detail":…}}` — the `host`
  field was added 2026-06-10; it stays inside the signed body so the `X-Request-ID = sha256(body)` dedup
  is coherent.
- Auth: HMAC-SHA256(body) → header `X-Webhook-Signature`; key from `~/.config/osquery/webhook-secret`
  (mode 600).
- Dedup: `X-Request-ID = osquery-<sha256(body)[:32]>` (gateway honours it 1h).
- **The quiet `#osquery` channel (`/webhooks/osquery`) is DROPPED** — non-page findings are log-only,
  never POSTed. (Confirmed by Stephen 2026-06-09.)

**Agent surface — finalized set (openclaw removed; codex added):**

| agent | binary / entrypoint to watch | local listener |
|---|---|---|
| hermes | `~/.hermes/hermes-agent/venv/bin/python -m hermes_cli` | **:8644** |
| qmd | `~/.volta/bin/qmd` | **:8181** |
| paseo | `~/.local/paseo-cli/bin/paseo` | outbound relay/TLS — no fixed local listener (confirm) |
| claude | `~/.local/bin/claude-restart.sh` → claude | none (no daemon) |
| codex | `/opt/homebrew/bin/codex` → `node_modules/@openai/codex/bin/codex.js` | none (interactive CLI) |

- **Binary-swap detection:** a change to any agent's launchd plist is already caught by the
  launchd-persistence page detection; add a hash-watch on the launch entrypoints above for the daemons.
- **Network-exposure detection:** applies only to the daemons with local listeners — **hermes :8644,
  qmd :8181** (page on a new bind-address or a different process on those ports).

**Auth/secret FIM targets — hash-diff on a schedule; page on content-hash change; the alert logs only the
*hash*, never the secret.** (All confirmed stable — mtimes Jun 1–5, not session-churned.)

- `~/.config/osquery/webhook-secret` — the alert pipeline's own HMAC key (tamper = forge/mute alerts)
- `~/.paseo/daemon-keypair.json`, `~/.paseo/cli-client-id`
- `~/.hermes/.env` (NOT `~/.hermes/config.yaml` — runtime-mutated by Hermes, excluded from FIM as noise)
- `~/.codex/config.toml` (stable). `~/.codex/auth.json` is **excluded** — it rotates on OAuth refresh.

**Skipped (chatty / session-state / rotating, not stable credentials):** `~/.claude.json`,
`~/.hermes/auth.json` + `channel_directory.json`, `~/.codex/auth.json` (OAuth rotation), `~/.codex/*.sqlite`
+ sessions. **openclaw removed entirely** (no longer used).

**Phase 0 query validation (2026-06-09, `osqueryi` on Dresden):**

- `new_ssh_key` (authorized_keys table) — ✅ runs. **But:** selecting only `(username, algorithm, key_file)`
  yields identical rows for two keys of the same algorithm in one file, so the differential can't tell a
  new key apart. Fix: add the `key` column (a per-key discriminator) to the query for diff fidelity, and
  **omit it from the alert text** (show username/algorithm/key_file only).
- `new_admin_user` — ✅ validated, but the guidance file's SQL was wrong: use `WHERE g.groupname='admin'`
  (or `ug.gid=80`), **not** `g.gname`. Baseline = root + stephen.
- agent auth-file FIM (`SELECT path, sha256 FROM hash WHERE path IN (…)`) — ✅ validated on all 6 stable
  credential files (logs only the hash).
- `suid_bin_unexpected`, all-users `authorized_keys`, and the agent-file hashes rely on the **root
  osqueryd daemon** (osqueryi-as-user can't read all of them); the daemon already runs as root.

## Open items the plan must resolve

1. **One-action allowlist mechanism — RESOLVED.** A Hermes `pre_gateway_dispatch` **plugin** matches an
   `allow <label>` reply in the security channel (owner + channel guarded) and appends the launchd label to
   the per-host label allowlist. No v1/CLI interim; the emoji-reaction UX stays deferred until Hermes
   reaction callbacks exist.
2. **Baseline-discard / calibration in the alerter.** Skip the first differential batch per query (osquery
   `counter == 0`) or seed the allowlist from it — the current alerter keys on a byte offset, not the
   counter. Define the mechanism.
3. **Protections-off is covered twice** — the 60s `firewall-gatekeeper-monitor` poller *and* the
   security-policy-regression pack. Pick one page path per protection to avoid double-paging (e.g. the
   poller owns firewall+gatekeeper; the pack owns sip/filevault/screenlock/sharing).
4. **Heartbeat = a deliberate daily ping** to the one channel (the price of trustable silence). Confirm
   cadence + that a daily "✅" is wanted vs. a quieter form.
5. **Alerter tiering:** enumerate the exact PAGE query-name set the alerter forwards; everything else →
   log-only (unsent). Drop the `#osquery` quiet channel.
6. **new_ssh_key — SUPERSEDED.** SSH paging moved to real-time `file_events` (PAGE item 2); the
   `authorized_keys` table is now enrichment only, so the differential-discriminator concern is moot.
