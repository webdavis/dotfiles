# osquery Alerting — Master Design Spec (v2)

**Date:** 2026-06-10 · **Status:** the single source of design truth for the osquery security-alerting
work. **Supersedes the *tiering* of** `specs/2026-06-10-osquery-alerting-master-spec-v1.md` (two-tier
page/log-only). v1 remains intact as history; v2 adds the middle **digest** tier and re-triages every
detector. **Implemented by** `plans/2026-06-10-osquery-alerting-master-plan-v2.md`. **Pairs with** the
tier matrix (`…-tier-matrix-v2.md`), the test matrix (`…-test-matrix-v2.md`), and the decision addendum
(`decisions/2026-06-10-osquery-alerting-v2-decision-addendum.md`). **Grounded in** read-only frequency
analysis of the live `results.log` and a multi-agent critique+design pass.

______________________________________________________________________

## 1. North star — the calm channel is a security requirement

Stephen has **ADHD** and watches **one Discord channel** for everything in his life. The product
requirement *is* the security requirement: that channel must stay **rare, trustworthy, and actionable**.
A recurring false positive is not a cosmetic annoyance — it *trains the ignore-response*, and that
response then spreads to the real alerts, defeating the entire system. So:

- **Silence must mean "nothing needs Stephen right now"** — not "the system is broken," and not "Stephen
  learned to tune it out."
- **Precision ≫ recall** on the page tier. We do not buy coverage with noise.
- **We never solve security by making Stephen babysit noise.** Ambiguous-but-useful signals get a
  *digest*, not an interruption; noisy/forensic signals stay in the logs.

This north star is why v2 exists: v1's two-tier model forced every useful-but-ambiguous signal into a
binary (page or silence), so several detectors that are *useful* but *not rare* were paging on normal
developer activity — the exact failure mode above.

## 2. Scope — dotfiles-now vs homelab-later

**This build is Dresden-only** (one macOS host), provisioned via this chezmoi/dotfiles repo. Multi-host /
fleet is **explicitly out of scope** and is a future migration *out* of dotfiles into homelab automation
(Ansible / Kubernetes / Wazuh). v2 builds **migration seams**, not fleet features.

| Area                    | `dotfiles-now` (Dresden, this repo)                  | `homelab-later` (out of this repo)                     |
| ----------------------- | ---------------------------------------------------- | ------------------------------------------------------ |
| osquery config          | local packs + flags, chezmoi-templated               | central policy distribution (Fleet/Wazuh)              |
| Alert dispatch          | local bash `send_alert` → localhost Hermes           | per-host routing / a central collector                 |
| Digest builder          | local LaunchAgent, local store                       | optional central roll-up of per-host digests           |
| Allowlist               | local chezmoi-tracked file (+ optional Hermes reply) | central allowlist policy / inventory                   |
| Delivery                | **localhost-only** `127.0.0.1:8644` + HMAC           | Hermes bound off-loopback, per-host URLs, firewall ACL |
| Host inventory / fan-in | n/a (single host)                                    | host inventory + multi-host alert routing              |
| Storage / search        | local `results.log`                                  | centralized storage/search (ELK/Wazuh)                 |
| Liveness                | local heartbeat + 15-min watchdog                    | off-host consumer pages on an overdue heartbeat        |

**Migration seams** (so homelab is not blocked): the POST URL, the `host` tag in the body, and the
allowlist file format are all already parameterized/present, so the later layer changes *configuration
and routing*, not the detection design.

## 3. Architecture — three tiers, one gate

A single default-deny `case` in the alerter's enrich loop routes each finding to exactly one destination:

- **`page/core`** — an immediate Discord notification. Reserved for events that are **immediate AND
  high-confidence AND actionable AND rare**. Interruptive (a sound). A healthy host returns **zero** page
  rows.
- **`digest/suspicious`** — the finding is appended to an on-disk store; a **once-daily (configurable)**
  job groups, dedups, and renders **one** concise summary message. **An empty digest sends nothing.**
  Non-interruptive.
- **`log-only/noisy`** — retained in `results.log` for querying; **never delivered**.

The gate has three outcomes per finding: set `severity=CRIT` and fall through to dispatch (page); call
`_digest_append` then `continue` (digest); or `continue` (log-only). **Separation is structural, not
conventional** (§7): a page row never reaches the digest store, and a log-only row never does either,
because the digest store has exactly one append call site (the explicit `digest)` arms).

**Deterministic, no LLM on the page or digest decision.** Both delivery decisions are osquery SQL + bash.
The deferred "Mouse" LLM may never gate delivery (prompt-injection-to-suppress); capability-independent.
(Unchanged from v1.)

## 4. Delivery & webhook security

- Page/digest/heartbeat all POST `http://127.0.0.1:8644/webhooks/osquery-priority` (the one watched
  channel). **localhost-only in this phase** (off-loopback bind is the homelab layer).
- Body (the signed bytes):
  `{"event_type":"osquery.alert","host":"<hostname>","alert":{"title":…,"detail":…}}`, built with
  `jq -cn` so HMAC and the `X-Request-ID = osquery-<sha256(body)[:32]>` dedup are over identical bytes.
- Auth: HMAC-SHA256(body) → `X-Webhook-Signature`; key from `~/.config/osquery/webhook-secret` (600).
- **Durable spool:** a page that exhausts retries on a transient 429/5xx is spooled and re-POSTed (same
  body/signature/request-id → idempotent ≤1h) by the next alerter run and each watchdog tick. The spool
  drain runs under `set -euo pipefail` and **must be guarded so a malformed/empty spool cannot abort the
  alerter** (a delivery feature must never cause a detection outage).
- **Required security tests** (test matrix §Delivery): unsigned rejected, bad-HMAC rejected, duplicate
  request-id deduped, spool replay idempotent, **secrets never written to any log/spool/payload line**,
  localhost-only target, transient-failure → spool-not-loss, drain-cannot-abort-the-alerter.
  Dispatcher-side behaviors are bats; gateway-rejection behaviors are Dresden-only integration (Hermes is
  third-party — not modified).
- **Routing wart (documented):** v1 deletes the quiet `#osquery` route, and `send_alert` routes only
  `CRIT` to the priority channel. So the **digest and heartbeat both use `CRIT` to reach the channel but
  pass an empty `sound`** → silent/non-interruptive. CRIT here is a *channel selector*, not an interrupt;
  the page tier's interruptiveness comes from the `Sosumi` sound, which digest/heartbeat omit.
- **Residuals (accepted):** the spool stores the alert body base64 (not encryption) at mode 600 until
  drained — acceptable (the body carries no secret by the redaction rule); a gateway restart inside the
  dedup window could double-post a spooled page (rare, small trust cost). Both documented.

## 5. The digest tier (new in v2)

- **Store:** `~/.local/state/osquery-digest-spool/digest.ndjson` (dir 700 / file 600), env-overridable
  via `OSQUERY_DIGEST_STORE`. One append-only NDJSON line per suspicious finding, written by a
  best-effort `_digest_append` (never fails the alerter):
  `{"ts":…,"detector":"<q>","category":…,"id":"<best-identifier>","action":…,"summary":"<one human line>"}`.
- **Builder:** a new LaunchAgent script `executable_osquery-digest.sh` (mirrors the heartbeat shape):
  **atomic rotate** the store aside (closes the truncation race), **empty-suppress** (no entries → exit
  0, nothing sent), **group by detector** and render a concise, capped, roll-up'd summary (**not a raw
  telemetry dump**), send **one** `send_alert CRIT "$title" "$body" ""` (silent), then clear (keeping
  `digest.ndjson.last` for recovery). Title `🗒️ osquery daily digest · <date> · <N> item(s)`.
- **Cadence:** daily by default **at 18:00** (evening review), set in the plist `StartCalendarInterval`,
  **parameterized through chezmoi** (`[data.osquery].digestHour/digestMinute`, default **18:00**).
  Evening, not morning: the digest is a *review/triage* artifact, decoupled from the morning 09:00 ✅
  heartbeat — "system-alive AM / footnotes PM." `RunAtLoad = false` (a digest at every reload would drain
  off-cadence).
- **Empty suppression:** enforced twice (before work; after the rotate). An empty day yields **only** the
  heartbeat ✅ — exactly "silence = safe."
- **Separation from the heartbeat:** different script, different agent (`com.webdavis.osquery-digest`),
  different schedule, no shared state. The heartbeat is a fixed daily affirmation; the digest is content.
  The watchdog guards the digest agent's liveness too.

## 6. The page/core set (the only things that interrupt you)

Per the tier matrix. **Nine detectors + the poller:** `new_admin_user`; `suid_bin_unexpected`;
`agent_exposure_changed`; `file_events` **authorized_keys** and **sshd_config** (CREATED/UPDATED only);
`persistence_launchd` **system LaunchDaemons** (`/Library/LaunchDaemons/…`, not `/System`, action=added,
**not allowlistable**); `filevault_state` **OFF transition**; the **webhook-secret + paseo
`daemon-keypair.json`** file changes (split out of the credential set — the pipeline's own HMAC key + the
paseo daemon's auth); **`pipeline_integrity` on content-mismatch** (a watched alerter file whose `sha256`
≠ the source-derived, root-owned baseline manifest — a legit `chezmoi apply` matches and stays silent;
§6a); and `firewall_state`/`gatekeeper_state` **OFF** via the existing **60s poller** (the pack rows
themselves stay log-only to avoid double-paging).

Every page detector requires **both** a positive test (it pages on the real event) and a negative test
(it does *not* page on the known-good/noisy case). v1 shipped only negatives for the protection-off pack
— a critical gap (the one behavior justifying the pack was unverified). **Two page detectors are gated on
a "confirm it emits rows on Dresden" step** before they are trusted (a no-op page is false assurance):
`screenlock_state` (0 rows ever) and `remote_access_sharing_state` (dead code — must be rebuilt as an
ON-transition detector; until then it is log-only).

## 7. The digest/suspicious set

Per the tier matrix: `persistence_launchd` **user LaunchAgents** (new label ∉ allowlist — *revises* v1,
which paged these; the system-daemon half still pages); `system_extensions_new`; `agent_binary_changed`;
`agent_authfile_changed` (minus the webhook-secret **and** the paseo keypair — i.e. `.env`,
`codex/config.toml`, `cli-client-id`); `file_events` **sudoers** (19 real events); `screenlock_state`
OFF. (`pipeline_integrity` moved to page-on-content-mismatch — §6 + §6a.)

### 6a. pipeline_integrity baseline (layers 1 + 2 ship in round 1)

`pipeline_integrity` pages only when a watched alerter file's content is **not** what the deployer
legitimately produced — judged by **content, not timing**, so a `chezmoi apply` is silent and a same-day
tamper is not masked by memory. Three layers (1 + 2 now; 3 deferred — D-V2-13):

- **Layer 1 — source-derived baseline:** the manifest is the hash of the **source artifact** (reviewed
  git state), not the live file, so tampering the deployed file can never get blessed by a re-baseline.
- **Layer 2 — root-owned manifest + root re-baseline:** watched scripts are user-owned; the manifest is
  **root-owned** and re-baselined by a root context (osqueryd already runs as root), so a user-level
  compromise can't rewrite it → still pages, forcing the attacker up to root (which trips other page
  detectors).
- **Layer 3 — off-host (deferred):** a root attacker beats all local controls; the guaranteed close is
  the off-host heartbeat-absence (Homelab layer). The manifest is produced by a **deployer-agnostic
  baseline script** (chezmoi `run_after` now → Homelab post-deploy later — detector unchanged; the
  manifest path is the migration seam).

## 8. The log-only/noisy set

`kernel_extensions_new` (**657 real load/unload events** — load-state is a firehose and the wrong signal;
redesign to install-state to ever deliver), `sip_state` (SIP intentionally off → no transition; plus a
stale-name split to reconcile), `remote_access_sharing_state` (dead until rebuilt), `es_launchd_writes`
(forensic enrichment), `listening_ports_non_loopback`, `installed_apps`, `homebrew_packages`,
`recent_logins`, `persistence_startup_items_crontab`, `persistence_launchd_overrides`, and the
`firewall_state`/`gatekeeper_state` **pack rows** (the poller owns the page).

## 9. Allowlist — fallback-first, two interchangeable writers of one file

The allowlist holds known-good launchd **labels** so they stop notifying. v2 does **not** assume the
Hermes plugin is correct. **Ship both, fallback-first:**

- **NOW (the durable floor): a chezmoi-managed, git-tracked plain `.txt` file**
  (`dot_config/osquery/page-launchd-allowlist.txt`). Works headless today (plain file, no KeePassXC/TTY
  prompt), maximally auditable (git), reversible (revert the line), zero unverified surface. This is the
  interim allowlisting path the **calibration week depends on** — it must not depend on the convenience
  layer (skill or buttons).
- **LATER (the low-friction daily driver, PR #2): tap-to-approve Approve/Deny buttons under Stephen's
  spare Discord bot** (its own token — a component tap is delivered only to the application that posted
  it, so it cannot ride Hermes/Bob; D-V2-15). The alerter drops a pending-request file and
  `launchctl kickstart`s a small discord.py listener; `KeepAlive.PathState` keeps it alive exactly while
  a request is pending (crash self-heals; persistent views — `timeout=None`, stable `custom_id`,
  `bot.add_view()` on boot, immediate defer — survive restarts), and on resolution it disables the
  buttons and exits: **zero extra processes in steady state**, no LLM in the button path. Owner+channel
  auth on the tap, fail-closed. The typed path in the same PR is an **`osquery` Hermes skill** —
  `/osquery allow|deny|list <label>`, a drop-in `~/.hermes/skills/osquery/SKILL.md` that **rides Bob**
  (slash→skill dispatch verified 2026-06-13, `agent/skill_commands.py:254`; D-V2-15), no fork and no
  second daemon. The skill is **agent-mediated** (the agent runs the shared writer), so the buttons stay
  the LLM-free primary and the writer stays the fail-closed boundary; it supersedes the
  `pre_gateway_dispatch` typed-reply plugin (D-V2-12). All writers honor the **identical file under the
  identical contract**, so the floor never stops working. ("Local CLI" and "webhook route" remain
  dominated — an on-box shell for a phone-first owner / net-new surface on a security-control file.)

**Shared contract (stable regardless of UX):** one file, UTF-8, **one exact reverse-DNS label per line**,
`#` comments, blank lines ok. **No wildcards/globs/regex/ranges — ever.** Writers validate fail-closed
with `^[A-Za-z0-9][A-Za-z0-9._@-]+$` (**the `@` is required** — `homebrew.mxcl.postgresql@17` is a live
label v1's regex would reject), dedup before append, and the reader matches with `grep -qxF` (exact,
full-line). **System LaunchDaemons are never allowlistable.** The file is itself watched by
`pipeline_integrity` (editing a page-suppressor must be surfaced), and each write carries an audit
comment (who/when).

**Required allowlist UX additions (v1 gaps):** an in-channel/headless **`deny <label>` / `list`** path so
a fat-fingered `allow` is reversible without SSH; and **confirmation feedback** so a typo that writes a
dead entry (label that matches no paging job) is visible, not silently believed-silenced.

**Why the typed path is a `/osquery` skill, not free-text matching.** An earlier design scraped typed
messages for `allow`/`deny`/`list`, which risked a normal prompt containing "list" being eaten. The fix
is to use Hermes's **skill-command** surface: `/osquery allow|deny|list <label>` is a **registered slash
command** (dispatched by `agent/skill_commands.py`, shared by CLI + gateway; the trailing text reaches
the agent as the skill instruction, line 254 — verified 2026-06-13), so it can never collide with prose —
`list my PRs` is just an ordinary message to Bob. This is a drop-in `~/.hermes/skills/osquery/SKILL.md`
(the supported primitive — no fork), it **rides Bob** (no second daemon, no `@Butters`), and it
**retires** both the `pre_gateway_dispatch` plugin and the `osq`-prefix scheme. The skill is
**agent-mediated** (the agent runs the shared fail-closed writer with the parsed label), so the writer —
not the agent — is the security boundary, and the **tap buttons stay the LLM-free primary** (a tap is an
interaction Butters owns, no agent). The SKILL.md `description` scopes it to the owner + security
channel.

**Two v1 defects this fixes:** the regex omitting `@`; and a **path mismatch** — the live alerter reads
`launch-allowlist.txt` while the plan/plugin write `page-launchd-allowlist.txt`, so every allow is
currently a silent no-op. v2 standardizes on `page-launchd-allowlist.txt` and updates the alerter's
env/path in the same change.

## 10. Calibration (per host)

Validate each query with `osqueryi --json`; discard the `counter==0` baseline; label one week (real vs
mine → allowlist via the manual file path, no plugin dependency); calibrated = a week of
only-real-or-nothing on the page tier. During this week, new user-LaunchAgent labels flow to the
**digest**, not pages — a far calmer calibration experience.

## 11. Heartbeat & reliability (unchanged, kept separate)

A daily ✅ at 09:00 (`RunAtLoad=true`) means "pipeline healthy"; its absence is the alarm. **It is not the
digest** (different agent/schedule; the digest is content and is empty-suppressed; the heartbeat is a
fixed affirmation). The 15-min watchdog asserts osqueryd + every LaunchAgent (alerter, watchdog,
heartbeat, **and the new digest agent**) is loaded, and drains the delivery spool.

## 12. Locked decisions (do not re-litigate)

- **Three tiers** (page/core, digest/suspicious, log-only/noisy) replace two-tier page/log-only. The calm
  channel is the north star and the page tier is conservative by default.
- **Demotions** out of page: kernel_extensions_new (load-state firehose → log-only), sip/remote-sharing
  (dead → log-only), and user-LaunchAgents / sysext / agent-binary /
  agent-authfile(−secret,−paseo-keypair) / sudoers / screenlock → digest. (Tier matrix + decision
  addendum hold the per-item rationale.)
- **Promotions into page:** the **webhook-secret + paseo `daemon-keypair.json`** split (pipeline HMAC key
  \+ paseo daemon auth); and **`pipeline_integrity` on content-mismatch** vs a source-derived, root-owned
  baseline manifest (layers 1+2 in round 1 — D-V2-13).
- **launchd = existence-on-label**, signing/writer as enrichment text only (carried from v1; never a
  suppressor).
- **Allowlist:** fallback-first (manual git file in PR #1; **PR #2 = tap-to-approve buttons** via
  Stephen's spare Discord bot, a pending-scoped daemon with zero steady-state processes, plus the
  `/osquery` Hermes skill as in-PR fallback — D-V2-15); exact-label, no wildcards, `@`-aware regex,
  owner+channel scoped, reversible (`deny`/`list`), watched + audited.
- **Delivery:** localhost-only now; HMAC + dedup + idempotent spool; secrets never logged; the drain
  cannot abort detection.
- **Single host (Dresden)**; multi-host is the homelab migration out of this repo.

## 13. Deferred & residuals

Mouse (LLM second opinion — noisy-tier only, never delivery; clean deferred build = an agent-driven cron
job under a dedicated `--profile mouse` that reads the deterministic digest read-only and posts a
separate, monotonic advisory — never the digest runner; ships as **PR #3** after the core + the PR #2
approval UX; D-V2-12/D-V2-14); Wazuh (fleet management); homelab multi-host migration; cross-host
machine-death (off-host consumer on an overdue heartbeat); the install-state kext redesign. **Accepted
residuals:** the who-watches-the-watchers circularity (an attacker neutering the alerter before osquery's
next event); a root attacker writing to `/System` on this SIP-off host; Hermes's un-attestable editable
tree; an at-rest spool body (base64, not encryption); allowlist auth == Discord account auth.

## 14. Open questions (with proposed answers)

1. **pipeline_integrity tier — RESOLVED → page on content-mismatch** vs a source-derived, root-owned
   baseline manifest (layers 1+2 ship in round 1; D-V2-13). Not the timing/sentinel approach
   (memory-like, maskable).
1. **screenlock / remote-sharing** — *Proposed:* confirm `screenlock_state` emits rows under `osqueryi`
   (else it is a no-op page — keep at digest); rebuild `remote_access_sharing_state` as an ON-transition
   detector before it can earn page/core.
1. **kext** — *Proposed:* log-only now; a future install-state detector (differential over
   `/Library/Extensions`
   - the staged kext DB) could be page/core. Not in v2.
1. **agent_authfile split — RESOLVED:** page on webhook-secret **and** paseo `daemon-keypair.json`;
   digest the rest (`.env`, `codex/config.toml`, `cli-client-id`).

## 15. Phase-0 confirmed facts (read-only, 2026-06-10)

- **Frequency (live `results.log`, 5,251 rows):** persistence_launchd 928 baseline / **16 real** (12 user
  \+ 4 system, 0 `/System`); kernel_extensions_new 252 baseline / **657 real** load/unload;
  system_extensions_new **9 real**; file_events sudoers **19 real** vs sshd_config **1** vs
  authorized_keys **0**; protections 1–4 rows; screenlock **0 rows ever**; remote-sharing **1
  baseline-only**; a stale `pack_security-regression_sip_state` alongside
  `pack_security-policy-regression_sip_state`.
- **Real row schema:** the 9-key envelope
  (`action, calendarTime, columns, counter, epoch, hostIdentifier, name, numerics, unixTime`); launchd
  `program` empty (command in `program_arguments`); file_events real verb in `columns.action`;
  `es_process_file_events` has no `label`. (Used for the fixture-realism contract.)
- **Allowlist path mismatch + `@`-regex defect** confirmed against the live alerter and live launchd
  labels.
- **Allowlist UX:** the Hermes reply-plugin API was **verified** (D-V2-12) but **superseded** by tap
  Approve/Deny buttons + the `/osquery` skill (D-V2-15); the manual git file is the floor. The Discord UX
  is human-approval-gated before building.
