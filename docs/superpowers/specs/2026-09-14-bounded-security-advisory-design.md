# Immediate delivery plus a separate bounded advisory

Status: design, written 2026-09-14 while the operator was asleep. NOT approved and NOT built. Every
choice made in the operator's place is recorded under Assumptions with the alternative it displaced,
and the questions only they can answer are the last section. Nothing here authorizes a build.

This is the second document in the chain. The first, the Hermes-owned investigation design also
written 2026-09-14, reconciled the four recovered documents from pull request #24 with the
Critical-alert-only decision and settled the workflow's ownership. This one answers the ledger bullet
that follows it: preserve immediate deterministic alert delivery, start an ephemeral investigator
over a bounded evidence copy, publish a separate associated advisory, make failure and timeout
distinguishable from an all-clear, and verify the advisory limits outside the prompt before promising
enforcement.

Its two deliverables are the two sentences in that bullet that the first document left as prose: an
enforcement point named for every advisory limit, and a proof that the investigator cannot suppress,
delay, rewrite or clear the original alert.

## What this changes in the first document

Nine facts measured for this document contradict or materially extend the first one. They are listed
here because the first document's stage 5 is not buildable as written.

**1. `kanban_task_blocked` never fires on a timeout, a crash, or a circuit-breaker trip.** The first
document proposed publishing the failure notice from that hook. Only `block_task()` calls
`_fire_kanban_lifecycle_hook("kanban_task_blocked", ...)`
(`hermes_cli/kanban_db.py`, around line 4437). The timeout path is `enforce_max_runtime()`, which
signals the worker, writes `status = 'ready'` by direct SQL, appends a `timed_out` event, and then
calls `_record_task_failure()`. That function flips the row to `blocked` with its own
`UPDATE tasks SET status = 'blocked'` and appends a `gave_up` event. Neither touches `block_task()`,
so no hook fires. A timed-out investigation would have published nothing at all.

**2. `kanban_task_completed` fires in the WORKER process.** `hermes_cli/plugins.py` says so in its
own comment on `VALID_HOOKS`, and the worker is the `hermes -p <profile> chat -q` subprocess that
drives the model. A validator running there is deterministic code we own, but it runs inside the
process whose tool calls and session the model influences, and it is gone the moment that process is
killed. A publisher should not live in the process it is publishing about.

**3. A supported terminal-event notifier exists, and it is unsafe here.**
`hermes kanban notify-subscribe <task> --platform discord --chat-id <id>` registers a subscription
that the gateway's `_kanban_notifier_watcher` polls, delivering
`TERMINAL_KINDS = ("completed", "blocked", "gave_up", "crashed", "timed_out")`
(`gateway/kanban_watchers.py`). That covers every failure state including the two the hooks miss. But
for `completed` it delivers the worker's own summary text verbatim (first line, 200 characters), and
then calls `_deliver_kanban_artifacts`, which extracts bare absolute paths from that summary and
uploads those files to the chat as native documents and images. The path filter is
`validate_media_delivery_path` in `gateway/platforms/base.py`: in the default non-strict mode it
accepts **any existing regular file** that is not under a credential denylist. The denylist is
`/etc`, `/proc`, `/sys`, `/dev`, `/root`, `/boot`, `/var/log`, `/var/lib`, `/var/run`, and inside the
home directory `.ssh`, `.aws`, `.gnupg`, `.kube`, `.docker`, `.config`, `.azure`, `.gcloud` and
`Library/Keychains`. It does not cover `~/Documents`, `~/workspaces`, `~/.local/state`, `~/.claude`,
or `~/.claude.json`. So a subscription hands the investigator a host-side file upload channel to
Discord for most of the machine, chosen by writing a path into its completion summary, and
`--network=none` does nothing about it because the gateway performs the upload. Strict mode
(`gateway.strict`, or `HERMES_MEDIA_DELIVERY_STRICT=1`) narrows this to Hermes caches, operator
allowlisted roots and a recency window, at the cost of changing media delivery for every other
profile on the machine. The notifier's `timed_out` message also reads "will retry", which is false
under the retry ceiling this design sets.

**4. A `dir:` workspace survives completion.** `_cleanup_workspace` in `hermes_cli/kanban_db.py`
removes only `scratch` workspaces and says in its own docstring that `worktree` and `dir` workspaces
are intentionally preserved. The evidence directory and the result file are still on disk after the
card completes, which is what makes an out-of-process publisher possible.

**5. The correlation identity is not stable across every retry.** `judge_results.rs` builds
`occurrence_id` as `inode:from:offset`, where `from` is the stored cursor and `offset` is
`from + records.bytes` for the rows this run read. The cursor advances only on
`Submission::Accepted`, so a failed submission is re-judged, but the next run reads to the log's
current size. If more rows arrived in between, `records.bytes` differs, the occurrence identity
differs, the `request_id` differs, and `--idempotency-key` does not collapse the two. The first
document's assumption that a retried alert reuses its card holds only while the results log is
quiet.

**6. The wire already has a place for structured producer data.**
`pns/crates/pns-protocol/src/request.rs` carries
`pub extensions: Map<String, Value>` with `#[serde(default)]`, documented as "producer-specific data,
carried verbatim and never read here". Adding a severity word and an evidence reference there is an
additive change to one workspace, not a schema break across two. It does not reach Hermes (pns's
`body_with_id` sends only `agent`, `state`, `project`, `detail` and `request_id`), but it does reach
pns's own durable ledger, which matters for point 8.

**7. The automatic container mounts are ungated but profile-scoped.**
`tools/environments/docker.py` bind mounts every credential file a loaded skill registered, the
skills directory, and five cache directories (`cache/documents`, `cache/images`, `cache/audio`,
`cache/videos`, `cache/screenshots`), all read-only, with no configuration key to disable any of
them. All of it resolves through `get_hermes_home()`, and the dispatcher pins the worker's
`HERMES_HOME` to the assignee profile's root (`resolve_profile_env`). A profile root with no skills
directory, no registered credential files and no cache directories therefore produces none of these
mounts. That is the enforcement point, and it is a property of the profile's directory contents
rather than of a setting.

**8. pns's ledger is a durable, host-side record of every committed alert.**
`ledger_events` has `UNIQUE(producer, request_id)` and, since schema version 5, a `producer_request`
column holding the request as submitted (`persistence/sqlite/ledger/schema.rs`). The database is
`~/.local/state/pns/pns.db`. A row exists only after `pns submit` committed the page, so reading that
table is a trigger input that cannot precede the page.

**9. A killed worker leaks a running container.** Containers are started `docker run -d ... sleep
infinity`, and `reap_orphan_containers` filters on `status=exited` with the code stating that running
containers are never reaped. With `docker_persist_across_processes: false` the teardown runs from
`cleanup()` on an `atexit` hook, which a SIGKILL from `enforce_max_runtime` skips. So the timeout path
leaves a container running with the evidence directory still bind mounted.

One more input the first document did not weigh. The ledger names
`~/Documents/Sandboxed_Agent_Prompt_Injection_Research_20260603/report.md` as a review input. Its
verdict is **"Keep the deterministic pipeline"**, on the ground that sandboxing closes the
exfiltration axis while a security monitor's real exposure is the integrity of the verdict a human
acts on, which no sandbox touches. It cites 11.8 percent injection success against the strongest
tested defense in a study of this exact use case, and its recommendations are: keep the security
decision deterministic; if a model is wanted at all, constrain it to a length-capped, clearly
delimited blurb on a side channel, generated only from already-sanitized structured fields, never
able to remove or downgrade the deterministic alert; and never feed raw attacker-controlled fields to
the model as instructions-eligible context. Recommendation 3 answers the first document's open
question 1 before the operator gets to it: the research says the model reads the host-produced fact
sheet, not the artifact bytes. This design therefore treats the fact sheet as the decision rather
than the default.

## The problem

posture pages on a Critical security finding. The page is deterministic, capped at eight blocks and
1900 characters (`posture/crates/posture-domain/src/page.rs`), and says what fired plus one next
step. It does not explain the artifact.

The wanted behavior, from the ledger bullet: the deterministic page arrives immediately and
unchanged; a separate, clearly subordinate advisory follows, associated with the alert; the advisory
may add concern or explanation and may recommend from a fixed vocabulary; it may never clear the
finding, suppress it, delay it, rewrite it, grant trust, or remediate; and a failure or a timeout
must be visibly different from an all-clear.

The design work is entirely in the last two clauses. The first three are a delivery arrangement. The
last two are a question about where a limit is enforced when the thing being limited is a language
model, and the ledger has already ruled that a prompt is not an enforcement point.

## Constraints that are already settled

Inputs, not choices this document makes.

1. **The investigator belongs to Hermes** (operator, 2026-09-12). posture produces findings.
2. **Critical alerts only; immediate original delivery; separate advisory** (operator, 2026-09-12).
3. **Configured through dotfiles, with no agent orchestration in posture and no modification of
   third-party Hermes code.** Every Hermes-side mechanism is a config file, a profile, a supported
   command-line interface, or a published extension point.
4. **The workflow accepts alerts from other producers through the same supported alert contract.**
5. **No workspace may depend on another** (operator, 2026-09-10). A copy of a wire contract held
   honest by golden fixtures is the sanctioned pattern.
6. **Rust files target 300 lines and never exceed 500** (operator, 2026-09-02), and Rust follows the
   clean-code skill (operator, 2026-09-06).
7. **The operator runs applies. Agents do not.** The Hermes configs are age-encrypted, so everything
   here reaches the machine only through an operator apply.
8. **Deployed scripts invoked by launchd live under `~/.local/libexec`**, not `~/.local/bin`, which
   holds only what the operator types.
9. **Bash follows the Wooledge BashGuide practices** recorded in this repository's CLAUDE.md: arrays
   not word-split strings, `jq -n --arg` for JSON construction, `while read` not `for x in $(...)`,
   validated numeric arguments, and an error on an unknown argument.
10. **Implementation is blocked** on the execution and network boundary, permitted evidence and
    model-provider disclosure, credentials, and enforceable advisory limits. This document states the
    options precisely; it does not settle the boundary.

## The existing state

Everything here was read from installed source or live config on dresden for this document.

### The producer handshake, which is stronger than it looked

`posture alert` submits synchronously and treats submission as a gate on its own state.
`PnsProducer::submit` runs `pns submit --json` with the encoded request on standard input, then
requires three things of the answer: the returned `request_id` equals the one it sent, the status is
`Accepted`, and the diagnostics contain `ledger_committed`
(`posture/crates/posture-adapters/src/pns_producer.rs`). Anything else is
`Submission::NotAccepted`, and `judge_results.rs` then leaves the cursor where it was so the next run
re-judges the same rows. An engine failure also fires `LastResortBanner`, an independent alarm that
does not go through pns at all.

Two consequences for this design. The page's delivery is decided before any investigation exists, by
a process that has already exited. And an alert that could not be committed is retried by posture
itself rather than being lost, so a trigger reading committed ledger rows never sees a page that was
not delivered.

There is also an existing omission path: an oversized `NeedsAttention` alert is replaced by a bounded
`notification-omitted` notice whose acceptance explicitly does not acknowledge the omitted finding
(`pns_producer/request.rs`). That is the repository's own precedent for "a notice that is not an
all-clear", and the failure notice in this design follows its shape.

### What crosses each boundary

| Boundary | Carries | Does not carry |
| --- | --- | --- |
| posture to pns | `producer`, `event`, `signal`, `class`, `route`, `request_id`, `detail`, `occurred_at`, and an unused `extensions` map | the severity tier, any structured finding row, any artifact path |
| pns to Hermes | `agent`, `state`, `project`, `detail`, `request_id` in the body, `Idempotency-Key` and `X-Webhook-Signature` as headers | `class`, `route`, `extensions` |
| Hermes to Discord | the route's rendered prompt template to one `chat_id` | any reply or thread association on a `deliver_only` route |

`class` defaults to `["security"]` in `delivery.bypass_silence_classes`
(`pns/crates/pns-adapters/src/config/model.rs`), which is how a security page crosses the operator's
mute. Nothing downstream of pns reads it.

The installed Hermes webhook adapter ignores `Idempotency-Key`. Its delivery identifier is the first
of `X-GitHub-Delivery`, `svix-id`, `X-Request-ID`, and failing all three a millisecond timestamp
(`gateway/platforms/webhook.py`), and `_record_delivery_id` deduplicates on that. pns sends neither
of the first three, so today the transport's deduplication key is an incidental timestamp.

### What the installed Hermes offers

Installed pin `a4091e49f10ddceaac1a902848aabfb1b9aae210`, confirmed against
`git -C ~/.hermes/hermes-agent rev-parse HEAD` and `.chezmoidata/hermes.yaml`.

`kanban create` is a rich deterministic trigger: `--assignee`, `--workspace dir:<path>`,
`--idempotency-key` (documented as returning the existing non-archived task's id),
`--max-runtime <duration>`, `--max-retries N`, `--skill <name>` repeatable, `--body`,
`--initial-status` (default `running`), and `--json`. `--board <slug>` is a **global** flag on the
`kanban` parser, declared before the subparsers, so `kanban create --board x` is rejected;
`HERMES_KANBAN_BOARD` is the environment equivalent. `kanban attach` does not exist on this pin, so
`--workspace dir:` is the evidence transfer mechanism.

Docker contains tool execution, not the worker. The worker is a host subprocess; only terminal
commands enter the container through `docker exec`, and file tools route through the same environment
because `_get_file_ops` builds `ShellFileOperations(terminal_env)` over whatever backend is
configured (`tools/file_tools.py`). The container gets `--cap-drop ALL`,
`--security-opt no-new-privileges`, `--pids-limit 256` and size-limited `tmpfs` mounts.
`DockerEnvironment` has a `network: bool = True` parameter that appends `--network=none` when false
and **nothing ever passes it**; `terminal.docker_extra_args` is validated for string-ness and
appended last to the `docker run` argv, so `--network=none` supplied there does reach Docker. That
split is what makes a networkless container workable: the model call is on the host side of the
boundary.

Live config facts that bound the timing: `kanban.dispatch_in_gateway: true`,
`kanban.dispatch_interval_seconds: 60`, `kanban.failure_limit: 2`,
`kanban.max_in_progress_per_profile: null`, `kanban.dispatch_stale_timeout_seconds: 14400`,
`kanban.auto_decompose: true` (which only touches triage cards, and `--triage` is opt-in),
`model.default: gpt-5.6-sol`, `model.provider: openai-codex`,
`model.base_url: https://chatgpt.com/backend-api/codex`. Root `terminal.backend` is `local` and
`docker_extra_args` is empty, so a Docker-backed profile would be the machine's first. Three webhook
routes exist, all `deliver_only`: `priority`, `pns`, `unattended-upgrades`. **There is no `posture`
route**, which is a separate open ledger item and a prerequisite here.

`env = dict(os.environ)` in the worker spawn means the worker inherits the gateway's whole
environment, including whatever `~/.hermes/.env` gave it. The container does not see it; the worker
process does, and the worker process is what talks to the model provider.

## Three approaches to the publisher

The trigger, the evidence copy and the investigation itself are settled in shape by the first
document. What is genuinely open is where the advisory and the failure notice come from, because that
is where every advisory limit is enforced.

### A. Hermes lifecycle plugin hooks

A plugin this repository owns, deployed under the investigator profile's `plugins/` directory, hooks
`kanban_task_completed` to validate and publish and `kanban_task_blocked` to publish a failure
notice. This is the first document's stage 5.

Trade-offs. It uses a published extension point and needs no timer. But `kanban_task_blocked` does
not fire on a timeout, a crash, or a breaker trip, so the entire failure half is missing; a
timed-out investigation publishes nothing, which is exactly the all-clear-indistinguishable-from-
failure outcome the ledger forbids. `kanban_task_completed` fires in the worker process, so the
publisher dies with the process it reports on, and a SIGKILL leaves no notice. Hook returns are
ignored and hook exceptions are swallowed by `_fire_kanban_lifecycle_hook`, so a publisher that
throws fails silently. The plugin also lives under a profile root that
`run_after_59-hermes-config-migrate` does not migrate. Rejected.

### B. The gateway's terminal-event notifier

`kanban notify-subscribe` at card creation. The gateway delivers all five terminal kinds from its own
long-lived process, which survives a killed worker.

Trade-offs. It is the smallest change, needs no new code, and covers every failure state. But it
publishes the worker's own summary text on `completed`, and then uploads any host file whose absolute
path appears in that summary, filtered only by a credential denylist that misses `~/Documents`,
`~/workspaces`, `~/.local/state` and `~/.claude.json`. That is a raw completion publisher and an
evidence upload channel in one, both of which the ledger records as deliberately not enabled. Its
`timed_out` text also claims a retry that will not happen. Rejected for the advisory; it remains a
reasonable belt for the failure half if the operator accepts `gateway.strict: true` machine-wide, and
that variant is Open Question 3.

### C. A host-side reconciler outside both processes

One deterministic host process, owned by this repository, run on a timer by a LaunchAgent. Each tick
does three things: turn newly committed Critical alerts into cards, turn terminal cards into a
validated advisory or a failure notice, and turn overdue cards into a not-investigated notice.

Trade-offs. It is the only option where the publisher is outside both the worker process and the
gateway, so no state of either can prevent a notice: a killed worker, a crashed gateway and an
unparseable result all reach the same reconciler on its next tick. It closes the first document's one
silent failure state, because "the card never became terminal" is a condition the reconciler can
observe and the hooks cannot. It reads the corroborating facts from a directory the container never
had, which is what makes mitigator corroboration mean anything. It costs one script, one LaunchAgent,
one state directory, and a polling interval instead of an event. It also adds no new Hermes extension
surface, so a pin bump cannot silently stop the publisher.

**Recommendation: C, with the evidence collector added to posture rather than written in the
reconciler.** The dangerous part of the work is the bounded no-follow copy and the out-of-band
verification, and posture already owns that machinery: `posture enrich <path>` exists, `codesign.rs`
and `SystemInspection` exist, and the converge tool's refuse-rather-than-repair posture for
irregular filesystem entries is already written and already reviewed. A new `posture evidence`
subcommand produces evidence, which is the ledger's own description of a security producer's job, and
is not agent orchestration. The reconciler is then thin glue: read a ledger table, run two commands,
validate a small JSON document, submit a request.

## The recommended design

Five stages. The first is unchanged from today. Components are named so the behaviors below can be
written test-first.

```
posture alert ──submit──> pns ──POST──> hermes ──> Discord          (stage 1, unchanged)
      │
      └─ commits ledger_events row
                     │
   reconcile-security-investigations.sh (timer)                     (stages 3 and 5)
         ├─ posture evidence <occurrence-id> --into <dir>           (stage 2)
         ├─ hermes kanban --board security create ...               (stage 3)
         │         └─> dispatcher ──> worker ──> container          (stage 4)
         └─ validate result.json, then pns submit                   (stage 5)
```

### Stage 1: the deterministic page, unchanged

Nothing in stages 2 through 5 can delay, suppress, alter or gate it. The invariant from #24 section 2
survives intact and is the design's first behavior.

Behavior to pin: with the investigator profile absent, with Docker stopped, with the reconciler
script deleted and its LaunchAgent unloaded, a synthetic Critical finding still produces the
identical page, on the identical route, with the identical body bytes, and `posture alert` still
exits 0.

### Stage 2: `posture evidence`, a bounded no-follow copy

A new posture subcommand, `posture evidence <occurrence-id> --into <directory>`. It reads the results
log span the occurrence identity names, re-judges it to recover the same `PageFinding` rows the page
was built from, and writes:

```
<state-root>/investigations/<correlation-id>/
  workspace/            the card's --workspace dir:, bind mounted read-write into the container
    finding.json        the structured rows, schema-versioned, no rendered prose
    artifacts/          the resolved paths, copied, one regular file each
    facts.json          the out-of-band verification results
  record/               never reachable from the container
    facts.json          the corroboration copy
    card                the card id returned by `kanban create --json`
    state               created | advised | notified
```

The split into `workspace/` and `record/` is the correction to the first document's "a sibling record
the workspace cannot reach". The card's workspace is `workspace/`, so `record/` is outside the bind
mount by construction rather than by convention.

Boundaries. It copies regular files only and refuses symlinks, directories, devices, sockets and
FIFOs rather than following or repairing them, matching `osquery-converge`'s existing posture. It caps
per-file bytes and total bytes. It reads only paths the finding named, resolved from
`PageFinding::enrichment_path`, never from a name a model chose.

`facts.json` is the out-of-band mitigator set that #24 section 6 permits, and nothing else:
`codesign --verify --strict` result, `spctl` assessment, quarantine extended attribute presence, and
allowlist membership by hash or label. These are the only facts that may lower concern, and they come
from the host, never from the artifact's self-description.

Failure mode: a collector that cannot complete still writes `finding.json` plus an explicit
`collection_incomplete` reason, and the reconciler proceeds to a failure notice rather than to a
silent all-clear.

Behaviors to pin:

- a symlink at a named artifact path is refused and recorded, not followed
- a file over the per-file cap is refused and recorded, not truncated into `artifacts/`
- the total cap stops copying and records `collection_incomplete`
- `record/facts.json` and `workspace/facts.json` are byte-identical at write time
- a path not named by any finding in the batch is never opened
- the occurrence identity round-trips: the same span produces the same `<correlation-id>`

### Stage 3: the trigger

The reconciler's first tick action. Its input is pns's ledger, read read-only:

```sql
SELECT request_id, producer_request FROM ledger_events
WHERE producer = 'posture' AND seq > :cursor ORDER BY seq
```

and it keeps `:cursor` in its own state file. A row exists only after `pns submit` committed the
page, so the trigger cannot run before the page is committed. It selects rows whose
`producer_request` JSON has `event == "alert"`, which is the Critical page event; `cursor-reset` and
`notification-omitted` are not investigated.

For each selected row it runs stage 2, then:

```
hermes kanban --board security create "<correlation-id>" \
  --assignee <investigator-profile> \
  --workspace dir:<state-root>/investigations/<correlation-id>/workspace \
  --idempotency-key <correlation-id> \
  --max-runtime 10m \
  --max-retries 1 \
  --initial-status running \
  --json
```

`--board` precedes the subcommand because it is a global flag. `--max-retries 1` trips the breaker on
the first failure, so a timeout goes `running -> ready -> blocked` with a `gave_up` event in one pass
rather than looping. The returned card id is written to `record/card`.

The card body carries no evidence and no finding text. It names the workspace layout and the result
contract, and nothing else, so that the card body is not a second place attacker-influenced text can
reach the model.

Behaviors to pin:

- a committed `alert` row produces exactly one card, and a second tick over the same row produces
  none
- a `cursor-reset` row and a `notification-omitted` row produce no card
- a row whose `producer_request` will not parse is recorded and skipped, and the cursor still advances
- stage 2 failing still produces a card (so the investigation records the incomplete collection) but
  never produces an advisory
- the cursor advances only after the card id is durably written, so a crash between the two repeats
  the `create` and the idempotency key absorbs it

### Stage 4: the investigation

The card's worker runs under a dedicated profile whose own `config.yaml` sets:

| Key (under `terminal` unless noted) | Value | Why |
| --- | --- | --- |
| `backend` | `docker` | the execution boundary |
| `container_persistent` | `false` | `/workspace` and `/root` become `tmpfs` |
| `docker_persist_across_processes` | `false` | no reuse of a labeled container |
| `docker_mount_cwd_to_workspace` | `true` | the evidence workspace becomes `/workspace` |
| `docker_extra_args` | `["--network=none"]` | the only supported route to no network |
| `docker_volumes` | `[]` | no operator-declared mounts |
| `docker_forward_env`, `docker_env` | empty | no host environment crosses in |
| `docker_run_as_host_user` | `false` | no host uid inside |
| `lifetime_seconds`, `timeout` | short, bounded | a hung command frees the card |
| `toolsets` (top level) | the measured minimum | see below |
| `skills` (top level) | none | see below |
| agent dangerous-command auto-approval | off | never approve in here |

`docker_mount_cwd_to_workspace` works because the dispatcher pins `TERMINAL_CWD` to the card's
workspace when it is an absolute existing directory; `terminal_tool.py` then promotes it to
`host_cwd` and the Docker environment bind mounts it onto `/workspace`. The mount is read-write,
which the result file needs and which stage 5 therefore does not trust.

Three rows need a sentence. The **toolset** must be measured at build time; `terminal`, `file` and
`kanban` is the starting hypothesis, and `kanban` is present only because the worker completes its
own card. Any tool that reaches the host directly rather than through the terminal environment
belongs out of that list, and measuring which those are is a build task rather than a design claim.
The **skills** constraint is the enforcement point for the automatic mounts: with `HERMES_HOME` at
the profile root, the credential, skills and cache mounts resolve under that root, so a profile
directory holding no `skills/` and no `cache/` produces none of them. The profile therefore gets no
`private_skills` directory in the chezmoi source, which is a directory that must stay absent rather
than a setting that can drift.

The worker's job is #24 section 6's job unchanged: the finding is already a confirmed Critical, so
the question is never "is this real". It may add concern, explain, or recommend from a fixed
vocabulary. It may not issue a de-escalating verdict. Mitigating context comes only from
`facts.json`.

Its output is not prose. It writes `workspace/result.json`:

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

Three closed vocabularies:

| Field | Permitted values |
| --- | --- |
| `concern` | `raise`, `same` |
| `mitigators` | `allowlisted`, `signature-verified`, `no-quarantine-xattr` |
| `recommended` | `quarantine-file`, `disable-launch-item`, `revert-setting` |

`concern` has no lowering value, which is how the monotonic rule stops being a prompt instruction and
becomes a parse error. `recommended` is #24 section 6's fixed remediation vocabulary. There is no
command field, no path the model chose, and no free text outside `explanation`.

The worker completes its card with a fixed literal summary (the correlation identity and nothing
else). That is belt rather than enforcement, since nothing in this design reads the summary, and it
is the one place the design relies on the prompt; it is stated as such and it carries no limit of its
own.

### Stage 5: validation and publication

The reconciler's second tick action. For each investigation whose `record/state` is `created`, it
reads the card's status and latest run outcome from the board
(`hermes kanban --board security show <card> --json`, and `runs <card> --json` for the outcome), and
branches:

**Card is `done`.** Read `workspace/result.json` and validate:

1. it parses as one JSON object and `schema` is a version this reconciler knows
2. `correlation_id` equals the directory's own correlation identity, and that identity names a
   `ledger_events` row this machine committed
3. `concern` is in the closed set; `mitigators` and `recommended` are subsets of theirs
4. every claimed mitigator is corroborated by `record/facts.json`
5. `explanation` is within its length cap after control characters are stripped

All five pass: publish the advisory. Any one fails: publish a failure notice naming which check
failed, and quote the offending value. Write `record/state`.

**The corroborating facts are read from `record/`, never from the workspace.** The workspace is bind
mounted read-write because the result file has to be written there, so everything else in it is
writable from inside the container. Corroborating a claim against a file the claimant could edit
corroborates nothing.

**Card is `blocked`, or its latest run outcome is `timed_out`, `gave_up` or `crashed`.** Publish a
failure notice naming the outcome. This is the branch that approach A cannot reach.

**Card is still `ready` or `running` past its deadline.** Publish a not-investigated notice. The
deadline is the card's creation time plus a bound derived from `--max-runtime` and the dispatcher
interval. This is the branch that covers a stopped gateway, and it closes the first document's one
silent state.

**Any branch that published a notice or an advisory also reaps the container**, by
`docker rm -f` on containers labeled `hermes-task-id=<card>` and `hermes-agent=1`, because Hermes's
own reaper skips running containers and a SIGKILLed worker leaves one.

The advisory is published through the producer-agnostic alert contract, as a second `pns submit`:

- `producer` names the investigation workflow, not `posture`, so the ledger's
  `UNIQUE(producer, request_id)` cannot collide with the page's row
- `request_id` is derived from the correlation identity under a distinct prefix, so page and advisory
  are two distinct delivery attempts under one correlation key, which is what the ledger's
  deduplication sentence asks for and what keeps them from collapsing if a future Hermes starts
  honoring `Idempotency-Key`
- `signal` is `Observation` and `class` is unset, so the advisory does not bypass the operator's mute
  the way a security page does
- `detail` opens with an unmissable untrusted-commentary label, carries the correlation identity as
  text because a Discord reply is not reachable on this pin, and then the capped `explanation` and
  the two vocabulary lists rendered by the reconciler from the validated fields
- `extensions` carries the correlation identity and the validated vocabulary values as structured
  data, for whatever reads the ledger later

Behaviors to pin:

- a `result.json` claiming `signature-verified` where `record/facts.json` says otherwise publishes a
  failure notice, and the notice quotes the claim
- a `result.json` with a `concern` value outside the closed set publishes a failure notice
- a `result.json` whose `correlation_id` names a different investigation publishes a failure notice
- an absent, empty, or unparseable `result.json` on a `done` card publishes a failure notice
- a `timed_out` run publishes a failure notice naming the timeout
- a card still `ready` past its deadline publishes a not-investigated notice
- an `explanation` containing newlines, control characters, or Discord markdown publishes as inert
  display text within the cap
- a workspace whose `facts.json` was edited after the copy does not change the corroboration result
- each investigation publishes exactly once: a second tick over an `advised` or `notified` record
  publishes nothing
- the advisory's `request_id` differs from the page's and its `signal` is `Observation`

### Where the reconciler lives

`~/.local/libexec/reconcile-security-investigations.sh`, source
`dot_local/libexec/executable_reconcile-security-investigations.sh`, driven by a chezmoi-tracked
`Library/LaunchAgents/com.webdavis.security-investigation-reconcile.plist` with a matching
`run_onchange_after_*` loader. It is invoked by launchd, so it belongs under `libexec` and not `bin`.
It stays a flat file until it has private helpers, per the second libexec rule, and it is verb-first
because a bare noun would not say what happens.

Bash rather than a fifth Rust workspace, because after `posture evidence` takes the file handling the
remaining work is reading a SQLite table, running two commands, validating a six-field JSON document
and building one request, all of which `sqlite3` and `jq` do directly, and because a fifth cargo
workspace is a shippable-product shape for glue that is specific to this machine. The alternative is
recorded in Assumptions.

## Every advisory limit and its enforcement point

This table is the first of the two deliverables. Each row names the limit, the mechanism that
enforces it, where that mechanism lives, and what makes a violation visible.

| Limit | Enforced by | Where | Violation surfaces as |
| --- | --- | --- | --- |
| cannot clear or downgrade the finding | `concern` has no lowering value in the closed set | reconciler stage 5 check 3 | failure notice |
| cannot invent mitigating context | every mitigator corroborated against `record/facts.json` | reconciler stage 5 check 4, over a file outside the bind mount | failure notice quoting the claim |
| cannot recommend outside the vocabulary | `recommended` is a subset of the closed set | reconciler stage 5 check 3 | failure notice |
| cannot remediate | no command field exists in the result schema; no tool in the profile toolset acts on the host | result schema plus profile `toolsets` | nothing to execute |
| cannot grant trust | trust is `CodeTrust` computed by posture's enricher before the page; no advisory field feeds it | posture, upstream of the card | not expressible |
| cannot emit unbounded prose | `explanation` length cap plus control-character stripping | reconciler stage 5 check 5 | truncated and inert, or failure notice |
| cannot address the wrong alert | `correlation_id` equality against the directory identity and a committed ledger row | reconciler stage 5 check 2 | failure notice |
| cannot choose what evidence it sees | paths come from `PageFinding::enrichment_path` | `posture evidence` | path never opened |
| cannot read the host filesystem | file tools route through `ShellFileOperations` over the docker environment | profile `backend: docker` | nothing outside the mounts is readable |
| cannot reach the network from execution | `--network=none` through `docker_extra_args` | profile config | no route out of the container |
| cannot receive host credentials in the container | the profile root holds no skills directory and no registered credential files | absence of `private_skills` in the chezmoi source for that profile | no credential mount is emitted |
| cannot upload host files to Discord | no notify subscription exists for the card | reconciler is the only publisher | no upload path |
| cannot run forever | `--max-runtime 10m` plus `--max-retries 1` | `kanban create` argv | `timed_out` then `gave_up`, then a failure notice |
| cannot escalate past the page's urgency | `signal: Observation` with no `class`, so no silence bypass | reconciler's submitted request | advisory obeys the mute |
| cannot publish twice | `record/state` transitions once | reconciler | second tick is a no-op |
| cannot fail silently | three terminal branches plus a deadline branch, all publishing | reconciler stage 5 | a notice on every path |

Two limits are deliberately not in the table because they are not enforced anywhere, and saying so is
the point. The worker's completion summary is shaped by the prompt alone; nothing reads it, which is
why that is acceptable. And the fixed literal the worker is asked to use is a convention, not a
control.

## The proof that the original alert is untouchable

This is the second deliverable. Four properties, each anchored in a measured fact, covering the four
verbs in the ledger bullet.

**Cannot delay.** `posture alert` submits synchronously and its own state machine gates on the
answer: `Submission::Accepted` requires the correlated `request_id`, `Status::Accepted` and the
`ledger_committed` diagnostic, and only then does the cursor advance. The investigation's trigger
input is a committed `ledger_events` row, which cannot exist before that. There is no code path in
which any stage after 1 runs before the page is committed, because the trigger's precondition is the
commit itself. `PRODUCER_BUDGET` is five seconds per producer command, so the page's own submission is
bounded independently of everything here.

**Cannot suppress.** Suppression would need either a write to pns's ledger or a delete on Discord.
The container has no network and no host filesystem beyond the workspace and the read-only mounts, so
no process the model directs can reach `~/.local/state/pns/pns.db` or run the `pns` binary. The
worker process could in principle, being a host process, but it acts only through its tool list, and
the terminal and file tools route into the container. That makes the profile's `toolsets` the
enforcement point, and it is why the toolset must be measured rather than assumed. Deleting a
delivered Discord message needs a messaging tool and a gateway adapter; the investigator profile has
neither.

**Cannot rewrite.** The page and the advisory are two separate ledger events with different producers
and different request identities, and `ledger_events` is `UNIQUE(producer, request_id)`. An advisory
cannot occupy the page's row, and a failed advisory leg cannot dead-letter or retry the page's leg,
because legs belong to events. Nothing in the advisory path reads or edits the page's `detail`.

**Cannot clear.** Clearing is not expressible: `concern` has two values and neither lowers, and the
check that enforces that runs in a process the model never touches, over a corroboration file that
was written before the container existed and lives outside its only writable mount. A compromised
investigator's best outcome is a failure notice, which is louder than silence and is textually
distinct from an advisory.

The residual is stated plainly, and it is the research report's point rather than a gap in this
design: a compromised investigator can still **withhold concern it should have added**. The page
already said everything deterministic code knows, so the loss is the marginal explanation, not the
alert. That is the bounded middle path the report's recommendation 2 describes, and it is the reason
the model's output is display text on a side channel rather than an input to any decision.

## Failure modes

| Failure | Behavior | Distinguishable from an all-clear |
| --- | --- | --- |
| `posture evidence` refuses an artifact | card carries `collection_incomplete`; advisory still validated | yes, named in the notice or advisory |
| `posture evidence` cannot run at all | no card; reconciler records and reports | yes, reconciler's own report |
| Docker daemon down | worker fails, breaker trips at one, `gave_up` | yes, failure notice |
| gateway down, dispatcher never ticks | card stays `ready` past its deadline | yes, not-investigated notice |
| `--max-runtime` exceeded | worker signalled, `timed_out` then `gave_up` | yes, failure notice |
| worker process crashes | `crashed` outcome recorded by the dispatcher | yes, failure notice |
| `result.json` absent or unparseable | validation failure | yes, failure notice |
| uncorroborated mitigator claim | validation failure | yes, and the claim is quoted |
| container left running after a SIGKILL | reconciler reaps by label on the same tick | yes, reaping is recorded |
| reconciler itself not running | no advisory and no notice for any alert | **no**, see Open Question 5 |
| `posture` route missing on the gateway | pns dead-letters a 404 as a permanent refusal | yes, through pns's delivery reporting |
| two alerts in one dispatcher interval | two cards, unbounded concurrency today | yes, but see Open Question 6 |

Exactly one row has no notice of its own, and it is the reconciler's own liveness. That is a strict
improvement on the first document, where the gateway being down was silent; it is now the last
remaining silence, and it is a question about watchdog coverage rather than about this workflow.

## Security

Beyond the two tables above, three exposures are named rather than mitigated, because mitigating them
is an operator decision.

**Credential forwarding to the worker process.** `env = dict(os.environ)` on a gateway-embedded
dispatcher hands the worker the gateway's environment. The container does not see it; the worker
process does, and the worker process is what talks to the model provider. Nothing in the installed
Hermes narrows this, and narrowing it by patching Hermes is forbidden. The available lever is what
the gateway's own environment contains.

**Model-provider disclosure.** The fact sheet, or the artifact bytes if the operator chooses them,
goes to `https://chatgpt.com/backend-api/codex`. The research report's recommendation 3 says the
model must never receive raw attacker-controlled fields as instructions-eligible context, which is
why this design makes the host-produced fact sheet the decision rather than a default.

**Amplified false positives.** The worker treats every Critical as ground truth, so a false-positive
Critical is explained rather than calmed. #24 section 10's dependency on alert precision still holds,
and `facts.json` is the only safe relief valve.

One mechanism deliberately not used deserves recording: `gateway.strict: true` would narrow media
delivery machine-wide and make approach B's notifier tolerable. It is not proposed here because it
changes behavior for every other profile on the machine to buy a mechanism this design does not need.

## Out of scope

- Investigating anything below Critical. The digest is explicitly not a trigger.
- The approval interface. Tap-to-approve and the `/osquery allow|deny|list` skill are a separate
  ledger item, and investigation must not grant the analyst approval authority.
- Remediation of any kind. The advisory recommends; nothing acts.
- A Discord threaded reply. The webhook adapter's `_deliver_cross_platform` never passes `reply_to`,
  so association is textual on this pin.
- The missing `posture` route and the route checker's coverage of it. A prerequisite, tracked
  separately.
- Per-run grouping of repeated findings, signature-chain verification in the Rust paths, and
  interpreter-payload assessment. Separate ledger items; the first would change what a correlation
  identity covers, which is why this design's identity is the alert's and not a finding's.
- Kernel-extension install-state monitoring and off-host machine-death detection. Homelab scope.
- Any Hermes source change, including a patch that would pass `network=False` rather than reaching it
  through `docker_extra_args`.
- Making the correlation identity stable across a retry that reads more rows. It is a real defect
  (change 5 above) but it belongs to posture's cursor contract, not to this workflow.

## Assumptions made in the operator's place

1. **The publisher is a host-side reconciler on a timer, not a Hermes plugin and not a notify
   subscription.** Alternatives: approach A, which is event-driven and needs no LaunchAgent but
   cannot publish a timeout notice at all; approach B, which needs no new code but publishes model
   text and uploads host files. Displaced by the measured hook and notifier behavior.
2. **The evidence collector is a new `posture evidence` subcommand rather than code in the
   reconciler.** Alternative: do the copying and the `codesign`/`spctl` calls in the reconciler
   script. Displaced because posture already owns the refusal posture, the enricher and the signing
   inspection, and because bash is the wrong language for a bounded no-follow copy.
3. **The reconciler is bash under `libexec`, not a fifth cargo workspace.** Alternative: a Rust tool
   installed to `~/.cargo/bin` beside the other four. Displaced because the remaining work after
   assumption 2 is a SQLite read, two command invocations and a six-field validation, and because the
   four Rust tools are products other people install while this is glue for one machine. This is the
   weakest of the three shape assumptions and the one most likely to be overruled, particularly if the
   validator grows.
4. **The model reads the host-produced fact sheet, not the artifact bytes.** Alternative: the bytes,
   which is what #24 wanted and is more useful. Displaced by the research report's recommendation 3,
   which the first document had not weighed. Reversible in one configuration line.
5. **The advisory is `signal: Observation` with no `class`,** so it obeys the operator's mute.
   Alternative: `NeedsAttention` with `class: security`, which would bypass the mute like the page.
   Displaced because the page already woke them and a second wake for commentary is worse than late
   commentary.
6. **One card per alert batch, not per finding.** Alternative: one card per Critical finding, matching
   #24's per-artifact framing. Displaced because the alert's identity is a byte range over a batch, so
   per-finding cards need a new identity scheme and would fan out up to eight investigations per page.
7. **A dedicated board named `security`.** Alternative: the default board. Displaced so concurrency
   and retention can be tuned without touching the operator's working board.
8. **The profile is named for its function.** Alternative: a character name, which is this machine's
   actual convention (`butters`, `concerned`, `elaine`, `nicodemus`) and what #24's plan used
   (`mouse`). Displaced by the self-documenting-names and user-agnostic-names rulings, but the local
   convention may simply win.
9. **`--max-runtime 10m`, `--max-retries 1`, a not-investigated deadline derived from those, and the
   evidence caps.** Alternative: any other numbers. Every one of them is a guess and none is
   measured. The retry ceiling is the only one with a reasoned basis: one timeout should be terminal
   rather than looping a failing investigation.
10. **The worker completes with a fixed literal summary.** Alternative: let it summarize freely, since
    nothing reads the summary. Displaced because a free summary is exactly the text approach B would
    have published, and leaving it unconstrained invites someone to wire a subscription later.

## Open questions

1. **Is the reconciler bash or Rust?** Assumption 3, stated so it can be overruled. The answer changes
   the pull-request ladder but not the design.
2. **Does `posture evidence` belong in posture at all?** It is a producer feature by the ledger's own
   description of source-specific facts, but it exists only to feed an investigation, and the operator
   ruled that no agent orchestration goes in posture. This document reads producing evidence as not
   orchestration. If that reading is wrong, the collector moves to the reconciler and assumption 3
   stops being optional.
3. **Is a notify subscription wanted as a belt for the failure half?** It would deliver `timed_out`,
   `gave_up`, `crashed` and `blocked` from the gateway even if the reconciler is down. The price is
   `completed` also delivering the worker's summary and uploading paths in it, unless
   `gateway.strict: true` is set machine-wide, which changes media delivery for every other profile.
4. **What does the gateway's own environment contain?** The worker inherits it wholesale, and that is
   the credential surface the container cannot close. Acceptable as it stands, or narrowed first?
5. **Who watches the reconciler?** It is the one remaining silent failure. posture's watchdog already
   probes gateway health and has `GatewayProbe` and a `WatchdogIntegrity` notion, so extending it is
   the cheap answer, but it needs saying rather than assuming.
6. **Should `kanban.max_in_progress_per_profile` be set for this profile?** It is `null` today, so
   concurrency is unbounded. A burst of Critical alerts would start a Docker container per card.
7. **Does the alert wire gain `extensions` data now or later?** Adding `severity` and an evidence
   reference to posture's request is additive and reaches pns's ledger, which would let the trigger
   read "Critical" and the evidence location instead of inferring the first from producer plus event
   name and deriving the second from a digest. It is one workspace's change plus its golden fixtures,
   and it is not required for this design to work.
8. **Is the advisory's Discord destination the same channel as the page?** A named route selects one
   destination rather than broadcasting, so this is a route decision. Same channel is adjacent and
   readable; a separate channel keeps the page's channel calm.
9. **Should `run_after_59-hermes-config-migrate` cover profile configs?** It migrates the active
   `HERMES_HOME`'s `config.yaml`, which is the root one. The investigator profile's config carries the
   whole execution boundary and nothing migrates it across a pin bump. Carried forward from the first
   document, now with more at stake in that file.
10. **Is the profile a character name or a function name?** Assumption 8, carried forward.
