# Relay — unified notifications

**Status:** design / awaiting review
**Date:** 2026-06-23

## Goal

`relay` is one notification pipeline that pings me when an agent (Claude, Codex) or a long-running
terminal command needs attention or finishes. Each notification states *which* and *what state* (done vs
blocked vs asked vs plan-ready) so I can triage at a glance, and is delivered to up to three independent
channels:

- **moshi push** — phone; tap takes me into the live session. The remote/actionable path.
- **Hermes webhook → Discord `#notify-log`** — a paper trail / at-a-glance log.
- **local macOS notification** — desktop; clicking brings Ghostty forward and focuses herdr on the exact
  pane that finished (`herdr agent focus`).

The channels are **failure-separated**: one being down never blocks the others.

## Current state (what we're changing)

- **Claude**: a `Stop` hook runs `claude-moshi-notify.sh`, which builds an enriched message
  (project/branch/last-reply) and POSTs to moshi inline. Permission prompts fire a local `alerter` via the
  `Notification` hook.
- **Codex**: moshi-hook owns `~/.codex/hooks.json` and fires its own generic pushes on four events
  (`PermissionRequest`, `SessionStart`, `Stop`, `UserPromptSubmit`) — including a per-prompt push we don't
  want. A separate herdr agent-state hook also sits on `SessionStart`.
- **Shell**: `dot_bashrc.tmpl`'s notifier fires `alerter` at ≥30s and a Hue pulse at ≥5m. No phone push, no
  click-to-focus.
- **Secrets**: moshi's secret lives in `~/.config/moshi/auth.json` (0600, chezmoi-rendered from KeePassXC).
  `~/.hermes/config.yaml` (0600) is **user-maintained, not chezmoi-managed**, and already routes
  `osquery` / `osquery-priority` to Discord.

## Architecture

```
Claude hooks  (done/blocked/asked/plan-ready) ─┐
Codex hooks   (done/blocked) ─────────────────┼─→ relay-agent.sh  (state + message + herdr ids) ─┐
                                               │                                                  ├─→ relay.sh ─→ moshi | hermes | local
shell notifier (command-done) ──────────────────────────────── (its own one-line message) ───────┘        (each failure-separated)
```

- **`relay.sh`** = *how to deliver* (the three channels). Shared by everything.
- **`relay-agent.sh`** = *what an agent says* (state-aware message). Shared by Claude + Codex.
- The shell notifier has nothing to enrich, so it calls `relay.sh` directly.

## States

| state | meaning | applies to |
| --- | --- | --- |
| `done` | finished its turn — your move | Claude, Codex, shell (`command-done`) |
| `blocked` | needs approval / waiting on you | Claude, Codex |
| `asked` | asked you a question | Claude only |
| `plan-ready` | a plan is ready for review | Claude only |

Codex has no AskUserQuestion / ExitPlanMode tools, so `asked` / `plan-ready` don't apply to it.

## Event → state mapping

- **Claude** (`private_dot_claude/modify_settings.json` hooks):
  - `Stop` → `done`
  - `Notification` (matcher `permission_prompt`) → `blocked`
  - `PostToolUse` (matcher `AskUserQuestion`) → `asked`
  - `PostToolUse` (matcher `ExitPlanMode`) → `plan-ready`
- **Codex** (`~/.codex/hooks.json`, newly chezmoi-managed):
  - `Stop` → `done`
  - `PermissionRequest` → `blocked`
  - keep the existing herdr agent-state `SessionStart` hook untouched
- **Shell** (`dot_bashrc.tmpl`): a command past the threshold → `done` (labelled as a command, not an
  agent).

Dropped as noise: `UserPromptSubmit`, `SessionStart`, `SessionEnd`, and the `PreToolUse` setup pair.

## Components

### `relay.sh` — `dot_local/bin/executable_relay.sh`

The single sender. Inputs (final flags settled in the plan): the agent/source label, the state, a title, a
message, an optional last-reply snippet, and the herdr context (`workspace`/`pane` ids) for click-to-focus.
Reads its secrets from `~/.config/relay/auth.json` (0600, chezmoi-rendered from KeePassXC). Fans out to
each requested channel, **each in its own backgrounded `|| true`** so a failure or hang in one never
affects the others, and always exits 0:

- **moshi** — `POST https://api.getmoshi.app/api/webhook`, secret as a token in the JSON body (unchanged
  from today).
- **hermes** — `POST http://127.0.0.1:8644/webhooks/relay` with a JSON body of our fields and header
  `X-Webhook-Signature: <hex HMAC-SHA256(body, hermes_secret)>` (computed with `openssl dgst`). The route
  is `deliver_only: true` → Discord `#notify-log`, so Hermes just renders our `prompt` and forwards it; no
  agent run. Any non-`200` (or an unreachable `:8644`) is a silent miss.
- **local** — `terminal-notifier -title "<agent> · <state> · <project>" -message "<detail>"
  -activate com.mitchellh.ghostty -execute "herdr agent focus <pane_id>"`. Clicking brings Ghostty forward
  and focuses the **exact** finishing pane (see Exact-pane focus). `alerter` can't run a command on click;
  `terminal-notifier` can, so the clickable notifications use it.

### `relay-agent.sh` — `dot_local/bin/executable_relay-agent.sh`

Replaces `claude-moshi-notify.sh`; the POST/secret logic moves to `relay.sh`. Each agent hook invokes it
with the state. It reads the hook's stdin JSON, derives `project` (cwd basename) and `branch`, captures
`HERDR_WORKSPACE_ID` / `HERDR_PANE_ID`, and builds the message:

- `done` → the last-response snippet (transcript parse, as today).
- `blocked` / `asked` / `plan-ready` → the relevant detail from the payload (the command awaiting
  approval, the question text, etc.) when present.

Then calls `relay.sh`. Shared by Claude and Codex.

### Claude hooks — `private_dot_claude/modify_settings.json`

Wire `Stop`, `Notification[permission_prompt]`, `PostToolUse[AskUserQuestion]`,
`PostToolUse[ExitPlanMode]` to `relay-agent.sh <state>`. The existing local `alerter` on permission and the
`claude-stop-pulse.sh` Hue pulse stay (or merge into the local channel — settled in the plan).

### Codex hooks — `dot_codex/hooks.json` (new chezmoi management)

chezmoi takes over `~/.codex/hooks.json`: `Stop` → `relay-agent.sh done`, `PermissionRequest` →
`relay-agent.sh blocked`, and the herdr agent-state `SessionStart` hook is preserved. Codex is removed from
moshi-hook: drop `codex` from the `--target` list in `run_once_after_60-moshi-hook-setup.sh.tmpl`, and run
`moshi-hook uninstall --target codex` once (mirrors the existing Claude-exclusion note).

### Shell notifier — `dot_bashrc.tmpl`

- ≥1m: local clickable notification (terminal-notifier → zoom this pane). (Local threshold raised from
  30s.)
- ≥5m: additionally moshi + hermes (via `relay.sh`) + the existing Hue pulse.
- Add `codex` to the interactive-TUI skip-list (alongside `claude`) so agent sessions never double-notify.

### Hermes route — `~/.hermes/config.yaml` (user-maintained)

Add a `relay` route: a new `secret`, `deliver: discord`, `deliver_only: true`,
`deliver_extra.chat_id: <#notify-log id>`, and a `prompt` template that renders our fields (e.g.
`{agent} · {state} · {project}\n\n{detail}`). Because `~/.hermes` isn't chezmoi-managed, this is added by
hand (or via `hermes webhook subscribe`). The same secret is stored in a new KeePassXC entry and rendered
into `~/.config/relay/auth.json` for `relay.sh`.

### Secrets — `~/.config/relay/auth.json` (chezmoi template, KeePassXC)

Holds `{ moshi_secret, hermes_secret }`. The moshi secret migrates here from `~/.config/moshi/auth.json` as
part of the rename; the Hermes secret is a new KeePassXC entry. 0600, never on a command line.

## Exact-pane focus

`herdr agent focus <target>` (added in 0.5.10) focuses an exact pane by id and switches the active
workspace/tab to bring it into view — no zoom, no tab-only compromise. Its targets "accept terminal ids,
unique agent names, detected/reported agent labels, and legacy pane ids," so it works for any pane, not
just detected agents. `relay-agent.sh` and the shell notifier capture `HERDR_PANE_ID` (format `wW:p8`, a
valid target) at notify time, so the click-action is simply `herdr agent focus <pane_id>`. (Plain
directional `pane focus` only moves left/right/up/down, and passing a pane id to a plain `focus` resolves
to the pane's tab — neither lands on an arbitrary pane; `agent focus` is the one that does. Confirmed
against the CLI, the socket API, and the 0.5.10 changelog.)

## Failure separation

Every channel call is backgrounded and `|| true`. moshi down ≠ Hermes lost; Hermes down ≠ moshi lost;
neither blocks the local notification. `relay.sh` always exits 0 so it can never delay or fail a hook or a
shell prompt.

## Naming

The pipeline is **relay**: `relay.sh` (sender) and `relay-agent.sh` (agent message-builder), config under
`~/.config/relay/`, Hermes route `relay`. `claude-moshi-notify.sh` is retired into these.

## Open questions (resolve during the plan, not blockers)

1. **Codex `Stop` / `PermissionRequest` payload** — does it provide `cwd` + `transcript_path` like
   Claude's? If not, `relay-agent.sh` skips the snippet for Codex and uses project/branch only.
2. **`herdr-agent-state.sh` origin** — it lives only in `~/.codex/` today. Once chezmoi owns `hooks.json`,
   that reference must resolve, so we track it in `dot_codex/` (after confirming what writes it).

## Out of scope

- A Hermes priority-split (`#notify-log` is one channel for now; the state label sorts).
- chezmoi-managing the whole `~/.hermes/config.yaml`.

## Testing

- `relay.sh`: each channel exercised against a fake endpoint; HMAC matches `openssl`; one channel failing
  leaves the others delivered; exits 0 with secrets absent.
- `relay-agent.sh`: state→message mapping; `done` snippet from a real transcript; shellcheck-clean.
- Hermes: a live POST to the `relay` route returns 200 and `#notify-log` receives it.
- Codex: excluded from moshi-hook; the chezmoi `hooks.json` fires `relay-agent.sh` on `Stop` /
  `PermissionRequest`; herdr agent-state still works.
- Shell: 1m local / 5m webhook thresholds; `codex` skipped; clicking the notification focuses the right
  pane (`herdr agent focus`).
