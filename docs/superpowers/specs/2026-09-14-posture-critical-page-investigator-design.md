# The sandboxed explainer for a posture critical page

Status: design, written 2026-09-14 (evening) with the operator asleep. Nothing built, nothing
changed. Every choice made in the operator's place is listed under "Assumptions" with the
alternative it displaced, and the questions only the operator can answer are the last section
before the build plan.

## What the operator decided

Recorded 2026-09-14 (evening), in the same breath as the ruling that pns gains a direct Discord
destination:

> Agentic (non `deliver_only`) hermes routes stay off for machine events; two opt-in candidates
> noted: uu failure triage and posture critical-page explanation.

and then: build the agent explanation of posture critical pages.

So this is one of the two named opt-ins, and it is an **explanation**, not an investigation. The
distinction is the whole design and it is stated once here: an explanation tells the operator what
the words on the page mean and where to look first. It does not verify anything on the machine, it
issues no verdict, it recommends no remediation, and it has no authority of any kind.

## Where this sits among the three designs written today

Three documents now describe agents touching posture findings, and they are not alternatives to
each other. Read in this order:

1. **`2026-09-14-osquery-approval-authority-design.md`** builds the fence. It answers where the
   authority to suppress a finding lives (`posture allowlist`, narrowed to a pending set and
   announcing every grant), and it deliberately did not design the investigator, because the fence
   has to exist before a party that reads attacker-controlled evidence does. **This document
   respects that fence absolutely**, and the section "The explanation carries no approval action"
   is where that is spelled out.
2. **`2026-09-14-hermes-critical-alert-investigation-design.md`** and its sibling
   **`2026-09-14-bounded-security-advisory-design.md`** build the investigation: a bounded evidence
   copy, a kanban card assigned to a Docker-backed profile with no network, a closed result schema,
   and a deterministic validator that publishes the advisory. That is a large, correct pipeline for
   a large, correct problem.
3. **This document** is the first rung of the same ladder: the cheapest thing that answers the
   operator's actual complaint, which is that a page at three in the morning says
   `New startup item` and a path, and says nothing about what that means.

The two are compatible. The explanation route can be retired the day the pipeline lands, or kept as
the fast lane in front of it. What must not happen is this route growing tools until it becomes a
worse copy of the pipeline. The rule that prevents that is in "The sandbox": the explanation route
has no tools, and the day it needs one is the day the pipeline is the right answer instead.

### The investigation design rejected exactly this approach, and why that reverses

`2026-09-14-hermes-critical-alert-investigation-design.md` lists "Approach A: agent webhook route"
and rejects it on four counts. The operator has now chosen it, so each count is answered or
accepted here rather than left standing.

| Rejection | Disposition |
| --- | --- |
| "runs in the **default** profile, whose `terminal.backend` is `local`" | **Answered.** The backend is irrelevant when the agent has no terminal tool. The sandbox is a measured empty toolset, not a container. |
| "no bounded workspace" | **Answered.** There is no workspace. The agent reads one message and writes one message. |
| "no per-route runtime cap, no retry ceiling" | **Partly answered.** A sender-side per-hour cap and one header fix remove duplicate runs; `agent.gateway_timeout: 1800` remains the only run ceiling and is loose. Accepted residual. |
| "the model's free text is delivered verbatim, so the advisory limits live only in the prompt" | **Accepted, and it is why this is safe.** Those limits exist because that design's output is an advisory carrying a concern verdict and remediation verbs. This design removes the verdict and the verbs. There is nothing left to validate because there is nothing left to abuse. |

**The reason this is safe where the advisory was not is that it says less.**

## Constraints that bind this design

1. **Hermes is third-party code and is not modified.** Every mechanism below is a config file, a
   supported route property, or a published extension point.
2. **The operator runs applies. Agents do not.** `~/.hermes/config.yaml` reaches the machine only
   through an operator apply.
3. **`~/.hermes/config.yaml` is owned through a chezmoi modify template** (ruling 2026-09-14,
   afternoon): `private_dot_hermes/modify_private_config.yaml.tmpl` declares the routes and their
   secrets, hermes keeps every other line, one secret per route and one channel id per route come
   from KeePassXC by entry name, and `${VAR}` is never an option because the gateway reads those
   values as literals.
4. **Routes are exactly six today:** `general`, `pns`, `pns-recap`, `posture`, `priority`, `uu`.
   Adding one means two KeePassXC entries and a block in the modify template, or one entry when the
   route reuses an existing channel.
5. **`priority` means machine health and security only** (ruling 2026-09-14, morning): posture
   critical pages, a failed unattended upgrade, a dead daemon. Agent events never go there.
6. **Tools are producer-agnostic** (ruling 2026-09-14, afternoon): posture must not learn that this
   feature exists, and agent interpretation is a hermes configuration concern.
7. **No removal mechanisms** (ruling 2026-08-02), **defaults visible in config** (2026-08-31),
   **opt-in features ship commented** (2026-08-31).
8. **We test the behavior of tools we wrote, and nothing else** (2026-08-05). A test asserting that
   a hermes config key exists would be deleted on sight.
9. **Optimal over cheap** (2026-09-05) and **no manual-intervention designs**: a design whose
   failure mode is "the operator remembers the second step" is not finished.

## What is actually built today

Everything in this section was read or run on this host on 2026-09-14. Nothing is recalled.

### The page, and what an attacker chooses inside it

`posture/crates/posture-domain/src/page.rs`: `BLOCK_LIMIT` is 8 findings, `BODY_LIMIT` is 1900
characters, and only `Severity::Critical` rows render. A block is a plain-English header
(`New startup item`, `Firewall turned OFF`, `New setuid root binary`), then decision fields, then
one next step.

The fields are machine-derived and **attacker-influenceable**: a launchd `Label`, a
`ProgramArguments` path, a process name, a certificate subject, a username. An attacker who installs
a launch agent chooses its label and its path, and both land in the page.

`posture/crates/posture-domain/src/sanitize.rs` is the single chokepoint every rendered value passes
through, and it already does more for this design than it was written for:

- backticks are stripped, so a value cannot close its inline-code span;
- `\r`, `\n` and `\t` become spaces, so **a value cannot forge a line of its own**;
- asterisks are stripped from a signing verdict, which renders outside a code span;
- `FIELD_LIMIT` is 240 characters per value, with an explicit truncation marker.

So an injected instruction arrives as at most 240 characters on a line it shares with our own label,
inside a code span. That is a meaningful bound and it is not a solution. See "Prompt injection".

### The route table, and the severity tier that is held

`severity_route` in `posture/crates/posture-domain/src/severity.rs` maps `Critical` to `"posture"`
today, with a docblock saying plainly that this is **held, not chosen**: `priority` is where a
critical page belongs and cannot deliver it in any configuration today, because it is signed with
the retired Bash alerter's key and its prompt names `{alert.title}` and `{alert.detail}` rather than
a pns body's fields. Notice and Info also route to `posture`.

This matters more than it looks. **Once the flip lands, the route is the severity tier**: a page on
`priority` is a critical page, and nothing else posture sends goes there. That gives this design its
selector without adding a `severity` field to the producer wire, which the investigation design
listed as its own open question.

The live table has three routes (`priority`, `pns`, `unattended-upgrades`), all `deliver_only`. The
six-route modify template that replaces it is in flight.

### What pns posts

`pns/crates/pns-adapters/src/destinations/hermes.rs`, `body_with_id`, posts one JavaScript Object
Notation (JSON) document with a fixed key set, every key always present:

```
agent, state, project, detail, header, subheader, body, thread_id, request_id
```

For a posture page, `agent` is the literal `posture`, `state` is `blocked` (mapped from
`Signal::NeedsAttention` in `pns/crates/pns/src/event_flow/submit/mapping.rs`), `detail` is the page
title and the page body joined by one newline, and `thread_id` is the empty string, reserved for the
day pns's direct Discord destination creates a thread and can hand back its identity.

`pns/crates/pns-hermes/src/post.rs` sends exactly two headers: `X-Webhook-Signature`, a hash-based
message authentication code (HMAC-SHA256) over the exact body bytes, and `Idempotency-Key`.

### The webhook adapter in agent mode

Read from `gateway/platforms/webhook.py` on the installed pin:

- **A route without `deliver_only` runs the agent.** The rendered `prompt` template becomes the
  agent's input; the agent's response is delivered to `deliver` with `deliver_extra`.
- **The POST returns `202 Accepted` immediately** and the run happens in a background asyncio task
  (`asyncio.create_task(self.handle_message(event))`). **Nothing upstream ever waits on the model.**
- The session key is `webhook:<route>:<delivery_id>`, so concurrent deliveries get independent runs
  rather than queueing behind each other.
- `deliver_extra` is read for `chat_id` and `thread_id`, and its values support the same
  `{dot.notation}` templates as `prompt`.
- **`Idempotency-Key` is ignored.** The delivery identifier is the first of `X-GitHub-Delivery`,
  `svix-id`, `X-Request-ID`, and failing all three **a millisecond timestamp** (lines 589 to 592).
  The cache holds one hour. So two posts of the same page today are two agent runs.
- Rate limit is 30 requests per minute per route; body limit is one megabyte.
- The delivery info dict is read by **every** `send()` for that chat identity, "interim status
  messages and the final response". With no tools there are no tool-progress bubbles, so this is
  inert here, and it would not be inert on a route with tools.

### No profile is a sandboxed analyst

All four profiles were read from their deployed copies under `~/.hermes/profiles/`:

| Profile | Its own words | `toolsets` | `terminal.backend` |
| --- | --- | --- | --- |
| `butters` | "the browser, GUI, and background computer-use profile" | `[hermes-cli]` | `local` |
| `concerned` | "the creative, media, content-production, design, and research-brief profile" | `[hermes-cli]` | `local` |
| `elaine` | "the mail, calendar-triage, document, and pipeline-manager profile" | `[hermes-cli]` | `local` |
| `nicodemus` | "the engineering, security, research, codebase, MCP, GitHub/code-inspection, and Discord-facing engineering profile" | `[hermes-cli]` | `local` |

None declares `approvals` or `command_allowlist` of its own, so all four inherit the root config's
`approvals.mode: manual` and its two permanent allowlist entries, one of which is
`script execution via -e/-c flag`.

**There is no sandboxed analyst profile on this machine.** `nicodemus` is the closest by charter and
is the least sandboxed thing here: engineering, security, GitHub, a local terminal and the full
`hermes-cli` toolset. Naming it the investigator would hand the explanation a terminal on the host.

### A webhook route cannot choose a profile here

`gateway/platforms/webhook.py` registers `/p/{profile}/webhooks/{route_name}` and resolves the
prefix **only when `gateway.multiplex_profiles` is on**; with it off the prefix is ignored and the
request is handled as the default profile. Both spellings are `false` in the live config (the
`gateway` block and the top-level convenience key).

Turning it on is not a small knob. Per `website/docs/user-guide/multi-profile-gateways.md`, the
default profile's gateway becomes the sole inbound process for every profile, a named-profile
`hermes gateway start` becomes a hard error, and a secondary profile that enables a port-binding
platform makes the gateway refuse to start. Three gateway LaunchAgents are installed today
(`ai.hermes.gateway.plist`, `ai.hermes.gateway-butters.plist`, `ai.hermes.gateway-nicodemus.plist`),
so flipping it means unwinding two of them.

**So the agent route runs in the default profile, and the sandbox has to be built somewhere else.**

### Where the sandbox actually is: `platform_toolsets.webhook`

`gateway/run.py` resolves an agent run's tool surface with
`_get_platform_tools(user_config, _platform_config_key(source.platform))`, and
`_platform_config_key` maps the enum value straight through. `Platform.WEBHOOK = "webhook"`
(`gateway/config.py` line 158) and `hermes_cli/platforms.py` gives it a default toolset of
`hermes-webhook`, which the toolsets reference describes as "Same as `hermes-cli`", the full set.

So `platform_toolsets.webhook` in the root `config.yaml` decides what every webhook agent run can
do. Measured on this host by calling the resolver against the live config:

| `platform_toolsets.webhook` | Resolved toolsets |
| --- | --- |
| absent (today's default) | `clarify, codegraph, cua-driver, qmd, scalebar, vision, web` |
| `[]` | `codegraph, cua-driver, qmd, scalebar` |
| `["no_mcp"]` | *(empty)* |
| `["no_mcp", "safe"]` | `image_gen, safe, vision, web` |
| `["no_mcp", "terminal"]` | `terminal` |

Three findings, all load-bearing.

**The empty list is a trap.** `[]` is the obvious spelling of "no tools" and it leaves four Model
Context Protocol server toolsets live, because `hermes_cli/tools_config.py` treats an unqualified
platform as "every globally enabled server is available". One of the four is `cua-driver`, which
drives this Mac's graphical interface. An explanation route configured with `[]` could move the
operator's mouse.

**`no_mcp` is the documented sentinel** for that case, in the same function: `"no_mcp" in
toolset_names` zeroes the server set. `["no_mcp"]` resolves to nothing at all, measured.

**`safe` is not "read-only and nothing else".** It expands to `image_gen`, `vision` and `web`, and
`web` is outbound network access from the agent.

### The cost model is subscription quota, not tokens

`model.default: gpt-5.6-sol`, `model.provider: openai-codex`,
`model.base_url: https://chatgpt.com/backend-api/codex`. There is no per-token invoice here. The
budget being spent is the operator's ChatGPT quota, shared with every other hermes agent on the
box, and the failure mode is a critical-page storm starving an interactive session at the worst
moment. `agent.max_turns` is 150 and `agent.gateway_timeout` is 1800 seconds.

The root `agent.system_prompt` is long (task management, Todoist policy, project conventions) and
every webhook agent run pays for it, because the route cannot override it. That is the largest
avoidable cost in this design and the strongest argument for a dedicated profile later.

## Decision 1: how a critical page reaches the explainer

### The candidates

**(a) pns posts the same event to a second named route.** pns already owns route naming, per-route
signing keys (`[plugins.hermes.keys]`, in flight) and the delivery ledger. One additional leg,
opt-in by config, selecting on the event's own fields.

**(b) posture's producer request names two routes.** `Request::route` is a single `Name`
(`posture/crates/posture-pns-wire/src/request.rs`), so this is a wire change in two independent
workspaces plus their golden fixtures, and it makes posture carry knowledge of an interpretation
feature. It contradicts the producer-agnostic ruling directly.

**(c) the explainer route is triggered from the posture route.** Not buildable. A `deliver_only`
route calls `_direct_deliver` and returns; there is no chain, no follow-on and no hook on a
delivered message. Verified in `gateway/platforms/webhook.py`.

**(d) a host watcher reads posture's results log or pns's ledger and posts.** A fifth background job
to do what an existing delivery already does. Rejected on the same grounds the investigation design
rejected a bespoke bot.

### Recommendation: (a), gated on the `priority` route

**pns gains one opt-in explanation leg.** When a delivered event matches the selector, pns posts the
same body a second time, to the `posture-explain` route, as a separate delivery leg with its own
ledger row.

The selector is three fields pns already has, and no new wire field:

```
producer == "posture"  AND  class == "security"  AND  route == "priority"
```

`route == "priority"` is the critical-tier test, because `severity_route` sends nothing else there
once the held flip lands. Until it lands, the selector matches nothing and the feature is inert,
which is the correct behavior for an unshipped prerequisite: no page is explained twice, and no
Notice-tier page is explained at all.

Why (a) beats (b) and (d):

- posture is untouched, so the producer-agnostic ruling holds without argument.
- The explanation leg is a **separate leg of the same event** in pns's ledger, so its failure is
  reported by the 2026-09-08 delivery-failure design and it cannot dead-letter, delay or retry the
  page's own leg. Legs belong to events; one leg's refusal is not another's.
- It is opt-in configuration. An absent `[plugins.hermes]` key is today's behavior exactly.
- The leg posts in `ReportMode::Silent`, so it runs on the ten-second asynchronous deadline rather
  than inside the producer's five-second budget. The page's submission is unaffected in wall-clock
  terms, not merely in principle.

Ordering, stated honestly: the page's leg is posted first and a `deliver_only` route answers `200`
only after Discord accepted it, while the explanation leg answers `202` before its model call
starts. So the page lands first in every realistic case and **nothing enforces it**. The available
enforcement is to post the explanation leg only after the page's leg reports delivered, and that is
what the build should do.

## Decision 2: the sandbox

### The profile

**The default profile**, because a webhook agent route cannot select another one without
`gateway.multiplex_profiles`, and turning that on unwinds two installed gateway LaunchAgents. This
is a constraint, not a preference, and it is why the tool surface has to carry the whole boundary.

### The tools: none

```yaml
platform_toolsets:
  webhook: ["no_mcp"]
```

Measured to resolve to zero toolsets against the live config. Concretely the explanation agent has
no terminal, no file tools, no web, no kanban, no memory, no delegation, no code execution, and no
Model Context Protocol servers.

**Which commands it may run: none, because it has no way to run one.** There is no command allowlist
to write here, and that is the point. A command allowlist is a list of things you trust an agent to
do; an absent terminal tool is a thing the agent cannot do. Hermes's own dangerous-command approval
never enters the picture, which matters because the root config already permanently allows
`script execution via -e/-c flag`.

The reason zero tools is enough is that **the page already carries the facts**. The header says what
fired in plain English, the fields carry the label, path, program and signing verdict, and the next
step carries the one inspection command. Explaining what a launch agent is, why a new setuid root
binary matters, and what an unsigned binary in `~/Library` usually turns out to be is knowledge
work, not evidence work. The moment the explanation needs `osqueryi`, a log read or a signature
check, the honest answer is the investigation pipeline, which was designed for exactly that and
puts those reads inside a networkless container with a validator behind them.

### What that enforces, versus what it asks

| Property | Enforced by | Where |
| --- | --- | --- |
| cannot run any command | no terminal tool in the resolved toolset | `platform_toolsets.webhook` |
| cannot read any file on the host | no file tools | same |
| cannot reach the network | no `web` toolset; delivery is the adapter's, not a tool's | same |
| cannot touch the allowlist | nothing to execute `posture allowlist` with | same |
| cannot create or claim a kanban card | no `kanban` toolset | same |
| cannot drive the graphical interface | `no_mcp` removes `cua-driver` | same |
| cannot delay or suppress the page | different leg, different route, `202` before the run | pns ledger plus the adapter |
| stays within the fixed structure | the prompt asks | **nothing** |
| stays under the length cap | the prompt asks | **nothing** |
| never says the finding is benign | the prompt asks | **nothing** |

The bottom three rows are the honest gap and they are listed deliberately, in the style the
bounded-advisory design used for its two unenforced limits. They are acceptable here only because
the output has no authority: the worst outcome of a violated prompt rule is a message that is too
long, badly structured or wrong, in a channel where the deterministic page sits directly above it.

### The trade the platform-wide setting makes

`platform_toolsets` is per platform, not per route. Every agent route on the webhook platform shares
this setting. Today that costs nothing, because the other five routes are `deliver_only` and the
field is ignored for them. It costs something the day uu failure triage becomes the second agent
route and wants a tool: the two cannot differ, and the answer then is `gateway.multiplex_profiles`
with a profile per route, with the LaunchAgent unwinding that implies. Recorded here so it is a
known ceiling rather than a surprise.

## Decision 3: what it posts and where

### Order and destination

The original page arrives first, unchanged, on `priority`, exactly as it does now. The explanation
arrives as **one separate message in the same channel**, because the route's
`deliver_extra.chat_id` is the `priority` channel's own identity from KeePassXC.

Threads are reachable later without a redesign: `deliver_extra` is read for `thread_id` and its
values take `{dot.notation}` templates, and pns's hermes body already carries a `thread_id` key
reserved for the day the direct Discord destination creates a thread. So
`thread_id: "{thread_id}"` is the eventual spelling. Whether an empty string in that field is
ignored or breaks delivery is a build-time check, not a design decision, and it is listed in the
open questions.

### The fixed structure

Four parts, in this order, nothing else:

```
Explanation (model output, not a verification)

What fired
  One sentence restating the page's header and the one field that identifies the subject.

What it means
  Two or three sentences: what this detector watches, why it is Critical, and what
  ordinarily produces it.

What to check first
  Up to three numbered lines, the page's own next step first.

What this cannot tell you
  A fixed sentence: this did not inspect the machine, verify a signature, or check
  the allowlist, and it cannot tell you whether this particular item is malicious.
```

Three rules on the content, all prompt-level and all labelled as such:

1. **It names no commands of its own.** "What to check first" points at the page's own next step and
   otherwise describes where to look in words. This removes the surface where an operator pastes a
   model-authored command at three in the morning, and it costs very little, because the page
   already carries the one command worth running.
2. **It may not clear the finding.** No "benign", no "expected", no "no action needed". The
   monotonic rule is carried over from the investigation design's `concern` field, where it was
   enforced by a closed vocabulary; here it is an instruction, and it is written down as an
   instruction rather than dressed as a control.
3. **The first line always says it is model output.** A forged sentence and an honest one carry the
   same label, which is the only defense that survives a prompt injection.

### The length cap

**1200 characters.** Discord's message limit is 2000 and the page already spends up to 1900 of its
own; a second message of similar weight turns a glance into a read. 1200 leaves room for the label
line and keeps the whole thing on one phone screen.

Enforcement: none. The prompt asks. An over-long reply is ugly, not dangerous, and Discord's own
limit is the backstop. This is the clearest single difference from the bounded-advisory design,
where the cap is enforced by a validator, and it is acceptable for the same reason all three
unenforced rows above are.

### When it stays silent

An agent route always replies, so **the silence rule lives in the sender, not in hermes.** pns posts
the explanation leg only when all of these hold:

- the event is a critical page, by the `producer` plus `class` plus `route == "priority"` selector;
- it is the first explanation leg for that `request_id`, so a retried page is explained once;
- the per-hour cap in Decision 5 has room.

Nothing else posture sends is ever explained: not the daily digest, not the heartbeat, not the
funnel record, not the watchdog, not a cursor-reset warning, and not the
`notification-omitted` notice. The omission notice deserves a sentence of its own: it is the message
posture sends when a finding exceeded notification limits, it carries no finding to explain, and
explaining it would produce confident prose about nothing.

## Decision 4: failure modes

| Failure | Behavior | Distinguishable |
| --- | --- | --- |
| the model provider is down or refuses | the run fails inside the gateway; the page is already delivered | **no**, see below |
| the gateway is down | pns's leg fails to connect and is retried, then reported by the delivery-failure design | yes, through pns's own report |
| the `posture-explain` route does not exist | 404, classified permanent, dead-lettered on first failure | yes, through pns's report |
| the run is slow | the page never waits: different leg, different route, `202` before the model call | not applicable |
| the page is a false positive | the explanation explains it as though real | partly, by part four of the structure |
| the page content carries an injected instruction | see "Prompt injection" | no |
| the explanation exceeds the cap | Discord truncates or rejects; the page is unaffected | yes, visibly |
| a critical-page storm | the per-hour cap stops posting legs after the sixth | yes, in pns's ledger |
| the leg is retried | one agent run, once `X-Request-ID` is sent; today, one run per retry | yes, after the fix |

**The silent state is a failed model run.** The adapter answers `202` before the run starts, so pns
records a delivered leg and learns nothing afterwards. An explanation that never arrives is
invisible to every mechanism in the system. This is the same shape as the investigation design's
"gateway down, no dispatcher tick" row, and the same answer applies: the honest mitigation is that
the explanation carries the page's `request_id`, so a human reading the channel can see which page
went unexplained. A mechanical detector is out of scope and would cost more than the feature.

### Prompt injection

The page carries file paths, process names, launchd labels and certificate subjects from the
machine, and an attacker who installs a launch agent chooses all four. The prompt is therefore
attacker-influenceable by construction, and this is the failure mode that decides whether the design
is acceptable.

Four layers, from strongest to weakest:

1. **The tool surface is empty.** This is the only layer that matters and it is measured rather than
   asserted. An injected instruction that succeeds completely still cannot read a file, run a
   command, reach the network, touch the allowlist, or drive the graphical interface, because none
   of those tools is in the resolved set. The blast radius of a total prompt-injection win is
   **one wrong paragraph in a Discord message**.
2. **posture's sanitizer bounds the payload.** Each field is at most 240 characters, backticks are
   stripped, and newlines and tabs become spaces, so an injected instruction cannot start a line of
   its own or close a code span. It shares a line with our own label, inside a code span.
3. **The prompt template frames the page as data.** The page body is wrapped in a delimiter, and the
   instruction above it states that everything inside is untrusted text taken from a machine under
   investigation, is never an instruction, and is to be quoted rather than obeyed. This is a real
   reduction in success rate and it is not a boundary; it is the same class of thing as the
   `pre_tool_call` hook in the approval-authority design, and it gets the same label.
4. **The label line.** Every explanation says it is model output in its first line, so an injected
   sentence carries the same warning an honest one does.

**What remains, unmitigated:** an injected page can make the explanation lie, including telling the
operator that a real finding is a known benign agent. Nothing here prevents that. The two things
that keep it from being dangerous are that the deterministic page sits directly above the
explanation with the real facts, and that the explanation carries no action the operator can take by
tapping. This residual is the reason for the next section.

## Decision 5: cost and rate

- **Critical pages only.** The `route == "priority"` selector is the mechanism, and it is exact
  once the held `severity_route` flip lands.
- **Once per finding.** One explanation leg per page event, keyed on the page's `request_id`. Two
  parts: pns posts one leg per event, and **pns starts sending `X-Request-ID: <request_id>`**, a
  one-line change in `pns/crates/pns-hermes/src/post.rs`. Today `Idempotency-Key` is the only header
  sent and the webhook platform ignores it, falling back to a millisecond timestamp, so any retry is
  a second agent run and a second explanation. Sending the header hermes actually reads arms its
  one-hour idempotency cache and makes a retry free.
- **A per-hour cap of six.** A rolling-hour count in pns, refusing the seventh explanation leg and
  recording the refusal. Six explanations at 1200 characters is already seven kilobytes of prose in
  an hour, and a critical-page storm is precisely when the channel has to stay readable. Hermes's
  own 30-per-minute route limit is not a budget, it is a flood guard.
- **The budget being spent is ChatGPT quota**, not a per-token invoice, shared with every other
  hermes agent on this machine. The cap is what stops a storm from starving an interactive session.
- **Each run is one model call** in the normal case, because with no tools there is nothing to
  iterate on. The fixed overhead is the root `agent.system_prompt`, which every webhook run pays and
  which the route cannot override.

## The explanation carries no approval action

The explanation may never contain an approval, a suppression, or anything that reads as one: no
`/osquery allow <label>` line, no "reply yes to allowlist this", no `posture allowlist` invocation,
and no command at all under rule 1 of the fixed structure. The operator's approval path stays what
`2026-09-14-osquery-approval-authority-design.md` specifies, a typed slash command reaching a
deterministic handler that calls the one writer, unless the parallel buttons research changes it,
which is not this document's concern. Three properties hold that line: the explainer has no tool
with which to write anything, its output vocabulary contains no verb that acts, and the writer
itself is being narrowed to a pending set that announces every grant. An explanation that suggested
an approval would be an explanation that had started adjudicating the finding, which is the job the
fence exists to keep separate, and it is a prompt rule here precisely because the tool surface
already makes it unreachable in practice.

## Out of scope

- Everything the investigation and bounded-advisory designs cover: the bounded evidence copy,
  `posture evidence`, the kanban card, the Docker-backed profile, the closed result schema, the
  validator and the failure notice. This document does not supersede either; it precedes both.
- Any verification of the finding. No signature check, no quarantine attribute, no allowlist
  membership test, no live osquery read.
- uu failure triage, the other named opt-in. It shares the `platform_toolsets.webhook` setting and
  therefore shares this design's ceiling, which is worth knowing when it is designed.
- Turning on `gateway.multiplex_profiles` and the per-profile route topology behind it.
- Threads. Reachable through `deliver_extra.thread_id` once pns's direct Discord destination can
  create one and report its identity; the body key is already reserved and empty.
- The `priority` route's key and prompt reconciliation, and the `severity_route` flip. Both are
  prerequisites named here and tracked separately.
- A mechanical detector for "the page was never explained".
- Any `.chezmoiremove`, retirement script or removal mechanism, per the standing ruling.

## Assumptions made in the operator's place

1. **The route is named `posture-explain`.** Self-documenting, and it says explanation rather than
   investigation so the two do not blur. *Alternative:* `posture-investigator`, which matches the
   ledger's own wording and promises more than this route delivers.
2. **The explanation posts into the page's own channel, reusing the `priority` channel identity, so
   only one new KeePassXC entry is needed (the route's webhook secret).** *Alternative:* its own
   `#posture-explain` channel with a second entry, which keeps `priority` to deterministic machine
   output only and splits the page from its explanation across two channels.
3. **The sandbox is `platform_toolsets.webhook: ["no_mcp"]` on the default profile.**
   *Alternative:* a dedicated Docker-backed profile behind `gateway.multiplex_profiles`, which is
   the investigation design's boundary and costs unwinding two gateway LaunchAgents to get a
   container around an agent that has nothing to execute.
4. **The explanation has no tools at all.** *Alternative:* `["no_mcp", "terminal"]`, measured to
   resolve to exactly `terminal`, with a hermes `command_allowlist` and the `pre_tool_call` hook as
   the guard, so the explanation could run read-only posture commands. Displaced because the
   backend is `local`, so that terminal is a host shell as the operator, and because the guard would
   be exactly the trust-based boundary the approval-authority design spent itself removing.
5. **The explanation names no commands of its own.** *Alternative:* let it suggest read-only
   inspection commands, which is more useful and reintroduces "the operator pastes what a model
   wrote" as a surface.
6. **The trigger is a pns delivery leg, selected on producer plus class plus route.**
   *Alternative:* posture names a second route, which needs a wire change in two workspaces and
   teaches posture about agent interpretation.
7. **The selector is `route == "priority"` rather than a new `severity` field on the wire.**
   *Alternative:* add `severity` to the producer request, which the investigation design lists as
   its own open question and which is the honest fix for several consumers at once.
8. **Once per finding, with `X-Request-ID` sent so a retry does not re-run.** *Alternative:* leave
   the header alone and mark the explanation leg non-retryable, which loses the explanation whenever
   the gateway blips.
9. **A cap of six explanations per rolling hour.** *Alternative:* any other number, or no cap and a
   reliance on how rarely critical pages fire. Six is a judgement, not a measurement.
10. **A 1200-character cap, unenforced.** *Alternative:* a deterministic truncator in pns, which
    means pns reading and rewriting a message it did not compose, on a leg it only posts.
11. **The label line and the fixed four-part structure are prompt rules.** *Alternative:* the
    bounded-advisory design's closed schema plus validator, which enforces every one of them and is
    the pipeline this route exists to avoid building yet.

## Open questions

1. **Is the explanation worth a message at all, given that it cannot verify anything?** The honest
   framing of this whole design is "a knowledgeable paragraph next to the page". If the operator's
   real want is "tell me whether this is bad", the answer is the investigation pipeline and this
   route is a detour. Everything below assumes the answer is yes.
2. **Same channel as the page, or its own?** Assumption 2 chose the same channel. Its own channel
   keeps `priority` purely deterministic, at the cost of reading two places at three in the morning.
3. **Is a platform-wide tool setting acceptable, knowing uu failure triage will share it?** The
   alternative is `gateway.multiplex_profiles` and a profile per route, which unwinds two gateway
   LaunchAgents and is a topology change, not a config line.
4. **Does the explanation get to name read-only commands?** Assumption 5 says no. Saying yes makes
   it more useful and puts model-authored command text in front of a tired operator.
5. **Is six per hour right, and should the cap refuse or queue?** Refusing loses the explanation for
   pages seven and beyond during a storm, which is arguably when explanations matter most and
   certainly when the channel can least afford them.
6. **Does pns send `X-Request-ID` generally, or only on this leg?** Sending it on every hermes post
   arms idempotency for `pns`, `pns-recap`, `uu` and `posture` at once, which is probably right and
   changes the deduplication behavior of four existing routes in one line.
7. **Is the held `severity_route` flip a prerequisite, or does the selector match `posture` for
   now?** Matching `posture` would explain Notice-tier pages too, which is wrong. This document
   treats the flip as a hard prerequisite; the alternative is a `severity` field on the wire.
8. **Does `deliver_extra.thread_id: "{thread_id}"` degrade cleanly while pns posts an empty
   string?** A build-time check against the running gateway, not a design decision, but it decides
   whether the thread wiring lands in the first pull request or the third.
9. **Should the explanation be gated on the approval-authority change landing, the way the
   investigation is?** This document's position is **no**, and it is the one place it departs from
   its sibling: that gate exists because an agent with a terminal could write the suppression file
   in the same session, and this agent has no terminal. If the operator disagrees, the order is
   fence first, explanation second, and nothing here changes.

## Build plan

Three pull requests, in this order. Prerequisites first, and they are not part of this plan: the
six-route modify template lands, `priority` carries the pns signing key and a prompt naming a pns
body's fields, and `severity_route` flips `Critical` to `priority`.

**PR 1, hermes configuration.** `private_dot_hermes/modify_private_config.yaml.tmpl` gains the
`posture-explain` route (agent mode, the prompt template, `deliver: discord`, `deliver_extra.chat_id`
from the `priority` channel entry) and the `platform_toolsets.webhook: ["no_mcp"]` field. The
operator creates one KeePassXC entry, `Hermes :: Webhook Secret (#posture-explain)`. Inert until
something posts to it. Verification is a rendered template plus `hermes tools` reporting an empty
webhook toolset after the apply, and a hand POST with the route's secret.

**PR 2, the pns explanation leg.** The opt-in `[plugins.hermes]` key, the selector, the per-hour
cap, the `X-Request-ID` header, and the leg posted only after the page's leg reports delivered.
Behaviors worth a test, all of them behavior of a tool we wrote: a critical page on `priority`
produces exactly one explanation leg; a Notice page produces none; the digest, heartbeat, funnel,
watchdog and omission notice each produce none; the seventh page in an hour produces none and
records the refusal; a retried page produces one leg, not two; a failed explanation leg does not
change the page leg's outcome.

**PR 3, documentation.** The ledger entry and one runbook paragraph naming the route, the toolset
setting, the `no_mcp` trap and the platform-wide ceiling.
