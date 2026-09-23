# pns Hermes Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended)
> or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`)
> syntax for tracking.

**Goal:** Hermes Agent terminal sessions notify exactly like Claude Code sessions, including the phone
approval through `moshi-hook hermes-hook`, while Hermes gateway sessions are recorded for the recap and
delivered nowhere.

**Architecture:** A Python shim embedded in the pns binary (`pns-hooks`) runs inside every Hermes process,
maps seven Hermes hooks to the existing `pns hook` verbs, stamps each payload with the process's front-end
platform, and spawns pns asynchronously. pns decides from `platform` and `surface` whether a Hermes event
is a terminal session (every existing arm) or recorded only (one activity row). `moshi_subcommand` admits
`hermes` for a CLI approval, and a CLI answer is forwarded so moshi clears its card. `pns hermes
install-plugin` writes and enables the shim; on dresden an apply runs it, removes the Hermes shell hook
pair from the config modify template, and uninstalls moshi's own Hermes plugin.

**Tech Stack:** Rust 2024 (the `pns` workspace), Python 3 for the shim and its test driver, bashunit and
chezmoi templates for the dotfiles pull request. Gates: `just test-rust`, `just test-unit`,
`just lint-check`.

**Spec:** `docs/superpowers/specs/2026-09-22-pns-hermes-support-design.md`

## Global Constraints

- The producer value is `hermes`. The shim sets `PNS_PRODUCER=hermes` in each child's environment, and
  pns matches the literal `"hermes"`.
- The plugin is named `pns-hooks` and lives in `$HERMES_HOME/plugins/pns-hooks/`, default
  `~/.hermes/plugins/pns-hooks/`. Its files are `__init__.py` and `plugin.yaml`.
- The shim registers exactly seven hooks: `pre_llm_call`, `post_llm_call`, `on_session_end`,
  `pre_tool_call`, `post_tool_call`, `pre_approval_request`, `post_approval_response`.
- `blocked` is always sent as `hook blocked --remind=5m`. No other verb carries a flag.
- A Hermes event is recorded only when `platform` is a non-empty value other than `cli` and `tui`, or
  when `platform` is empty and `surface` is `gateway`. Everything else, including both fields missing, is
  a terminal session.
- moshi receives a Hermes payload only when `surface` is `cli`: the request through `blocked`, presence
  gated; the answer through `resolved`, never gated.
- pns ships no bash. The shim is Python, embedded with `include_str!` and written by the binary. Test
  stand-ins may be bash (`support::write_script`) or Python fixtures.
- No test reaches the real Hermes, moshi-hook or Codex: `PNS_HERMES_BIN`, `PNS_MOSHI_HOOK_BIN` and the
  sandbox's `PNS_CODEX_BIN` point at stand-ins, and every Hermes home is a scratch directory.
- Nothing reads `~/.hermes/config.yaml`, `~/.hermes/.env` or `~/.config/pns/config.toml`.
- Every `.rs` file stays at 300 lines ideal and 500 hard cap, unit tests included.
- Comments say what the code does or why it is that way. None explains an absence, a rejected option,
  or this plan.
- No em-dashes in any comment, message, commit or document.
- Conventional commits, `SKIP_AI_COMMIT=1`, no co-author trailer.
- Each task is one pull request on its own branch. A task ends at its commit: pushing, opening the pull
  request, review and merging belong to the orchestrator, not the implementer.
- Create each worktree with
  `herdr worktree create --cwd /Users/stephen/workspaces/Ivy/webdavis/dotfiles --branch <branch> --no-focus`
  and run every command from that worktree's root (`~/.herdr/worktrees/dotfiles/<branch with / as ->`).
  Rebase on `origin/main` first: open pull requests #917 and #919 touch `hook_dispatch.rs`,
  `routing.rs`, `moshi_submission.rs` and `legacy/usage.rs`, and the code below does not depend on
  either landing.
- The Rust blocks below are not pre-formatted: run `cargo fmt --all --manifest-path pns/Cargo.toml`
  before the gates.
- Every pns task ends with `just test-rust` and `just lint-check` green. `just test-rust` needs `chord`
  installed (`cargo install --git https://github.com/webdavis/chord chord`) and `python3` on `PATH`.

## Order

Task 1 first. Tasks 2, 3 and 4 follow Task 1 and are independent of each other. Task 5 is independent of
Tasks 1 to 4. Task 6 follows Task 5. Task 7 follows all six merges. Task 8 follows the apply of Task 7.

## File Structure

| File | Task | Responsibility |
| --- | --- | --- |
| `pns/crates/pns-adapters/src/harness/payload.rs` | 1, 2 | `platform` and `surface` fields; the approval's card text in the `message` chain |
| `pns/crates/pns-adapters/src/harness/hermes.rs` (new) | 1 | `hermes_records_only`, the terminal-or-recorded rule |
| `pns/crates/pns-adapters/src/harness/message.rs` | 2 | `guarded_command`, a Hermes approval's text |
| `pns/crates/pns-adapters/src/harness/routing.rs` | 3 | `moshi_subcommand(agent, surface)` |
| `pns/crates/pns-adapters/src/hermes_plugin.rs` (new) | 5, 6 | render, install and enable the plugin |
| `pns/crates/pns-adapters/src/hermes_plugin/pns_hooks.py` (new) | 5 | the shim Hermes loads |
| `pns/crates/pns-adapters/src/hermes_plugin/fixtures/{drive,record}.py` (new) | 5 | the shim's test driver and pns stand-in |
| `pns/crates/pns/src/hermes_session.rs` (new) | 1 | the recorded-only activity row |
| `pns/crates/pns/src/hook_dispatch.rs` | 1, 4 | the recorded-only branch; the answer forward call |
| `pns/crates/pns/src/moshi_submission.rs` | 3, 4 | the surface at the call site; `forward_resolution` |
| `pns/crates/pns/src/command_hermes.rs` (new) | 6 | `pns hermes install-plugin` |
| `pns/crates/pns/tests/hooks/hermes_sessions.rs` (new) | 1, 3, 4 | end-to-end hook behavior for producer `hermes` |
| `pns/crates/pns/tests/hermes_commands.rs` (new) | 6 | end-to-end `pns hermes install-plugin` |
| `.chezmoiscripts/run_after_72-pns-hermes-plugin.sh.tmpl` (new) | 7 | runs `install-plugin` on every apply |
| `private_dot_hermes/modify_private_config.yaml` | 7 | removes the Hermes shell hook pair |
| `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` | 7 | uninstalls moshi's Hermes plugin |

---

### Task 1: A Hermes session with no operator at a pane is recorded and delivered nowhere

Branch: `feat/pns-hermes-recorded-only-sessions`.

**Files:**
- Modify: `pns/crates/pns-adapters/src/harness/payload.rs` (struct fields and `parse_payload`)
- Create: `pns/crates/pns-adapters/src/harness/hermes.rs`
- Modify: `pns/crates/pns-adapters/src/harness.rs`, `pns/crates/pns-adapters/src/lib.rs` (exports)
- Create: `pns/crates/pns-adapters/src/harness/tests/hermes.rs`; modify `harness/tests.rs`
- Create: `pns/crates/pns/src/hermes_session.rs`; modify `pns/crates/pns/src/lib.rs`
- Modify: `pns/crates/pns/src/hook_dispatch.rs` (`hook_mode`, `session_only_event`)
- Create: `pns/crates/pns/tests/hooks/hermes_sessions.rs`; modify `pns/crates/pns/tests/hooks.rs`
- Modify: `pns/docs/specs/hook-compatibility.md` (intro, new behavior 29)

**Interfaces:**
- Produces: `HookPayload::platform: String`, `HookPayload::surface: String`;
  `pub fn hermes_records_only(platform: &str, surface: &str) -> bool` exported from `pns_adapters`;
  `pub(crate) fn hermes_session::record_only(event: &str, payload: &HookPayload, agent: &str)`;
  `hook_dispatch::session_only_event` becomes `pub(crate)`. The test file
  `tests/hooks/hermes_sessions.rs` with helpers `hermes(&Sandbox) -> Command`,
  `recorded(&Sandbox) -> Vec<(String, String)>` and `const SESSION: &str`, which Tasks 3 and 4 extend.

- [ ] **Step 1: Write the failing unit tests**

Create `pns/crates/pns-adapters/src/harness/tests/hermes.rs`:

```rust
use super::*;

#[test]
fn a_terminal_platform_notifies_whatever_the_surface_says() {
    for surface in ["", "cli", "gateway", "mcp-elicitation"] {
        assert!(!hermes_records_only("cli", surface), "cli/{surface}");
        assert!(!hermes_records_only("tui", surface), "tui/{surface}");
    }
}

#[test]
fn every_other_named_platform_is_recorded_only() {
    for platform in ["discord", "telegram", "webhook", "api_server", "cron", "acp", "subagent"] {
        assert!(hermes_records_only(platform, "cli"), "{platform}/cli");
        assert!(hermes_records_only(platform, ""), "{platform}/none");
    }
}

#[test]
fn with_no_platform_only_a_gateway_surface_is_recorded_only() {
    assert!(hermes_records_only("", "gateway"));
    for surface in ["", "cli", "mcp-elicitation"] {
        assert!(!hermes_records_only("", surface), "none/{surface}");
    }
}

#[test]
fn a_payload_yields_its_platform_and_surface_and_misses_neither_as_an_error() {
    let payload = parse_payload(r#"{"session_id":"s1","platform":"tui","surface":"gateway"}"#);
    assert_eq!(payload.platform, "tui");
    assert_eq!(payload.surface, "gateway");
    let bare = parse_payload(r#"{"session_id":"s1"}"#);
    assert_eq!((bare.platform.as_str(), bare.surface.as_str()), ("", ""));
}
```

Add `mod hermes;` to `pns/crates/pns-adapters/src/harness/tests.rs`, in alphabetical order after
`use super::*;`.

- [ ] **Step 2: Write the failing end-to-end tests**

Create `pns/crates/pns/tests/hooks/hermes_sessions.rs`:

```rust
use super::lights_waits::{LAMPS_ON, waiting_sessions};
use super::*;

pub(crate) const SESSION: &str = "20260922_120000_abc123";

/// The engine as the pns-hooks plugin runs it.
pub(crate) fn hermes(sandbox: &Sandbox) -> Command {
    let mut command = with_state_dir(sandbox);
    command.env("PNS_PRODUCER", "hermes");
    command
}

/// Every activity row's state and session, in the order written.
pub(crate) fn recorded(sandbox: &Sandbox) -> Vec<(String, String)> {
    stored_records::database(sandbox)
        .prepare("SELECT state, session FROM activity_events ORDER BY seq")
        .expect("the activity table")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("read the rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("activity rows")
}

#[test]
fn a_chat_gateway_turn_is_recorded_as_done_and_delivered_nowhere() {
    let sandbox = Sandbox::new("hermes-gateway-turn");
    let output = hook_with(
        hermes(&sandbox),
        &sandbox,
        "stop",
        &format!(
            r#"{{"hook_event_name":"AgentEnd","session_id":"{SESSION}","cwd":"/tmp","last_assistant_message":"posted the summary","platform":"discord"}}"#
        ),
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(recorded(&sandbox), vec![("done".to_string(), SESSION.to_string())]);
    for channel in ["banner", "phone", "hermes"] {
        assert!(!sandbox.fired(channel), "{channel} fired for a gateway session");
    }
}

#[test]
fn a_gateway_prompt_and_approval_leave_no_marker_and_start_no_round_trip() {
    let sandbox = Sandbox::new("hermes-gateway-approval");
    sandbox.write_config(LAMPS_ON);
    let mut prompt = hermes(&sandbox);
    sandbox.stub_moshi(&mut prompt, 0);
    hook_with(
        prompt,
        &sandbox,
        "prompt",
        &format!(r#"{{"session_id":"{SESSION}","prompt":"summarize the thread","platform":"discord"}}"#),
    );
    let mut approval = hermes(&sandbox);
    sandbox.stub_moshi(&mut approval, 0);
    hook_with(
        approval,
        &sandbox,
        "blocked",
        &format!(r#"{{"hook_event_name":"PermissionRequest","session_id":"{SESSION}","command":"rm -rf /tmp/x","surface":"gateway"}}"#),
    );
    assert!(!marker(&sandbox, SESSION).exists(), "a gateway turn starts no turn clock");
    assert_eq!(waiting_sessions(&sandbox), Vec::<String>::new());
    assert_eq!(submissions(&sandbox), Vec::<String>::new());
    assert_eq!(
        recorded(&sandbox),
        vec![
            ("prompt".to_string(), SESSION.to_string()),
            ("blocked".to_string(), SESSION.to_string()),
        ]
    );
}

#[test]
fn a_hermes_event_naming_neither_platform_nor_surface_notifies() {
    let sandbox = Sandbox::new("hermes-no-origin");
    hook_with(
        hermes(&sandbox),
        &sandbox,
        "stop-failure",
        &format!(r#"{{"hook_event_name":"TurnFailed","session_id":"{SESSION}","cwd":"/tmp","error":"the budget ran out"}}"#),
    );
    assert_eq!(sandbox.event("hermes")["state"], "failed");
    assert_eq!(recorded(&sandbox), vec![("failed".to_string(), SESSION.to_string())]);
}

#[test]
fn a_tui_approval_is_a_terminal_session_despite_its_gateway_surface() {
    let sandbox = Sandbox::new("hermes-tui-approval-terminal");
    sandbox.write_config(LAMPS_ON);
    hook_with(
        hermes(&sandbox),
        &sandbox,
        "blocked",
        &format!(r#"{{"hook_event_name":"PermissionRequest","session_id":"{SESSION}","command":"rm -rf /tmp/x","surface":"gateway","platform":"tui"}}"#),
    );
    assert_eq!(waiting_sessions(&sandbox), vec![SESSION.to_string()]);
}
```

In `pns/crates/pns/tests/hooks.rs`, add beside the other `#[path]` modules, in alphabetical order:

```rust
#[path = "hooks/hermes_sessions.rs"]
mod hermes_sessions;
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters harness::tests::hermes`
Expected: compile error, `cannot find function hermes_records_only` and `no field platform`.

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hooks hermes_sessions`
Expected: `a_chat_gateway_turn...` and `a_gateway_prompt_and_approval...` FAIL (the gateway turn fires
`hermes`, the prompt writes a turn marker); the other two pass already.

- [ ] **Step 4: Add the payload fields**

In `pns/crates/pns-adapters/src/harness/payload.rs`, append to `HookPayload` after `file_path`:

```rust
    /// Which Hermes front end sent the event: `cli`, `tui`, a gateway
    /// platform's name, `cron`, `acp`, `subagent`. The pns-hooks plugin stamps
    /// it on every event it sends; no other harness sends the key.
    pub platform: String,
    /// Which approval surface a Hermes approval came through: `cli`, `gateway`
    /// or `mcp-elicitation`. Empty on every other event.
    pub surface: String,
```

and append to the struct literal in `parse_payload`, after `file_path: text("file_path"),`:

```rust
        platform: text("platform"),
        surface: text("surface"),
```

- [ ] **Step 5: Add the rule**

Create `pns/crates/pns-adapters/src/harness/hermes.rs`:

```rust
/// Whether a Hermes event is recorded for the recap and delivered nowhere.
///
/// THE PLATFORM DECIDES FIRST. `cli` and `tui` are Hermes's terminal front
/// ends, and every other platform it names (a chat gateway, `webhook`,
/// `cron`, `acp`, `subagent`) has no operator at a pane. The surface decides
/// only when no platform arrived, because the TUI's approvals report
/// `gateway` as well. An event that states neither notifies.
pub fn hermes_records_only(platform: &str, surface: &str) -> bool {
    match platform {
        "cli" | "tui" => false,
        "" => surface == "gateway",
        _ => true,
    }
}
```

In `pns/crates/pns-adapters/src/harness.rs` add `mod hermes;` after `mod message;` and
`pub use hermes::hermes_records_only;` after the `pub use message::flattened;` line. In
`pns/crates/pns-adapters/src/lib.rs`, add `hermes_records_only` to the `pub use harness::{...}` list in
alphabetical position.

- [ ] **Step 6: Run the unit tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters harness::tests::hermes`
Expected: 4 passed.

- [ ] **Step 7: Record the row and return**

Create `pns/crates/pns/src/hermes_session.rs`:

```rust
//! A Hermes event from a session with no operator at a pane: one activity row
//! for the recap, and nothing delivered.

use crate::*;

/// The row `event` would have written, and nothing else: no turn or wait
/// marker, no reminder, no summarizer, no moshi forward and no destination,
/// so no banner, phone card, lamp or Discord line.
pub(crate) fn record_only(event: &str, payload: &HookPayload, agent: &str) {
    let recorded = match event {
        "prompt" => {
            crate::hook_dispatch::session_only_event(agent, event, name_session(payload, agent))
        }
        _ => pns_domain::EventArgs {
            agent: agent.to_string(),
            state: recorded_state(event).to_string(),
            detail: recorded_detail(event, payload),
            pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
            ..attribution(payload, agent)
        },
    };
    crate::activity::record(&recorded, payload);
}

/// The state word the delivering arm would have raised, without its summarizer.
fn recorded_state(event: &str) -> &str {
    match event {
        "stop" => "done",
        "stop-failure" => "failed",
        other => other,
    }
}

/// A turn's end names its reply; every other event names what its payload states.
fn recorded_detail(event: &str, payload: &HookPayload) -> String {
    match event {
        "stop" => pns_domain::render::preview(&crate::turn_text::turn_reply(payload)),
        _ => payload.message.clone(),
    }
}
```

In `pns/crates/pns/src/lib.rs` add `mod hermes_session;` beside `mod hook_dispatch;`. In
`pns/crates/pns/src/hook_dispatch.rs` change `fn session_only_event` to `pub(crate) fn session_only_event`,
and in `hook_mode` insert directly after the `let agent = ...;` line:

```rust
    // A Hermes session with no operator at a pane is recorded for the recap
    // and reaches no arm below.
    if agent == "hermes" && pns_adapters::hermes_records_only(&payload.platform, &payload.surface) {
        crate::hermes_session::record_only(event, &payload, &agent);
        return 0;
    }
```

- [ ] **Step 8: Run the end-to-end tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hooks hermes_sessions`
Expected: 4 passed.

- [ ] **Step 9: State the behavior in the pns spec**

In `pns/docs/specs/hook-compatibility.md`, change the intro's "a coding harness (Claude Code or Codex)"
to "a coding harness (Claude Code, Codex or Hermes Agent)". Append, before "## Environment inputs this
path reads":

```markdown
## 29. A Hermes event from a session with no operator at a pane is recorded and delivered nowhere

Given a payload whose producer is `hermes` and whose `platform` is a non-empty value other than `cli`
and `tui`, or whose `platform` is empty and whose `surface` is `gateway`

When `pns hook <event>` runs

Then one activity row is written with the event's state word (`stop` as `done`, `stop-failure` as
`failed`, every other word as itself) and the hook exits 0 without reaching any arm.

- Success: a Discord turn end writes one `done` row and fires no channel
  (`tests/hooks/hermes_sessions.rs:a_chat_gateway_turn_is_recorded_as_done_and_delivered_nowhere`).
- Forbidden side effects: no turn marker, no wait marker, no reminder, no summarizer, no moshi spawn
  (`a_gateway_prompt_and_approval_leave_no_marker_and_start_no_round_trip`).
- Fail direction: toward notifying. A payload naming neither field, or a TUI approval reporting
  `surface: "gateway"` with `platform: "tui"`, takes the ordinary arms
  (`a_hermes_event_naming_neither_platform_nor_surface_notifies`,
  `a_tui_approval_is_a_terminal_session_despite_its_gateway_surface`).
```

- [ ] **Step 10: Run the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 11: Commit**

```bash
git add pns/crates/pns-adapters/src/harness pns/crates/pns-adapters/src/harness.rs \
  pns/crates/pns-adapters/src/lib.rs pns/crates/pns/src/hermes_session.rs pns/crates/pns/src/lib.rs \
  pns/crates/pns/src/hook_dispatch.rs pns/crates/pns/tests/hooks.rs \
  pns/crates/pns/tests/hooks/hermes_sessions.rs pns/docs/specs/hook-compatibility.md
SKIP_AI_COMMIT=1 git commit -m "feat(pns): record a Hermes gateway session's events without delivering them"
```

---

### Task 2: A Hermes approval's card names what it asks

Branch: `feat/pns-hermes-approval-card-text`. Needs Task 1 merged.

**Files:**
- Modify: `pns/crates/pns-adapters/src/harness/message.rs` (new `guarded_command`)
- Modify: `pns/crates/pns-adapters/src/harness/payload.rs` (import and the `message` chain)
- Test: `pns/crates/pns-adapters/src/harness/tests/requests.rs`
- Modify: `pns/docs/specs/hook-compatibility.md` (behavior 6)

**Interfaces:**
- Consumes: `parse_payload` as it stands after Task 1.
- Produces: `HookPayload::message` for a payload with top-level `description` and `command` and no tool
  is `"<description>: <command>"`, either half alone when the other is empty, capped at
  `TOOL_REQUEST_MAX_CHARS`.

- [ ] **Step 1: Write the failing tests**

Append to `pns/crates/pns-adapters/src/harness/tests/requests.rs`:

```rust
#[test]
fn a_hermes_approval_names_why_it_asks_and_then_the_command() {
    let payload = parse_payload(
        r#"{"hook_event_name":"PermissionRequest","session_id":"s1","command":"rm -rf /tmp/drill",
                "description":"recursive delete","pattern_key":"rm_rf","surface":"cli"}"#,
    );
    assert_eq!(payload.message, "recursive delete: rm -rf /tmp/drill");
}

#[test]
fn either_half_of_a_hermes_approval_stands_alone() {
    assert_eq!(parse_payload(r#"{"command":"rm -rf /tmp/drill"}"#).message, "rm -rf /tmp/drill");
    assert_eq!(parse_payload(r#"{"description":"recursive delete"}"#).message, "recursive delete");
}

#[test]
fn a_hermes_command_spanning_lines_reaches_the_card_as_one_line() {
    let payload = parse_payload(r#"{"command":"echo one\necho two","description":"two commands"}"#);
    assert!(!payload.message.contains('\n'), "{}", payload.message);
    assert!(payload.message.starts_with("two commands: echo one"), "{}", payload.message);
}

#[test]
fn a_tool_request_wins_over_a_top_level_command() {
    let payload = parse_payload(r#"{"tool_name":"shell","tool_input":{"command":"ls"},"command":"ignored"}"#);
    assert_eq!(payload.message, "shell: command=ls");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters harness::tests::requests`
Expected: the first three FAIL with `left: ""`; the fourth passes.

- [ ] **Step 3: Implement**

Add to `pns/crates/pns-adapters/src/harness/message.rs`, after `tool_request`:

```rust
/// What a Hermes approval asks for: why it needs asking, then the command.
///
/// Hermes states `description` and `command` at the top level and names no
/// tool, so this is reached only after `tool_request` found nothing. Both
/// halves are flattened and the HEAD is kept, like a tool request's.
pub(super) fn guarded_command(payload: &serde_json::Value) -> String {
    let stated = |key: &str| {
        payload
            .get(key)
            .filter(|value| !value.is_null())
            .map(one_line)
            .unwrap_or_default()
    };
    let (description, command) = (stated("description"), stated("command"));
    let request = match (description.as_str(), command.as_str()) {
        ("", command) => command.to_string(),
        (description, "") => description.to_string(),
        (description, command) => format!("{description}: {command}"),
    };
    request.chars().take(TOOL_REQUEST_MAX_CHARS).collect()
}
```

In `pns/crates/pns-adapters/src/harness/payload.rs`, import it:

```rust
use super::message::{elicitation_request, guarded_command, reported_error, tool_request};
```

and replace `.unwrap_or_else(|| tool_request(&payload)),` in the `message` chain with:

```rust
        .unwrap_or_else(|| {
            [tool_request(&payload), guarded_command(&payload)]
                .into_iter()
                .find(|stated| !stated.is_empty())
                .unwrap_or_default()
        }),
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters harness::tests::requests`
Expected: all pass.

- [ ] **Step 5: State the behavior in the pns spec**

In `pns/docs/specs/hook-compatibility.md` behavior 6, change the heading to "One `message` is composed
from four payload roads and two fallbacks, in a fixed order", change the Then line's ending to "and
`tool_request`, then `guarded_command`, are the fallbacks when all four say nothing.", and append to its
Success bullet: "A Hermes `PermissionRequest` carrying `description` and `command` yields
`recursive delete: rm -rf /tmp/drill`
(`src/harness/tests/requests.rs:a_hermes_approval_names_why_it_asks_and_then_the_command`)."

- [ ] **Step 6: Run the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 7: Commit**

```bash
git add pns/crates/pns-adapters/src/harness/message.rs pns/crates/pns-adapters/src/harness/payload.rs \
  pns/crates/pns-adapters/src/harness/tests/requests.rs pns/docs/specs/hook-compatibility.md
SKIP_AI_COMMIT=1 git commit -m "feat(pns): name a Hermes approval's command on its card"
```

---

### Task 3: A Hermes CLI approval is handed to `moshi-hook hermes-hook` when the operator is away

Branch: `feat/pns-hermes-approval-forward`. Needs Task 1 merged.

**Files:**
- Modify: `pns/crates/pns-adapters/src/harness/routing.rs` (`moshi_subcommand`)
- Test: `pns/crates/pns-adapters/src/harness/tests/routing.rs`
- Modify: `pns/crates/pns/src/moshi_submission.rs` (`blocking_event` call site)
- Test: `pns/crates/pns/tests/hooks/hermes_sessions.rs`
- Modify: `pns/docs/specs/blocking-approval.md` and `pns/docs/specs/hook-compatibility.md` (the roster)

**Interfaces:**
- Consumes: `HookPayload::surface`, and `hermes`, `SESSION` from Task 1's test file.
- Produces: `pub fn moshi_subcommand(agent: &str, surface: &str) -> Option<String>`; `Some("hermes-hook")`
  only for `("hermes", "cli")`. Task 4 calls it.

- [ ] **Step 1: Write the failing unit tests**

In `pns/crates/pns-adapters/src/harness/tests/routing.rs`, replace
`only_the_harnesses_pns_registers_for_are_forwarded_to_moshi` with:

```rust
#[test]
fn only_the_harnesses_pns_registers_for_are_forwarded_to_moshi() {
    assert_eq!(moshi_subcommand("claude", "").as_deref(), Some("claude-hook"));
    assert_eq!(moshi_subcommand("codex", "").as_deref(), Some("codex-hook"));
    assert_eq!(moshi_subcommand("pi", "cli"), None);
    assert_eq!(moshi_subcommand("", ""), None);
    assert_eq!(moshi_subcommand("claude; rm -rf /", ""), None);
}

#[test]
fn a_hermes_approval_goes_to_moshi_only_from_the_cli_prompt() {
    assert_eq!(moshi_subcommand("hermes", "cli").as_deref(), Some("hermes-hook"));
    for surface in ["", "gateway", "mcp-elicitation"] {
        assert_eq!(moshi_subcommand("hermes", surface), None, "{surface}");
    }
}
```

- [ ] **Step 2: Write the failing end-to-end tests**

Append to `pns/crates/pns/tests/hooks/hermes_sessions.rs`:

```rust
pub(crate) const CLI_APPROVAL: &str = r#"{"hook_event_name":"PermissionRequest","session_id":"20260922_120000_abc123","cwd":"/tmp","action_id":"a1","command":"rm -rf /tmp/drill","description":"recursive delete","pattern_key":"rm_rf","pattern_keys":["rm_rf"],"surface":"cli","platform":"cli"}"#;

#[test]
fn an_away_cli_approval_is_handed_to_moshi_hermes_hook_byte_for_byte() {
    let sandbox = Sandbox::new("hermes-cli-approval-forward");
    let mut command = hermes(&sandbox);
    sandbox.stub_moshi(&mut command, 0);
    hook_with(command, &sandbox, "blocked", CLI_APPROVAL);
    assert_eq!(submissions(&sandbox), vec!["hermes-hook".to_string()]);
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the payload"),
        CLI_APPROVAL
    );
}

#[test]
fn a_tui_approval_is_carded_by_pns_and_never_handed_to_moshi() {
    let sandbox = Sandbox::new("hermes-tui-approval-card");
    let mut command = hermes(&sandbox);
    sandbox.stub_moshi(&mut command, 0);
    let tui = CLI_APPROVAL.replace(
        r#""surface":"cli","platform":"cli""#,
        r#""surface":"gateway","platform":"tui""#,
    );
    hook_with(command, &sandbox, "blocked", &tui);
    assert_eq!(submissions(&sandbox), Vec::<String>::new());
    assert!(sandbox.fired("phone"), "the away operator still gets pns's own card");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters harness::tests::routing`
Expected: compile error, `moshi_subcommand` takes 1 argument but 2 were supplied.

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hooks hermes_sessions`
Expected: `an_away_cli_approval...` FAILS with `left: []`.

- [ ] **Step 4: Implement**

Replace `moshi_subcommand` in `pns/crates/pns-adapters/src/harness/routing.rs`:

```rust
/// Whether a blocking event is handed to moshi for a round trip.
///
/// Only the harnesses pns registers itself for: the name arrives from a config
/// file, so it is MATCHED rather than pasted into a subcommand handed to a
/// third-party binary. A Hermes approval goes only from the classic CLI's own
/// prompt, the one Hermes surface moshi's terminal bridge answers.
pub fn moshi_subcommand(agent: &str, surface: &str) -> Option<String> {
    match agent {
        "claude" | "codex" => Some(format!("{agent}-hook")),
        "hermes" if surface == "cli" => Some("hermes-hook".to_string()),
        _ => None,
    }
}
```

In `pns/crates/pns/src/moshi_submission.rs` `blocking_event`, change `moshi_subcommand(agent)` to
`moshi_subcommand(agent, &payload.surface)`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters harness::tests::routing`
Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hooks hermes_sessions`
Expected: all pass.

- [ ] **Step 6: State the roster in the pns specs**

In `pns/docs/specs/blocking-approval.md` and `pns/docs/specs/hook-compatibility.md`, every sentence that
says `moshi_subcommand` admits only `claude` and `codex` becomes "admits `claude`, `codex`, and `hermes`
for an approval whose `surface` is `cli`". Find them with:

```bash
grep -n 'and `codex`' pns/docs/specs/blocking-approval.md pns/docs/specs/hook-compatibility.md
```

- [ ] **Step 7: Run the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 8: Commit**

```bash
git add pns/crates/pns-adapters/src/harness/routing.rs pns/crates/pns-adapters/src/harness/tests/routing.rs \
  pns/crates/pns/src/moshi_submission.rs pns/crates/pns/tests/hooks/hermes_sessions.rs pns/docs/specs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): hand a Hermes CLI approval to moshi-hook hermes-hook"
```

---

### Task 4: A Hermes CLI answer clears moshi's card

Branch: `feat/pns-hermes-answer-forward`. Needs Task 3 merged.

**Files:**
- Modify: `pns/crates/pns/src/moshi_submission.rs` (new `forward_resolution`)
- Modify: `pns/crates/pns/src/hook_dispatch.rs` (the `resolved` arm and its comment)
- Test: `pns/crates/pns/tests/hooks/hermes_sessions.rs`
- Modify: `pns/docs/specs/hook-compatibility.md` (behavior 19)

**Interfaces:**
- Consumes: `moshi_subcommand(agent, surface)` from Task 3; the `hermes` test helper from Task 1.
- Produces: `pub(crate) fn forward_resolution(agent: &str, payload: &HookPayload, payload_json: &str)`.

- [ ] **Step 1: Write the failing tests**

Append to `pns/crates/pns/tests/hooks/hermes_sessions.rs`:

```rust
const CLI_ANSWER: &str = r#"{"hook_event_name":"PermissionResolved","session_id":"20260922_120000_abc123","cwd":"/tmp","action_id":"a1","command":"rm -rf /tmp/drill","description":"recursive delete","choice":"once","surface":"cli","platform":"cli"}"#;

#[test]
fn a_cli_answer_is_handed_to_moshi_even_at_the_desk() {
    let sandbox = Sandbox::new("hermes-cli-answer");
    let mut command = hermes(&sandbox);
    command.env("PNS_SCREEN_IDLE", "0");
    sandbox.stub_moshi(&mut command, 0);
    let output = hook_with(command, &sandbox, "resolved", CLI_ANSWER);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(submissions(&sandbox), vec!["hermes-hook".to_string()]);
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the answer"),
        CLI_ANSWER
    );
}

#[test]
fn only_a_hermes_cli_answer_is_handed_to_moshi() {
    let tui = CLI_ANSWER.replace(
        r#""surface":"cli","platform":"cli""#,
        r#""surface":"gateway","platform":"tui""#,
    );
    for (producer, payload) in [("hermes", tui.as_str()), ("claude", CLI_ANSWER)] {
        let sandbox = Sandbox::new(&format!("hermes-answer-kept-{producer}"));
        let mut command = with_state_dir(&sandbox);
        command.env("PNS_PRODUCER", producer);
        sandbox.stub_moshi(&mut command, 0);
        hook_with(command, &sandbox, "resolved", payload);
        assert_eq!(submissions(&sandbox), Vec::<String>::new(), "{producer}: {payload}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hooks hermes_sessions`
Expected: `a_cli_answer_is_handed_to_moshi_even_at_the_desk` FAILS with `left: []`.

- [ ] **Step 3: Implement**

Append to `pns/crates/pns/src/moshi_submission.rs`, after `blocking_event`:

```rust
/// A Hermes CLI approval's answer, handed to moshi so the card it raised for
/// that approval clears.
///
/// NOT PRESENCE-GATED: an operator who left the desk after the card went out
/// can come back and answer at the pane, and that card still has to clear.
/// moshi pairs the answer to its card by `action_id`; its own Hermes plugin
/// sends answers with an empty `action_id` when it holds nothing to pair, so
/// an answer moshi never carded is one it already takes. Bounded by the same
/// acknowledgement deadline as every forward.
pub(crate) fn forward_resolution(agent: &str, payload: &HookPayload, payload_json: &str) {
    if agent != "hermes"
        || payload.hook_event_name != "PermissionResolved"
        || !payload_is_whole(payload_json)
    {
        return;
    }
    let Some(subcommand) = moshi_subcommand(agent, &payload.surface) else {
        return;
    };
    if let Some(child) =
        pns_application::ApprovalForwarder::forward(&MoshiApprovalForwarder, &subcommand, payload_json)
    {
        pns_application::ApprovalForwarder::answer(&MoshiApprovalForwarder, child);
    }
}
```

In `pns/crates/pns/src/hook_dispatch.rs`, in the `"resolved"` arm, add as its last statement:

```rust
            crate::moshi_submission::forward_resolution(&agent, &payload, &payload_json);
```

and replace the arm comment paragraph that begins "IT LOADS NO CONFIG AND DELIVERS NOTHING." with:

```rust
        // IT DELIVERS NOTHING, and the clearing loads no config. A record exists
        // only because the feature was on when the approval arrived, so clearing
        // it is right regardless of what the config says now, and that keeps this
        // per-batch path to a payload read, a parse and two file operations. A
        // Hermes CLI answer is also handed to moshi, whose bounded wait reads its
        // deadline from the config: see `forward_resolution`.
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hooks hermes_sessions`
Expected: all pass.

- [ ] **Step 5: State the behavior in the pns spec**

In `pns/docs/specs/hook-compatibility.md` behavior 19, change the heading to "`resolved` clears and
delivers nothing, and hands a Hermes CLI answer to moshi" and add a bullet: "Required side effects: a
payload from producer `hermes` with `hook_event_name` `PermissionResolved` and `surface` `cli` is handed
byte for byte to `moshi-hook hermes-hook`, at the desk as well, so the card moshi raised clears
(`tests/hooks/hermes_sessions.rs:a_cli_answer_is_handed_to_moshi_even_at_the_desk`)."

- [ ] **Step 6: Run the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 7: Commit**

```bash
git add pns/crates/pns/src/moshi_submission.rs pns/crates/pns/src/hook_dispatch.rs \
  pns/crates/pns/tests/hooks/hermes_sessions.rs pns/docs/specs/hook-compatibility.md
SKIP_AI_COMMIT=1 git commit -m "feat(pns): hand a Hermes CLI answer to moshi so its card clears"
```

---

### Task 5: The embedded shim maps each Hermes hook to its pns verb

Branch: `feat/pns-hermes-plugin-shim`. Independent of Tasks 1 to 4.

**Files:**
- Create: `pns/crates/pns-adapters/src/hermes_plugin.rs`
- Create: `pns/crates/pns-adapters/src/hermes_plugin/pns_hooks.py`
- Create: `pns/crates/pns-adapters/src/hermes_plugin/tests.rs`
- Create: `pns/crates/pns-adapters/src/hermes_plugin/fixtures/drive.py`
- Create: `pns/crates/pns-adapters/src/hermes_plugin/fixtures/record.py`
- Modify: `pns/crates/pns-adapters/src/lib.rs`

**Interfaces:**
- Produces: `pub const HERMES_PLUGIN_NAME: &str = "pns-hooks"`;
  `pub struct HermesPlugin { pub init_py: String, pub manifest: String }`;
  `pub fn render_hermes_plugin(binary: &str, version: &str) -> HermesPlugin`. All exported from
  `pns_adapters`. Task 6 consumes them.

- [ ] **Step 1: Write the test fixtures**

Create `pns/crates/pns-adapters/src/hermes_plugin/fixtures/record.py`:

```python
#!/usr/bin/env python3
"""Stands in for pns: appends each call's argv, producer and payload to $PNS_HOOKS_RECORD."""

import json
import os
import sys

call = {
    "argv": sys.argv[1:],
    "producer": os.environ.get("PNS_PRODUCER"),
    "payload": json.load(sys.stdin),
}
with open(os.environ["PNS_HOOKS_RECORD"], "a", encoding="utf-8") as record:
    record.write(json.dumps(call) + "\n")
```

Create `pns/crates/pns-adapters/src/hermes_plugin/fixtures/drive.py`:

```python
"""Loads a rendered pns-hooks plugin, fires one scenario of Hermes hooks at it,
waits for every delivery, and prints what registered and what each call returned."""

import importlib.util
import json
import sys

spec = importlib.util.spec_from_file_location("pns_hooks", sys.argv[1])
plugin = importlib.util.module_from_spec(spec)
spec.loader.exec_module(plugin)
hooks = {}


class Context:
    def register_hook(self, name, callback):
        hooks[name] = callback


plugin.register(Context())
returned = [hooks[name](**kwargs) for name, kwargs in json.loads(sys.argv[2])]
plugin._delivery.shutdown(wait=True)
print(json.dumps({"registered": sorted(hooks), "returned": returned}))
```

- [ ] **Step 2: Write the failing tests**

Create `pns/crates/pns-adapters/src/hermes_plugin/tests.rs`:

```rust
use super::render_hermes_plugin;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;

const DRIVER: &str = include_str!("fixtures/drive.py");
const RECORDER: &str = include_str!("fixtures/record.py");
const SESSION: &str = "20260922_120000_abc123";

/// What one scenario produced: each call the stand-in pns received, and the
/// driver's report of what registered and what each callback returned.
struct Fired {
    calls: Vec<Value>,
    report: Value,
}

/// Fires `scenario`, a list of `[hook, kwargs]` pairs, at the rendered plugin
/// through `python3`, with a stand-in recording every pns call.
fn fire(name: &str, scenario: Value) -> Fired {
    let root = crate::state_fixtures::scratch(name);
    let recorder = root.join("pns");
    std::fs::write(&recorder, RECORDER).expect("the stand-in pns");
    std::fs::set_permissions(&recorder, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let plugin = render_hermes_plugin(recorder.to_str().expect("a UTF-8 path"), "0.0.0");
    std::fs::write(root.join("__init__.py"), plugin.init_py).expect("the plugin");
    std::fs::write(root.join("drive.py"), DRIVER).expect("the driver");
    let record = root.join("record.jsonl");
    let output = std::process::Command::new("python3")
        .arg(root.join("drive.py"))
        .arg(root.join("__init__.py"))
        .arg(scenario.to_string())
        .env("PNS_HOOKS_RECORD", &record)
        .output()
        .expect("python3 on PATH");
    assert!(output.status.success(), "{output:?}");
    Fired {
        calls: std::fs::read_to_string(&record)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).expect("one JSON call per line"))
            .collect(),
        report: serde_json::from_slice(&output.stdout).expect("the driver's report"),
    }
}

/// Each call's words after the binary.
fn argv(fired: &Fired) -> Vec<Value> {
    fired.calls.iter().map(|call| call["argv"].clone()).collect()
}

/// One call's payload keys, sorted.
fn keys(call: &Value) -> Vec<String> {
    let mut keys: Vec<String> = call["payload"].as_object().expect("an object").keys().cloned().collect();
    keys.sort();
    keys
}

#[test]
fn the_binary_path_is_a_python_string_literal_and_the_manifest_names_the_plugin() {
    let plugin = render_hermes_plugin(r#"/opt/odd "dir"\pns"#, "0.2.0");
    assert!(plugin.init_py.contains(r#"PNS = "/opt/odd \"dir\"\\pns""#), "{}", plugin.init_py);
    assert!(!plugin.init_py.contains("__PNS_BINARY__"));
    assert_eq!(
        plugin.manifest,
        "name: pns-hooks\nversion: \"0.2.0\"\ndescription: Hands Hermes Agent session events to pns\n"
    );
}

#[test]
fn the_plugin_registers_exactly_the_seven_hooks_pns_serves() {
    let fired = fire("hermes-plugin-registered", json!([]));
    assert_eq!(
        fired.report["registered"],
        json!([
            "on_session_end", "post_approval_response", "post_llm_call", "post_tool_call",
            "pre_approval_request", "pre_llm_call", "pre_tool_call"
        ])
    );
}

#[test]
fn a_prompt_reaches_pns_as_the_prompt_verb_from_the_hermes_producer() {
    let fired = fire(
        "hermes-plugin-prompt",
        json!([["pre_llm_call", {"session_id": SESSION, "user_message": "list the files", "model": "m1", "platform": "cli"}]]),
    );
    assert_eq!(argv(&fired), vec![json!(["hook", "prompt"])]);
    let call = &fired.calls[0];
    assert_eq!(call["producer"], "hermes");
    assert_eq!(call["payload"]["hook_event_name"], "UserPromptSubmit");
    assert_eq!(call["payload"]["session_id"], SESSION);
    assert_eq!(call["payload"]["prompt"], "list the files");
    assert_eq!(call["payload"]["platform"], "cli");
}

#[test]
fn a_completed_turn_is_one_stop_carrying_the_reply_post_llm_call_kept() {
    let fired = fire(
        "hermes-plugin-stop",
        json!([
            ["post_llm_call", {"session_id": SESSION, "assistant_response": "all three files listed", "platform": "cli"}],
            ["on_session_end", {"session_id": SESSION, "completed": true, "interrupted": false, "platform": "cli"}]
        ]),
    );
    assert_eq!(argv(&fired), vec![json!(["hook", "stop"])]);
    assert_eq!(fired.calls[0]["payload"]["last_assistant_message"], "all three files listed");
}

#[test]
fn a_turn_that_did_not_complete_is_one_stop_failure_and_never_also_a_stop() {
    let fired = fire(
        "hermes-plugin-failure",
        json!([
            ["post_llm_call", {"session_id": SESSION, "assistant_response": "the budget ran out", "platform": "cli"}],
            ["on_session_end", {"session_id": SESSION, "completed": false, "interrupted": false, "platform": "cli"}]
        ]),
    );
    assert_eq!(argv(&fired), vec![json!(["hook", "stop-failure"])]);
    assert_eq!(fired.calls[0]["payload"]["error"], "the budget ran out");
}

#[test]
fn an_interrupted_turn_resolves_its_waits() {
    let fired = fire(
        "hermes-plugin-interrupt",
        json!([["on_session_end", {"session_id": SESSION, "completed": false, "interrupted": true, "platform": "cli"}]]),
    );
    assert_eq!(argv(&fired), vec![json!(["hook", "resolved"])]);
}

#[test]
fn an_approval_and_its_answer_carry_moshis_keys_and_one_action_id() {
    let approval = json!({
        "command": "rm -rf /tmp/drill", "description": "recursive delete", "pattern_key": "rm_rf",
        "pattern_keys": ["rm_rf"], "session_key": SESSION, "surface": "cli"
    });
    let mut answer = approval.clone();
    answer["choice"] = json!("once");
    let fired = fire(
        "hermes-plugin-approval",
        json!([
            ["pre_llm_call", {"session_id": SESSION, "platform": "cli"}],
            ["pre_approval_request", approval],
            ["post_approval_response", answer]
        ]),
    );
    assert_eq!(
        argv(&fired)[1..],
        [json!(["hook", "blocked", "--remind=5m"]), json!(["hook", "resolved"])]
    );
    let (request, resolution) = (&fired.calls[1], &fired.calls[2]);
    assert_eq!(
        keys(request),
        ["action_id", "command", "cwd", "description", "hook_event_name", "pattern_key", "pattern_keys", "platform", "session_id", "surface"]
    );
    assert_eq!(
        keys(resolution),
        ["action_id", "choice", "command", "cwd", "description", "hook_event_name", "platform", "session_id", "surface"]
    );
    assert_eq!(request["payload"]["hook_event_name"], "PermissionRequest");
    assert_eq!(resolution["payload"]["hook_event_name"], "PermissionResolved");
    let action = &request["payload"]["action_id"];
    assert_eq!(action.as_str().map(str::len), Some(32));
    assert_eq!(&resolution["payload"]["action_id"], action);
}

#[test]
fn an_approval_carries_the_platform_its_process_last_named() {
    let fired = fire(
        "hermes-plugin-stamp",
        json!([
            ["pre_llm_call", {"session_id": SESSION, "user_message": "clean up", "platform": "tui"}],
            ["pre_approval_request", {"command": "rm -rf /tmp/drill", "session_key": SESSION, "surface": "gateway"}]
        ]),
    );
    assert_eq!(fired.calls[1]["payload"]["platform"], "tui");
    assert_eq!(fired.calls[1]["payload"]["surface"], "gateway");
}

#[test]
fn a_subagent_reports_its_own_platform_without_becoming_the_process_front_end() {
    let fired = fire(
        "hermes-plugin-subagent",
        json!([
            ["pre_llm_call", {"session_id": SESSION, "user_message": "delegate it", "platform": "cli"}],
            ["pre_llm_call", {"session_id": "20260922_120001_child1", "user_message": "the delegated part", "platform": "subagent"}],
            ["pre_approval_request", {"command": "rm -rf /tmp/drill", "session_key": SESSION, "surface": "cli"}]
        ]),
    );
    assert_eq!(fired.calls[1]["payload"]["platform"], "subagent");
    assert_eq!(fired.calls[2]["payload"]["platform"], "cli");
}

#[test]
fn only_a_clarify_call_reaches_pns_as_asked_and_then_resolved() {
    let fired = fire(
        "hermes-plugin-clarify",
        json!([
            ["pre_tool_call", {"tool_name": "terminal", "args": {"command": "ls"}, "session_id": SESSION}],
            ["pre_tool_call", {"tool_name": "clarify", "args": {"question": "Red or blue?"}, "tool_call_id": "t1", "session_id": SESSION}],
            ["post_tool_call", {"tool_name": "clarify", "tool_call_id": "t1", "session_id": SESSION}],
            ["post_tool_call", {"tool_name": "terminal", "session_id": SESSION}]
        ]),
    );
    assert_eq!(argv(&fired), vec![json!(["hook", "asked"]), json!(["hook", "resolved"])]);
    assert_eq!(fired.calls[0]["payload"]["tool_input"], json!({"question": "Red or blue?"}));
}

#[test]
fn an_mcp_elicitation_is_asked_rather_than_blocked() {
    let fired = fire(
        "hermes-plugin-elicitation",
        json!([["pre_approval_request", {"command": "Authorize Gmail access", "description": "composio asks", "session_key": SESSION, "surface": "mcp-elicitation"}]]),
    );
    assert_eq!(argv(&fired), vec![json!(["hook", "asked"])]);
}

#[test]
fn no_callback_returns_anything_hermes_would_act_on() {
    let fired = fire(
        "hermes-plugin-returns",
        json!([
            ["pre_llm_call", {"session_id": SESSION, "platform": "cli"}],
            ["pre_tool_call", {"tool_name": "clarify", "session_id": SESSION}],
            ["post_tool_call", {"tool_name": "clarify", "session_id": SESSION}],
            ["post_llm_call", {"session_id": SESSION, "assistant_response": "done"}],
            ["pre_approval_request", {"session_key": SESSION, "surface": "cli"}],
            ["post_approval_response", {"session_key": SESSION, "surface": "cli", "choice": "deny"}],
            ["on_session_end", {"session_id": SESSION, "completed": true}]
        ]),
    );
    assert_eq!(fired.report["returned"], json!([null, null, null, null, null, null, null]));
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Add `mod hermes_plugin;` beside `mod codex_hooks;` in `pns/crates/pns-adapters/src/lib.rs`, and create
`pns/crates/pns-adapters/src/hermes_plugin.rs` holding only `#[cfg(test)] mod tests;`.

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters hermes_plugin`
Expected: compile error, `unresolved import super::render_hermes_plugin`.

- [ ] **Step 4: Write the shim**

Create `pns/crates/pns-adapters/src/hermes_plugin/pns_hooks.py`:

```python
"""pns-hooks: Hermes Agent's session events, handed to pns.

`pns hermes install-plugin` writes this file whole on every run.
"""

from collections import defaultdict, deque
from concurrent.futures import ThreadPoolExecutor
import json
import os
import subprocess
import threading
import uuid

PNS = __PNS_BINARY__
# pns bounds its own waits; this only stops one wedged call from holding the queue.
CALL_DEADLINE_SECS = 120
REMIND = "--remind=5m"

# One worker keeps Hermes's order: a prompt before its approval, an approval
# before its answer, and a turn's end after both.
_delivery = ThreadPoolExecutor(max_workers=1, thread_name_prefix="pns-hooks")
_pending = defaultdict(deque)
_pending_lock = threading.Lock()
_replies = {}
_front_end = ""


def _deliver(verb, flags, payload):
    try:
        subprocess.run(
            [PNS, "hook", verb, *flags],
            input=json.dumps(payload, ensure_ascii=False, default=str),
            text=True,
            env={**os.environ, "PNS_PRODUCER": "hermes"},
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=CALL_DEADLINE_SECS,
            check=False,
        )
    except Exception:
        pass


def _send(verb, event_name, session_id, flags=(), **extra):
    payload = {"hook_event_name": event_name, "session_id": session_id, "cwd": os.getcwd(), **extra}
    try:
        _delivery.submit(_deliver, verb, list(flags), payload)
    except RuntimeError:
        # Interpreter shutdown has closed the pool, so the last event goes inline.
        _deliver(verb, list(flags), payload)


def _session(primary="", kwargs=None):
    kwargs = kwargs or {}
    return str(kwargs.get("session_id") or primary or kwargs.get("task_id") or "hermes")


def _platform(stated=""):
    """The event's own platform, else the last one this process named for a session it hosts."""
    global _front_end
    if stated and stated != "subagent":
        _front_end = stated
    return stated or _front_end


def _approval_key(session_key, command, pattern_key):
    return (str(session_key or ""), str(command or ""), str(pattern_key or ""))


def _is_clarify(tool_name):
    return str(tool_name).strip().lower() == "clarify"


def on_pre_llm_call(session_id="", user_message="", model="", platform="", **kwargs):
    _send(
        "prompt", "UserPromptSubmit", _session(session_id, kwargs),
        prompt=user_message, model=model, platform=_platform(platform),
    )


def on_post_llm_call(session_id="", assistant_response="", **kwargs):
    _replies[_session(session_id, kwargs)] = assistant_response


def on_session_end(session_id="", completed=False, interrupted=False, model="", platform="", **kwargs):
    session = _session(session_id, kwargs)
    reply = _replies.pop(session, "")
    stated = _platform(platform)
    if interrupted:
        _send("resolved", "TurnInterrupted", session, interrupted=True, platform=stated)
    elif completed:
        _send("stop", "AgentEnd", session, last_assistant_message=reply, model=model, platform=stated)
    else:
        _send("stop-failure", "TurnFailed", session, error=reply, model=model, platform=stated)


def on_pre_tool_call(tool_name="", args=None, tool_call_id="", **kwargs):
    if _is_clarify(tool_name):
        _send(
            "asked", "PreToolUse", _session(kwargs=kwargs),
            tool_name=tool_name, tool_call_id=tool_call_id, tool_input=args or {},
            platform=_platform(),
        )


def on_post_tool_call(tool_name="", tool_call_id="", **kwargs):
    if _is_clarify(tool_name):
        _send(
            "resolved", "PostToolUse", _session(kwargs=kwargs),
            tool_name=tool_name, tool_call_id=tool_call_id, platform=_platform(),
        )


def on_pre_approval_request(
    command="", description="", pattern_key="", pattern_keys=None, session_key="", surface="", **kwargs
):
    action_id = uuid.uuid4().hex
    with _pending_lock:
        _pending[_approval_key(session_key, command, pattern_key)].append(action_id)
    asks = surface == "mcp-elicitation"
    _send(
        "asked" if asks else "blocked",
        "Elicitation" if asks else "PermissionRequest",
        _session(session_key, kwargs),
        () if asks else (REMIND,),
        action_id=action_id,
        command=command,
        description=description,
        pattern_key=pattern_key,
        pattern_keys=pattern_keys or [],
        surface=surface,
        platform=_platform(),
    )


def on_post_approval_response(
    command="", description="", pattern_key="", pattern_keys=None, session_key="", surface="", choice="",
    **kwargs
):
    key = _approval_key(session_key, command, pattern_key)
    with _pending_lock:
        queue = _pending.get(key)
        action_id = queue.popleft() if queue else ""
        if queue is not None and not queue:
            _pending.pop(key, None)
    _send(
        "resolved", "PermissionResolved", _session(session_key, kwargs),
        action_id=action_id,
        command=command,
        description=description,
        choice=choice,
        surface=surface,
        platform=_platform(),
    )


def register(ctx):
    ctx.register_hook("pre_llm_call", on_pre_llm_call)
    ctx.register_hook("post_llm_call", on_post_llm_call)
    ctx.register_hook("on_session_end", on_session_end)
    ctx.register_hook("pre_tool_call", on_pre_tool_call)
    ctx.register_hook("post_tool_call", on_post_tool_call)
    ctx.register_hook("pre_approval_request", on_pre_approval_request)
    ctx.register_hook("post_approval_response", on_post_approval_response)
```

- [ ] **Step 5: Write the renderer**

Replace the contents of `pns/crates/pns-adapters/src/hermes_plugin.rs` with:

```rust
//! pns's Hermes Agent plugin: the Python shim Hermes loads from
//! `$HERMES_HOME/plugins/pns-hooks/`, rendered for one pns binary.
//!
//! THE SHIM CALLS THE BINARY THAT RENDERED IT, by absolute path, the way the
//! Codex hooks do, so a plugin never reaches whatever later answers to `pns`.

#[cfg(test)]
mod tests;

/// The name Hermes registers the plugin under, which is also its directory.
pub const HERMES_PLUGIN_NAME: &str = "pns-hooks";

/// The shim's source, with `__PNS_BINARY__` where the binary's path goes.
const SHIM: &str = include_str!("hermes_plugin/pns_hooks.py");

/// The plugin's two files, as Hermes reads them.
#[derive(Debug, PartialEq, Eq)]
pub struct HermesPlugin {
    /// `__init__.py`, the shim.
    pub init_py: String,
    /// `plugin.yaml`, the manifest.
    pub manifest: String,
}

/// Both files for `binary`, the manifest stamped with `version`.
///
/// The path goes in as a JSON string literal, which Python reads as the same
/// string whatever quotes or backslashes the path holds.
pub fn render_hermes_plugin(binary: &str, version: &str) -> HermesPlugin {
    let literal = serde_json::Value::String(binary.to_string()).to_string();
    HermesPlugin {
        init_py: SHIM.replace("__PNS_BINARY__", &literal),
        manifest: format!(
            "name: {HERMES_PLUGIN_NAME}\nversion: \"{version}\"\n\
             description: Hands Hermes Agent session events to pns\n"
        ),
    }
}
```

In `pns/crates/pns-adapters/src/lib.rs`, beside the `pub use codex_hooks::{...}` block add:

```rust
pub use hermes_plugin::{HERMES_PLUGIN_NAME, HermesPlugin, render_hermes_plugin};
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters hermes_plugin`
Expected: 11 passed, each well under a second.

- [ ] **Step 7: Run the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 8: Commit**

```bash
git add pns/crates/pns-adapters/src/hermes_plugin.rs pns/crates/pns-adapters/src/hermes_plugin \
  pns/crates/pns-adapters/src/lib.rs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): embed a Hermes plugin that hands session events to pns"
```

---

### Task 6: `pns hermes install-plugin` writes and enables the plugin

Branch: `feat/pns-hermes-install-plugin`. Needs Task 5 merged.

**Files:**
- Modify: `pns/crates/pns-adapters/src/hermes_plugin.rs` (install and enable)
- Modify: `pns/crates/pns-adapters/src/lib.rs` (exports)
- Create: `pns/crates/pns/src/command_hermes.rs`
- Modify: `pns/crates/pns/src/command_codex.rs` (`running_binary` becomes `pub(crate)`)
- Modify: `pns/crates/pns/src/lib.rs`, `invocation.rs`, `subcommand_usage.rs`, `legacy/usage.rs`
- Create: `pns/crates/pns/tests/hermes_commands.rs`

**Interfaces:**
- Consumes: `HERMES_PLUGIN_NAME`, `render_hermes_plugin` from Task 5.
- Produces: `pub enum HermesPluginInstall { Changed, Unchanged }`;
  `pub fn hermes_plugin_dir(hermes_home: &Path) -> PathBuf`;
  `pub fn install_hermes_plugin(hermes_home: &Path, binary: &str, version: &str) -> Result<HermesPluginInstall, String>`;
  `pub fn enable_hermes_plugin(hermes: &str) -> Result<(), String>`; the CLI verb
  `pns hermes install-plugin`, which Task 7's script runs.

- [ ] **Step 1: Write the failing tests**

Create `pns/crates/pns/tests/hermes_commands.rs`:

```rust
//! `pns hermes install-plugin`, through the real binary, against a Hermes home
//! and a Hermes stand-in inside the sandbox, so nothing here can reach the
//! operator's own Hermes.

mod support;

use support::{ENGINE, Sandbox, run, run_expecting, stderr, write_script};

fn plugin_dir(home: &std::path::Path) -> std::path::PathBuf {
    home.join("plugins").join("pns-hooks")
}

fn hermes_home(sandbox: &Sandbox) -> std::path::PathBuf {
    sandbox.root.join("hermes-home")
}

/// The command, with its Hermes home and a Hermes stand-in that records its
/// argv and exits `code`.
fn install(sandbox: &Sandbox, code: i32) -> std::process::Command {
    let hermes = sandbox.root.join("hermes");
    write_script(
        &hermes,
        &format!("printf '%s\\n' \"$*\" >>\"{}/hermes.argv\"; exit {code}", sandbox.display()),
    );
    let mut command = sandbox.pns();
    command
        .env("HERMES_HOME", hermes_home(sandbox))
        .env("PNS_HERMES_BIN", &hermes)
        .args(["hermes", "install-plugin"]);
    command
}

#[test]
fn the_first_run_writes_and_enables_the_plugin_and_the_second_says_nothing() {
    let sandbox = Sandbox::without_config("hermes-install-plugin");
    let first = run(&mut install(&sandbox, 0));
    assert_eq!(first.status.code(), Some(0), "{first:?}");
    assert!(stderr(&first).contains("hermes gateway restart"), "{first:?}");
    let directory = plugin_dir(&hermes_home(&sandbox));
    let init = std::fs::read_to_string(directory.join("__init__.py")).expect("the shim");
    assert!(init.contains("def register(ctx):"));
    let manifest = std::fs::read_to_string(directory.join("plugin.yaml")).expect("the manifest");
    assert!(manifest.starts_with("name: pns-hooks\n"), "{manifest}");
    let second = run(&mut install(&sandbox, 0));
    assert_eq!(second.status.code(), Some(0), "{second:?}");
    assert_eq!(stderr(&second), "", "an idempotent re-run stays silent");
    assert_eq!(std::fs::read_to_string(directory.join("__init__.py")).expect("the shim"), init);
    assert_eq!(
        std::fs::read_to_string(sandbox.root.join("hermes.argv")).expect("hermes ran"),
        "plugins enable pns-hooks\nplugins enable pns-hooks\n"
    );
}

#[test]
fn the_plugin_calls_the_binary_that_wrote_it() {
    let sandbox = Sandbox::without_config("hermes-install-plugin-binary");
    run(&mut install(&sandbox, 0));
    let engine = std::fs::canonicalize(ENGINE).expect("the engine");
    let literal = serde_json::Value::String(engine.display().to_string()).to_string();
    let init = std::fs::read_to_string(plugin_dir(&hermes_home(&sandbox)).join("__init__.py"))
        .expect("the shim");
    assert!(init.contains(&format!("PNS = {literal}\n")), "{init}");
}

#[test]
fn a_hermes_that_cannot_enable_it_is_an_exit_two_naming_the_command() {
    let sandbox = Sandbox::without_config("hermes-install-plugin-refused");
    let output = run_expecting(2, &mut install(&sandbox, 1));
    assert!(stderr(&output).contains("plugins enable pns-hooks"), "{output:?}");
    assert!(plugin_dir(&hermes_home(&sandbox)).join("__init__.py").exists());
}

#[test]
fn without_hermes_home_the_plugin_lands_under_home() {
    let sandbox = Sandbox::without_config("hermes-install-plugin-default-home");
    let mut command = install(&sandbox, 0);
    command.env_remove("HERMES_HOME");
    run(&mut command);
    assert!(plugin_dir(&sandbox.root.join(".hermes")).join("plugin.yaml").exists());
}

#[test]
fn an_unknown_verb_or_a_stray_argument_is_the_usage_and_exit_two() {
    let sandbox = Sandbox::without_config("hermes-install-plugin-usage");
    let mut verb = sandbox.pns();
    verb.args(["hermes", "install-hooks"]);
    assert!(stderr(&run_expecting(2, &mut verb)).contains("pns hermes install-plugin"));
    let mut stray = install(&sandbox, 0);
    stray.arg("now");
    assert!(stderr(&run_expecting(2, &mut stray)).contains("pns hermes install-plugin"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hermes_commands`
Expected: all FAIL; `pns hermes` prints the tool-wide usage and exits 2.

- [ ] **Step 3: Implement install and enable**

Append to `pns/crates/pns-adapters/src/hermes_plugin.rs` (add `use std::path::{Path, PathBuf};` and
`use std::time::Duration;` at the top):

```rust
/// Whether an install had anything to write.
#[derive(Debug, PartialEq, Eq)]
pub enum HermesPluginInstall {
    /// A file was written, which Hermes reads at its next start.
    Changed,
    /// Both files already held these bytes.
    Unchanged,
}

/// Where the plugin lives under one Hermes home.
pub fn hermes_plugin_dir(hermes_home: &Path) -> PathBuf {
    hermes_home.join("plugins").join(HERMES_PLUGIN_NAME)
}

/// Write both files, each only when its bytes differ. EVERY FILE IN THE
/// DIRECTORY IS PNS'S, so each is replaced whole.
pub fn install_hermes_plugin(
    hermes_home: &Path,
    binary: &str,
    version: &str,
) -> Result<HermesPluginInstall, String> {
    let directory = hermes_plugin_dir(hermes_home);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("pns: cannot create {}: {error}", directory.display()))?;
    let plugin = render_hermes_plugin(binary, version);
    let mut changed = false;
    for (name, text) in [("__init__.py", &plugin.init_py), ("plugin.yaml", &plugin.manifest)] {
        changed |= publish(&directory, name, text)?;
    }
    Ok(match changed {
        true => HermesPluginInstall::Changed,
        false => HermesPluginInstall::Unchanged,
    })
}

/// One file, published by rename so Hermes reads the old bytes or the new;
/// true when it was written.
fn publish(directory: &Path, name: &str, text: &str) -> Result<bool, String> {
    let path = directory.join(name);
    if std::fs::read_to_string(&path).is_ok_and(|current| current == text) {
        return Ok(false);
    }
    let pending = directory.join(format!(".{name}.{}", std::process::id()));
    std::fs::write(&pending, text)
        .map_err(|error| format!("pns: cannot write {}: {error}", pending.display()))?;
    std::fs::rename(&pending, &path).map_err(|error| {
        let _ = std::fs::remove_file(&pending);
        format!("pns: cannot publish {}: {error}", path.display())
    })?;
    Ok(true)
}

/// How long Hermes's own enable may take: it starts a Python interpreter.
const ENABLE_DEADLINE: Duration = Duration::from_secs(30);

/// Enable the plugin through Hermes's `plugins enable`, which is idempotent
/// and is the one writer of Hermes's `plugins.enabled` list.
pub fn enable_hermes_plugin(hermes: &str) -> Result<(), String> {
    let mut command = std::process::Command::new(hermes);
    command.args(["plugins", "enable", HERMES_PLUGIN_NAME]);
    match crate::run_bounded(command, None, ENABLE_DEADLINE, crate::PROBE_READ_MAX) {
        Some(_) => Ok(()),
        None => Err(format!(
            "pns: Hermes did not enable the plugin; run `{hermes} plugins enable {HERMES_PLUGIN_NAME}`"
        )),
    }
}
```

Extend the export in `pns/crates/pns-adapters/src/lib.rs`:

```rust
pub use hermes_plugin::{
    HERMES_PLUGIN_NAME, HermesPlugin, HermesPluginInstall, enable_hermes_plugin, hermes_plugin_dir,
    install_hermes_plugin, render_hermes_plugin,
};
```

- [ ] **Step 4: Implement the verb**

Create `pns/crates/pns/src/command_hermes.rs`:

```rust
//! `pns hermes install-plugin`: pns's Hermes plugin, written and enabled.
//!
//! THE PLUGIN NAMES THIS BINARY, taken from `current_exe` and canonicalized
//! the way `pns codex install-hooks` does, so the plugin a run writes calls
//! the engine that wrote it.

use pns_adapters::HermesPluginInstall;
use std::path::PathBuf;

pub(crate) const HERMES_USAGE: &str = "pns: usage: pns hermes install-plugin; writes pns's \
Hermes plugin into $HERMES_HOME/plugins/pns-hooks (default ~/.hermes) and enables it";

pub(crate) fn hermes_mode(verb: &str) -> i32 {
    match verb {
        "install-plugin" => install_plugin(),
        // UNKNOWN IS AN ERROR, never a silent fallthrough: an operator who
        // mistyped the verb believes the plugin is installed.
        _ => {
            eprintln!("{HERMES_USAGE}");
            2
        }
    }
}

fn install_plugin() -> i32 {
    if !crate::arguments_after_verb().is_empty() {
        eprintln!("{HERMES_USAGE}");
        return 2;
    }
    let Some(home) = hermes_home() else {
        eprintln!("pns: HERMES_HOME and HOME are both unset, so there is no Hermes home to write into");
        return 2;
    };
    let Some(binary) = crate::command_codex::running_binary() else {
        eprintln!("pns: cannot resolve this binary's own path, so there is no command to install");
        return 2;
    };
    let installed =
        match pns_adapters::install_hermes_plugin(&home, &binary, env!("CARGO_PKG_VERSION")) {
            Ok(installed) => installed,
            Err(refusal) => {
                eprintln!("{refusal}");
                return 2;
            }
        };
    let hermes = std::env::var("PNS_HERMES_BIN").unwrap_or_else(|_| "hermes".to_string());
    if let Err(refusal) = pns_adapters::enable_hermes_plugin(&hermes) {
        eprintln!("{refusal}");
        return 2;
    }
    if installed == HermesPluginInstall::Changed {
        eprintln!(
            "pns: wrote the Hermes plugin to {}. Hermes loads it at its next start; restart the \
gateway with `hermes gateway restart`.",
            pns_adapters::hermes_plugin_dir(&home).display()
        );
    }
    0
}

/// Hermes's own home, resolved the way Hermes resolves it: `HERMES_HOME`
/// trimmed when it names one, else `~/.hermes`.
fn hermes_home() -> Option<PathBuf> {
    let named = |key: &str| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    named("HERMES_HOME")
        .map(PathBuf::from)
        .or_else(|| named("HOME").map(|home| PathBuf::from(home).join(".hermes")))
}
```

Wire it:

- `pns/crates/pns/src/command_codex.rs`: `fn running_binary()` becomes `pub(crate) fn running_binary()`.
- `pns/crates/pns/src/lib.rs`: `mod command_hermes;` after `mod command_github;`, and
  `pub(crate) use command_hermes::hermes_mode;` after `pub(crate) use command_codex::codex_mode;`.
- `pns/crates/pns/src/invocation.rs`, directly after the `if first == "codex" { ... }` block:

```rust
    // Hermes Agent's plugin, written and enabled. A MODE beside Codex's for the
    // same reason: it is one-time wiring the apply runs, and it delivers nothing.
    if first == "hermes" {
        std::process::exit(hermes_mode(&second_argument(&flagless)));
    }
```

- `pns/crates/pns/src/subcommand_usage.rs`: add `("hermes", crate::command_hermes::HERMES_USAGE),` after
  the `("codex", ...)` row.
- `pns/crates/pns/src/legacy/usage.rs`: add after the `pns codex install-hooks` line, aligned to the same
  description column:

```text
  pns hermes install-plugin        write and enable pns's Hermes plugin
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hermes_commands`
Expected: 5 passed.

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns subcommand_usage`
Expected: the listing tests pass with the new row.

- [ ] **Step 6: Run the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 7: Commit**

```bash
git add pns/crates/pns-adapters/src/hermes_plugin.rs pns/crates/pns-adapters/src/lib.rs \
  pns/crates/pns/src/command_hermes.rs pns/crates/pns/src/command_codex.rs pns/crates/pns/src/lib.rs \
  pns/crates/pns/src/invocation.rs pns/crates/pns/src/subcommand_usage.rs \
  pns/crates/pns/src/legacy/usage.rs pns/crates/pns/tests/hermes_commands.rs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): add pns hermes install-plugin"
```

---

### Task 7: This machine runs pns's Hermes plugin instead of moshi's and the shell hook pair

Branch: `feat/hermes-pns-plugin-cutover`. Needs Tasks 1 to 6 merged. This is the only task that changes
what an apply does.

**Files:**
- Create: `.chezmoiscripts/run_after_72-pns-hermes-plugin.sh.tmpl`
- Modify: `private_dot_hermes/modify_private_config.yaml` (header list, the approval-pair block)
- Modify: `test/unit/hermes-config-modify-template.test.sh`
- Modify: `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` (header comment, line 63)
- Modify: `CLAUDE.md` (the two `run_after_72` sentences)

**Interfaces:**
- Consumes: `pns hermes install-plugin` from Task 6.

- [ ] **Step 1: Write the failing template test**

In `test/unit/hermes-config-modify-template.test.sh`, replace
`test_the_approval_pair_is_declared_and_a_hermes_owned_hook_event_survives` with:

```bash
function test_the_approval_pair_is_removed_and_a_hermes_owned_hook_event_survives() {
  local rendered
  rendered="$(
    hermes_config_render <<'LIVE'
hooks:
  pre_approval_request:
    - command: /usr/bin/env PNS_PRODUCER=hermes /Users/x/.cargo/bin/pns hook blocked --remind
  post_approval_response:
    - command: /usr/bin/env PNS_PRODUCER=hermes /Users/x/.cargo/bin/pns hook resolved
  pre_tool_call:
    - command: /bin/true
LIVE
  )"
  assert_not_contains 'pre_approval_request' "$rendered"
  assert_not_contains 'post_approval_response' "$rendered"
  assert_not_contains 'PNS_PRODUCER' "$rendered"
  assert_contains 'pre_tool_call:' "$rendered"
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `just test-bashunit`
Expected: `test_the_approval_pair_is_removed...` FAILS on the first `assert_not_contains`.

- [ ] **Step 3: Remove the pair in the template**

In `private_dot_hermes/modify_private_config.yaml`:

- In the header's "WHAT THIS OWNS" list, replace the two `hooks.*` rows with:

```text
    hooks.pre_approval_request       removed
    hooks.post_approval_response     removed
```

  and change "overlays the five things above" to "overlays the three things above and removes the two".
- Replace the whole block from `{{- /* THE APPROVAL PAIR,` through the second `setValueAtPath (list
  "hooks" "post_approval_response")` statement, including the `{{- $pns := ... -}}` line, with:

```text
{{- /* BOTH APPROVAL HOOKS ARE REMOVED: pns's Hermes plugin (`pns hermes
       install-plugin`, run by run_after_72-pns-hermes-plugin) reports these two
       events itself, so a shell hook on either would report each approval
       twice. Every other hook event is hermes's and passes through. */ -}}
{{- $hooks := get $merged "hooks" -}}
{{- if eq (kindOf $hooks) "map" -}}
{{-   $hooks = unset $hooks "pre_approval_request" -}}
{{-   $hooks = unset $hooks "post_approval_response" -}}
{{- end -}}
```

In `test/unit/hermes-config-modify-template.test.sh`, the template no longer reads `.rust_tools`:
delete the `cp "$repo/.chezmoidata/rust_tools.yaml" ...` line and its comment in both helpers (keep the
`mkdir -p` of `.chezmoidata`), change the helper comment "`$1` is the HOME the render resolves the pns
hook path against" to "`$1` is the HOME the render runs under", and change the file header's "owns four
things" sentence to "owns three things and removes the two approval hooks".

- [ ] **Step 4: Run it to verify it passes**

Run: `just test-bashunit`
Expected: every test in `hermes-config-modify-template.test.sh` passes, including the byte-for-byte
no-op test.

- [ ] **Step 5: Add the apply script**

Create `.chezmoiscripts/run_after_72-pns-hermes-plugin.sh.tmpl`:

```bash
{{ if eq .chezmoi.os "darwin" -}}
#!/bin/bash
# Write and enable pns's Hermes plugin on every apply (idempotent; a run that
# changes nothing prints nothing). Runs after run_once_after_60, which
# uninstalls moshi's own Hermes plugin, so an approval raises one phone card.
set -euo pipefail
engine={{ joinPath .chezmoi.homeDir .rust_tools.install_dir "pns" | quote }}
if [[ -x $engine ]] && command -v hermes >/dev/null 2>&1; then
  # A refusal is a printed warning rather than a failed apply.
  "$engine" hermes install-plugin || true
fi
{{ end -}}
```

- [ ] **Step 6: Uninstall moshi's Hermes plugin**

In `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl`, change line 63 to:

```bash
/opt/homebrew/bin/moshi-hook uninstall --target codex,hermes 2>/dev/null || true
```

In its header comment, change "before run_after_72, which wires relay's Codex hooks and depends on the
Codex exclusion below" to "before the two run_after_72 scripts, which wire pns's Codex hooks and its
Hermes plugin and depend on the Codex and Hermes uninstall below", and add after the paragraph that
begins "Claude Code and Codex are deliberately EXCLUDED":

```text
# Hermes is uninstalled for the same reason: pns's own Hermes plugin forwards
# CLI approvals to `moshi-hook hermes-hook`, so moshi's plugin would raise a
# second card for each one. `install --target` below already leaves hermes out.
```

- [ ] **Step 7: Update CLAUDE.md**

Change "the Codex hooks are wired by `pns codex install-hooks`, run by `run_after_72`." to "the Codex
hooks are wired by `pns codex install-hooks`, run by `run_after_72-relay-codex-hooks`, and the Hermes
plugin by `pns hermes install-plugin`, run by `run_after_72-pns-hermes-plugin`." Change "and
`run_after_72` is what runs it; pns ships no bash at all." to "and `run_after_72-relay-codex-hooks` is
what runs it; `pns hermes install-plugin` writes and enables pns's Hermes plugin, a Python shim embedded
in the binary; pns ships no bash at all." Then run `just m`.

- [ ] **Step 8: Prove the apply scripts before anyone applies**

```bash
scratch="$(mktemp -d)"
CI=1 chezmoi --source "$PWD" execute-template --no-tty \
  < .chezmoiscripts/run_after_72-pns-hermes-plugin.sh.tmpl > "$scratch/72.sh"
shellcheck "$scratch/72.sh"
cargo build --locked --release --manifest-path pns/Cargo.toml -p pns
sed "s|^engine=.*|engine=\"$PWD/pns/target/release/pns\"|" "$scratch/72.sh" > "$scratch/72-local.sh"
HERMES_HOME="$scratch/hermes" PNS_HERMES_BIN=/usr/bin/true bash "$scratch/72-local.sh"; echo "exit $?"
HERMES_HOME="$scratch/hermes" PNS_HERMES_BIN=/usr/bin/true bash "$scratch/72-local.sh"; echo "exit $?"
ls "$scratch/hermes/plugins/pns-hooks"
sed 's|(keepassxc "moshi-hook :: Device Token").Password|"stub-token"|' \
  .chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl \
  | CI=1 chezmoi --source "$PWD" execute-template --no-tty > "$scratch/60.sh"
bash -n "$scratch/60.sh" && shellcheck "$scratch/60.sh"
grep -n 'uninstall --target codex,hermes' "$scratch/60.sh"
```

Expected: shellcheck silent; the first run prints the "wrote the Hermes plugin" line and `exit 0`; the
second prints only `exit 0`; the listing shows `__init__.py` and `plugin.yaml`; the rendered 60 script
parses, lints clean and shows the new uninstall line. Never run the rendered 60 script: it pairs and
installs moshi for real. Clear the scratch directory with `trash "$scratch"`.

- [ ] **Step 9: Run the gates**

Run: `just test-unit && just lint-check`
Expected: both exit 0.

- [ ] **Step 10: Commit**

```bash
git add .chezmoiscripts/run_after_72-pns-hermes-plugin.sh.tmpl private_dot_hermes/modify_private_config.yaml \
  test/unit/hermes-config-modify-template.test.sh .chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl \
  CLAUDE.md
SKIP_AI_COMMIT=1 git commit -m "feat(hermes): run pns's Hermes plugin in place of moshi's and the shell hooks"
```

The pull request body must tell the operator that the next apply re-fires `run_once_after_60` once (its
content changed), that KeePassXC must be unlocked for it and for the config template, and that
`hermes gateway restart` follows the apply.

---

### Task 8: The drills

No branch and no pull request. The operator runs these after applying Task 7 and running
`hermes gateway restart`. Record each result under "pns validation follow-ups" in
`docs/remaining-work.md`.

- [ ] **Preflight.** `hermes plugins list --user` shows `pns-hooks` enabled and no `moshi-hooks`.
  `hermes hooks list` shows no `pns hook` command.
- [ ] **H1, desk approval.** In a herdr pane: `hermes chat --cli`, then ask it to run
  `rm -rf /tmp/pns-hermes-drill`. Expect a pns banner naming the command, no phone card, the blocked
  lamp if lamps are on. Answer once at the pane: the lamp clears, no moshi card or push appears, the turn
  ends with a `hermes` done banner.
- [ ] **H2, phone approval.** Same prompt with the surface Mobile or Away, set the way drill D8 set it.
  Expect exactly one moshi card. Approve on the phone: moshi types the answer, the command runs, the card
  clears. Repeat with Deny and record the result.
- [ ] **H3, answered at the desk after the card.** Go away, let the card arrive, answer at the pane.
  Expect the phone card to clear.
- [ ] **H4, question.** Ask Hermes to use its clarify tool to ask which of two colors to use. Expect an
  `asked` notification, cleared when answered.
- [ ] **H5, failure.** `hermes chat --cli --max-turns 1` with a request that needs two tool calls.
  Expect a `failed` notification.
- [ ] **H6, gateway.** Message the Hermes Discord bot with a request that runs a tool. Expect no banner,
  no phone card, no lamp change and no `#pns-events` line; `pns recap --since 15m` lists the session's
  `hermes` rows.
