# osquery Alerting — Master Design Spec (v1)

> **Note (2026-06-10):** a three-tier revision supersedes this spec's *tiering* (two-tier page/log-only) — see
> `specs/2026-06-10-osquery-alerting-master-spec-v2.md` (+ v2 plan, tier matrix, test matrix, decision
> addendum). This v1 remains intact as history.

**Date:** 2026-06-10 · **Status:** the single source of design truth for the osquery security-alerting work.
**Implemented by:** `plans/2026-06-10-osquery-alerting-master-plan-v1.md` (the master plan; §PAGE set below
maps one-to-one to its tasks).
**Consolidates / supersedes:** `specs/2026-06-09-osquery-alerting-reshape-design.md` (reshape v2) and the
**Senior re-review (2026-06-10)** section of `decisions/2026-06-08-osquery-alerter-redesign-decisions.md`.
Full rationale/history lives in that decision log and in `research/` (the macOS-persistence research, the
Hermes `config.yaml` tracking note); this spec folds their *conclusions* inline so an agent needs only this
spec + the master plan.
**Provenance:** two research passes, an external build-guidance file, and **two ultracode multi-agent
reviews** (the second, post-Fable-5, produced 12 confirmed findings that reshaped the launchd detector and
added the reliability + coverage items below; runs in the decision log).

---

## North star

**One calm, trustworthy Discord channel.** The failure loop: noise → ADHD overwhelm → ignored alerts →
quiet anxiety. The channel must stay rare enough that **silence is trustworthy** and Stephen can stop
watching it. Precision ≫ recall: a *recurring* false positive trains the ignore-response, which then spreads
to the real alerts. A page fires only on a **proven, high-confidence, actionable** threat; everything else
is log-only on disk.

## Scope — single host (Dresden)

This implementation targets **only Dresden** (the macOS laptop, a server ~97% of the time, accessed
remotely via agents — Claude Code mobile, Paseo, Hermes), provisioned via this chezmoi/dotfiles repo.
**Multi-host is explicitly NOT built here:** when Hermes moves to the homelab NUC, this implementation
migrates *out* of dotfiles into homelab automation (Ansible/K8s), where multi-host fan-in (per-host alert
URLs, Hermes bound off `127.0.0.1`, a firewall ACL) is built. The detection design here is the durable part;
deployment relocates later. Alerts still carry a `host` tag (payload hygiene + future-proofing).

## Architecture — two tiers, one gate

- **PAGE tier** — a small fixed set of high-confidence detectors. Posts to Discord immediately (no batching
  beyond one message per alerter run — a fired alert must leave the box before any tampering).
- **LOG-ONLY tier** — everything else osquery collects. Stays in `results.log`, queryable; **never** pushed.
- **One gate.** A single default-deny `case` in the alerter's enrich loop decides page-vs-drop; a healthy
  host returns **zero** page rows. Nothing in the noisy/ambiguous middle is ruled into submission — it is
  deferred to log-only.
- **Deterministic, no LLM on the page path.** The page decision is osquery SQL + bash only. An LLM (the
  deferred "Mouse") may never gate the page tier (an attacker can prompt-inject telemetry to make a gating
  LLM suppress its own alert); it may only ever gate the *noisy* tier, where a miss is low-stakes. This is
  **capability-independent** — a more capable model does not earn the page path.

## Delivery (Hermes contract)

- Page → POST `http://127.0.0.1:8644/webhooks/osquery-priority` (the one channel Stephen watches; the quiet
  `#osquery` route is dropped — non-page findings are never POSTed).
- Body: `{"event_type":"osquery.alert","host":"<hostname>","alert":{"title":…,"detail":…}}`. The `host`
  field is inside the signed body so the dedup key stays coherent.
- Auth: HMAC-SHA256(body) → `X-Webhook-Signature`; key from `~/.config/osquery/webhook-secret` (mode 600,
  identical to the gateway's two route secrets — verified).
- Dedup: `X-Request-ID = osquery-<sha256(body)[:32]>` (gateway honours it 1h).
- **Durable spool:** a page that exhausts retries on a transient 429/5xx is written to
  `~/.local/state/osquery-alert-spool/` and re-POSTed (idempotent ≤1h via `X-Request-ID`) on the next
  alerter run and at each watchdog tick — never silently lost on this headless box.

## Collection

Scheduled **differential** queries (state tables) plus **evented** `file_events` (FSEvents) for the
files that have no state table. The first-run "everything added" baseline is **discarded** (`counter == 0`,
numeric). The `launchd` table is disk-based (reflects plist files; ~929 rows on Dresden, low churn).

## PAGE set (the only things that ping you)

Each detector below maps to a master-plan task. For each: the **page condition** and **why it stays quiet**.

1. **New launchd persistence — existence-based on the LABEL** (differential `launchd` table; *master-plan
   Task 3*). A new label pages: `/System/Library/*` is skipped (Apple OS-update churn — provenance via the
   path); any other `…/LaunchDaemons/…` (third-party system daemon) pages **always**; user `LaunchAgents`
   page **unless the label is in the per-host label allowlist**. Quiet because the counter==0 baseline
   discards all existing labels and the agents' labels are allowlisted in one week of calibration.
   - The earlier multi-signal/writer-trust gate is **deleted as bypassable** — on this host legit agents and
     a dropper are the identical `interpreter + script` shape, so signing/location/writer cannot separate
     them. Signing/quarantine/writer ride as **enrichment text, never a page suppressor**. (`program` is
     empty on this host; the command is in `program_arguments`.)
2. **Security-config & pipeline-integrity files — real-time `file_events`** (*Task 4*). Page on
   CREATED/UPDATED (never DELETED — a revert's trailing delete and a legit removal stay quiet) for
   categories: `authorized_keys` (SSH key planted — catches a sub-hour add-then-revert the hourly table
   misses), `sudoers`, `sshd_config`, and **`pipeline_integrity`** (the alerter/dispatch/watchdog/heartbeat
   scripts + their LaunchAgent plists — "watch the watchers"). Quiet because these files change ~never
   except on a `chezmoi apply` the operator ran.
3. **New admin / new user account** (*Task 5*) — `groups.groupname='admin'`; baseline root + stephen.
4. **New setuid-root binary** (`suid_bin_unexpected`, *gate in Task 1*).
5. **New kernel / system extension** (*Task 6*) — keyed on a **new identifier**, Apple-filtered; sysext
   filtered to `state='activated_enabled'` to dodge upgrade `terminated_waiting` churn. A new third-party,
   boot-persistent extension is rare here (1 non-Apple kext / 6 stable sysexts live).
6. **Agent attack surface** (Stephen's primary access path → highest stakes) (*Task 7*): **auth-file FIM**
   (hash-diff on the stable credential set; alert says "X changed", never the contents);
   **binary integrity** (hash the **resolved native binary** — the codex vendored aarch64 binary, paseo,
   claude-restart.sh — not launcher symlinks); **network exposure** (a known agent port `8644`/`8181`
   binding off-loopback, or a different process there).
7. **A protection turned OFF** — **firewall + Gatekeeper owned by the 60s poller**; **SIP / FileVault /
   screen-lock / sharing owned by the security-policy-regression pack** (page only on the OFF transition).
   Split so no protection double-pages.

## LOG-ONLY (collected, queryable, never paged)

Installed apps, Homebrew packages, browser extensions, generic listening ports, recent logins/sessions,
launchd overrides, cron/startup items, `es_launchd_writes` (forensic "written by"), and all other
`file_events` (the launch-dir watches — launchd existence already covers persistence).

## Calibration (what makes it quiet)

1. Validate each query with `osqueryi --json` on Dresden.
2. **Discard the first differential run** (`counter == 0` baseline).
3. Label one week: each fired row → *real* or *mine*; "mine" → allowlist.
4. **One-action allowlist** — reply **`allow <label>`** in the priority channel (a Hermes plugin appends the
   launchd label). Never edit-config-and-redeploy.
5. Calibrated = a week of only-real-or-nothing.

## Heartbeat

Once daily at 09:00 local → "✅ pipeline healthy." Makes silence mean **safe**; its absence is the alarm
(Dresden is headless — no local popup reaches you). **`RunAtLoad=true`** so a boot/reload after 09:00 still
emits that day's beat (an extra ✅ on reboot is fine; a missing ✅ is not). The ✅ is an operator-facing
affirmation — automated liveness is the 15-min watchdog's job; the heartbeat does **not** page on its own
absence (no central no-beat detector — that would page a roaming host overnight; it belongs to the deferred
homelab layer).

## Reliability — the watchdog

The existing 15-min uptime-watchdog asserts osqueryd is alive and the LaunchAgents are loaded — now
extended to also assert the **heartbeat** agent is loaded, **drain the delivery spool**, and page if a
spooled alert is older than one tick. The deterministic alert always fires first and independently.

## Allowlist UX

A Hermes **`pre_gateway_dispatch` plugin** (supported extension surface — no core patch) matches an
`allow <label>` reply, **fail-closed on owner AND channel** (only Stephen, only the security channel), and
appends the label to `~/.config/osquery/page-launchd-allowlist.txt`. Emoji-reaction UX stays deferred until
Hermes exposes reaction callbacks.

## Secrets & chezmoi

Hermes's own convention: **secrets in `~/.hermes/.env`** (chezmoi + KeePassXC), non-secret settings in
`config.yaml` referencing them via `${VAR}`. `config.yaml` is **runtime-mutated** (bug #4775 can resolve
`${VAR}`→plaintext on rewrite), so it is tracked as a chezmoi **`create_` template** — written once on a
fresh host, never re-synced. The osquery webhook secret is a KeePassXC-templated file. Value-based leak gate
before commit; never `chezmoi add` the live `config.yaml` again. (Full reasoning: the Hermes config research
note.)

## Locked decisions (do not re-litigate)

- **launchd = existence-on-label**, not multi-signal/writer-trust (bypassable). System daemons always page;
  `/System` is Apple churn; user agents use the label allowlist.
- **SSH/sudoers/sshd_config = real-time `file_events`**, not the polled table (the table misses sub-hour
  transients). The `authorized_keys` table is enrichment only (never the key/sha256).
- **Deterministic page path; no LLM gating** — capability-independent.
- **Single host (Dresden)**; multi-host is the homelab migration out of this repo.
- **Agent binary integrity = the resolved native binary**, not launcher symlinks; Hermes's editable source
  tree is un-attestable and is a stated gap (pin a wheel later if wanted).
- **config.yaml = chezmoi `create_`**; secrets in `.env` via `${VAR}` (Hermes convention).
- **Mouse (LLM second opinion) deferred**; if ever built, noisy-tier only.

## Deferred (not now)

- **The LLM judge ("Mouse")** — noisy-tier only; never the page path.
- **Wazuh** — revisit for fleet-scale management after the page tier proves calm.
- **Homelab migration → multi-host** — per-host URLs, Hermes off-loopback, firewall ACL, built in homelab
  automation, not this repo.
- **Cross-host machine-death** — a host can't detect its own death locally; needs an off-host consumer that
  pages on an overdue heartbeat. Folds into the homelab/Wazuh layer.
- **Residuals consciously accepted** (→ the future off-host layer): (a) an attacker who modifies the
  alerter/dispatch script *before* osquery's next event fires can blind the pipeline (the irreducible
  "who watches the watchers" limit); (b) on this **SIP-off** host, a root attacker writing to
  `/System/Library/LaunchDaemons` is skipped by the OS-churn filter; (c) Hermes's editable install can't be
  hash-attested. Beaconing / new-listening-port detection (normal dev-box churn).

## Phase 0 — confirmed facts

- **Hermes contract** as in Delivery above; the two route secrets equal the osquery-side webhook-secret
  (sha256 `d6312715…`), so no reconciliation is needed.
- **Agent surface:** hermes `:8644`, qmd `:8181` (the only local listeners); paseo (outbound, no listener);
  claude (no daemon); codex (interactive CLI). Auth/secret FIM set: `~/.config/osquery/webhook-secret`,
  `~/.paseo/daemon-keypair.json`, `~/.paseo/cli-client-id`, `~/.hermes/.env`, `~/.codex/config.toml`
  (**not** `~/.hermes/config.yaml` — runtime-mutated; **not** `~/.codex/auth.json` — OAuth rotation).
- **Verified schema (osqueryi on Dresden, 2026-06-10):** `launchd(label, path, program, program_arguments)`
  — `program` empty, `path` populated for all 929 rows; `kernel_extensions(name, version)`;
  `system_extensions(identifier, team, state, category)`; `es_process_file_events` has **no** `label`/
  `program` column (so it can't supply launchd identity — confirming the differential `launchd` table is the
  page source). `groupname='admin'` baseline = root + stephen.
