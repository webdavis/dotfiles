# osquery Alerting — Detector Tier Matrix (v2)

**Date:** 2026-06-10 · **Status:** the authoritative per-detector classification for v2's three-tier
model. **Pairs with:** spec `2026-06-10-osquery-alerting-master-spec-v2.md`, plan
`2026-06-10-osquery-alerting-master-plan-v2.md`, decision addendum
`2026-06-10-osquery-alerting-v2-decision-addendum.md`, test matrix
`2026-06-10-osquery-alerting-test-matrix-v2.md`.

**The three tiers** (see spec §Architecture): **`page/core`** = immediate Discord notification, reserved
for immediate + high-confidence + actionable + **rare**; **`digest/suspicious`** = a once-daily grouped
summary, empty-suppressed; **`log-only/noisy`** = retained in `results.log`, never delivered.

**Evidence base.** Every frequency figure below was reproduced read-only against the live
`~/.local/log/osquery/osqueryd.results.log` (5,251 rows, multi-week history) — names/counts only, no
payloads. The decisive split is **baseline vs steady-state**: osquery's `counter==0` first-run rows are
discarded, so a detector's real load is its `counter>0` events.

## Classification table

| #   | Detector (query / category)                                                                                                             | v1 tier        | **v2 tier**                         | Act. | Conf. | Real events (history)                 | Primary false-positive source                                                            | Calibration / allowlist                                                                                   | Tests (see test matrix)                        |
| --- | --------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ----------------------------------- | ---- | ----- | ------------------------------------- | ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| 1   | `new_admin_user`                                                                                                                        | page           | **page/core**                       | high | high  | 0 (clean state table)                 | a legit admin you created                                                                | counter==0 baseline discard                                                                               | T-PAGE-admin                                   |
| 2   | `suid_bin_unexpected`                                                                                                                   | page           | **page/core**                       | high | high  | 0                                     | an installer dropping a suid helper (rare)                                               | baseline discard; pack `*_off` floor                                                                      | T-PAGE-suid                                    |
| 3   | `agent_exposure_changed` (8644/8181 off-loopback)                                                                                       | page           | **page/core**                       | high | high  | 0                                     | you deliberately exposing a port (you'd know)                                            | none — binary loopback test                                                                               | T-PAGE-exposure                                |
| 4   | `file_events` category=`authorized_keys` (CREATED/UPDATED)                                                                              | page           | **page/core**                       | high | high  | 0                                     | adding your own key (rare, expected)                                                     | none                                                                                                      | T-PAGE-authkeys, T-NEG-authkeys-delete         |
| 5   | `file_events` category=`sshd_config` (CREATED/UPDATED)                                                                                  | page           | **page/core**                       | high | high  | 1                                     | a macOS point-update rewriting it (rare)                                                 | none                                                                                                      | T-PAGE-sshd                                    |
| 6   | `persistence_launchd` — **system** LaunchDaemons (`/Library/LaunchDaemons/…`, not `/System`, action=added)                              | page           | **page/core**                       | high | high  | 4 added                               | a third-party installer (Docker) — rare, worth one page                                  | **not allowlistable** (system daemons always page)                                                        | T-PAGE-launchd-sysdaemon                       |
| 7   | `filevault_state` — OFF transition                                                                                                      | page           | **page/core**                       | high | high  | 4 rows                                | you disabling FileVault (you'd know)                                                     | baseline; positive OFF test required                                                                      | T-PAGE-filevault-off                           |
| 8   | **webhook-secret + paseo `daemon-keypair.json` FIM** (split from `agent_authfile_changed`)                                              | page (bundled) | **page/core**                       | high | high  | n/a                                   | rotating the HMAC key / the daemon keypair (rare, you'd know)                            | none — the pipeline's own HMAC key + the paseo daemon's auth (your primary remote-access path)            | T-PAGE-webhooksecret, T-PAGE-paseokey          |
| 9   | `firewall_state` / `gatekeeper_state` — OFF                                                                                             | page (poller)  | **page/core (60s poller)**          | high | high  | 1 each                                | you toggling them                                                                        | poller owns the page; **pack rows are log-only** (row 18)                                                 | T-PAGE-firewall-off (poller)                   |
| 10  | `persistence_launchd` — **user** LaunchAgents (new label ∉ allowlist)                                                                   | page           | **digest/suspicious**               | med  | med   | 12 (8 labels)                         | installing any tool that adds a LaunchAgent                                              | **label allowlist** (`allow <label>`); near-empty post-calibration                                        | T-DIG-launchd-user, T-SEP-page-not-in-digest   |
| 11  | `system_extensions_new` (`state=activated_enabled`, non-Apple)                                                                          | page           | **digest/suspicious**               | med  | med   | 9 (Tailscale activate/terminate skew) | app upgrade tearing down + re-activating a sysext                                        | digest dedups on identifier-seen-before                                                                   | T-DIG-sysext                                   |
| 12  | `agent_binary_changed` (resolved native binaries)                                                                                       | page           | **digest/suspicious**               | low  | low   | n/a                                   | `brew upgrade` / `npm` / agent self-update                                               | digest groups; investigate if you didn't update                                                           | T-DIG-agentbin                                 |
| 13  | `agent_authfile_changed` (minus webhook-secret + paseo keypair)                                                                         | page (bundled) | **digest/suspicious**               | low  | med   | n/a                                   | **planned secret rotation**, `.env`/`config.toml`/`cli-client-id` edits                  | digest line "credential changed: `<file>`"                                                                | T-DIG-authfile                                 |
| 14  | `file_events` category=`sudoers` (CREATED/UPDATED)                                                                                      | page           | **digest/suspicious**               | med  | med   | **19 (12 C + 7 D)**                   | `visudo` / chezmoi atomic-write churn (an order of magnitude noisier than sshd_config)   | digest                                                                                                    | T-DIG-sudoers                                  |
| 15  | `file_events` category=`pipeline_integrity` (alerter's own scripts/plists) — **page only on content-mismatch vs the baseline manifest** | page           | **page/core**                       | high | high  | n/a (new in v2)                       | a legit `chezmoi apply` (content **matches** the source-derived manifest → silent)       | not allowlistable; legitimacy = `sha256` matches the **root-owned, source-derived** manifest              | T-PAGE-pipeline-mismatch, T-NEG-pipeline-match |
| 16  | `screenlock_state` — OFF transition                                                                                                     | page           | **digest/suspicious**               | med  | low   | **0 rows ever**                       | n/a                                                                                      | **ACTION ITEM: confirm the query emits on Dresden, else it is a no-op**                                   | T-DIG-screenlock (+ verify)                    |
| 17  | `kernel_extensions_new` (LOADED-kext table, non-Apple)                                                                                  | page           | **log-only/noisy**                  | low  | low   | **657 (328 add + 329 rm)**            | **kexts load/unload on demand — load-state is a firehose; wrong signal**                 | none — too noisy even for digest; **redesign to install/on-disk state** to ever deliver (§open-questions) | T-LOG-kext-no-deliver                          |
| 18  | `sip_state`                                                                                                                             | page           | **log-only/noisy**                  | low  | high  | 1 (baseline only)                     | **SIP intentionally OFF here → an OFF transition cannot occur**; plus a stale-name split | remove from page; reconcile `security-regression` vs `security-policy-regression`                         | T-LOG-sip-no-page                              |
| 19  | `remote_access_sharing_state`                                                                                                           | page           | **log-only/noisy (dead → rebuild)** | low  | high  | 1 (baseline only)                     | **dead code: never emits a deliverable CRIT row**                                        | **ACTION ITEM: rebuild as a Remote-Login/Screen-Sharing ON-transition detector → then page/core**         | T-LOG-sharing (+ rebuild)                      |
| 20  | `es_launchd_writes`                                                                                                                     | log-only       | **log-only/noisy**                  | low  | high  | 20 (≈3 writers)                       | n/a                                                                                      | forensic "written by `<process>`" enrichment only (no `label` column)                                     | T-LOG-es                                       |
| 21  | `listening_ports_non_loopback`                                                                                                          | log-only       | **log-only/noisy**                  | low  | high  | 2,071                                 | constant dev-box port churn                                                              | unchanged                                                                                                 | —                                              |
| 22  | `installed_apps`, `homebrew_packages`, `recent_logins`, `persistence_startup_items_crontab`, `persistence_launchd_overrides`            | log-only       | **log-only/noisy**                  | low  | high  | 637 / 424 / — / 116 / 3               | normal install/login churn                                                               | unchanged                                                                                                 | —                                              |

**Net:** page/core = 9 detectors + the poller (firewall/gatekeeper); digest/suspicious = 6;
log-only/noisy = the rest. **Moved OUT of page (v1→v2):** kernel_extensions_new, sip_state,
remote_access_sharing_state → log-only; user-LaunchAgents, system_extensions_new, agent_binary_changed,
agent_authfile_changed(−secret,−paseo-keypair), sudoers, screenlock → digest. **Stayed/into page:**
admin, suid, exposure, authorized_keys, sshd_config, system-LaunchDaemons, filevault-OFF,
firewall/gatekeeper(poller), the webhook-secret **+ paseo-keypair** split, and **pipeline_integrity
(page-on-content-mismatch)**.

## Per-detector rationale (the "why" the table compresses)

- **Rows 1–9 (page/core).** Each is *rare* in the real history (0–4 real events),
  *binary/high-confidence* (a new admin, a setuid-root file, an off-loopback agent port, a written SSH
  key, a system daemon, FileVault off, the HMAC key changing), and *actionable* (you investigate or
  revert). These earn an interruption. Row 6 splits launchd by **path**: a third-party *system* daemon
  (`/Library/LaunchDaemons`) is rare and not user-allowlistable; `/System/Library` is Apple OS-update
  churn and is dropped (log-only).
- **Row 8 (webhook-secret split).** The single most important credential is the alerter's *own* HMAC key:
  tampering forges or mutes every alert. It is split out of the churny `agent_authfile_changed` set so it
  pages while the rotation-prone files digest. (Decision-addendum §allowlist/secret.)
- **Row 10 (user LaunchAgents → digest).** This **revises** the v1/spec-v1 stance (page user agents minus
  the allowlist). Justification: 12 real events across 8 labels — i.e. ordinary tool installs add
  LaunchAgents; the build-guidance file itself sanctions splitting user-level agents to a lower tier "if
  too chatty." During the calibration week a daily *digest* of new user-agent labels is far calmer than
  per-install pages; once the labels are allowlisted the digest is near-empty. The **system-daemon half
  (row 6) still pages always**, so persistence at the privileged tier loses no recall.
- **Rows 10–14, 16 (digest).** Each is *useful but not rare/immediate*: user-LaunchAgents (tool
  installs), sysext (app-upgrade churn), agent-binary (`brew`/`npm` updates), agent-authfile (rotation),
  sudoers (19 real events — visudo/chezmoi), screenlock. They belong in the once-daily summary, not an
  interruption.
- **Row 15 (pipeline_integrity → page on content-mismatch).** Watching the alerter's own scripts/plists
  is high-value (tamper = blind the system) but fires on every `chezmoi apply`, so paging *every* change
  would be self-inflicted noise. Resolved by judging **content, not timing**: page only when a watched
  file's `sha256` does NOT match a **source-derived, root-owned baseline manifest**; a legit apply
  produces the expected hash → silent. Layers 1 (source-derived baseline) + 2 (root-owned manifest, root
  re-baseline) ship in round 1; the residual root-attacker case is the deferred off-host
  heartbeat-absence (layer 3). See D-V2-13.
- **Row 17 (kernel_extensions_new → log-only).** 657 real add/remove events because the
  `kernel_extensions` table lists *loaded* kexts, which load/unload on demand. The Apple-name filter
  masks today's count (1 loaded non-Apple kext) but does not change that the **signal is load-state, not
  install-state** — a single third-party kext that loads on demand would re-page forever. Too noisy even
  for digest. The only safe delivery is a redesigned detector keyed on install/on-disk state
  (§open-questions in the spec/plan).
- **Rows 18–19 (sip / remote-sharing → log-only, with fixes).** `sip_state` cannot fire here (SIP is
  intentionally off, so there is no on→off transition) and carries a stale-name split that would be
  silently dropped — remove it from page and reconcile the name. `remote_access_sharing_state` is **dead
  code** in v1 (never emits a deliverable CRIT row), so it provides false assurance; it must be
  **rebuilt** as a working Remote-Login/Screen-Sharing **ON-transition** detector before it can earn
  page/core (enabling remote access *is* rare + high-value + actionable). Until rebuilt it is a no-op
  (log-only).
- **Row 16 (screenlock) and Row 19 are the two "zero-row" hazards:** a page detector that has *never*
  emitted a deliverable row implies coverage that isn't there — worse than no detector. Both are gated on
  an explicit "confirm it evaluates under `osqueryi` on Dresden" acceptance step before they are trusted.

## Notes / open questions (carried into the spec & plan)

1. **pipeline_integrity tier — RESOLVED → page on content-mismatch.** Not the timing/mtime approach
   (memory- like, maskable); instead legitimacy = the watched file's `sha256` matches a **source-derived,
   root-owned baseline manifest**. A legit `chezmoi apply` matches → silent; any other content → page.
   **Layers 1 + 2 ship in round 1** (source-derived baseline; root-owned manifest + root re-baseline).
   The manifest is produced by a **deployer-agnostic baseline script** (chezmoi `run_after` now → Homelab
   post-deploy later; detector unchanged). Layer 3 (root-attacker close) = the deferred off-host
   heartbeat-absence. (D-V2-13.)
1. **agent_authfile split — RESOLVED.** Page on `webhook-secret` **and** paseo `daemon-keypair.json`;
   digest the rest (`.env`, `codex/config.toml`, `cli-client-id`).
1. **kext redesign.** A future install-state kext detector (differential over `/Library/Extensions` + the
   staged kext database) could be page/core (installs are rare + boot-persistent). Not built in v2.
1. **Zero-row detectors.** screenlock_state and remote_access_sharing_state must be confirmed-or-dropped
   before the page tier is trusted (a no-op page is a false-assurance regression).
