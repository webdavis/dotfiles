# pns Hermes support design

Status: proposed, 2026-09-22. Implementation plan:
`docs/superpowers/plans/2026-09-22-pns-hermes-support.md`.

## Purpose

pns reports Claude Code and Codex sessions: a finished turn is `done`, a dead one is `failed`, a question
or an approval reaches the operator through the presence gate, and an away operator answers an approval
from the phone because pns hands it to `moshi-hook`. Hermes Agent sessions get none of that today. This
design gives Hermes the same treatment for terminal sessions, records chat gateway sessions and kanban
workers' turns for the recap, moves the Hermes phone card from moshi's own plugin to pns's forward, and
keeps moshi's own Hermes screens fed as moshi's plugin fed them.

## Rulings in force

Operator rulings of 2026-09-22, binding:

- **Q14b.** pns ships its own Hermes plugin and installs it with a new `pns hermes install-plugin`, the
  way `pns codex install-hooks` installs Codex's hooks. The plugin is a small Python shim, because
  Hermes loads only Python plugins from `~/.hermes/plugins/<name>/`. The pns binary embeds its source
  and writes it. pns ships no bash.
- **Q14e.** Gateway sessions (Discord and the other chat platforms) are recorded for the recap only: no
  banner, no phone card, no lamp signal, no Discord post.
- **Q14f.** Terminal sessions behave exactly like Claude Code: a turn end is `done`, failures and
  questions notify, approvals go through pns's presence gate and, when the operator is away, pns
  forwards them to `moshi-hook hermes-hook` so the phone card works and moshi types the answer into the
  pane. moshi's own Hermes plugin is removed so an approval never produces two cards. A phone drill
  with `hermes chat` proves it.
- **Q14d** is superseded by Q14f.
- pns's own summarizer runs (the recap's `hermes` backend and every other) never fire hooks and never
  register as sessions.

The operator's answers to this design's four open questions, also 2026-09-22, binding:

- **A1, moshi reinstalling its plugin.** Guard against it: the Hermes config modify template owns
  `moshi-hooks` as a member of `plugins.disabled`.
- **A2, the TUI.** The operator uses terminal Hermes (chat and the TUI) and Discord Hermes. The TUI is a
  terminal session, approvals included, once a drill proves moshi can type the answer into it.
- **A3, moshi's Hermes screens.** The operator uses moshi's Chat View for Hermes. pns's plugin forwards
  every non-approval event to `moshi-hook hermes-hook` unconditionally, with the payload moshi's own
  plugin sends today, so nothing new is pushed. Only the approval pair goes through pns's presence gate:
  forwarded when away, held at the desk.
- **A4, kanban workers.** The operator uses them. Their turn ends are recorded for the recap only; pns
  notifies only when a worker is blocked, fails, or asks something.

## What exists today

All of this was read on dresden against Hermes Agent v0.17.0 (`~/.hermes/hermes-agent`, pinned by
`.chezmoidata/hermes.yaml`) and moshi-hook 0.3.26.

**Hermes plugins are opt-in.** Hermes scans `$HERMES_HOME/plugins/<name>/` for a `plugin.yaml` and an
`__init__.py` with `register(ctx)` (`hermes_cli/plugins.py:1-20`, `:1240`). A user plugin loads only
when its name is in `plugins.enabled` in `config.yaml` (`plugins.py:223-248`, `:1333-1345`: "not enabled
in config (run `hermes plugins enable {}` to activate)"). `hermes plugins enable <name>`
(`hermes_cli/plugins_cmd.py:772`) is Hermes's own writer of that list; it is idempotent and prints
"already enabled" when there is nothing to do. `HERMES_HOME` defaults to `~/.hermes`
(`hermes_constants.py:54-76`). Discovery runs in every Hermes process: the classic CLI
(command-line interface) and the TUI (terminal user interface) through `model_tools.py:201`, the
gateway at `gateway/run.py:5700`.

**Plugin callbacks run inside the Hermes process, synchronously.** A callback that blocks holds up the
agent. The two approval hooks are observers: "return values are ignored. Plugins cannot veto or
pre-answer an approval from these hooks" (`plugins.py:159-161`).

**moshi's plugin is the payload contract.** `~/.hermes/plugins/moshi-hooks/__init__.py` (generated
2026-08-25) registers nine hooks and pipes one JSON object per event to
`/opt/homebrew/bin/moshi-hook hermes-hook` from a single-worker thread pool, fire and forget, with a
10 s timeout. Its approval pair (`__init__.py:82-117`) sends only when `surface == "cli"`, mints a
`uuid4().hex` `action_id` per request, and pairs the answer to it through a queue keyed by
`(session_key, command, pattern_key)`. The request carries `hook_event_name: "PermissionRequest"`,
`session_id` (the `session_key`), `cwd`, `action_id`, `command`, `description`, `pattern_key`,
`pattern_keys` and `surface`; the answer carries `hook_event_name: "PermissionResolved"`, `session_id`,
`cwd`, `action_id`, `command`, `description`, `choice` and `surface`. When the queue is empty it sends
the answer with an empty `action_id`. The template embedded in moshi-hook 0.3.26 differs from the
installed file only in blank lines (extracted with `strings` and diffed), so the contract has not moved
between 0.3.3 and 0.3.26. moshi's `docs/usage.md:284` says its installer "also enables `moshi-hooks` in
the matching `config.yaml`".

**This repository already wires two Hermes shell hooks to pns.** Hermes also runs shell commands
declared under `hooks:` in `config.yaml` (`agent/shell_hooks.py`). The chezmoi modify template
`private_dot_hermes/modify_private_config.yaml:224-248` declares `pre_approval_request` as
`/usr/bin/env PNS_PRODUCER=hermes <pns> hook blocked --remind` and `post_approval_response` as
`... hook resolved`. pns's retained activity window (every row since 2026-09-21) holds no row with agent
`hermes`, so nothing observed depends on the pair.

**"hermes" already means something inside pns, and it is the same product.** The research report
called pns's `[plugins.log] type = "hermes"` destination an unrelated product. It is not: that
destination posts to `http://127.0.0.1:8644/webhooks/pns-events`
(`pns-adapters/src/destinations/hermes.rs:27`), which is Hermes Agent's own gateway webhook platform
(`gateway/platforms/webhook.py:196`, `/webhooks/{route_name}`), configured by the routes in the same
`~/.hermes/config.yaml`. After this design Hermes plays two roles for pns: the gateway pns posts its
durable log through, and a producer of hook events.

## The plugin: `pns-hooks`

`pns hermes install-plugin` writes two files into `$HERMES_HOME/plugins/pns-hooks/` (default
`~/.hermes/plugins/pns-hooks/`):

- `plugin.yaml`: `name: pns-hooks`, a fixed `version: "1"`, and a one-line description. Hermes reads
  the version only for display (`hermes_cli/plugins.py:1516`), and a fixed one means a pns release that
  leaves the shim alone rewrites nothing.
- `__init__.py`: the shim, embedded in the binary with `include_str!`, with two substitutions, each
  written as a JSON string literal, which Python reads as a string literal: the absolute, canonicalized
  path of the binary that wrote it (`current_exe`, as `install-hooks` does), and the moshi-hook path
  every other pns forward uses (`pns_adapters::moshi_hook_bin`: `PNS_MOSHI_HOOK_BIN`, else
  `/opt/homebrew/bin/moshi-hook`).

The shim registers nine hooks and has two destinations:

- **pns.** Seven hooks spawn `<pns> hook <verb> [flags]` with the event's JSON on stdin,
  `PNS_PRODUCER=hermes` added to the inherited environment, and stdout and stderr discarded. The
  inherited environment carries `HERDR_PANE_ID` and `HERDR_WORKSPACE_ID` from the Hermes process, which
  is how pns learns the pane (`hook_dispatch.rs` reads `HERDR_PANE_ID` on every arm). In a kanban worker
  every payload also carries `kanban_task`, the process's `HERMES_KANBAN_TASK`.
- **moshi.** Every hook except the approval pair spawns `<moshi-hook> hermes-hook` with the payload
  moshi's own Hermes plugin built for it; "What moshi receives" below has the list.

Delivery copies moshi's model. Every callback enqueues and returns `None` at once, so no callback
blocks Hermes, injects context (`pre_llm_call`'s return value) or blocks a tool (`pre_tool_call`'s).
Each destination has one worker thread that runs its calls in order, so a prompt reaches pns before its
approval and an approval before its answer, and a slow pns call (the `stop` summarizer) never delays
moshi. A pns call has a 120 s backstop timeout; pns bounds its own waits well inside that (the
summarizer at 30 s, `pns-adapters/src/codex.rs:86`, and moshi's acknowledgement at 5 s by default). A
moshi call has the 10 s moshi's own plugin gave it. A call submitted after interpreter shutdown closed
its pool runs inline. Every exception is swallowed: the shim can lose a notification and can never fail
a Hermes turn.

### Two runs pns never hears about

**Hermes's background review.** Every `memory.nudge_interval` turns (default 10,
`agent/agent_init.py:1162`, counted in `agent/turn_context.py:290-297`), and when the skills nudge
fires, `finalize_turn` spawns a review fork before the turn's own `on_session_end`
(`agent/turn_finalizer.py:446-452`). The fork is an `AIAgent` built with the parent's `platform` and
then given the parent's `session_id` (`agent/background_review.py:641-702`), and it calls
`run_conversation` (`:750`), which fires `pre_llm_call`, `post_llm_call` and `on_session_end` like any
turn. Reported, it would restart the operator's turn clock, raise a second `done` whose text is the
review's "memory updated", and could take the real turn's kept reply. Nothing on the payload tells it
apart: same session, same platform, a fresh `task_id` like any turn. The thread does:
`_spawn_background_review` runs it on a thread named `bg-review` (`run_agent.py:1461`), and every hook
of `run_conversation` fires synchronously on the calling thread (`agent/turn_context.py:416`,
`agent/turn_finalizer.py:342` and `:468`). So no pns call is made, and no reply is kept, when
`threading.current_thread().name` is `bg-review`. moshi is still fed from that thread, exactly as its
own plugin fed it (A3 forwards unconditionally).

**pns's own summarizer.** A recap or `pns doctor` with `[recap.summarizer] type = "hermes"` runs
`hermes chat -Q -t "" -q <prompt>` (`pns-domain/src/recap/summarizer.rs:151-158`), a one-shot whose agent
is `platform="cli"` (`hermes_cli/oneshot.py:343`) in the default Hermes home, so pns-hooks would load in
it and report the recap prompt as a terminal session, and the child could not exit until its shim had
run the `stop` summarizer. `run_summarizer` (`pns-adapters/src/recap/summarizer.rs:32`, the one runner
both the recap and the doctor probe use) sets `PNS_SUMMARIZING=1` on every backend's child, the variable
the Codex turn summarizer already sets on its own run (`pns-adapters/src/codex.rs:27`), and the shim's
`register(ctx)` registers no hook at all when that variable is set. Such a run never fires a hook and
never becomes a session, in pns or in moshi.

### The hook-to-verb mapping

What pns receives. "What moshi receives" below has moshi's side.

| Hermes hook | When the shim sends | `hook_event_name` | pns verb | Payload beyond `session_id`, `cwd`, `platform` |
| --- | --- | --- | --- | --- |
| `pre_llm_call` | every turn | `UserPromptSubmit` | `prompt` | `prompt`, `model` |
| `post_llm_call` | never; keeps the reply for the turn's end | none | none | none |
| `on_session_end` | `interrupted` | `TurnInterrupted` | `resolved` | `interrupted` |
| `on_session_end` | `completed` | `AgentEnd` | `stop` | `last_assistant_message` (the kept reply), `model` |
| `on_session_end` | neither | `TurnFailed` | `stop-failure` | `error` (the kept reply), `model` |
| `pre_approval_request` | surface `mcp-elicitation` | `Elicitation` | `asked` | moshi's request keys |
| `pre_approval_request` | any other surface | `PermissionRequest` | `blocked --remind=5m` | moshi's request keys |
| `post_approval_response` | always | `PermissionResolved` | `resolved` | moshi's answer keys |
| `pre_tool_call` | tool `clarify`, or `kanban_block` in a kanban worker | `PreToolUse` | `asked` | `tool_name`, `tool_call_id`, `tool_input` |
| `post_tool_call` | tool `clarify` only | `PostToolUse` | `resolved` | `tool_name`, `tool_call_id` |

The turn's end comes from `on_session_end`, which fires once at the end of every `run_conversation`
with `completed` and `interrupted` (`agent/turn_finalizer.py:466-478`). The CLI fires one more,
`interrupted=True`, when it exits mid-turn (`cli.py:1077-1110`, `:14904-14922`); that maps to
`resolved`, which clears whatever is still set and is harmless after the turn's own end.
`post_llm_call` fires whenever a turn has a final response and was not interrupted (`:339-354`), and a
failed turn can have one (`completed` also requires `not failed` and an unexhausted budget, `:126-133`),
so mapping `post_llm_call` to `stop` would report a failed turn twice. The shim keeps the reply from
`post_llm_call`, keyed by session, and hands it to whichever verb `on_session_end` picks.

An interrupted turn maps to `resolved`, as Codex's `Interrupt` does (`codex_hooks.rs:49-53`): the
operator ended it, so any wait it held is answered. Questions notify three ways: `clarify` (Hermes's
ask-the-user tool) and an MCP (Model Context Protocol) elicitation map to `asked`, and a turn that ends
on a question becomes `asking` through the same summarizer every Claude turn goes through. A kanban
worker asks a fourth way: `kanban_block` moves its task to blocked because it "need[s] human input to
proceed" (`tools/kanban_tools.py:1193-1200`), with the question as its `reason`. Only inside a worker
(`HERMES_KANBAN_TASK` set) does it map to `asked`; there is no `resolved` for it, because the task stays
blocked after the tool returns, and the worker's turn end closes the wait (see "Terminal or recorded
only").

`on_session_start` and `on_session_finalize` reach moshi only: pns names a session from its first prompt
and has no session-boundary verb. `subagent_start`, `subagent_stop` and every other hook stay
unregistered. Tool calls other than `clarify` and a worker's `kanban_block` never reach pns.

### The platform stamp

Only the lifecycle hooks carry `platform`. The approval hooks carry `surface` and `session_key`
(`plugins.py:162-167`), and the tool hooks carry `session_id` but no platform
(`hermes_cli/plugins.py:1933-1943`, `model_tools.py:880-886`). The shim therefore stamps `platform` on
every payload, remembered PER SESSION:

1. A lifecycle event sends its own platform and records it against its session id.
2. Any other event sends the platform its session last recorded: a tool event looks up its
   `session_id`, an approval its `session_key`.
3. A session no lifecycle event named sends the last terminal platform (`cli` or `tui`) this process
   named, and nothing when the process never named one.

Per session, because one Hermes process hosts more than one agent. The classic CLI builds its agent
with `platform="cli"` (`hermes_cli/cli_agent_setup_mixin.py:372`; one-shot `hermes chat -q` at
`hermes_cli/oneshot.py:343`), the TUI backend with `"tui"` (`tui_gateway/server.py:4233`), the gateway
with each message's platform. Inside those processes run agents of other platforms, each with its own
session id: delegated children with `platform="subagent"` (`tools/delegate_tool.py:1246`), and the
weekly curator, which the CLI starts on a daemon thread at launch (`cli.py:12555-12561`, on by default,
`hermes_cli/config.py:2149-2151`) as an agent with `platform="curator"` and a fresh session id
(`agent/curator.py:1826-1840`) that runs for 50 to 100 model calls. A process-wide "last named
platform" would let the curator's events stamp an operator's CLI approval `curator` and silence it.

The approval's `session_key` is the agent's session id on both terminal front ends: the CLI binds
`self.session_id` before each turn (`cli.py:11682`), and the TUI builds its agent with
`session_id=session_id or key` (`tui_gateway/server.py:4234`) and binds `session["session_key"]`
(`:8253`). Compression can rotate an agent's session id mid-session
(`agent/conversation_compression.py:580`) while the CLI keeps binding the old one until the turn ends
(`cli.py:11894`); the old id stays recorded, so the lookup still hits. The third rule covers the rest,
and it fails toward notifying: in a terminal process a miss is stamped with that terminal front end, and
in a gateway process, where no agent is `cli` or `tui`, a miss sends no platform and the surface
decides.

## Terminal or recorded only

pns decides, from the payload's `platform`, `surface` and `kanban_task`, for events whose producer is
`hermes` (`hermes_records_only(event, payload)`):

| `platform` | `surface` | `kanban_task` | The event is |
| --- | --- | --- | --- |
| `cli` or `tui` | anything | empty | terminal |
| `cli` or `tui` | anything | set | recorded only for `prompt` and `stop`, terminal for every other verb |
| any other non-empty value | anything | anything | recorded only |
| empty | `gateway` | anything | recorded only |
| empty | `cli`, `mcp-elicitation`, anything else, or absent | anything | terminal |

Every non-terminal platform Hermes names is recorded only: the chat gateways (`discord`, `telegram` and
the rest), `webhook` (the `explain` agent route), `api_server`, `cron` (`cron/scheduler.py:2497`), `acp`
(`acp_adapter/session.py:592`), `curator` and `subagent`. None of them has an operator at a pane.

**Platform decides before surface** because the TUI's approvals report `surface="gateway"`. The TUI
backend sets `HERMES_GATEWAY_SESSION=1` for its whole process (`tui_gateway/server.py:1828-1832`), so
`check_all_command_guards` takes the gateway branch and fires the hook with `surface="gateway"`
(`tools/approval.py:1686`). Read alone, the surface would silence every TUI approval, and Q14f names the
TUI as a terminal front end. The CLI's own prompt fires with `surface="cli"` (`approval.py:1766-1786`).

**An event missing both fields notifies.** It cannot say where it came from, and the repository's
direction is to fail toward notifying. On the paths above that happens only for an agent built with no
platform, such as `batch_runner.py`'s, in a process that never named a terminal platform.

**A kanban worker is a terminal process nobody watches (A4).** The kanban dispatcher starts each worker
as `hermes -p <assignee> chat -q "work kanban task <id>"` with `HERMES_KANBAN_TASK=<id>` in its
environment and its output in a log file (`hermes_cli/kanban_db.py:7345-7505`), so its agent is
`platform="cli"` and, on the default profile, pns-hooks loads in it. Its turn start and end are recorded
only: no turn clock, no summarizer, no `done` card. A failure (`stop-failure`), an approval (`blocked`)
and a question (`asked`, from `clarify` or `kanban_block`) take the ordinary arms and notify. A recorded
`prompt` or `stop` still ends the session's wait, as the ordinary arms do, so a worker's question lights
the blocked lamp until the worker's turn ends and never longer. A worker's approvals reach the presence
gate like a CLI approval, and away they go to moshi as moshi's own plugin sent them. A worker reads no
input (its stdin is `/dev/null`), so the stdin prompt denies at once (`tools/approval.py:1044-1076`) and
its answer clears the card.

## What each kind of session gets

**A terminal session gets exactly Claude Code's arms**, reached through the same `pns hook` verbs:
turn markers and the lamp, the summarizer on `stop`, `failed` on `stop-failure`, `asked` for questions,
the presence gate and the reminder on `blocked`, the wait cleared on `resolved`.

**A recorded-only event gets one activity row and nothing else from pns.** The row carries the verb's
state word (`stop` is recorded as `done` with a preview of the reply as its detail, `stop-failure` as
`failed`, the others as the verb itself) and the session is named from its first prompt. It writes no
turn marker and starts no wait or reminder, runs no summarizer, forwards nothing to moshi and fires no
destination, so no banner, no phone card, no lamp and no `#pns-events` line. A recorded `prompt` or
`stop` ends any wait the session holds, which only a kanban worker's ordinary arms can have started.
The recap reads these rows like any other (`pns-domain/src/recap/night.rs:180-227`). moshi's feed is
the shim's, and it reaches moshi whatever pns records.

This also closes a loop: pns posts to Hermes's gateway, and the `explain` route hands a posted page to
an agent run on platform `webhook`. That run's events are recorded only, so nothing pns records from
Hermes's gateway is ever posted back to it.

## What moshi receives

The operator uses moshi's Chat View for Hermes (A3), so moshi keeps receiving what its own plugin sent,
from two roads: the shim feeds it every non-approval event directly, and pns hands it the approval pair
behind the presence gate. moshi's category model is in the tap's `docs/hooks.md:14-30` (rev `971974c`)
and its Hermes rows at `:155-173`.

**The feed.** Each hook below calls `<moshi-hook> hermes-hook` with the payload moshi's own plugin
(`~/.hermes/plugins/moshi-hooks/__init__.py:53-80`) built for it, unconditionally: every platform,
gateway and kanban sessions included, the background review thread included. Every payload also carries
`hook_event_name`, `session_id` and `cwd`, built the way that plugin built them.

| Hermes hook | moshi event | Payload beyond the three | What it feeds in moshi |
| --- | --- | --- | --- |
| `on_session_start` | `SessionStart` | `model`, `platform` | a new session, "stored silently until the first prompt" |
| `pre_llm_call` | `UserPromptSubmit` | `prompt`, `model`, `platform` | `session_started`: the session's running inbox row |
| `pre_tool_call` | `PreToolUse` | `tool_name`, `tool_call_id`, and `tool_input` for `clarify` | `tool_running`: the running row's tool activity |
| `post_tool_call` | `PostToolUse` | `tool_name`, `tool_call_id` | `tool_finished` |
| `post_llm_call` | `AgentEnd` | `last_assistant_message`, `model`, `platform` | `task_complete`: the completed row |
| `on_session_end`, not completed or interrupted | `TurnInterrupted` | `interrupted`, `model`, `platform` | `task_complete` |
| `on_session_finalize`, with a session id | `SessionEnd` | `platform` | `session_ended`: clears the running state |

Those rows are moshi's view of a Hermes session; Chat View reads the conversation itself from
`$HERMES_HOME/state.db` (`docs/usage.md:307`). The shim's feed functions build each payload exactly as
that plugin's functions did, and a scratch run of both plugins over one scenario (ten events, the edge
cases included: a `task_id`-only session, a capitalized `Clarify`, an empty session id, a finalize with
no id) produced identical payloads. Since the feed repeats what moshi already received, it pushes nothing
new.

**The request.** `pns hook blocked` for producer `hermes` goes through `blocking_event`, the same code
Claude and Codex use: the activity row, the forward, the phone-leg suppression only when the spawn
started, the reminder, the notification, the bounded wait. `moshi_subcommand` gains one arm:
`hermes` maps to `hermes-hook` when `surface == "cli"` (and, once H10 passes, when `platform == "tui"`,
below). That is the one Hermes surface moshi's own plugin ever forwarded, and moshi's docs say its
bridge answers a Hermes prompt "only when its terminal bridge can verify the visible command and
approval menu" (`docs/hooks.md:170-173`). pns hands
moshi the payload byte for byte, so the shim's payload is moshi's contract plus `platform`, which
moshi's own plugin already sends on its lifecycle events, and, in a kanban worker, `kanban_task`.

**The answer.** moshi clears its card when `PermissionResolved` arrives with the card's `action_id`
("Approval resolves | Clears the pending action", `docs/hooks.md:167`). pns hands moshi the answer to
exactly the requests it handed moshi:

- When `blocking_event`'s spawn of `moshi-hook hermes-hook` starts, pns writes an empty marker named by
  the request's `action_id` under `<state dir>/moshi-forwards/`. An `action_id` that fails the
  session-id rule (`pns-domain/src/safety.rs:52-59`) writes nothing; the shim's is a 32-character hex
  `uuid4`.
- `pns hook resolved` for producer `hermes` with `hook_event_name == "PermissionResolved"` and a
  payload `moshi_subcommand` admits removes that marker, and only when the removal succeeds forwards
  the payload to `moshi-hook hermes-hook`, bounded by the same acknowledgement deadline.

The answer is not presence-gated: an operator who walked away after the card went out can come back and
answer at the pane, and the card still has to clear. The marker keeps that ungated forward to cards
moshi raised. moshi's own plugin sent every CLI request, so every non-empty `action_id` it later
resolved was one moshi had registered; with pns gating the request at the desk, a desk answer would
otherwise hand moshi a fresh `action_id` it never saw, a path the 0.3.26 binary names ("no pending
approval with this action id") with an unknown visible effect. It also spares a moshi-hook spawn per
desk answer.

**The TUI's approvals (A2).** They report `surface: "gateway"`, which moshi's own plugin never
forwarded and moshi's docs describe as observed by Hermes alone ("Smart-mode and gateway decisions are
observed by Hermes alone and are not exposed as terminal actions", `docs/hooks.md:172-173`), while the
same docs list "Approval request in the interactive CLI/TUI" as carded (`:166`). Whether moshi can type
an answer into the TUI is therefore measured, not assumed: the plan's last task admits a Hermes approval
whose `platform` is `tui` to `moshi_subcommand`, and its pull request opens only after a phone drill with
that branch's build passes. Until then, and if it fails, an away TUI approval gets pns's own phone card,
which cannot answer.

## Installing, enabling and upgrading

`pns hermes install-plugin` takes no arguments and:

1. Resolves the Hermes home: `HERMES_HOME` trimmed when non-empty, else `$HOME/.hermes`, the order
   `get_hermes_home` uses. Neither set is exit 2.
2. Renders both files and writes each one whose bytes differ, by a temporary file and a rename in the
   same directory. A run that would write the same bytes writes nothing.
3. Runs `hermes plugins enable pns-hooks` (the binary from `PNS_HERMES_BIN`, else `hermes` on `PATH`),
   bounded at 30 s, with its output captured. A failure is exit 2 with one line saying Hermes sessions
   do not reach pns until the plugin is enabled and naming the command to run; the files stay written.
4. When a file changed, prints one line on stderr naming the directory and saying Hermes loads it at
   its next start, with `hermes gateway restart` for chat sessions. A run that changed nothing prints
   nothing, so a no-op apply stays quiet.

Enabling goes through Hermes's own command because `config.yaml` is Hermes's file, and on dresden it is
also a chezmoi modify target holding secrets, so pns never parses it. The modify template does not own
`plugins.enabled`, so Hermes's write passes through the next apply untouched. Like `install-hooks`, the
command re-heals on every apply, so an operator who disables the plugin with `hermes plugins disable`
gets it back at the next apply.

**Upgrades.** The rendered bytes are the version: the shim's source and the two binary paths, since
the manifest's own `version` never moves. The pns builder
(`run_onchange_after_58-build-pns-engine.sh.tmpl`) installs a new binary before
`run_after_72-pns-hermes-plugin` runs `install-plugin`, so a new shim is written in the same apply and
an unchanged one is left alone. A running Hermes process keeps the shim it imported until it restarts;
the gateway needs `hermes gateway restart`. A stale shim still calls the same binary path, and pns treats
every payload field as optional and an unknown verb as one stderr line with exit 0, so version skew can
lose a notification and cannot fail a turn. A `cargo install` outside chezmoi needs `pns hermes
install-plugin` run again, the same as `install-hooks`.

## Retiring the shell hook pair and moshi's plugin

Both go in the same apply that first runs `install-plugin`, so no apply leaves two reporters of one
approval.

**The shell hook pair.** With the plugin installed, both would fire for every approval and pns would
report it twice. The modify template keeps ownership of `hooks.pre_approval_request` and
`hooks.post_approval_response` and removes both keys, the same whole-value ownership that removes an
undeclared webhook route. Dropping the two keys from the template's owned set instead would leave the
live entries in place, because the template hands back every key it does not own. Hermes's other hook
events pass through untouched. The consent entries in `~/.hermes/shell-hooks-allowlist.json` become
inert.

**moshi's plugin.** `moshi-hook uninstall --target hermes` is moshi's own supported removal:
`uninstall --help` lists `hermes` among its targets, `docs/usage.md:196` says it removes "Moshi-owned
entries", and the 0.3.26 binary carries `install.UninstallHermes` and `install.setHermesPluginEnabled`,
so it also takes the name back out of `plugins.enabled`. It runs from
`run_once_after_60-moshi-hook-setup.sh.tmpl` by widening that script's existing Codex line to
`moshi-hook uninstall --target codex,hermes`. This repository builds no removal mechanism of its own:
no `.chezmoiremove`, no retirement script. Editing the script re-fires it once on the next apply
(`run_once_` tracks content), which re-runs its idempotent pair check, install, tap trust and service
start. The re-fire does not re-pair: the script skips `pair` when `moshi-hook status` prints `paired`
(`run_once_after_60-moshi-hook-setup.sh.tmpl:49-62`), and the 2026-09-20 re-fire logged
`moshi-hook already paired -- skipping pair`. Script order puts it (60) before
`run_after_72-pns-hermes-plugin` (72).

**moshi's install targets already leave Hermes out.** Line 64 installs
`opencode,gemini,cursor,kimi,qwen,grok,omp,pi`, so no apply reinstalls moshi's Hermes plugin. Two other
paths can: the phone app's Integrations install (`POST /v1/integrations/install`, where `{}` means every
target) and a bare `moshi-hook install`, which the vendored moshi skill tells agents to run
(`dot_agents/skills/moshi/SKILL.md:198`). Either would load moshi's plugin beside pns's, so moshi would
receive every event twice and card every approval twice.

**The guard (A1).** The config modify template owns `moshi-hooks` as one MEMBER of `plugins.disabled`:
it appends the name when it is missing and hands the rest of the list back to Hermes. Hermes's deny list
wins over `plugins.enabled` for every plugin it discovers (`hermes_cli/plugins.py:1269-1285`), so a
reinstalled plugin stays unloaded. `hermes plugins enable` rewrites the list sorted and keeps the entry
(`hermes_cli/plugins_cmd.py:707-714`, `:772-798`). moshi's own installer or uninstaller may take the
entry out (the 0.3.26 binary carries `plugins:disabled`); the next apply puts it back, so the exposure
lasts until the next apply, not until someone notices. The first apply that adds the entry re-serializes
`config.yaml` the way every template change does (the template's header describes it); an apply after
that is a byte-for-byte no-op. The render was checked against sample live files in a scratch source
state: the entry is appended once, a file already carrying it is left alone, and `plugins.enabled`
survives.

## The producer value

The agent value is `hermes`, set by the shim as `PNS_PRODUCER=hermes` in each child's environment. It is
the value the retired shell hooks already set, it is what the activity rows, the sessions table and the
recap (`hermes/done`) show, and it is the only value `moshi_subcommand` and the recorded-only branch
match. It names the producer; the `[plugins.log] type = "hermes"` destination is a separate field.

## Drills

The operator runs these after the apply that lands the dotfiles pull request and after
`hermes gateway restart`. Record them in `docs/remaining-work.md` under "pns validation follow-ups".

- **Preflight.** `hermes plugins list --user` shows `pns-hooks` enabled and `moshi-hooks` absent or
  disabled; `hermes hooks list` shows no `pns hook` command.
- **H1, desk approval.** In a herdr pane run `hermes chat --cli` (the flag pins the classic CLI whatever
  `display.interface` says) and ask it to run `rm -rf /tmp/pns-hermes-drill`. Expect a pns banner naming
  the command, no phone card, and the blocked lamp if lamps are on. Answer once at the pane: the lamp
  clears, no moshi approval card appears (pns never forwarded the request, so it forwards no answer),
  and the turn ends with a `hermes` done banner.
- **H2, phone approval (the Q14f drill).** Same prompt with the surface Mobile or Away, as drill D8
  set it. Expect exactly one moshi card. Approve on the phone: moshi types the answer into the pane, the
  command runs, the card clears. Repeat with Deny and record what happens (D8 left a known upstream
  moshi-hook deny issue).
- **H3, answered at the desk after the card.** Go away, let the card arrive, come back and answer at the
  pane. Expect the phone card to clear.
- **H4, question.** Ask Hermes to use its clarify tool to ask which of two colors to use. Expect an
  `asked` notification, cleared when answered.
- **H5, failure.** `hermes chat --cli --max-turns 1` with a request that needs two tool calls. Expect a
  `failed` notification.
- **H6, gateway.** Message the Hermes Discord bot with a request that runs a tool. Expect no pns banner,
  no pns phone card, no lamp change and no `#pns-events` line; `pns recap --since 15m` lists the
  session's `hermes` rows; moshi's inbox shows the Discord session as it did under moshi's own plugin.
- **H7, moshi's screens (A3).** In `hermes chat --cli`, run two turns, then `/exit`. Expect moshi's
  inbox to show the session running during each turn and complete after it, Chat View to open it with
  both turns, and the row to clear at the exit, as before the cutover.
- **H8, the TUI as a terminal session (A2).** In `hermes chat --tui`, a turn ends with a `hermes` done
  banner and a desk approval raises a pns banner and the blocked lamp. Away, the approval gets pns's own
  phone card until the TUI task lands.
- **H9, a kanban worker (A4).** Two kanban tasks for the default profile: one the worker finishes on its
  own, one that tells it to block on a question with `kanban_block`. Expect no banner and no card for the
  first worker's turns and a `done` row for it in `pns recap --since 30m`, and one `asked` notification
  naming the second worker's question.
- **H10, moshi answers the TUI (A2's proof, the TUI task's gate).** With the TUI task's branch build
  written into the live plugin (`./pns/target/release/pns hermes install-plugin` from its worktree, since
  the shim calls the binary that wrote it), away, in `hermes chat --tui`: exactly one moshi card, and
  approving on the phone types the answer into the TUI and runs the command; then Deny. Afterwards
  `~/.cargo/bin/pns hermes install-plugin` points the plugin back. A pass opens that task's pull request;
  a fail closes the task with no pull request.

## Known limits

- **Gateway approvals leave no row.** The gateway keys approvals by `agent:<profile>:...`
  (`gateway/session.py:688`), which fails pns's session-id rule (`pns-domain/src/safety.rs:52-59`
  refuses `:`). The session's prompt and turn rows use the agent's timestamp id and reach the recap.
- **An away TUI approval gets pns's own phone card**, which cannot answer, until H10 passes and the TUI
  task lands; for good if H10 fails.
- **moshi sees what its own plugin showed it.** A3 forwards unconditionally, so moshi's inbox keeps
  showing Discord sessions, kanban workers and the background review's replayed turn, exactly as it did
  under moshi's plugin. pns's own delivery rules (Q14e, A4) govern pns's banner, card, lamp and Discord
  line only.
- **Only the default `HERMES_HOME`.** The four profiles under `~/.hermes/profiles/` have their own
  plugin directories and are not installed into, so a kanban task assigned to a named profile is
  invisible to pns (moshi's plugin did not reach it either).
- **A sticky Hermes profile is not the default home.** `install-plugin` writes under `$HERMES_HOME` or
  `~/.hermes`, but the `hermes` launcher follows `~/.hermes/active_profile` when it names a profile
  other than `default` (`hermes_cli/main.py:447-471`), so `hermes plugins enable pns-hooks` then targets
  that profile, exits 1 ("not installed", `hermes_cli/plugins_cmd.py:779-782`) and every apply warns.
  No `active_profile` exists on dresden.
- **A forwarded request whose answer never arrives** (Hermes killed at its prompt) leaves one empty
  marker file under `<state dir>/moshi-forwards/`.
- **Pinned Hermes facts.** The shim depends on these, each cited above; re-check them whenever
  `.chezmoidata/hermes.yaml` moves: the background review's thread name `bg-review`; the curator's
  `platform="curator"` and its own session id; the approval hooks' `session_key` being the terminal
  agent's session id; the TUI's approvals reporting `surface="gateway"`; `on_session_end` and
  `post_llm_call` firing on the thread that runs the turn; a kanban worker's `HERMES_KANBAN_TASK` and its
  `kanban_block` tool.
- **Pinned moshi facts.** The feed reproduces moshi-hook 0.3.26's generated Hermes plugin. A moshi-hook
  upgrade that changes that template is invisible to the shim; re-extract the template from the new
  binary (as the 0.3.26 one was, with `strings`) and diff it against the feed before taking the upgrade.
- **One pns worker per process.** A slow `stop` (the summarizer) delays the next pns event from the same
  Hermes process; order is kept, and moshi's feed has its own worker.
- **The activity row's model column stays empty** for Hermes: pns reads the model from a transcript file
  and Hermes keeps its history in `state.db`.

## Decisions made in the operator's place

1. The plugin is named `pns-hooks`, mirroring `moshi-hooks` and naming what it holds.
2. A turn's end comes from `on_session_end`, never from `post_llm_call`, which also fires for a failed
   turn that produced text.
3. An interrupted turn maps to `resolved`, as Codex's `Interrupt` does.
4. `platform` decides before `surface`, because TUI approvals report `surface="gateway"`.
5. Every named platform other than `cli` and `tui` is recorded only, including `subagent`, `curator`,
   `cron`, `acp`, `webhook` and `api_server`.
6. The shim stamps approval and tool events with the platform their own session named, falling back
   to the last terminal platform the process named, so a curator or subagent run never restamps an
   operator's approval.
7. pns hands moshi Hermes approvals from `surface == "cli"`, the only surface moshi's own plugin
   forwarded, and from `platform == "tui"` only after H10 passes.
8. The answer to an approval is forwarded to moshi without the presence gate, so a card clears however
   the operator answered, and only when pns forwarded that `action_id`'s request.
9. `blocked` carries `--remind=5m`, Codex's value: a bare `--remind` refuses with exit 2 when the
   config sets no delay (`remind_schedule_runtime.rs:84-86`).
10. An MCP elicitation (`surface="mcp-elicitation"`) maps to `asked`, matching Claude's `Elicitation`.
11. Only `clarify` tool calls, and `kanban_block` inside a kanban worker, cross to pns.
12. Delivery is asynchronous, one worker per destination, with a 120 s backstop for pns and moshi's own
    10 s for moshi.
13. `install-plugin` enables through `hermes plugins enable` on every run, re-healing like
    `install-hooks`.
14. The shell hook pair is removed by the modify template rather than left for Hermes to keep.
15. moshi's plugin goes through `moshi-hook uninstall --target codex,hermes` in `run_once_after_60`.
16. The producer value stays `hermes`.
17. The default Hermes home only; profiles are out of scope.
18. The background review is recognized by its thread name, the one thing that tells it apart.
19. Every summarizer run pns starts carries `PNS_SUMMARIZING=1`, the Codex turn summarizer's existing
    variable, and the shim registers nothing under it, for moshi as for pns.
20. The manifest's `version` is fixed at `"1"`, so only a changed shim or binary path rewrites the
    plugin.
21. The shim feeds moshi directly, rather than through pns, so each payload is built by moshi's own
    plugin's functions and pns's slow calls never delay it; it uses pns's moshi-hook path
    (`moshi_hook_bin`), rendered at install time.
22. A kanban worker's `kanban_block` is its "blocked" (A4): it maps to `asked`, which notifies and
    lights the blocked lamp, and there is no `resolved` for it.
23. A kanban worker is recognized by `kanban_task` on the payload, from `HERMES_KANBAN_TASK`, and a
    recorded `prompt` or `stop` ends the session's wait, so a worker's lamp ends with its turn.
24. The TUI's moshi forward is its own last task, gated on H10 run with that branch's build, so a
    moshi that cannot answer the TUI never suppresses pns's own card.

## Answered questions

All four were answered on 2026-09-22 (the rulings above); where each landed:

1. **Guard against moshi reinstalling its plugin (A1): yes.** The modify template owns `moshi-hooks` as
   a member of `plugins.disabled` ("The guard", under "Retiring"; plan Task 8).
2. **The TUI (A2): a terminal session, approvals included, once moshi is proven to answer it.** Its
   events already take the terminal arms; its moshi forward is the plan's Task 10, gated on H10.
3. **moshi's Hermes screens (A3): kept fed.** The shim forwards every non-approval event as moshi's own
   plugin did ("What moshi receives"; plan Task 5), and only the approval pair goes through pns's gate.
4. **Kanban workers (A4): turns recorded, blocks, failures and questions notify** ("Terminal or recorded
   only"; plan Tasks 1 and 5).

## Files this design touches

pns (`pns/crates/`):

- `pns-adapters/src/harness/payload.rs`: `platform`, `surface`, `kanban_task` and `action_id` fields.
- `pns-adapters/src/harness/hermes.rs` (new): `hermes_records_only(event, payload)`.
- `pns-adapters/src/harness/message.rs`: an approval's card text from `description` and `command`.
- `pns-adapters/src/harness/routing.rs`: `moshi_subcommand` takes the payload; later the TUI arm.
- `pns-adapters/src/hermes_plugin.rs` and `hermes_plugin/pns_hooks.py` (new): render, install, enable;
  the pns reports and the moshi feed.
- `pns-adapters/src/recap/summarizer.rs`: `PNS_SUMMARIZING=1` on every summarizer child.
- `pns/src/hermes_session.rs` (new): the recorded-only row.
- `pns/src/hook_dispatch.rs`: the recorded-only branch and the answer forward.
- `pns/src/moshi_submission.rs`: the forwarded-request marker, `forward_resolution` and the payload at
  the call site.
- `pns/src/command_hermes.rs` (new), `invocation.rs`, `lib.rs`, `subcommand_usage.rs`,
  `legacy/usage.rs`: the `pns hermes install-plugin` verb.
- `pns/tests/support/sandbox/commands.rs`: `PNS_HERMES_BIN` fenced off by default, like
  `PNS_MOSHI_HOOK_BIN`.

Dotfiles:

- `.chezmoiscripts/run_after_72-pns-hermes-plugin.sh.tmpl` (new).
- `private_dot_hermes/modify_private_config.yaml` (the shell hook pair removed, `moshi-hooks` kept in
  `plugins.disabled`) and `test/unit/hermes-config-modify-template.test.sh`.
- `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl`.
- `CLAUDE.md`: one sentence beside the Codex one.
