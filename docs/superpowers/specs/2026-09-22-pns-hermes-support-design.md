# pns Hermes support design

Status: proposed, 2026-09-22. Implementation plan:
`docs/superpowers/plans/2026-09-22-pns-hermes-support.md`.

## Purpose

pns reports Claude Code and Codex sessions: a finished turn is `done`, a dead one is `failed`, a question
or an approval reaches the operator through the presence gate, and an away operator answers an approval
from the phone because pns hands it to `moshi-hook`. Hermes Agent sessions get none of that today. This
design gives Hermes the same treatment for terminal sessions, records chat gateway sessions for the recap
only, and moves the Hermes phone card from moshi's own plugin to pns's forward.

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

## What exists today

All of this was read on dresden against Hermes Agent v0.17.0 (`~/.hermes/hermes-agent`, pinned by
`.chezmoidata/hermes.yaml`) and moshi-hook 0.3.26.

**Hermes plugins are opt-in.** Hermes scans `$HERMES_HOME/plugins/<name>/` for a `plugin.yaml` and an
`__init__.py` with `register(ctx)` (`hermes_cli/plugins.py:1-20`, `:1240`). A user plugin loads only
when its name is in `plugins.enabled` in `config.yaml` (`plugins.py:223-248`, `:1333-1345`: "not enabled
in config (run `hermes plugins enable {}` to activate)"). `hermes plugins enable <name>`
(`hermes_cli/plugins_cmd.py:772`) is Hermes's own writer of that list; it is idempotent and prints
"already enabled" when there is nothing to do. `HERMES_HOME` defaults to `~/.hermes`
(`hermes_constants.py:54-76`). Discovery runs in every Hermes process: the CLI and the TUI through
`model_tools.py:201`, the gateway at `gateway/run.py:5700`.

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

- `plugin.yaml`: `name: pns-hooks`, `version: "<pns version>"`, and a one-line description.
- `__init__.py`: the shim, embedded in the binary with `include_str!`, with one substitution: the
  absolute, canonicalized path of the binary that wrote it (`current_exe`, as `install-hooks` does),
  written as a JSON string literal, which Python reads as a string literal.

The shim registers seven hooks and spawns `<pns> hook <verb> [flags]` with the event's JSON on stdin,
`PNS_PRODUCER=hermes` added to the inherited environment, and stdout and stderr discarded. The
inherited environment carries `HERDR_PANE_ID` and `HERDR_WORKSPACE_ID` from the Hermes process, which
is how pns learns the pane (`hook_dispatch.rs` reads `HERDR_PANE_ID` on every arm).

Delivery copies moshi's model. Every callback enqueues and returns `None` at once, so no callback
blocks Hermes, injects context (`pre_llm_call`'s return value) or blocks a tool (`pre_tool_call`'s). One
worker thread runs the calls in order, so a prompt reaches pns before its approval and an approval
before its answer. Each call has a 120 s backstop timeout; pns bounds its own waits well inside that
(the summarizer at 30 s, `pns-adapters/src/codex.rs:86`, and moshi's acknowledgement at 5 s by default).
A call submitted after interpreter shutdown closed the pool runs inline. Every exception is swallowed:
the shim can lose a notification and can never fail a Hermes turn.

### The hook-to-verb mapping

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
| `pre_tool_call` | tool `clarify` only | `PreToolUse` | `asked` | `tool_name`, `tool_call_id`, `tool_input` |
| `post_tool_call` | tool `clarify` only | `PostToolUse` | `resolved` | `tool_name`, `tool_call_id` |

The turn's end comes from `on_session_end`, which fires exactly once at the end of every
`run_conversation` with `completed` and `interrupted` (`agent/turn_finalizer.py:466-478`).
`post_llm_call` fires whenever a turn has a final response and was not interrupted (`:339-354`), and a
failed turn can have one (`completed` also requires `not failed` and an unexhausted budget, `:126-133`),
so mapping `post_llm_call` to `stop` would report a failed turn twice. The shim keeps the reply from
`post_llm_call`, keyed by session, and hands it to whichever verb `on_session_end` picks.

An interrupted turn maps to `resolved`, as Codex's `Interrupt` does (`codex_hooks.rs:49-53`): the
operator ended it, so any wait it held is answered. Questions notify three ways: `clarify` (Hermes's
ask-the-user tool) and an MCP elicitation map to `asked`, and a turn that ends on a question becomes
`asking` through the same summarizer every Claude turn goes through.

`on_session_start`, `on_session_finalize`, `subagent_start`, `subagent_stop` and every other hook stay
unregistered: pns names a session from its first prompt and has no session-boundary verb. Tool calls
other than `clarify` never leave the process.

### The platform stamp

Only the lifecycle hooks carry `platform`. The approval hooks carry `surface` and `session_key`
(`plugins.py:162-167`), and the tool hooks carry neither (`plugins.py:1933-1943`). The shim therefore
stamps `platform` on every payload: the event's own platform when it has one, else the last platform
this process named. A platform of `subagent` is sent on its own events and never becomes the process's
remembered platform.

That works because one Hermes process hosts one front end. The classic CLI builds its agent with
`platform="cli"` (`hermes_cli/cli_agent_setup_mixin.py:372`; one-shot `hermes chat -q` at
`hermes_cli/oneshot.py:343`), the TUI backend with `"tui"` (`tui_gateway/server.py:4233`), the gateway
with each message's platform. Delegated children run inside their parent's process with
`platform="subagent"` (`tools/delegate_tool.py:1246`), which is why that value is excluded.

## Terminal or recorded only

pns decides, from the payload's `platform` and `surface`, for events whose producer is `hermes`:

| `platform` | `surface` | The session is |
| --- | --- | --- |
| `cli` or `tui` | anything | terminal |
| any other non-empty value | anything | recorded only |
| empty | `gateway` | recorded only |
| empty | `cli`, `mcp-elicitation`, anything else, or absent | terminal |

Every non-terminal platform Hermes names is recorded only: the chat gateways (`discord`, `telegram` and
the rest), `webhook` (the `explain` agent route), `api_server`, `cron` (`cron/scheduler.py:2497`), `acp`
(`acp_adapter/session.py:592`) and `subagent`. None of them has an operator at a pane.

**Platform decides before surface** because the TUI's approvals report `surface="gateway"`. The TUI
backend sets `HERMES_GATEWAY_SESSION=1` for its whole process (`tui_gateway/server.py:1828-1832`), so
`check_all_command_guards` takes the gateway branch and fires the hook with `surface="gateway"`
(`tools/approval.py:1686`). Read alone, the surface would silence every TUI approval, and Q14f names the
TUI as a terminal front end. The CLI's own prompt fires with `surface="cli"` (`approval.py:1766-1786`).

**An event missing both fields notifies.** It cannot say where it came from, and the repository's
direction is to fail toward notifying. On the paths above that happens only in a process that never
named a platform, such as `batch_runner.py`'s agents.

## What each kind of session gets

**A terminal session gets exactly Claude Code's arms**, reached through the same `pns hook` verbs:
turn markers and the lamp, the summarizer on `stop`, `failed` on `stop-failure`, `asked` for questions,
the presence gate and the reminder on `blocked`, the wait cleared on `resolved`.

**A recorded-only session gets one activity row per event and nothing else.** The row carries the
verb's state word (`stop` is recorded as `done` with a preview of the reply as its detail,
`stop-failure` as `failed`, the others as the verb itself) and the session is named from its first
prompt. It writes no turn marker, no wait marker and no reminder, runs no summarizer, forwards nothing
to moshi and fires no destination, so no banner, no phone card, no lamp and no `#pns-events` line. The
recap reads these rows like any other (`pns-domain/src/recap/night.rs:180-227`).

This also closes a loop: pns posts to Hermes's gateway, and the `explain` route hands a posted page to
an agent run on platform `webhook`. That run's events are recorded only, so nothing pns records from
Hermes's gateway is ever posted back to it.

## The moshi forward

**The request.** `pns hook blocked` for producer `hermes` goes through `blocking_event`, the same code
Claude and Codex use: the activity row, the forward, the phone-leg suppression only when the spawn
started, the reminder, the notification, the bounded wait. `moshi_subcommand` gains one arm:
`hermes` maps to `hermes-hook`, and only when `surface == "cli"`. That is the one Hermes surface moshi's
own plugin ever forwarded, and moshi's docs say its bridge answers a Hermes prompt "only when its
terminal bridge can verify the visible command and approval menu" (`docs/hooks.md:170-173`). pns hands
moshi the payload byte for byte, so the shim's payload is moshi's contract plus one key, `platform`,
which moshi's own plugin already sends on its lifecycle events.

**The answer.** moshi clears its card when `PermissionResolved` arrives with the card's `action_id`
("Approval resolves | Clears the pending action", `docs/hooks.md:167`). `pns hook resolved` for producer
`hermes` with `hook_event_name == "PermissionResolved"` and `surface == "cli"` forwards the payload to
`moshi-hook hermes-hook`, bounded by the same acknowledgement deadline. It is not presence-gated: an
operator who walked away after the card went out can come back and answer at the pane, and the card
still has to clear. A desk answer reaches moshi for an `action_id` it never carded; moshi's own plugin
already sends answers with an empty `action_id` when it has nothing to pair, and the drill confirms no
card or push results.

**Nothing else is forwarded**, which is Claude's arrangement: pns hands moshi approvals only.

## Installing, enabling and upgrading

`pns hermes install-plugin` takes no arguments and:

1. Resolves the Hermes home: `HERMES_HOME` trimmed when non-empty, else `$HOME/.hermes`, the order
   `get_hermes_home` uses. Neither set is exit 2.
2. Renders both files and writes each one whose bytes differ, by a temporary file and a rename in the
   same directory. A run that would write the same bytes writes nothing.
3. Runs `hermes plugins enable pns-hooks` (the binary from `PNS_HERMES_BIN`, else `hermes` on `PATH`),
   bounded at 30 s, with its output captured. A failure is exit 2 with one line naming the command to
   run; the files stay written.
4. When a file changed, prints one line on stderr naming the directory and saying Hermes loads it at
   its next start, with `hermes gateway restart` for chat sessions. A run that changed nothing prints
   nothing, so a no-op apply stays quiet.

Enabling goes through Hermes's own command because `config.yaml` is Hermes's file, and on dresden it is
also a chezmoi modify target holding secrets, so pns never parses it. The modify template does not own
`plugins.enabled`, so Hermes's write passes through the next apply untouched. Like `install-hooks`, the
command re-heals on every apply, so an operator who disables the plugin with `hermes plugins disable`
gets it back at the next apply.

**Upgrades.** The rendered bytes are the version: the shim's source and the binary path. The pns
builder (`run_onchange_after_58-build-pns-engine.sh.tmpl`) installs a new binary before
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
start. Script order puts it (60) before `run_after_72-pns-hermes-plugin` (72).

**moshi's install targets already leave Hermes out.** Line 64 installs
`opencode,gemini,cursor,kimi,qwen,grok,omp,pi`, so no apply reinstalls moshi's Hermes plugin. Two other
paths can: the phone app's Integrations install (`POST /v1/integrations/install`, where `{}` means every
target) and a bare `moshi-hook install`. Open question 1 covers them.

## The producer value

The agent value is `hermes`, set by the shim as `PNS_PRODUCER=hermes` in each child's environment. It is
the value the retired shell hooks already set, it is what the activity rows, the sessions table and the
recap (`hermes/done`) show, and it is the only value `moshi_subcommand` and the recorded-only branch
match. It names the producer; the `[plugins.log] type = "hermes"` destination is a separate field.

## Drills

The operator runs these after the apply that lands the dotfiles pull request and after
`hermes gateway restart`. Record them in `docs/remaining-work.md` under "pns validation follow-ups".

- **Preflight.** `hermes plugins list --user` shows `pns-hooks` enabled and no `moshi-hooks`;
  `hermes hooks list` shows no `pns hook` command.
- **H1, desk approval.** In a herdr pane run `hermes chat --cli` (the flag pins the classic CLI whatever
  `display.interface` says) and ask it to run `rm -rf /tmp/pns-hermes-drill`. Expect a pns banner naming
  the command, no phone card, and the blocked lamp if lamps are on. Answer once at the pane: the lamp
  clears, no moshi card or push appears (the answer forward for an `action_id` moshi never carded), and
  the turn ends with a `hermes` done banner.
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
- **H6, gateway.** Message the Hermes Discord bot with a request that runs a tool. Expect no banner, no
  phone card, no lamp change and no `#pns-events` line, and `pns recap --since 15m` listing the session's
  `hermes` rows.

## Known limits

- **Gateway approvals leave no row.** The gateway keys approvals by `agent:<profile>:...`
  (`gateway/session.py:688`), which fails pns's session-id rule (`pns-domain/src/safety.rs:52-59`
  refuses `:`). The session's prompt and turn rows use the agent's timestamp id and reach the recap.
- **TUI approvals get pns's own phone card**, which cannot answer. moshi's plugin never forwarded them.
  Open question 2.
- **moshi learns a Hermes session only at its first forwarded approval.** Its Hermes session list and
  its Hermes completion pushes stop with its plugin. Open question 3.
- **Only the default `HERMES_HOME`.** The four profiles under `~/.hermes/profiles/` have their own
  plugin directories and are not installed into.
- **One worker per process.** A slow `stop` (the summarizer) delays the next event from the same Hermes
  process; order is kept.
- **The activity row's model column stays empty** for Hermes: pns reads the model from a transcript file
  and Hermes keeps its history in `state.db`.

## Decisions made in the operator's place

1. The plugin is named `pns-hooks`, mirroring `moshi-hooks` and naming what it holds.
2. A turn's end comes from `on_session_end`, never from `post_llm_call`, which also fires for a failed
   turn that produced text.
3. An interrupted turn maps to `resolved`, as Codex's `Interrupt` does.
4. `platform` decides before `surface`, because TUI approvals report `surface="gateway"`.
5. Every named platform other than `cli` and `tui` is recorded only, including `subagent`, `cron`,
   `acp`, `webhook` and `api_server`.
6. The shim stamps approval and tool events with its process's last named platform, excluding
   `subagent`.
7. moshi receives Hermes approvals only from `surface == "cli"`, the only surface its own plugin
   forwarded.
8. The answer to a CLI approval is forwarded to moshi without the presence gate, so a card clears
   however the operator answered.
9. `blocked` carries `--remind=5m`, Codex's value: a bare `--remind` refuses with exit 2 when the
   config sets no delay (`remind_schedule_runtime.rs:84-86`).
10. An MCP elicitation (`surface="mcp-elicitation"`) maps to `asked`, matching Claude's `Elicitation`.
11. Only `clarify` tool calls cross to pns.
12. Delivery is asynchronous on one worker with a 120 s backstop, as moshi's is.
13. `install-plugin` enables through `hermes plugins enable` on every run, re-healing like
    `install-hooks`.
14. The shell hook pair is removed by the modify template rather than left for Hermes to keep.
15. moshi's plugin goes through `moshi-hook uninstall --target codex,hermes` in `run_once_after_60`.
16. The producer value stays `hermes`.
17. The default Hermes home only; profiles are out of scope.

## Open questions

1. **Guard against moshi reinstalling its Hermes plugin?** The phone app's Integrations install and a
   bare `moshi-hook install` write it back and enable it, and every approval then raises two cards. For
   Codex the answer was a merge-time strip in pns (#915). For Hermes the recommendation is to have the
   modify template own `moshi-hooks` in `plugins.disabled`, Hermes's deny list, which wins over
   `plugins.enabled` (`plugins.py:207-220`). The alternative is to accept the exposure.
2. **Do you use the Hermes TUI?** If so, should pns try forwarding TUI approvals to moshi? That needs
   its own drill, since moshi's plugin never sent them.
3. **Do you use moshi's Hermes session list, Chat View or Hermes completion pushes?** They stop once
   moshi's plugin is gone. pns could forward `UserPromptSubmit` and `SessionEnd` to moshi if you want
   them back, at the cost of moshi's own pushes outside the presence gate.
4. **Kanban workers on the default profile** run as headless `hermes chat -q` with `platform="cli"`, so
   this design notifies for them like any terminal session. Should they be recorded only instead?

## Files this design touches

pns (`pns/crates/`):

- `pns-adapters/src/harness/payload.rs`: `platform` and `surface` fields.
- `pns-adapters/src/harness/hermes.rs` (new): `hermes_records_only`.
- `pns-adapters/src/harness/message.rs`: an approval's card text from `description` and `command`.
- `pns-adapters/src/harness/routing.rs`: `moshi_subcommand` takes the surface.
- `pns-adapters/src/hermes_plugin.rs` and `hermes_plugin/pns_hooks.py` (new): render, install, enable.
- `pns/src/hermes_session.rs` (new): the recorded-only row.
- `pns/src/hook_dispatch.rs`: the recorded-only branch and the answer forward.
- `pns/src/moshi_submission.rs`: `forward_resolution` and the surface at the call site.
- `pns/src/command_hermes.rs` (new), `invocation.rs`, `lib.rs`, `subcommand_usage.rs`,
  `legacy/usage.rs`: the `pns hermes install-plugin` verb.

Dotfiles:

- `.chezmoiscripts/run_after_72-pns-hermes-plugin.sh.tmpl` (new).
- `private_dot_hermes/modify_private_config.yaml` and `test/unit/hermes-config-modify-template.test.sh`.
- `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl`.
- `CLAUDE.md`: one sentence beside the Codex one.
