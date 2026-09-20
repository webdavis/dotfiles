# Where osquery approval authority lives

Status: design, written 2026-09-14 with the operator asleep. Nothing built, nothing changed. Every
choice made in the operator's place is listed under "Assumptions" with its alternative, and the
questions that must be answered before implementation are at the end.

## What the ledger asks

From `docs/remaining-work.md`, under "Recover the remaining design from PR #24":

> Reconcile the approval interface separately: Butters tap-to-approve scoped to pending findings and
> the `/osquery allow|deny|list` Hermes skill. Verify the current posture command and trust
> contracts; investigation must not grant the analyst approval authority.

The triage object adds the operator's half ("Approve the trust boundary between investigation and
approval") and the completion test: *a written scope names where approval authority lives, verified
against the posture commands that exist today.*

The rest of that section is the recovered security-investigation work: posture produces findings,
Hermes owns the downstream investigation and the advisory reply, Critical alerts only, original alert
first and the investigation published separately. This document deliberately does not design the
investigator. It designs the fence between the investigator and the one file that can silence the
detector, because that fence has to exist before the investigator does.

## Constraints that bind this design

From `CLAUDE.md`, the runbooks and the recorded operator rulings:

1. **The operator runs applies. Agents do not.** Any interface that needs an unlocked KeePassXC
   password vault or an interactive terminal is an operator interface, not an automated one.
2. **No workspace may depend on another.** posture may spawn the deployed `pns` binary at runtime,
   and posture already does, but no build-time dependency may cross between `posture/`, `pns/`,
   `uu/` and `lights/`.
3. **We test the behavior of tools we wrote, and nothing else** (ruling 2026-08-05). Declarations go
   unguarded on purpose. A test that asserts a Hermes config key exists would be deleted on sight.
4. **Rust files target 300 lines and never exceed 500**, unit tests included (ruling 2026-09-02).
5. **No removal mechanisms** (ruling 2026-08-02): no `.chezmoiremove`, no retirement scripts.
6. **Defaults visible in config** (ruling 2026-08-31), **opt-in features ship commented**, and a
   secret-bearing key in `dot_config/pns/config-values.toml` names a vault entry, never a value.
7. **Optimal over cheap** (ruling 2026-09-05) and **no manual-intervention designs**: the first
   workable answer is not automatically the right one, and a design whose failure mode is "the
   operator remembers to do the second step" is not finished.
8. **Hermes is third-party code and is not modified.** The operator's 2026-09-12 clarification says
   this workflow is configured through dotfiles, without embedding agent orchestration in posture.
9. **Reviews and pull requests stay small** (ruling 2026-08-10).

## What is actually built today

Everything in this section was run or read on this host on 2026-09-14, not recalled. The PR #24 plan
predates the Rust port, the pns engine and the current Hermes install by three months, and most of
its architecture no longer describes anything that exists.

### The writer exists, and it is `posture allowlist`

`posture` ships three allowlist verbs today, and the deployed binary
(`~/.cargo/bin/posture`, built 2026-09-13) prints them in its own usage:

```
allowlist add <label> | allowlist deny <label> | allowlist list
```

What each one does, read from `posture/crates/posture-application/src/allowlist.rs` and its adapters:

- **`add <label>`** takes an exclusive write lock, validates the label's charset
  (`valid_allowlist_label`), then queries the live `launchd` table through `osqueryi` for that
  label's `path` and `program`. **A label with no currently installed agent is refused**
  (`CaptureRefusal::NoAgent`). It hashes the plist with SHA-256, refusing anything that is not a
  regular file, relativizes `$HOME` to `~/`, writes the tuple into the chezmoi **source** file, and
  then publishes: `chezmoi apply --force <deployed allowlist>` followed by
  `/bin/bash .chezmoiscripts/run_after_05-osquery-known-good-manifests.sh`. A failed apply rolls the
  source back and says whether the rollback itself failed.
- **`deny <label>`** removes an existing entry from the source and republishes. It is an **undo of a
  previous allow**, not a verdict on a finding: with no matching entry it answers
  `CurationOutcome::NotPresent` and changes nothing.
- **`list`** prints the **deployed** file, one JavaScript Object Notation (JSON) object per line,
  comments and blanks dropped. Nine entries today, 1750 bytes.

So the security boundary PR #24 describes ("the fail-closed script is the boundary, not the agent and
not the bot") does exist, in a stronger form than PR #24 planned, and it is a Rust command rather
than `osquery-allowlist.sh -a`.

### The 2026-07-26 boundary design shipped

`docs/superpowers/specs/2026-07-26-osquery-allowlist-boundary-design.md` recommended option B
(manifest the allowlist, route the writer through chezmoi) with option D-prime (the verdict refuses to
suppress when the deployed allowlist is not the manifest's tuple). Both are in place:

```
$ sudo -n grep page-launchd-allowlist /var/osquery/pipeline-known-good.sha256
c305a2b6... 0600 501 /Users/stephen/.config/osquery/page-launchd-allowlist.txt
```

and `allowlist_verdict` in `posture/crates/posture-domain/src/allowlist.rs` spends a `vouch(list_path)`
call before it ever returns `Suppress`, plus a `vouch(finding.path)` for the empty-hash own-agent
convention. Option E (suppress own agents by manifest membership) was not done; the nine entries
still carry an empty `sha256`.

This matters because it fixes the shape of the remaining problem. The allowlist is no longer the
softest component in the pipeline. What is soft is **who may call the writer**.

### Nothing in the system has a concept of a pending finding

The ledger asks for tap-to-approve "scoped to pending findings". No such scope exists anywhere:

- `posture alert` reads `~/.local/log/osquery/osqueryd.results.log` from the offset in
  `~/.local/state/osquery-results-offset`, judges the batch, submits the page to pns and appends to
  the digest spool. It records nothing about what it asked.
- `~/.local/state/posture` and `~/.local/state/osquery` do not exist. The only state files are the
  poll baseline, the results offset, the watchdog state, the undelivered-alert database and the
  digest spool.
- The pns ledger (`~/.local/state/pns/pns.db`) has a `decisions` table, but it records the engine's
  own **routing** decisions (which destinations a page went to and why), five deep, not an operator's
  answer.

The closest thing to a scope that exists is the writer's own refusal to allowlist a label with no
installed agent. That is narrower than "anything" and wider than "a finding the detector reported".

### Butters is no longer a bot you can write buttons into

PR #24 planned Butters as a bespoke Python bot: a `uv` project under
`dot_local/share/osquery-approval-bot/`, `discord.py` persistent views through `DynamicItem`, a
launchd agent kept alive by `KeepAlive.PathState` on a sentinel file, kickstarted by the alerter, and
a pending store of JSON files under `~/.local/state/osquery-approval-bot/`.

Today `butters` is a **Hermes profile**: `private_dot_hermes/profiles/private_butters/` with an
age-encrypted `config.yaml` and five declared skill symlinks. Its live config names it "the browser,
GUI, and background computer-use profile", running `gpt-5.5` at `reasoning_effort: xhigh` with 150
turns. It is a large language model (LLM) agent with a terminal, not an approval surface. Handing
approval to "Butters" now means handing it to a model.

### The Discord surface a posture page lands on cannot carry buttons

pns delivers Discord through the local Hermes webhook gateway as one signed POST
(hash-based message authentication code, HMAC-SHA256, over the exact body bytes). The live route
table in `~/.hermes/config.yaml` declares three routes, and **all three are `deliver_only: true`**,
which Hermes itself describes as "direct delivery (no agent, zero LLM cost)". A `deliver_only` route
renders a prompt template and posts text. There is no component, view or button in that path, and no
supported interface lets a third party attach one: the button classes live inside
`plugins/platforms/discord/adapter.py` in `_define_discord_view_classes`, private to the adapter
process.

So "tap to approve in Discord" cannot be built on the channel posture's pages arrive in, without
modifying third-party code, which is forbidden.

### Hermes does have approve and deny buttons, and they are the wrong ones

`ExecApprovalView` in the Discord adapter renders **Allow Once / Allow Session / Always Allow /
Deny** and resolves through `tools/approval.py`'s gateway approval queue. Verified properties:

- It is **session-scoped and ephemeral**: `super().__init__(timeout=300)`, and on timeout the buttons
  disable with "Prompt expired, no action taken". It is not restart-safe and holds no durable pending
  set. PR #24's persistent-view requirement has no counterpart here.
- It fires only when a command **matches `DANGEROUS_PATTERNS`** in `tools/approval.py`. That list
  covers `rm -r`, `mkfs`, `dd if=`, shell `-c` invocations, `curl | sh`, writes to `/etc`, ssh and
  credential paths, `docker stop`, `hermes gateway restart`, `sudo` with a privilege flag, and about
  forty more. **`posture allowlist add <label>` matches none of them.** Neither does a plain
  `sudo posture ...`.
- `approvals.mode` is `manual` today with a 60 second timeout, so the auxiliary-LLM "smart approval"
  path that can auto-approve low-risk commands is off. It is one config key away from on.
- "Always Allow" writes a permanent entry into `command_allowlist` in `~/.hermes/config.yaml`, which
  already holds two entries. A blanket grant is one tap away from a phone.

### Every Hermes agent already holds approval authority, unprompted

This is the finding that reframes the task. Verified:

- `hermes tools list` reports `terminal` and `code_execution` **enabled**.
- `~/.hermes/config.yaml` sets `terminal.backend: local` with `auto_source_bashrc: true`, so commands
  run on the host as the operator with the managed shell's `PATH`, which puts `~/.cargo/bin` in
  reach.
- `posture allowlist add` matches no dangerous pattern, so no approval prompt fires.
- `sudo -n true` succeeds, so the manifest refresh the writer invokes needs no password.

Therefore any Hermes agent, on any profile, can today grant a suppression for any installed launchd
agent, silently, and the pipeline will bless the result: the writer refreshes the root-owned manifest
itself, so the file-integrity arm and the D-prime vouch both come out clean. The trust boundary the
ledger asks about is not undesigned. It is open, and the first job of this design is to close it
rather than to put a nicer button in front of it.

### The slash-command surface is not what PR #24 assumed

PR #24 planned `dot_hermes/skills/osquery/SKILL.md`, an agent-mediated skill whose body instructs the
model to run the writer. Two verified problems:

1. **Skills are not top-level slash commands.** The Discord adapter registers built-ins first, then
   commands from `COMMAND_REGISTRY`, then plugin commands, and finally registers **skills under one
   consolidated `/skill` command group** ("1 top-level slot instead of N"). There is no `/osquery`.
2. **A skill puts the model in the approval path**, which is exactly what the ledger forbids.

What does exist is a **plugin** extension point. `PluginContext.register_command(name, handler,
description, args_hint)` registers an in-session slash command with a **deterministic Python
handler** (`fn(raw_args: str) -> str | None`), and the Discord adapter mirrors plugin commands into
the native slash picker with an argument field. A plugin is two files (`plugin.yaml` and an
`__init__.py` exposing `register(ctx)`), it lives under `~/.hermes/plugins/<name>/`, and four are
installed on this host already (`herdr-agent-state`, `hermes-achievements`, `moshi-hooks`,
`ponytail`). None of the four is chezmoi-managed today. `resolve_command` rejects a plugin name that
collides with a built-in, and `/approve` and `/deny` are built-ins, which is a second reason the
three verbs belong under one `osquery` command rather than as top-level `/allow` and `/deny`.

Slash authorization is real but partial: `_evaluate_slash_authorization` mirrors the `on_message`
gates, and it is a deliberate no-op when no allowlist variable is set.
`private_dot_hermes/private_dot_env.tmpl` renders `DISCORD_ALLOWED_USERS` from the vault, so the user
gate is active. It does not render `DISCORD_ALLOWED_CHANNELS`, so **channel scoping is inactive**: the
allowed user can invoke a slash command from any channel the bot can see. The same template already
renders a `DISCORD_OSQUERY_CHANNEL` value from the vault entry `Discord (Uriel) :: Channel ID
(#osquery)`, a leftover of the PR #24 plan that nothing in this repository and nothing in Hermes
reads. So a dedicated channel exists and its id is already on disk; only the reader is missing.

### pns has an approval seam, declared and unimplemented

`pns.request/1` carries `signal.kind = "approval_requested"` and
`interaction = { "kind": "await_decision" }`, documented in `pns/crates/pns-protocol/src/request.rs`
as "the blocking approval: the submission does not return until the operator's decision arrives or
the bounded wait expires". `pns.result/1` carries `interaction: NoOpinion | Answered { code }`.
(Superseded 2026-09-19: `interaction` and `InteractionResult` were removed from `pns.result/1`; see
`pns/docs/specs/protocol-v1.md`.)

Nothing implements it. `pns/crates/pns/src/event_flow/submit.rs` answers every `AwaitDecision` with
`InteractionResult::NoOpinion`. posture's own copy of the wire
(`posture/crates/posture-pns-wire/src/request.rs`) carries both spellings, and only its tests use
them. So the protocol slot for a blocking approval exists on both sides of the wire, with no
collector behind it.

### One live defect sits directly in this path

posture submits with `route: "posture"` (`posture/crates/posture/src/poll.rs` and `alert.rs`, both
"the fixed posture route"). pns maps a request route to the event channel, and the Hermes destination
swaps the final path segment for it. Probing the live gateway the way `pns doctor` does, with an
unsigned POST, which answers 401 for a route that exists and 404 for one that does not:

```
posture         404
pns             401
priority        401
nonexistent-xyz 404
```

**The `posture` route does not exist on the gateway.** The legacy `priority` route does, and its
prompt template is still the old Bash alerter's `{alert.title}` / `{alert.detail}` shape; posture's
watchdog still uses `/webhooks/priority` as its gateway health probe. Under the delivery-failure
design (`2026-09-08-pns-delivery-failure-reporting-design.md`) a 404 is a permanent refusal. Whatever
this design puts on the Discord surface, that surface is not receiving posture's pages today.

## What survives from PR #24

| PR #24 decision | Disposition |
| --- | --- |
| One writer is the only security boundary, every caller goes through it | **Keep.** It is `posture allowlist` now. |
| The writer refuses malformed and system labels, fail-closed | **Keep.** Shipped, plus a live-agent and plist-hash check PR #24 did not plan. |
| `add` / `deny` / `list` as the three verbs | **Keep the verbs, rename the second.** `deny` today means "undo an allow", not "reject this finding". |
| Approval is scoped to pending findings | **Keep as a requirement. Nothing implements it.** |
| Buttons are the primary surface, typing is the fallback | **Reverse.** Buttons are unreachable on the delivery path; a deterministic slash command is reachable today. |
| Buttons are LLM-free, the skill is agent-mediated | **Keep the principle, reject the skill.** A plugin handler is LLM-free; a skill is not. |
| A bespoke `discord.py` bot under launchd `KeepAlive.PathState`, kickstarted by the alerter | **Reject.** A second Discord identity, a second Python runtime, a `uv` project, a sentinel file and a fifth daemon, to reach a surface Hermes already owns. |
| A new Butters bot token in KeePassXC | **Reject.** Butters is a Hermes profile with its own token already. |
| Owner plus channel authorization inside the bot | **Keep as a requirement**, satisfied by `DISCORD_ALLOWED_USERS` plus a channel gate that is not configured yet. |
| A pending store of JSON files under `~/.local/state/osquery-approval-bot/` | **Reject the location and the format**; keep the need for a pending set. |
| `flake.nix`, `bats`, `nix develop`, `osquery-allowlist.sh` | **Dead.** The flake is gone, bats is gone, the Bash writer is gone. |

## The trust question, stated precisely

Three parties want to touch one file:

1. **The detector** (`posture alert`, `posture poll`) reads the allowlist and decides whether a
   finding pages. It never writes.
2. **The investigator** (a Hermes workflow, not yet built) reads evidence about a Critical finding and
   publishes an advisory. The ledger says it must never write, and must never cause a write.
3. **The operator** decides. Only the operator may cause a write.

Party 2 and party 3 currently reach the writer through the same door, as the same user id, with the
same passwordless sudo. On this host there is no mechanism that distinguishes them by identity. So the
boundary has to be built out of something other than identity, and the honest options are: **what the
writer will accept**, **what the writer announces**, and **a secret the agent cannot read**.

## Approaches

Each is judged on: does it stop the investigator from causing a suppression, does it survive a
same-user-id caller, how much machinery it costs, what it does to the operator's ordinary path, and
whether it needs anything Hermes does not already offer.

### Approach 1: authority stays in `posture allowlist`, narrowed and self-announcing

The writer keeps the authority it already has, and gains two properties. First, **pending scope**: it
refuses a label that is not currently a finding the detector reported and that nobody has decided.
Second, **announcement**: every grant and every denial submits its own pns event, so a suppression
nobody remembers authorizing arrives on the operator's phone within seconds. Interfaces (an
interactive terminal, a Hermes plugin slash command) become thin callers holding no authority of their
own.

- **Stops the investigator?** It stops the general case. The investigator cannot pre-arm a label for
  an agent that is not yet installed and has not yet been reported, which is the attack that matters:
  allowlist first, install second, never page. It does not stop an investigator that waits for a real
  pending finding and approves it, so the announcement is load-bearing rather than decorative.
- **Same user id?** No mechanism here survives a caller that reimplements the writer. An attacker who
  can run code as the operator can write the chezmoi source, run a targeted apply and refresh the
  manifest, exactly as the writer does. What changes is that the easy path is narrow and loud, and
  the announcement rides the same engine as every other page.
- **Machinery?** Moderate and mostly derivation: a pending set computed from files that already
  exist, one new read verb, one refusal in the existing `add` path, one alert submission.
- **Ordinary path?** Better. The operator gets `posture allowlist pending` and a Discord command, and
  the writer stops accepting a typo for a label that was never reported.
- **Needs anything new from Hermes?** Only the plugin extension point, which exists.

### Approach 2: authority moves to pns as the decision broker

Implement `Interaction::AwaitDecision`. posture submits the persistence finding with
`signal.kind = approval_requested` and `interaction.kind = await_decision`; pns records a pending
decision in its ledger, delivers the page, waits the bounded window for an answer, and returns
`Answered { code }`; posture applies the allowlist change itself when the code says allow.

- **Stops the investigator?** Better than approach 1 in principle: the writer would only ever run as
  a continuation of a decision pns holds, so an out-of-band `posture allowlist add` could be refused
  outright. In practice the same user id reads and writes `pns.db`, so the refusal is a lock with the
  key beside it.
- **Same user id?** No.
- **Machinery?** Large, and in the wrong workspace. pns would need a decision store, a collection
  surface (there is none: the Discord leg is `deliver_only`, the phone leg is a one-way webhook to
  `api.getmoshi.app`), a bounded wait inside `submit`, and a blocking path through a daemon whose
  whole design is fire-and-forget leases. The seam being declared is not evidence that the collector
  is cheap; it is evidence that somebody already noticed it was expensive.
- **Ordinary path?** It would be the best of the three once built, because the page and the decision
  would be one object with one correlation id.
- **Needs anything new?** A collection surface that does not exist on either delivery leg.

### Approach 3: authority is Hermes's, the agent proposes and the operator taps

Let an agent propose `posture allowlist add <label>`, and lean on Hermes's exec-approval gate to make
the operator tap Approve in Discord or on the phone (moshi-hook relays `approval_required` and holds
a websocket for the round trip; `hermes` is one of its supported hook targets).

- **Stops the investigator?** **No, and it inverts the trust.** The gate only fires for a command
  matching `DANGEROUS_PATTERNS`, and the writer matches none, so the default outcome is no prompt at
  all. Adding a pattern to a third-party list is a modification this repo forbids. Even with a prompt,
  the **agent chooses the label** and the operator approves a rendered string on a phone. A
  prompt-injected investigator that proposes a plausible label gets a tap. "Always Allow" then writes
  a permanent grant into `~/.hermes/config.yaml`.
- **Same user id?** No, and worse: the surface teaches the operator to approve what an agent asked
  for.
- **Machinery?** Almost none, which is the trap.
- **Verdict: reject.** This is the shape the ledger's warning exists to forbid.

## Recommended design

**Do approach 1 now, as two small changes. Keep approach 2 as the stated direction for the day a
decision surface exists. Reject approach 3.**

The reasoning in three lines: the only thing that can constrain an approval regardless of caller is
the writer's own admission rule, and approach 1 is that rule; the pending scope removes the one attack
that is silent (pre-arming), and the announcement removes silence from the rest; and every interface
worth having is a thin caller of the same command, so adding a surface later is a client change rather
than a second authority.

### The pending set

**A label is pending when the detector has reported it and nobody has decided.** Concretely, derived
rather than stored:

- there is a `persistence_launchd` row in `~/.local/log/osquery/osqueryd.results.log` naming that
  label with an `added` action, and
- the label is not in the deployed allowlist, and
- the label still resolves in the live `launchd` table to a path and program with a readable plist
  (the same capture `add` already performs), and
- no decline marker exists for it.

Derivation over persistence, for three reasons. The results log is already the pipeline's record of
what was reported, so a second store could disagree with it. A stored pending file would be a fourth
piece of unmanifested state in `~/.local/state` that decides whether a suppression is allowed, which
is the exact shape the 2026-07-26 design spent itself removing. And a derived set cannot drift out of
date across a restart.

The one thing that must be stored is the **decline marker**, because "I looked at this and said no"
is not derivable from anything: an append-only file of declined labels with the instant and the
surface that declined. It grants nothing, so its integrity matters less than the allowlist's; it goes
beside the allowlist under `~/.config/osquery/`, is not a chezmoi target, and is not manifested.

How far back the log is read is a bounded window, not the whole file. See the open questions.

### Behaviors to pin, test-first

The list below is the decomposition: one behavior per red test, named by the sentence it proves.

**Domain (`posture-domain`), pure:**

1. A label reported `added` by `persistence_launchd`, absent from the allowlist, with no decline
   marker, is pending.
2. A label already in the allowlist is not pending.
3. A label with a decline marker is not pending.
4. A label reported only outside the window is not pending.
5. A label reported `removed` is not pending.
6. Two reports of the same label yield one pending entry, the most recent.
7. A results row that is not `persistence_launchd` never yields a pending entry.
8. An unreadable results log yields **no** pending entries and is distinguishable from an empty one,
   the way `Allowlist::Unreadable` already differs from `Allowlist::Read(&[])`.

**Application (`posture-application`):**

9. `add` on a pending label proceeds exactly as it does today.
10. `add` on a non-pending label is refused before the write lock is taken, with a refusal that names
    which of the four conditions failed.
11. `add` on a label whose plist hash no longer matches what was reported is refused (a label
    reported, then swapped, then approved).
12. `decline` on a pending label writes the marker, changes no allowlist entry, and answers that the
    finding will stop paging.
13. `decline` on a label that is not pending answers not-pending and writes nothing.
14. `deny` keeps today's meaning, undoing a previous allow, and is refused for a label that was never
    allowed (`NotPresent`, unchanged).
15. Every outcome that changed state submits one announcement; every refusal submits none.
16. An announcement that cannot be submitted does not roll back the allowlist change, and says so.

**Command (`posture`):**

17. `posture allowlist pending` prints the pending set, one object per line, and exits 0 with an
    empty set.
18. `posture allowlist decline <label>` exists and routes to behavior 12.
19. Usage text names all five verbs, and an unknown verb exits 2 (the existing contract).

### The refusal is where the scope lives

`add` gains one gate, placed **before** the write lock and before the launchd capture, so a refused
call touches nothing:

```
add <label> -> pending set -> label present? -> no  -> refuse, exit non-zero, nothing written
                                             -> yes -> today's path unchanged
```

`CurationFailure` gains one variant for a not-pending label. It must not reuse `InvalidLabel`: the
operator reading the refusal needs to know the difference between "that is not a legal label" and
"that label is legal but was never reported".

### The announcement

Every state change submits one pns request through the producer posture already owns
(`PnsProducer`, `pns submit --json`). Shape:

- `producer: "posture"`, `event: "allowlist_changed"`, `signal.kind: "needs_attention"`,
- `detail` naming the verb, the label, the program the tuple pins, and the surface that asked,
- `scope: automatic`, so the presence gate decides whether it reaches the phone.

The surface is passed by the caller, not sniffed: an argument the terminal path leaves unset and the
plugin sets to its own name. A sniffed surface is a value an agent can forge just as easily and reads
as stronger than it is.

This is detection, and the document should not dress it as prevention. Its value is that a grant
nobody authorized is on the operator's phone in seconds instead of in a daily digest, which is the
same upgrade the 2026-07-26 design bought for the file itself.

### The Discord interface

One Hermes plugin, chezmoi-managed, registering one command with three read-and-write verbs:

```
/osquery pending            -> posture allowlist pending
/osquery allow <label>      -> posture allowlist add <label>
/osquery decline <label>    -> posture allowlist decline <label>
/osquery list               -> posture allowlist list
/osquery remove <label>     -> posture allowlist deny <label>
```

Properties that make it an interface rather than an authority:

- The handler is **deterministic Python**, `fn(raw_args: str) -> str`. No model reads the argument and
  no model composes the command. It splits the first word as the verb, passes the rest as one
  argument without shell interpretation (`subprocess` with a list and `shell=False`), and returns the
  writer's own bytes.
- It **passes the label verbatim or refuses it**. It does not normalize, expand or guess. The writer
  validates; the plugin does not pre-validate, because two validators disagree eventually.
- It reports the writer's **exit status and output**, unedited. A refusal reads as a refusal.
- `list` output needs a compact rendering: 1750 bytes today of JSON lines, against Discord's 2000
  character message limit, leaves no room for a fence. Label plus program, one line each, with a
  count.
- The plugin holds **no secret** and opens no socket.

The name `osquery` is the ledger's, and it does not collide with a Hermes built-in. `allow` and
`decline` rather than `allow` and `deny` keeps the tap-verb distinct from the writer's existing
undo-verb; `remove` is the undo, spelled the way it reads.

Two configuration items that are prerequisites rather than parts of the plugin:

1. **`DISCORD_ALLOWED_CHANNELS` must be set**, or the slash gate is user-scoped only and the command
   answers from any channel the bot can see. `DISCORD_OSQUERY_CHANNEL` is already rendered from the
   vault and read by nothing, so the value is available.
2. **The `posture` route must exist on the gateway**, or the interface sits in a channel that never
   received the page it is answering.

### Blocking the investigator inside Hermes

The pending scope is the boundary that holds regardless of caller. Inside Hermes there is one
supported, dotfiles-declarable gate worth adding as defense in depth: a **`pre_tool_call` shell
hook**. Hermes reads a `hooks:` block from `~/.hermes/config.yaml`, prompts once for consent per
`(event, command)` pair into `~/.hermes/shell-hooks-allowlist.json`, pipes a JSON payload
(`{"hook_event_name": "pre_tool_call", "tool_name": "terminal", "tool_input": {"command": ...}}`) to
the script, and honors `{"decision": "block", "reason": ...}`. `hermes hooks list`, `test`, `doctor`
and `revoke` manage it; no hook is configured today.

A hook that blocks any terminal command naming the writer makes the agent path a refusal with a
sentence, which is far better than a silent success. It is **not** a boundary: the same agent can run
`hermes plugins disable`, edit the config, or write the consent allowlist, all as the same user id.
State it as a speed bump and a signal, never as the fence.

`~/.hermes/config.yaml` is on the age-encrypted track in this repo
(`private_dot_hermes/profiles/.../encrypted_private_config.yaml.age`, see `docs/runbooks/age-key.md`),
because Hermes rewrites it at runtime, so declaring the hook is an encrypted-source edit.

### Failure modes

| Failure | Behavior |
| --- | --- |
| Results log unreadable or absent | The pending set is empty and says it could not be read. `add` refuses. Fail closed: no log means no approvals, not all approvals. |
| Results log readable but the window holds nothing | Empty set, exit 0. Distinguishable in wording from the unreadable case. |
| Label pending, plist replaced between report and approval | Refused (behavior 11). The tuple the writer would pin is not the tuple that was reported. |
| Decline marker file unwritable | `decline` fails and says so. Nothing is allowlisted, and the finding keeps paging. |
| Announcement submission fails | The state change stands and the command's exit status reports the announcement failure separately. A suppression that happened must not be reported as not having happened. |
| `chezmoi apply` or the manifest refresh fails | Today's behavior, unchanged: source rollback, and a refusal that names whether the rollback itself failed. |
| Plugin invoked with an unknown verb | Usage text, non-zero, no writer invocation. |
| Plugin invoked while `posture` is not on `PATH` | One sentence naming the missing binary. No fallback to a shell. |
| Gateway route missing | The command still works; the page it answers never arrived. This is why the route is a prerequisite. |

### Security, stated honestly

In the style of the coverage map in the 2026-07-26 design.

- **A user-level attacker on a passwordless-sudo host is not closed by anything here, and cannot be.**
  The writer's admission rule, the decline marker and the announcement all run as the operator. An
  attacker who can run code as the operator can write the chezmoi source, apply that one target and
  refresh the root-owned manifest, which is what the writer does. The claim this design makes is
  narrower: **the cheap path is now narrow and loud.** Pre-arming a label for an agent that was never
  reported stops working, and any grant that does happen arrives on the phone.
- **An LLM agent with the terminal toolset is exactly that attacker**, without malice. That is the
  finding, not a hypothetical: `terminal` is enabled, the backend is local, and the writer matches no
  dangerous pattern. Until the pending scope lands, the investigator the recovered section describes
  must not be enabled, because enabling it adds a party that reads attacker-controlled evidence and
  can write the suppression file in the same session.
- **Source compromise is unchanged and remains open.** The chezmoi source for the allowlist is
  user-writable; an attacker who edits it and waits for a legitimate apply gets the tuple deployed and
  manifested. Recorded in the 2026-07-26 design and in the manifest runner's docblock.
- **An attacker who pins a hash is not stopped.** Any allowlist an attacker can influence is one they
  can extend. What changes is that the influence now also needs a reported finding.
- **The announcement can be starved.** A grant issued while the gateway is down is a leg in the
  ledger, not a message on the phone. The delivery-failure design covers that class; this design
  inherits it and adds nothing.
- **The decline marker is a denial-of-service surface, not an escalation one.** Writing a decline for
  a label an attacker wants ignored stops the **re-page** for it but grants no suppression: the
  allowlist is untouched, so the detector still pages the finding itself on its own terms. Worth
  pinning in a test rather than asserting.

## Out of scope

- The investigator itself: its execution and network boundary, permitted evidence, model-provider
  disclosure, credentials, and the constrained result validator. That is the rest of the recovered
  section and is blocked on its own operator decisions.
- Implementing `Interaction::AwaitDecision` in pns, and any blocking submission.
- Buttons, components or any interactive Discord surface. Unreachable without modifying Hermes.
- A second Discord identity, a second Python runtime, a `uv` project, a launchd sentinel, or a fifth
  daemon.
- Option E from the 2026-07-26 design (suppressing own agents by manifest membership and deleting the
  empty-hash convention). Still a good follow-on, still not this change.
- Moving the `posture` route onto the gateway, and retiring the orphaned `priority` route and its
  `{alert.title}` prompt. A prerequisite named here, fixed in its own change.
- Any `.chezmoiremove`, retirement script or removal mechanism for the PR #24 artifacts. Per the
  standing ruling, the dead `DISCORD_OSQUERY_CHANNEL` value is either given a reader or left alone.
- Multi-host and homelab. Off-host machine-death detection stays where the ledger already put it.

## Assumptions made in the operator's place

Each is a choice this document made because the operator is asleep, with the alternative it rejected.

1. **"Tap-to-approve" is reinterpreted as "one deterministic command from a phone", not as Discord
   buttons.** Buttons are not reachable on the delivery path without modifying third-party code.
   *Alternative:* build the PR #24 bot as a second Discord identity with its own token and daemon,
   accepting a fifth background job and a second Python runtime to get literal buttons.
2. **Butters is not the approval surface.** It is an LLM profile with a terminal; naming it as the
   approver puts a model in the path the ledger forbids. The interface is a plugin command that any
   authorized surface reaches, including Bob's channel. *Alternative:* keep Butters as the named
   surface and accept a model between the operator's intent and the writer.
3. **Approval authority stays in `posture allowlist`.** *Alternative:* move it into pns behind
   `AwaitDecision`, which is better once a decision surface exists and is a much larger change now.
4. **The pending set is derived, not stored.** *Alternative:* have `posture alert` write a pending
   file when it pages, which gives an exact record of what was asked at the cost of a fourth piece of
   unmanifested state that decides whether a suppression is allowed.
5. **`deny` keeps its current meaning (undo an allow) and the tap-verb is a new `decline`.**
   *Alternative:* redefine `deny` as the finding verdict and rename the undo, which reads better in
   the ledger's own words and breaks the meaning of a verb that already shipped.
6. **A decline marker exists and suppresses only the re-page.** *Alternative:* no marker at all, so a
   declined finding pages on every pass until the agent is removed, which is honest and noisy and
   trains dismissal.
7. **The Hermes plugin is chezmoi-managed under `private_dot_hermes/plugins/`.** *Alternative:* a
   separate public repository installed with `hermes plugins install`, matching how `ponytail` and the
   custom Neovim plugins are handled, at the cost of a second lifecycle for forty lines of glue.
8. **A `pre_tool_call` shell hook is defense in depth, not the boundary.** *Alternative:* treat the
   hook as sufficient and skip the writer's admission rule, which fails the moment an agent runs
   `hermes plugins disable` or edits its own consent allowlist.
9. **No new secret is introduced.** *Alternative:* gate the writer on a one-time code from KeePassXC,
   which an agent genuinely cannot read and which is the only real prevention available on this host,
   at the cost of a vault lookup per approval and six digits typed on a phone.
10. **The window for reading the results log is bounded and configurable, defaulting to the digest's
    own day.** *Alternative:* read the whole log, which never misses an old finding and grows slower
    every week.

## Open questions

1. **Does the operator accept that approval authority stays with `posture allowlist`, bounded by a
   pending scope and an announcement, rather than moving to pns?** This is the trust-boundary
   approval the triage object asks for, and everything below depends on it.
2. **Is detection enough, or is prevention required?** A grant still succeeds if an agent runs the
   writer against a genuinely pending finding; the operator hears about it within seconds. If that is
   not acceptable, the answer is assumption 9's one-time code from the vault, and it should be decided
   now rather than retrofitted.
3. **How far back does the pending window reach?** The digest's own day is the obvious default. A
   finding reported on Friday and approved on Monday would be refused under it, which is either a
   safety property or an annoyance depending on how the operator works.
4. **Should `decline` be a verb at all, or should a declined finding keep paging?** The marker is the
   only new state this design stores, and dropping it is the smaller design.
5. **Chezmoi-managed plugin directory, or its own repository?** Assumption 7 chose the former. The
   repo's own precedent (custom Neovim plugins, the two herdr plugins) points the other way, and this
   plugin is smaller than any of them.
6. **Is `DISCORD_ALLOWED_CHANNELS` wanted?** Setting it scopes the slash surface to one channel and
   changes the behavior of every other Hermes command at the same time.
7. **Does the `posture` gateway route get added, or does posture stop naming a route and post to the
   default `pns` route?** A verified 404 today, and a prerequisite for any Discord approval surface.
   The orphaned `priority` route and its `{alert.title}` prompt need a disposition in the same breath.
8. **Is the investigator gated on this change landing?** This document's position is yes: an agent
   that reads attacker-controlled evidence and can write the suppression file in the same session is
   the single worst configuration in the system, and it is the configuration that exists today.
