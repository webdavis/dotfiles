# Hermes-owned investigation of Critical posture alerts

Status: design, written 2026-09-14 while the operator was asleep. NOT approved and NOT built. Every
choice made in the operator's place is recorded under Assumptions with the alternative it displaced,
and the questions only they can answer are the last section. Nothing here authorizes a build.

Scope of this document: it reconciles the four recovered documents from pull request #24 with the
Critical-alert-only trigger decision of 2026-09-12, and it writes the workflow scope as
Hermes-owned. It is the reconciliation task's deliverable, not the implementation plan.

## What this supersedes

Four documents live on the `docs/osquery-design` branch (head `2202dcbf`) and not in `main`. This
document supersedes the parts named below and keeps the rest as record.

**`specs/2026-06-03-osquery-analysis-agent-design.md`.** Superseded: section 11's orchestration
mechanism, section 4's sandbox claims, and section 8's threaded-reply output contract. Kept: sections
2, 3, 5, 6, 7, 9, 10 and 12, which are the invariant and the advisory contract.

**`plans/2026-06-03-osquery-analysis-agent.md`.** Superseded: the whole task ladder, including the
`mouse` profile name, the `--attach` flag, and the egress allow-list task. Kept: its recorded
immediate-alert and separate-advisory decision, and its failure-notification requirement.

**`decisions/2026-06-10-osquery-alerting-v2-decision-addendum.md`, D-V2-12.** Superseded: the
"Mouse's real role" paragraph, which proposed a read-only advisory over the deterministic daily
digest. Kept: the cron `no_agent` runner facts, which still bind the digest tier.

**`specs/2026-06-10-osquery-alerting-master-spec-v2.md`, section 13.** Superseded: the deferral
line's "noisy-tier only, never delivery" digest scope and its "ships as PR #3" sequencing. Kept: the
accepted-residuals list.

The single substantive reversal: **the investigation triggers on a Critical alert, not on the daily
digest.** The digest-scoped proposal in D-V2-12 and master spec v2 section 13 chose the digest
because it was the "noisy tier" the calm-channel invariant permitted a large language model to
touch. The 2026-09-12 decision moves the trigger to Critical alerts and makes the digest not a
trigger at all. Everything that followed from the digest premise goes with it: the read-only
`.last` file read, the cron cadence, the profile named for a digest runner, and the pull-request
sequencing behind the approval interface.

## The problem

posture detects a Critical security finding and pages. The page is deterministic, terse, and capped
at eight blocks and 1900 characters
(`posture/crates/posture-domain/src/page.rs`, `BLOCK_LIMIT` and `BODY_LIMIT`). It says what fired
and offers one next step. It does not say what the artifact is, whether its signature chain
verifies, or what the operator should do about this particular file at three in the morning.

The wanted behavior: the deterministic page arrives unchanged and immediately, and then a separate,
clearly subordinate advisory arrives that explains the confirmed finding and recommends a response
from a fixed vocabulary. The advisory may add concern. It may never clear the finding, delay it,
rewrite it, grant trust, or remediate anything.

## Constraints that are already settled

From the work ledger's own section, from the repository's CLAUDE.md, and from the recorded operator
rulings. These are inputs, not choices this document makes.

1. **The investigator belongs to Hermes** (operator, 2026-09-12). posture produces findings. Hermes
   owns the downstream investigation and the advisory reply.
2. **Critical alerts only, immediate original delivery, separate advisory** (operator, 2026-09-12).
   Daily digests do not trigger this workflow.
3. **Configured through dotfiles.** No agent orchestration embedded in posture, and no modification
   of third-party Hermes code. CLAUDE.md's rule against patching tools this repository does not own
   applies, so every Hermes-side mechanism must be a config file, a profile, a supported
   command-line interface, or Hermes's own published extension point.
4. **The workflow accepts alerts from other producers through the same supported alert contract.**
   posture is the first producer, not the only one.
5. **No workspace may depend on another** (operator, 2026-09-10). posture, pns, uu and lights are
   four independent cargo workspaces. A shared crate between them is forbidden; a copy of a wire
   contract held honest by golden fixtures is the sanctioned pattern.
6. **Rust files target 300 lines and never exceed 500** (operator, 2026-09-02), and Rust follows the
   clean-code skill (operator, 2026-09-06).
7. **The operator runs applies. Agents do not.** Fifteen targets need KeePassXC unlocked; the Hermes
   configs are age-encrypted, so anything written here reaches the machine only through an operator
   apply.
8. **Implementation is blocked** on the execution and network boundary, the permitted evidence and
   model-provider disclosure, credentials, and enforceable advisory limits. The ledger states the
   operator must settle that security boundary. This document does not settle it; it states the
   options precisely enough that settling it is one reading.

## The existing state, measured on dresden 2026-09-13

Everything in this section was read from the installed source or the live config on this machine,
not from the upstream documentation and not from memory.

### What posture hands pns

`posture/crates/posture-adapters/src/pns_producer/request.rs` encodes one request per alert:

| Field | Value |
| --- | --- |
| `producer` | the literal `posture` |
| `event` | the alert kind: `alert`, `cursor-reset`, and the other fixed event names |
| `signal` | `NeedsAttention` or `Observation`, nothing finer |
| `class` | `security`, only when the signal is `NeedsAttention` |
| `route` | the literal `posture`, set at the call site in `posture/crates/posture/src/alert.rs` |
| `request_id` | `posture-` plus sixteen digest bytes over the occurrence identity |
| `detail` | the page title and the page body, joined by one newline |
| `occurred_at` | an epoch second, when the clock answered |

The digest is SHA-256 (secure hash algorithm, 256-bit), truncated to its first sixteen bytes and
rendered as hexadecimal.

Three consequences matter and all three are load-bearing.

**The severity tier does not cross the wire.** `Severity` is `Critical`, `Notice` or `Info`
(`posture/crates/posture-domain/src/severity.rs`), and only `Critical` rows reach the page at all
(`render_page` filters on `Severity::Critical`). So today "the alert exists" is what encodes
"Critical", and the alert `class` says `security` rather than `critical`. A consumer that wants
Critical alerts only cannot read that from the request as it stands; it can only infer it from the
producer plus the event name.

**One alert carries many findings.** The alert's `detail` is a rendered page of up to eight Critical
findings, and its `occurrence_id` is the byte range of the results log the batch was judged from
(`posture/crates/posture-application/src/judge_results.rs`, `inode:from:offset`). The original #24
design assumed one suspect artifact per alert, resolved from a structured `path` field. That
assumption is false against the current producer.

**The structured facts exist and are discarded.** `PageFinding` carries the detector name, the
severity, an `enrichment_path` (the resolved path the next step offers to inspect), seventeen typed
display columns, the signing metadata and the triage state. All of it is rendered into Discord
markdown and then dropped. Nothing structured survives the boundary.

### What pns hands Hermes

`pns/crates/pns-adapters/src/destinations/hermes.rs`, `body_with_id`, posts exactly this body:

```json
{"agent": "...", "state": "...", "project": "...", "detail": "...", "request_id": "..."}
```

`class` is never in it. Inside pns, `class` is read for exactly one purpose, the
`delivery.bypass_silence_classes` check in `pns/crates/pns/src/event_flow/execution.rs`. The ledger's
claim that the webhook forwards no security class and no structured artifact reference is correct as
written.

pns sends an `Idempotency-Key` header (`pns/crates/pns-hermes/src/post.rs`). **The installed Hermes
webhook adapter ignores it.** Its delivery identifier is the first of `X-GitHub-Delivery`,
`svix-id`, `X-Request-ID`, and failing all three a millisecond timestamp
(`gateway/platforms/webhook.py`). `Idempotency-Key` is honored only by the separate `api_server`
platform. So the transport's deduplication key on this path is an incidental timestamp, which is
why two posts under one correlation identity are not collapsed today, and why they also are not
protected against collapsing if a future Hermes version starts reading the header.

### The live Hermes route table

Three routes exist in `~/.hermes/config.yaml` under `platforms.webhook.extra.routes`, all
`deliver_only: true`, all delivering to Discord with an explicit `chat_id`:

| Route | Prompt template | Producer |
| --- | --- | --- |
| `priority` | `{alert.title}`, `{alert.detail}` | the retired bash alerter's shape |
| `pns` | `{agent} · {state} · {project}`, `{detail}` | pns's default |
| `unattended-upgrades` | the `pns` shape, prefixed | uu's weekly record |

**There is no `posture` route.** posture names `posture` at every call site, and the tracked route
checker (`.chezmoiscripts/run_after_68-hermes-log-route-status.sh.tmpl`) checks only `pns` and
`unattended-upgrades`. `posture` is a usable path segment, so pns builds the URL (uniform resource
locator) and posts, the gateway answers 404, and pns dead-letters the leg as a permanent refusal.
Neither the config nor the checker currently agrees with the producer. That mismatch is a separate
open ledger item, and this design depends on it being resolved first: an investigation workflow
hanging off an alert path that dead-letters is an investigation nobody ever sees.

### What the installed Hermes can do

Installed pin is `a4091e49f10ddceaac1a902848aabfb1b9aae210`
(`.chezmoidata/hermes.yaml`, confirmed against `git -C ~/.hermes/hermes-agent rev-parse HEAD`). The
ledger's review compared it with upstream `b6b53c69`. Confirmed differences and mechanics:

**`kanban attach` does not exist on the installed pin.** There is no `attach` subcommand and no
`--attach` flag anywhere in `hermes_cli/kanban.py`. The `hermes_cli/kanban_db_dispatch.py` file the
ledger's upstream link names is also absent; the installed dispatcher is `hermes_cli/kanban_db.py`.
So evidence transfer by attachment is not available here.

**`kanban create` is a rich, supported trigger.** `--assignee <profile>`, `--workspace dir:<path>`
(alongside `scratch`, `worktree`, `worktree:<path>`), `--idempotency-key`, `--max-runtime <duration>`,
`--max-retries N`, `--skill <name>` repeatable, `--body`, `--initial-status`, and `--json`.
`--workspace dir:<path>` is the supported evidence-transfer mechanism that replaces attachment: the
card's workspace *is* a directory the trigger prepared.

**A profile's own `config.yaml` governs its worker.** The dispatcher sets
`HERMES_HOME` to the assignee profile's root before spawning
(`hermes_cli/kanban_db.py`, around line 7383), so a profile-scoped `terminal.backend: docker` really
does apply to that profile's workers. All four existing profiles on this machine (`butters`,
`concerned`, `elaine`, `nicodemus`) are `backend: local` with `container_persistent: true`, so a
Docker-backed profile would be the first.

**Docker contains tool execution, not the worker.** The worker is a host subprocess,
`hermes -p <profile> --accept-hooks ...`. Its model calls leave the host process. Only the terminal
tool's commands enter the container, through `docker exec`
(`tools/environments/docker.py`, line 948). File tools route through the same environment when the
backend is docker (`tools/file_tools.py` resolves `ShellFileOperations` over the active
environment), so reads and writes are contained too. **This split is the single most useful fact in
this document:** it means a container with no network still lets the worker reason, because the
model call is on the other side of the boundary.

**The container's hardening is real, and its network is not restricted.**
`tools/environments/docker.py` emits `--cap-drop ALL`, `--security-opt no-new-privileges`,
`--pids-limit 256`, and size-limited `tmpfs` mounts for `/tmp`, `/var/tmp` and `/run`. It has a
`network: bool = True` constructor parameter that appends `--network=none` when false, **and nothing
ever passes it.** `tools/terminal_tool.py` constructs `DockerEnvironment` without `network=`, and
there is no config key for it. The only lever is `terminal.docker_extra_args`, which is validated
for string-ness and appended last to the `docker run` argv, so `--network=none` supplied there does
reach Docker. The ledger's finding that proxy environment variables alone do not enforce an outbound
allowlist is correct, and this is the mechanism that does.

**Automatic mounts and credential forwarding are real.**
`tools/credential_files.py` exposes `get_credential_file_mounts()`, `get_skills_directory_mount()`
and `get_cache_directory_mounts()`, and the Docker environment bind-mounts every credential file a
loaded skill registered, read-only, plus the skills directory. So the set of credentials inside the
box is a function of which skills the profile loads.

**The worker inherits the dispatcher's whole environment.** `env = dict(os.environ)` in
`hermes_cli/kanban_db.py`. With `kanban.dispatch_in_gateway: true` in the live config, the
dispatcher is the gateway process, so a worker inherits whatever `~/.hermes/.env` gave the gateway.
This is the credential-forwarding surface, and it is on the host side of the container boundary, so
`--network=none` does not touch it.

**`container_persistent: true` makes the workspace survive.** With persistence on, `/workspace` and
`/root` are bind mounts under `~/.hermes/sandboxes/docker/<task_id>`; with it off, both are `tmpfs`
and vanish with the container. Separately, `terminal.docker_persist_across_processes` (default true)
makes a later process **reuse a labeled container by `(task_id, profile)` labels alone**, and the
code states plainly that it deliberately does not compare image, mounts or resources. Both knobs
must be off for an ephemeral box.

**A Discord reply is not reachable from the webhook path.** The Discord adapter's `send` does accept
`reply_to` and a `metadata.thread_id` (`plugins/platforms/discord/adapter.py`), but the webhook
adapter's `_deliver_cross_platform` calls `adapter.send(chat_id, content, metadata=metadata)` and
never passes `reply_to`; `deliver_extra` is read for `chat_id` and `thread_id` only. So the original
design's section 8, "posted as a reply threaded under the alert", is not buildable through a
`deliver_only` route on this pin. The only association available is textual.

**Kanban lifecycle hooks exist and are the publisher's trigger.** `hermes_cli/plugins.py`'s
`VALID_HOOKS` includes `kanban_task_claimed` (dispatcher process), `kanban_task_completed` (worker
process, with a `summary`) and `kanban_task_blocked` (with a `reason`). All three are observers;
return values are ignored, so a hook cannot veto anything, which is exactly right for a publisher
and wrong for a gate.

**Auto-decompose only touches triage cards.** `kanban.auto_decompose: true` is live, which looked
alarming for a security card, but `gateway/kanban_watchers.py` runs the decomposer over "up to N
**triage** tasks". `kanban create` defaults to `--initial-status running` and `--triage` is opt-in,
so an investigation card is never decomposed. Concern resolved, not mitigated.

**Latency has a floor.** `kanban.dispatch_interval_seconds: 60`, so a created card waits up to a
minute for the dispatcher tick. `kanban.max_in_progress_per_profile: null` means concurrency is
unbounded today.

**The model provider is disclosed by the config.** `model.default: gpt-5.6-sol`,
`model.provider: openai-codex`, `model.base_url: https://chatgpt.com/backend-api/codex`. Evidence
the investigator reasons over is sent to that endpoint. Docker 29.7.2 is installed and its daemon
answers, and `docker-desktop` is declared in `.chezmoidata/system_packages_autoinstall.yaml`, so the
Docker backend is available.

## Three approaches

### A. Agent webhook route: a second Hermes route in agent mode

Add a route with `prompt`, `skills` and `deliver`, and no `deliver_only`. The producer posts to it;
Hermes runs the agent and delivers its response. This is what #24 section 11 proposed.

Trade-offs. It is the smallest configuration change and needs no new code at all. But it puts a
large language model on the inbound path with attacker-influenceable text as its prompt and no
bounded workspace; the agent runs in the **default** profile, whose `terminal.backend` is `local`,
because `multiplex_profiles: false` in the live config means the `/p/<profile>/webhooks/<route>`
prefix is ignored rather than honored. There is no per-route runtime cap, no retry ceiling, and no
structured result: the model's free text is delivered verbatim, so the advisory limits live only in
the prompt, which the ledger already rules insufficient. Rejected.

### B. Kanban card with a Docker-backed profile, published by a validator

The producer's alert reaches Hermes as it does today. A deterministic trigger creates one kanban
card per Critical alert, assigned to a dedicated Docker-backed profile, with the evidence already
copied into a bounded directory that is the card's workspace. The worker reasons over that directory
with a networkless container, writes a structured result file, and completes. A `kanban_task_completed`
hook hands the result to a deterministic validator that checks it against a fixed schema and only
then publishes the advisory; a `kanban_task_blocked` hook publishes a distinguishable failure notice.

Trade-offs. It uses only installed, supported interfaces: `kanban create --workspace dir:`, a
profile `config.yaml`, `docker_extra_args`, and two published hooks. The advisory limits are
enforced by code outside the model rather than by the prompt. It costs one new small tool (the
validator and publisher) and one new trigger, and it introduces the machine's first Docker-backed
profile. The container boundary does not contain the worker's own host process or the environment it
inherited, so credentials must be addressed separately from the network.

### C. Networkless two-stage: collector card, then reasoner card

Split into two cards. The first runs a deterministic collector inside the networkless box over the
evidence and writes structured facts (signature verification result, quarantine attribute, strings
digest). The second reasons only over those facts, never over the artifact bytes, and never runs
anything.

Trade-offs. It is the strongest containment: the bytes the model sees are a bounded fact sheet this
repository's own code produced, so the prompt-injection surface shrinks from "the whole artifact" to
"field values in a schema we control", and the model-provider disclosure shrinks with it. It costs a
second card, a second dispatcher tick of latency (up to two minutes), and a collector that must be
written and maintained. It also loses the original design's stated value, which was a model reading
the artifact itself.

**Recommendation: B, with C's collector as the stage that produces the evidence, and C's fact sheet
as the default rather than raw bytes.** B's shape is right and C's discipline is right. The
difference between them is one configuration question, "does the model see the artifact bytes or
only our fact sheet", and that question is an operator decision about model-provider disclosure
rather than an architecture choice. Building B with the collector present makes both answers a
one-line change.

## The recommended design

Five stages. The first is unchanged from today and the other four are additive.

### Stage 1: the deterministic page, unchanged

posture judges its results log, renders the Critical page, and submits it to pns, which delivers it
to Hermes and thence to Discord. Nothing in stages 2 through 5 can delay, suppress, alter or gate
this. **The invariant from #24 section 2 survives intact and is the design's first behavior.**

Behavior to pin: with the investigator profile stopped, with Docker stopped, and with the validator
binary deleted, a synthetic Critical finding still produces the identical page on the identical
route with the identical body bytes.

### Stage 2: the evidence copy

A bounded, no-follow evidence collector, owned by this repository, runs on the host with the
producer's own privileges. It takes the alert's correlation identity and the structured finding rows
the alert was built from, and writes one directory:

```
~/.local/state/posture/investigations/<correlation-id>/
  finding.json      the structured rows, schema-versioned, no rendered prose
  artifacts/        the resolved paths, copied, one regular file each
  facts.json        the collector's own out-of-band verification results
```

Boundaries. It copies regular files only; it refuses symlinks, directories, devices, and sockets
rather than following or repairing them, matching the converge tool's existing refusal posture. It
caps per-file bytes and total bytes. It never reads anything the finding did not name. It resolves
paths from `PageFinding::enrichment_path`, never from a name the model chose.

`facts.json` is the out-of-band mitigator set that #24 section 6 permits and nothing else:
`codesign --verify --strict` result, `spctl` assessment, quarantine extended attribute presence,
and allowlist membership by hash or label. These are the only facts allowed to lower concern, and
they come from the host, never from the artifact's own self-description. The collector writes
`facts.json` twice, once into the workspace for the worker to read and once into a sibling record
the workspace cannot reach, because stage 5 has to corroborate against a copy the worker could not
have edited.

Failure mode: a collector that cannot complete writes `finding.json` and an explicit
`collection_incomplete` reason, and the workflow proceeds to a failure notice rather than to a
silent all-clear.

### Stage 3: the trigger

One deterministic step, no model, turns a Critical alert into one card:

```
hermes kanban --board security create "<correlation-id>" \
  --assignee <investigator-profile> \
  --workspace dir:~/.local/state/posture/investigations/<correlation-id> \
  --idempotency-key <correlation-id> \
  --max-runtime 10m \
  --max-retries 1 \
  --initial-status running \
  --json
```

`--board` sits before the subcommand, not after it. It is a global flag on the `kanban` parser rather
than an argument of `create`, so `kanban create --board security` is rejected. This is the same class
of mistake as #24's `kanban create --attach`, which is why the recipe above is written out in full
rather than described.

`--idempotency-key <correlation-id>` is what makes a retried alert reuse its card instead of opening
a second investigation; the installed `create` returns the existing task's identity for a
non-archived card with that key. `--max-retries 1` blocks on the first failure rather than looping a
failing investigation. A dedicated board keeps security cards off the operator's working board.

Where the trigger lives is the one genuinely open placement question, and the three candidates are
in Open Questions. What is settled here: it is deterministic, it is not a model, and it does not
live in posture.

### Stage 4: the investigation

The card's worker runs under a dedicated profile whose `config.yaml` sets:

| Key (under `terminal` unless noted) | Value | Why |
| --- | --- | --- |
| `backend` | `docker` | the execution boundary |
| `container_persistent` | `false` | `/workspace` and `/root` become `tmpfs` |
| `docker_persist_across_processes` | `false` | no reuse of a labeled container |
| `docker_mount_cwd_to_workspace` | `true` | the evidence dir becomes `/workspace` |
| `docker_extra_args` | `["--network=none"]` | the only supported route to no network |
| `docker_volumes` | `[]` | no operator-declared mounts |
| `env_passthrough`, `docker_forward_env`, `docker_env` | empty | no host env crosses in |
| `lifetime_seconds`, `timeout` | short, bounded | a hung command frees the card |
| `toolsets` (top level) | the minimum that completes a card | see below |
| `agent` dangerous-command auto-approval | off | never approve in here |
| `skills` (top level) | none registering a credential file | see below |

Three of those rows need a sentence rather than a cell. The minimum toolset must be measured at
build time; `terminal`, `file` and `kanban` is the starting hypothesis, and `kanban` is there only
because the worker has to complete its own card. The skills constraint exists because the Docker
environment bind-mounts the skills directory and every credential file a loaded skill registered, so
the set of credentials inside the box is decided by the profile's skill list. And
`docker_mount_cwd_to_workspace` works here only because the dispatcher already pins `TERMINAL_CWD`
to the card's workspace: with the flag on, `tools/terminal_tool.py` promotes that absolute directory
to `host_cwd`, sets the container cwd to `/workspace`, and the Docker environment bind mounts one
onto the other. The mount is read-write, which stage 5 relies on and stage 5 also guards against.

The worker's job is #24 section 6's job, verbatim and unchanged: the finding is already a confirmed
Critical, so the question is never "is this real". It may add concern, explain, or recommend from a
fixed vocabulary. It may not issue a de-escalating verdict. Mitigating context comes only from
`facts.json`.

Its output is not prose. It writes `result.json` into the workspace:

```json
{
  "schema": "posture.advisory/1",
  "correlation_id": "posture-0a1b2c3d4e5f60718293a4b5c6d7e8f9",
  "concern": "raise",
  "explanation": "one length-capped paragraph of display text",
  "mitigators": ["signature-verified"],
  "recommended": ["quarantine-file"]
}
```

Three closed vocabularies, and every one of them is the design rather than a suggestion:

| Field | Permitted values |
| --- | --- |
| `concern` | `raise`, `same` |
| `mitigators` | `allowlisted`, `signature-verified`, `no-quarantine-xattr` |
| `recommended` | `quarantine-file`, `disable-launch-item`, `revert-setting` |

`concern` has no lowering value, which is how the monotonic rule stops being a prompt instruction
and becomes a parse error. `recommended` is #24 section 6's fixed remediation vocabulary, unchanged.
There is no command field, no path the model chose, and no free text outside `explanation`.

### Stage 5: the validator and publisher

A `kanban_task_completed` hook, a small deterministic module this repository owns, reads
`result.json` and validates it:

1. The schema version is one it knows.
2. `correlation_id` equals the card's identity, and the card's identity names a real alert this
   machine submitted. A result that correlates to nothing is a failure, not an advisory.
3. `concern` is in the closed set. `mitigators` and `recommended` are subsets of their closed sets.
4. Every mitigator claimed is corroborated by the collector's own out-of-band record. A mitigator
   the host did not observe is a validation failure, which is what makes a planted "cleared in
   ticket SEC-4471" cover story produce a failure notice rather than a softened advisory.
5. `explanation` is length-capped and stripped of control characters, and is published as display
   text.

**The corroborating facts are not read back from the workspace.** The evidence directory is bind
mounted read-write, because the worker has to write `result.json` into it, so everything else in
that directory is writable from inside the box too. The validator therefore compares the claimed
mitigators against a copy of `facts.json` the collector kept outside the workspace. Corroborating
a claim against a file the claimant could edit corroborates nothing.

On success it publishes the advisory through the producer-agnostic alert contract, labeled
unmissably as untrusted commentary subordinate to the alert above it, and carrying the correlation
identity as text because a Discord reply is not reachable on this pin. On any validation failure,
and from the `kanban_task_blocked` hook, and on a `--max-runtime` timeout, it publishes a **failure
notice** that says the investigation did not complete. The notice is textually distinct from both an
advisory and an all-clear, because #24's plan recorded that failure and timeout must be
distinguishable from an all-clear and that requirement is kept.

### Boundaries

| Boundary | What it contains | What it does not |
| --- | --- | --- |
| the container | every command and file read the worker runs | the host process and its inherited env |
| `--network=none` | outbound network from evidence execution | the worker process, so the model works |
| the evidence directory | what the model can see | what the host can see |
| `result.json`'s schema | what the model can say | what the model can think |
| the validator | what reaches Discord | what the model wrote to its workspace |

### Failure modes

| Failure | Behavior | Distinguishable |
| --- | --- | --- |
| collector cannot copy an artifact | card carries `collection_incomplete` | yes, named in the notice |
| Docker daemon down | card fails, `--max-retries 1` blocks it | yes, failure notice |
| gateway down, no dispatcher tick | card sits ready, nothing publishes | **no**, the one silent state |
| `--max-runtime` exceeded | worker is terminated and re-queued, then blocked | yes, failure notice |
| `result.json` absent or unparseable | validation failure | yes, failure notice |
| an uncorroborated mitigator claim | validation failure | yes, and the claim is quoted |
| route missing on the gateway | pns dead-letters a 404 on first failure | yes, through pns's report |

The gateway-down row is the only one with no notice of its own, and it is Open Question 4. The
route-missing row relies on the permanent-status classification recorded in the 2026-09-08 pns
delivery-failure-reporting design, which is where 404 stopped being retried twenty times.

### Security

The safety rests on four properties that need no cooperation from the model, which is #24 section
7's own framing and is kept:

1. **The page fires first and independently.** Stage 1 has no dependency on stages 2 through 5.
2. **The evidence is chosen by the host.** Paths come from `enrichment_path`, never from the model.
3. **Execution has no network.** The one artifact-reading, command-running stage cannot beacon.
4. **The output vocabulary is closed and corroborated.** De-escalation is not expressible, and a
   mitigator must be independently observed.

Three residual exposures are named rather than mitigated, because mitigating them is an operator
decision:

- **Credential forwarding to the worker process.** `env = dict(os.environ)` on a gateway-embedded
  dispatcher hands the worker the gateway's environment. The container does not see it, but the
  worker process does, and the worker process is what talks to the model. Nothing in the installed
  Hermes narrows this, and narrowing it by patching Hermes is forbidden. The available lever is what
  the gateway's own environment contains.
- **Model-provider disclosure.** Evidence bytes, or the fact sheet, go to
  `https://chatgpt.com/backend-api/codex`. Approach C's fact sheet shrinks this to field values this
  repository produced. A local model removes it at a quality cost.
- **Amplified false positives.** The worker treats every Critical as ground truth, so a
  false-positive Critical is explained rather than calmed. #24 section 10's dependency on alert
  precision still holds, and `facts.json` is the only safe relief valve.

## Out of scope

- Investigating anything below Critical. The digest is explicitly not a trigger.
- The approval interface. Tap-to-approve and the `/osquery allow|deny|list` skill are a separate
  ledger item, and the ledger's own constraint that investigation must not grant the analyst
  approval authority is why they stay separate.
- Remediation of any kind. The advisory recommends; nothing acts.
- A Discord threaded reply. Not reachable on this pin; revisit if a Hermes version passes `reply_to`
  from `deliver_extra`.
- The missing `posture` route and the route-checker's coverage of it. A prerequisite, tracked
  separately.
- Per-run grouping of repeated findings about the same subject, signature-chain verification in the
  Rust paths, and interpreter-payload assessment. All three are their own ledger items, and the
  first one changes what a correlation identity covers, so this design's identity is the alert's,
  not a finding's.
- Kernel-extension install-state monitoring and off-host machine-death detection. Homelab scope.
- Any Hermes source change, including a patch that would pass `network=False`.

## Assumptions made in the operator's place

Each is a choice this document made because the work could not proceed without one, stated with what
it displaced.

1. **The trigger is deterministic code, not an agent route.** Alternative: #24's second agent route,
   which is far less work and needs no new binary. Displaced because it runs in the `local`-backend
   default profile, has no runtime cap, and publishes unvalidated model text.
2. **Evidence transfers as a directory through `--workspace dir:`.** Alternative: wait for a Hermes
   version with `kanban attach`, or read the artifact through the card body. Displaced because
   `attach` does not exist on the installed pin and a body is not a bounded no-follow copy.
3. **The advisory is published by a validator outside the model, from a schema-constrained file.**
   Alternative: publish the model's response text with a warning label, as #24 section 8 proposed.
   Displaced by the ledger's own ruling that a prompt cannot guarantee semantic limits.
4. **The default is the fact sheet, not the artifact bytes.** Alternative: the artifact bytes, which
   is what #24 wanted and is more useful. Displaced pending the model-provider disclosure decision,
   and reversible in one configuration line.
5. **The advisory associates by carrying the correlation identity as text.** Alternative: a Discord
   reply or thread. Displaced because the webhook delivery path never passes `reply_to`.
6. **One card per alert batch, not per finding.** Alternative: one card per Critical finding, which
   matches #24's per-artifact framing. Displaced because the alert's own identity is the byte range
   of a batch, so per-finding cards would need a new identity scheme and would fan out up to eight
   investigations from one page.
7. **A dedicated kanban board named `security`.** Alternative: the default board. Displaced so that
   `max_in_progress_per_profile` and the board's own retention can be tuned without touching the
   operator's working board.
8. **The profile is named for its function.** Alternative: a character name, which is this machine's
   actual convention (`butters`, `concerned`, `elaine`, `nicodemus`), and which #24's plan used
   (`mouse`) against its own design's `security-analyst`. Displaced by the self-documenting-names
   and user-agnostic-names rulings, but this is the weakest assumption here and the operator's
   convention may simply win.
9. **`--max-runtime 10m` and `--max-retries 1`.** Alternative: any other numbers. Chosen so a
   failed investigation blocks rather than loops; both are guesses and neither is measured.

## Open questions

1. **Does the model see the artifact bytes, or only the host-produced fact sheet?** This is the
   model-provider disclosure decision the ledger says is blocking. The design builds either; the
   answer is one configuration line.
2. **Where does the trigger live?** Three candidates, all deterministic: a Hermes `no_agent` cron
   job (which needs a wrapper script physically under `~/.hermes/scripts/` and depends on the
   gateway daemon); a `kanban_task_claimed`-style Hermes plugin this repository owns; or a host
   watcher of posture's own results log driven by a LaunchAgent. The second is the most Hermes-owned
   and the third is the most independent of the gateway's liveness.
3. **What does the gateway's own environment contain?** The worker inherits it wholesale, and that
   is the credential surface the container cannot close. Whether it is acceptable as it stands, or
   whether the gateway's environment needs narrowing first, is the operator's call.
4. **How is "the gateway is down, so nothing was investigated" surfaced?** It is the one silent
   state in the failure table. A card that sits ready forever publishes neither an advisory nor a
   notice. posture's existing watchdog already probes the gateway, so the cheap answer is to let the
   watchdog own it, but that needs saying rather than assuming.
5. **Is the profile a character name or a function name?** Assumption 8, stated so the local
   convention can overrule it.
6. **Does the alert wire gain a severity field, a structured evidence reference, or neither?**
   Today the trigger must infer "Critical" from producer plus event name. Adding `severity` and an
   evidence reference to the request is the honest fix and the ledger's own instruction that
   "any producer/transport changes should carry data" points at it, but it is a change to two
   independent workspaces plus their golden fixtures and needs its own approval.
7. **Should `run_after_59-hermes-config-migrate` cover profile configs?** It migrates the root
   `config.yaml` forward to the installed schema. A new profile's `config.yaml` with a Docker block
   is a second file that can fall behind a pin bump, and nothing currently migrates it.
8. **Is the advisory's Discord destination the same channel as the page?** The page goes to the
   `priority` chat identity. An advisory in the same channel is adjacent and readable; a separate
   channel keeps the page's channel calm. The ledger notes a named route selects one destination
   rather than broadcasting to two, so this is a route decision, not a message decision.
