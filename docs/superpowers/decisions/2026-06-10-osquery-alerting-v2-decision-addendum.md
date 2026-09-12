# osquery Alerting — v2 Decision Addendum

**Date:** 2026-06-10 · **Type:** decision log (addendum to
`2026-06-08-osquery-alerter-redesign-decisions.md`). **Scope:** records *why* v2 replaces the two-tier
page/log-only model with a three-tier escalation model and re-triages every detector. **Authoritative
artifacts:** spec `…master-spec-v2.md`, plan `…master-plan-v2.md`, tier matrix `…tier-matrix-v2.md`, test
matrix `…test-matrix-v2.md`. **Evidence:** read-only frequency analysis of the live `results.log` + a
multi-agent critique/design pass (all numbers reproduced against the live log).

______________________________________________________________________

## D-V2-1 — The calm channel is the north star (and the security requirement)

The operator has **ADHD** and watches **one** Discord channel for his whole life. A recurring false
positive *trains the ignore-response*, which then spreads to the real alerts — so noise is not a UX wart,
it is a **security failure**. Therefore: **silence must mean "nothing needs you,"** precision ≫ recall on
pages, and we never buy coverage with noise. This reframes alert design as notification-fatigue triage.
Everything below follows from it.

## D-V2-2 — Three tiers replace two tiers

**Why:** v1 was strictly `page` vs `log-only`. That binary forced every *useful-but-ambiguous* signal
into one of two bad outcomes — interrupt the calm channel, or bury it where it's never seen. The live
data proved this was already happening: detectors that are useful but **not rare** were in the page set
firing on normal developer activity. The fix is a **middle tier**: `digest/suspicious`, a once-daily
grouped summary that is **empty-suppressed** — useful signals get same-day visibility without an
interruption. New gate outcome per finding: page (fall through to dispatch) / digest (`_digest_append`
then `continue`) / log-only (`continue`).

## D-V2-3 — What moved OUT of page, and the evidence

| Detector                                   | New tier           | Decisive evidence                                                                                                                                                                                                                                               |
| ------------------------------------------ | ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `kernel_extensions_new`                    | log-only           | **657 real load/unload events**; the `kernel_extensions` table is *loaded* kexts (load on demand). Load-state is a firehose and the **wrong signal**; Apple-filtering masks the count but not the signal. Too noisy even for digest.                            |
| `sip_state`                                | log-only           | SIP is **intentionally OFF** here → no on→off transition can occur (dead page); plus a stale `pack_security-regression_sip_state` name the alerter would silently drop.                                                                                         |
| `remote_access_sharing_state`              | log-only (rebuild) | **Dead code** — never emits a deliverable CRIT row (1 baseline-only row in history). A page that never fires is *false assurance*. Must be rebuilt as an ON-transition detector to earn page.                                                                   |
| `persistence_launchd` user LaunchAgents    | digest             | **12 real events / 8 labels** — ordinary tool installs add LaunchAgents. The build-guidance file itself sanctions splitting user agents to a lower tier "if too chatty." **System daemons still page (D-V2-4)**, so privileged-persistence recall is unchanged. |
| `system_extensions_new`                    | digest             | **9 real events**, app-upgrade activate/terminate churn (Tailscale).                                                                                                                                                                                            |
| `agent_binary_changed`                     | digest             | Fires on routine `brew upgrade` / `npm` / agent self-update.                                                                                                                                                                                                    |
| `agent_authfile_changed` (−webhook-secret) | digest             | Fires on the **planned post-ship secret rotation** and `.env`/`config.toml` edits — ambiguous timing.                                                                                                                                                           |
| `file_events` sudoers                      | digest             | **19 real events** (12 CREATED + 7 DELETED) — `visudo`/chezmoi atomic-write churn, an order of magnitude noisier than sshd_config (1) or authorized_keys (0). v1 lumped all three into one page arm; the data splits them.                                      |
| `file_events` pipeline_integrity           | digest             | Fires on **every `chezmoi apply`** that touches the alerter scripts — a predictable, self-inflicted page. (Open: page-with-apply-suppression-window — §D-V2-9.)                                                                                                 |
| `screenlock_state`                         | digest             | **0 rows ever** — uncertain it even evaluates; low confidence ⇒ digest until confirmed.                                                                                                                                                                         |

## D-V2-4 — What stayed/moved INTO page, and why

**Stayed page (rare + high-confidence + actionable):** `new_admin_user`, `suid_bin_unexpected`,
`agent_exposure_changed`, `file_events` authorized_keys + sshd_config, `persistence_launchd` **system
LaunchDaemons** (path split: `/Library/LaunchDaemons` pages always and is not allowlistable; `/System` is
Apple churn → log-only), `filevault_state` OFF, and firewall/gatekeeper OFF (via the 60s poller).

**Promoted INTO page — the webhook-secret split.** `agent_authfile_changed` bundled the alerter's **own
HMAC key** with rotation-prone credential files. A change to `~/.config/osquery/webhook-secret` lets an
adversary **forge or mute every alert** — the highest-stakes single file in the system. v2 splits it out
so the secret pages while the churny credentials digest. **Updated 2026-06-12:** the paseo
`daemon-keypair.json` (the auth for the paseo daemon — Stephen's primary remote-access path) is **also
promoted to page**; the remaining credential files (`.env`, `codex/config.toml`, `cli-client-id`) digest.

## D-V2-5 — Digest cadence and empty suppression

- **Cadence: daily by default at 18:00** (evening), set in the builder LaunchAgent's
  `StartCalendarInterval`, **parameterized through chezmoi** (`[data.osquery].digestHour/digestMinute`,
  default **18:00**). Evening, not morning (updated 2026-06-12): the digest is a *review/triage* artifact
  ("investigate or not?"), so it belongs at a calm end-of-day glance — decoupled from the morning 09:00 ✅
  heartbeat, giving the rhythm "system-alive AM / footnotes PM." `RunAtLoad=false` (a reload-triggered
  digest would drain off-cadence — anti-calm).
- **Empty suppression:** enforced twice (before any work; after an atomic rotate). No store entries ⇒
  **zero** Discord traffic. A quiet day yields *only* the heartbeat ✅ → "silence = safe."
- **Not the heartbeat:** different script, agent, schedule; no shared state. The heartbeat is a fixed
  daily affirmation; the digest is empty-suppressible content.
- **Concise, not a dump:** group by detector, dedup repeated identifiers, cap body length with a
  `+K more` roll-up. A digest that becomes a telemetry firehose would just be a slower page storm.

## D-V2-6 — Allowlist UX decision and alternatives considered

**Decision: fallback-first, two interchangeable writers of one file.**

| Option                                  | Friction (phone-first owner)                            | Auditable              | Reversible      | Verdict                                     |
| --------------------------------------- | ------------------------------------------------------- | ---------------------- | --------------- | ------------------------------------------- |
| Manual chezmoi-tracked `.txt` file      | medium (SSH + edit + targeted apply)                    | **git (best)**         | **revert line** | **Adopt NOW — durable floor**               |
| Tap **Approve/Deny buttons** (Butters)  | **lowest (one tap, LLM-free)**                          | via git file it writes | yes             | **Adopt PR #2 — primary (D-V2-15)**         |
| **`/osquery` skill** (rides Bob)        | low (one slash command)                                 | via git file it writes | yes             | **Adopt PR #2 — typed fallback (D-V2-15)**  |
| ~~Hermes `allow <label>` reply plugin~~ | lowest (one reply)                                      | via git file           | yes             | **Superseded by buttons + skill (D-V2-15)** |
| Local CLI                               | medium (still needs on-box shell)                       | git file               | yes             | Dominated                                   |
| Webhook command route                   | medium + **net-new surface on a security-control file** | —                      | —               | **Rejected**                                |

The reply-plugin was later **API-verified** (D-V2-12) but **superseded** as the daily driver by tap
**Approve/Deny buttons** (primary, LLM-free) + the **`/osquery` skill** (typed fallback) — D-V2-15. The
**manual file remains the interim path the calibration week depends on** — calibration is never blocked
on an unbuilt UX. All writers (button bot, skill, manual edit) write the **identical file under the
identical contract**, so the floor never stops working.

**Contract + v1 defects fixed:** exact reverse-DNS label per line, **no wildcards ever**, fail-closed
validation `^[A-Za-z0-9][A-Za-z0-9._@-]+$` (**the `@` is required** — `homebrew.mxcl.postgresql@17` is a
live label v1's `[A-Za-z0-9._-]+` rejects), `grep -qxF` exact match, dedup, system daemons never
allowlistable, owner+channel scoped (fail-closed). **Path mismatch fixed:** the live alerter reads
`launch-allowlist.txt` while the plan writes `page-launchd-allowlist.txt` → every allow is currently a
silent no-op; v2 standardizes on `page-launchd-allowlist.txt` and updates the alerter env in the same
change. **New requirements:** a `deny`/`list` path (a fat-fingered `allow` must be reversible without
SSH); confirmation feedback (a typo that writes a dead entry must be visible); the allowlist file is
itself watched by `pipeline_integrity` and each write carries a who/when audit comment.

## D-V2-7 — Hermes plugin path decision

**Docs-only this run.** The plugin is *designed/compared*, not built. If adopted it requires **explicit
human approval** and **verification of the hook name, registration API, event shape, and reply API
against the live Hermes plugin docs** before any code. v1 wrote the plugin as concrete code with pytest
mocking the *assumed* shapes — green tests would "prove" an API that may not exist (false confidence on
the one risky component). v2 treats the plugin as the **deferred ergonomic layer** on top of the verified
manual fallback. No edits to `dot_hermes/plugins/` in v2.

## D-V2-8 — Webhook delivery security test requirements

Required (test matrix §Delivery): unsigned rejected; bad-HMAC rejected (+ correct-key sibling = 2xx);
duplicate `X-Request-ID` deduped; **spool replay idempotent** (replays the stored request-id *verbatim*,
never recomputed → collides with the original in the gateway's 1h cache); **secrets never written** to
any log/spool/payload line; **localhost-only** target (send + drain); transient 429/5xx →
**spool-then-retry, never silent loss**; and **the drain cannot abort the alerter** under `set -e` (a
delivery feature must never cause a detection outage). The double-send guarantee is split across the
boundary: the **dispatcher half** (byte-for-byte request-id replay) is bats; the **gateway half** (1h
dedup) is Dresden-only integration. Build the body with the same `jq -cn` the implementation uses —
HMAC/dedup are over literal bytes.

## D-V2-9 — Open questions (proposed answers recorded)

1. **pipeline_integrity:** digest by default; a page-with-apply-suppression-window promotion is allowed
   *only* after a chezmoi-apply provenance sentinel is proven reliable.
1. **Zero-row detectors:** confirm `screenlock_state` emits rows under `osqueryi` on Dresden (else it is
   a no-op page — keep at digest); rebuild `remote_access_sharing_state` as an ON-transition detector
   before it can earn page. A page detector that has never emitted a deliverable row is worse than none.
1. **kext redesign:** future install-state detector for page; log-only now.

## D-V2-10 — dotfiles-now vs homelab-later boundary

**Now (Dresden, this repo):** local osquery config, local `send_alert` → localhost Hermes, local digest
builder + store, local allowlist file, localhost-only delivery, local heartbeat + watchdog. **Later (out
of this repo, homelab automation):** host inventory + fan-in, Hermes off-loopback + per-host URLs +
firewall ACL, centralized storage/search, fleet policy, multi-host routing, and an off-host consumer for
cross-host machine-death. v2 builds **migration seams** (the POST URL, the `host` body tag, the allowlist
file format are parameterized/present) so the later layer changes configuration and routing, not the
detection design. **v2 does not widen scope to fleet.**

## D-V2-11 — Document hygiene fixed

The v1 master plan had an **unbalanced code fence** (an orphan trailing ```` ``` ```` wrapping the
Self-review section; fence count 65 = odd) and normative two-tier prose ("the page set is the only things
that ping you") that the digest tier falsifies. v2 docs use three-tier framing throughout; the orphan
fence is removed from v1 as a tiny consistency fix, and v1 carries a one-line pointer to v2.

## D-V2-12 — Hermes plugin API VERIFIED; cron/Mouse decision

> **Superseded for the allowlist UX by [D-V2-15] (2026-06-13):** the verified `pre_gateway_dispatch`
> reply plugin below is **not built** — the typed path is the `/osquery` skill, the primary is tap
> buttons. This verification stands as record (and the hook facts still inform the cron/Mouse decision);
> it is no longer the allowlist plan.

**Plugin API — verified 2026-06-10** against the live Hermes source (read-only multi-agent pass; evidence
file:line in the verification run). The `pre_gateway_dispatch` allowlist plugin is **mostly correct**:
`event.text` ✓; `event.source` ✓ (concrete type `SessionSource`, `gateway/session.py`); `source.user_id`
/ `chat_id` ✓ (`user_id` is `Optional[str]` → the empty-default guard is *required*; `source.platform` is
a `Platform` **enum**, not a string); the `{"action":"skip","reason":…}` suppression ✓ (drops the message
before auth/dispatch, `reason` is logged, `None` = pass-through); and `register(ctx)` +
`register_hook("pre_gateway_dispatch", fn)` ✓ (it is in `VALID_HOOKS`). **One real defect — the
in-channel ack:** `adapter.send` is `async def`, but the hook is a synchronous `def` the gateway invokes
**without `await`**, so the planned `gateway.adapters[platform].send(...)` builds an **orphan coroutine
that never runs** (no confirmation message, a "coroutine never awaited" warning). **Fix:** schedule it on
the live loop —
`asyncio.get_running_loop().create_task(gateway.adapters.get(s.platform).send(s.chat_id, msg))` (use
`.get()` — the adapter can be absent; the bundled Discord adapter uses this exact pattern) — **or drop
the in-hook ack** and rely on the silent skip. Also: the plugin must be **opt-in enabled**
(`plugins.enabled: [osquery-allowlist]`); **no bundled plugin uses this hook for an outbound ack**, so
there is no shipped example — which *reinforces fallback-first*. The `@`-aware label regex from D-V2-6
stands (the verification covered the Hermes API, not the label regex). **Net:** the plugin is now
`VERIFIED (1 correction)` rather than `UNVERIFIED`; building it is still human-approval-gated, but the
API risk is retired.

**Cron / Mouse decision** (answers "have Mouse run the digest builder on a Hermes cron job instead of the
webhook route"). The idea bundles three separable choices; each verified against the live cron source:

- **Runner — do NOT use Mouse (an LLM) to run the builder.** The builder is deterministic aggregation; an
  agent-driven cron job costs real tokens per tick, runs through a two-layer prompt-injection scanner,
  and would break the deterministic `T-DIGM-*` tests + the empty-suppression guarantee for zero benefit —
  and the digest store holds attacker-influenceable text (labels/paths), so an LLM *composing* the digest
  is itself a prompt-injection surface. **Use Hermes cron's `no_agent` mode** (verified: `no_agent=True`
  short-circuits before any LLM import, runs a plain script, **zero tokens**) — Hermes scheduling
  *without* Mouse.
- **Scheduler/delivery — Hermes `no_agent` cron `--deliver` is a viable alternative to the webhook route
  for the DIGEST** (not the page tier). Upsides: native channel delivery, **native secret redaction**,
  and a **native `[SILENT]` / `{"wakeAgent":false}` empty-suppression marker** that maps cleanly onto the
  digest. Two hard constraints: the script must physically live in `~/.hermes/scripts/` (absolute/`~`
  paths rejected at create+run time) → use a one-line wrapper
  `exec "$HOME/.local/bin/osquery-digest.sh" "$@"` (it inherits full env, reads `~/.local/state` freely);
  and it **depends on the Hermes gateway daemon running** (a second liveness surface the macOS
  `launchctl` watchdog can't see). **Decision: keep the macOS LaunchAgent as the v2 default** (zero new
  dependency, already watchdog-guarded + tested) and **document `no_agent` cron `--deliver` as a
  first-class swap**. The builder's core (rotate/group/render) is identical both ways; only the delivery
  tail differs (`send_alert` webhook vs print-stdout + `[SILENT]`) → a config swap, not a redesign. The
  **page tier keeps the webhook + HMAC + spool unconditionally** (it needs the reliability; the digest is
  lower-stakes, and a missed daily digest auto-recovers — the store just accumulates until the next
  successful run).
- **Mouse's real role — a deferred ADVISORY layer, never the runner.** The digest tier IS the "noisy
  tier" the locked invariant permits an LLM to touch. The clean deferred build: an **agent-driven cron
  job under a dedicated `--profile mouse`** (the dedicated-profile concept is verified real) that reads
  the deterministic digest's `.last` **read-only** and posts a **separate labeled advisory**,
  **monotonic** (only ever raises concern), **never gating or suppressing** the deterministic digest.
  This is the original "Option X" Mouse pattern with a concrete Hermes mechanism — build only with
  explicit approval, after the tiers prove calm.

## D-V2-13 — pipeline_integrity pages on content-mismatch; manifest layers 1+2 in round 1

**Decision (2026-06-12):** `pipeline_integrity` (watching the alerter's own scripts/plists) moves from
digest → **page on content-mismatch**. The earlier "digest it and I'll recognize my own `chezmoi apply`"
was rejected by Stephen: it relies on **memory**, and an attacker could deliberately tamper on a day he
deploys to hide in that blind spot. Legitimacy is judged by **content, not timing**.

**Mechanism — a baseline manifest (the Tripwire/AIDE pattern):** the detector compares a changed file's
`sha256` (already present in the `file_events` row) against a known-good **baseline manifest**. Match →
it is the legitimately-deployed content (a `chezmoi apply`) → **silent**. Mismatch → the file is
something the deployer never produced → **page**. The check never calls `chezmoi`; it reads a manifest at
a fixed path, so it is **deployer-agnostic** (chezmoi `run_after` regenerates it now → Homelab
post-deploy regenerates the same manifest later; the detector is unchanged — a clean migration seam, no
Homelab↔dotfiles coupling).

**Three layers; 1 + 2 ship in round 1 (Stephen's call):**

- **Layer 1 — source-derived baseline (un-blessable tampering).** The manifest is the hash of the
  **source of truth** (the reviewed git artifact / what the file *should* be), NOT the live deployed
  file. So tampering the deployed file can never get blessed by a later re-baseline (the source didn't
  change) → it always mismatches → pages. Eliminates the re-baseline race.
- **Layer 2 — root-owned manifest + root re-baseline (privilege bar).** The watched scripts are
  user-owned (`stephen`); the manifest is **root-owned** (root-writable, world-readable) and re-baselined
  by a **root** context (osqueryd already runs as root). A user-level compromise (malicious
  npm/brew/cask, a trojaned tool, an agent gone wrong — none have root) can tamper a script but
  **cannot** rewrite the root-owned manifest → it still pages. Forces the attacker up to **root**, whose
  escalation itself trips other page detectors (new admin / new suid). Implementation note: chezmoi runs
  as the user, so the root re-baseline uses a root LaunchDaemon or a tightly-scoped NOPASSWD entry for
  exactly the baseline script.
- **Layer 3 — off-host (deferred, the only real close for a root attacker).** No purely-local control
  survives root (they can rewrite the root manifest, patch the alerter, disable osquery). The guaranteed
  catch is the daily ✅ heartbeat consumed **off-host**, whose *absence* alarms — an attacker who silences
  the whole local pipeline cannot silence the off-host "no heartbeat." This is the deferred Homelab
  layer; **not built in round 1.**

**Honest scope:** the manifest is no stronger than the alerter that reads it (same trust domain), so
layers 1+2 *raise the bar* (un-blessable + requires root) rather than make it tamper-proof; the
tamper-proof guarantee is layer 3 (off-host), deferred. Net effect in round 1: a legit `chezmoi apply` is
silent, a user-level tamper pages, and a root tamper is forced to either trip another page detector or go
fully dark (which the future off-host heartbeat-absence catches).

## D-V2-14 — PR sequencing (review gate on everything)

Per \[[github-pr-merge-convention]\], the v2 work ships as **separate, GitHub-reviewed PRs**, never
direct to `main`: **PR #1 = the core** (three-tier gate, digest builder + 18:00 LaunchAgent, all detector
tiering incl. pipeline_integrity layers 1+2, delivery/spool, the **manual** allowlist file as the floor);
**PR #2 = tap-to-approve** (the spare-bot button daemon, pending-scoped — D-V2-15 — plus the **`/osquery`
Hermes skill** as the typed fallback, a drop-in SKILL.md that rides Bob), opened immediately after #1
merges — convenience is a top priority for Stephen (ADHD), so it is fast-followed, not indefinitely
deferred; **PR #3 = the Mouse advisory** (monotonic, sandboxed, never the runner). Each is reviewed and
approved on GitHub before merge.

## D-V2-15 — Approve/Deny buttons via Stephen's spare Discord bot; pending-scoped daemon (PR #2)

**Decision (owner's call, 2026-06-12 — buttons are essential ADHD ergonomics, not a nice-to-have).** PR
#2 ships tap-to-approve: the alerter drops a pending-request file; a small discord.py listener running
under **Stephen's spare Discord bot** (its own token — NOT Hermes/Bob's) posts the Approve/Deny message
and catches the tap, writing the allowlist on an owner-authenticated Approve. The **typed path** ships in
the same PR as the **fallback** — an **`osquery` Hermes skill** invoked
`/osquery allow|deny|list <label>` (verified 2026-06-13: the slash→skill dispatch in
`agent/skill_commands.py`, shared by CLI + gateway, loads the skill and passes the trailing text to the
agent as the skill instruction, line 254), a drop-in `~/.hermes/skills/osquery/SKILL.md` that **rides
Bob** — no fork, no second daemon, no `@Butters`. It **supersedes** the earlier `pre_gateway_dispatch`
typed-reply plugin (D-V2-12). The manual git file (PR #1) stays the floor. All writers honor the
identical file contract (spec §9).

**Why it cannot ride Hermes/Bob (verified 2026-06-12, official Discord docs).** A component interaction
is delivered ONLY to the application that posted the components — over that app's gateway connection or
its Interactions Endpoint URL (mutually exclusive) — and must be acked within ~3 s or it fails. A button
posted under Bob's token routes the tap into Hermes's process, and Hermes's plugin surface
(`pre_gateway_dispatch`) forwards typed messages only, never component interactions → catching it would
require forking Hermes (forbidden). This is specific to **component interactions (button taps)**; the
typed **`/osquery` skill** path is different — a slash command is a message Bob's skill-command dispatch
handles natively, so it rides Bob. The trade is that the agent executes the skill; the buttons stay the
LLM-free path precisely because a tap is an interaction Butters owns directly, no agent involved.

**Security note (Stephen's argument, accepted + extended for the skill).** The real attack surface in
every path is the same fail-closed writer (validates against the `@`-aware regex, refuses system
LaunchDaemons, dedups, audits, git-tracked, watched by `pipeline_integrity`) — neither a button nor the
agent can widen it. The **button** path is LLM-free. The **skill** path is agent-mediated, so
prompt-injection is conceivable there, but it is bounded by that writer and gated by owner+channel, and
it never gates a page in real time (it only pre-suppresses known-good labels) — preserving the locked
"LLM never on the page/digest trust path" invariant. Buttons stay the LLM-free **primary**; the skill is
the convenience fallback. The same-privilege residual (D-V2-13 layer 3) is unchanged.

**Fire-and-exit investigated and REFUTED — but the daemon need only run while a request is pending.** A
tap is a push with a ~3 s answer deadline and no retrieval API: Discord's two delivery modes are a live
gateway websocket (outbound-only; NAT/headless fine) or a public HTTPS endpoint (violates localhost-only
scope). Missed = gone; nothing to poll later. The one true fire-and-exit variant — a rented always-on
cloud listener (serverless worker) the Mac polls — adds external infra, a new secret, and a third party
inside the approval path: rejected. **Lock-in: a pending-scoped daemon.**

- **Start:** the alerter writes `~/.local/state/osquery-approval/pending/<request>.json` then
  `launchctl kickstart gui/$UID/<label>` — explicit and race-free (`WatchPaths` is officially "highly
  discouraged ... race-prone", launchd.plist(5)).
- **Stay:** LaunchAgent `KeepAlive.PathState` on the pending path — "kept alive as long as the path
  exists" (launchd.plist(5)) — so a mid-window crash self-heals; persistent views (`timeout=None`, stable
  `custom_id`, `bot.add_view()` on every boot, immediate `defer()`) mean a restarted process still owns
  previously posted buttons.
- **Exit:** on resolution (tap, or the `/osquery` skill fallback observed) the bot edits the message
  (buttons disabled, outcome shown), removes the pending file, and exits; path gone → launchd leaves it
  dead. No dead taps on stale buttons because resolved buttons are disabled before exit.
- **Steady state: zero extra processes** — allowlisting is a calibration-week burst, then ~never. The
  watchdog asserts "pending exists ⇒ bot running" (kickstart if not).
- **Auth on tap:** `interaction.user.id` == owner AND channel == the security channel, fail-closed — the
  same contract as the skill path (owner + security channel).
- **New secret:** the spare bot (**Butters**) token, KeePassXC → chezmoi template, mirroring the
  webhook-secret pattern. Butters needs only the `bot` invite scope and **View Channel + Send Messages on
  the security channel** (a channel overwrite, not server-wide); **zero gateway intents** (component
  interactions are delivered regardless of intents, so no Message Content / Members / Presence — it
  cannot read chats or see members).
- **Runtime: uv.** The bot is a uv-managed Python project (`pyproject.toml` + `uv.lock`, dep
  `discord.py`). A chezmoi `run_onchange` keyed on `uv.lock` runs `uv sync` at apply time (network there,
  not at boot); the LaunchAgent execs the resulting stable `.venv/bin/python` (deterministic, offline
  boot); `.venv` is chezmoi-ignored.
- **Tests: pytest is sufficient** — add only the `pytest-asyncio` plugin (still pytest, not a heavier
  framework) to drive the async button callbacks against a mocked `Interaction`; run via `uv run pytest`.
  The shared allowlist writer keeps its bats coverage.
- **Typed path = the `/osquery` skill (verified live 2026-06-13).** A drop-in
  `~/.hermes/skills/osquery/SKILL.md`; `/osquery allow|deny|list <label>` dispatches through Hermes's
  skill-command router (`agent/skill_commands.py`, shared by CLI + gateway), which loads the skill and
  hands the trailing text to the agent as the skill instruction (line 254). Because `/osquery` is a
  **registered slash command** (not free-text the gateway scrapes), the v1 false-positive worry — a
  normal prompt containing "list" — **cannot occur**; "list my PRs" is just an ordinary message to Bob.
  This **retires** both the `pre_gateway_dispatch` plugin and the `osq`-prefix/exact-match scheme. The
  agent executes the skill (tokens spent; not LLM-free), so the fail-closed writer stays the security
  boundary and the buttons remain the LLM-free primary. SKILL.md frontmatter `description` should scope
  it tightly (owner+security channel) and the body instructs the agent to call the shared writer with the
  parsed label and nothing else.
