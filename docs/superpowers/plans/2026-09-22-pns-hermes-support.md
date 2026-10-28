# pns Hermes Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended)
> or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`)
> syntax for tracking.

**Goal:** Hermes Agent terminal sessions notify exactly like Claude Code sessions, including the phone
approval through `moshi-hook hermes-hook`; Hermes gateway sessions and kanban workers' turns are recorded
for the recap; and moshi's own Hermes screens keep receiving what moshi's plugin sent them.

**Architecture:** A Python shim embedded in the pns binary (`pns-hooks`) runs inside every Hermes process
and registers nine hooks. Seven report to pns through the existing `pns hook` verbs, stamped with the
platform their session named (and a kanban worker's task), except on Hermes's background review thread.
Every hook but the approval pair also feeds `moshi-hook hermes-hook` directly, with the payload moshi's
own Hermes plugin built. The shim registers nothing inside a run pns's own summarizer started
(`PNS_SUMMARIZING`). pns decides from `platform`, `surface` and `kanban_task` whether an event takes
every existing arm or is recorded only (one activity row). `moshi_subcommand` admits `hermes` for a CLI
(command-line interface) approval, and the answer to a request pns forwarded is forwarded too, so moshi
clears its card; a drill-gated last task admits the TUI. `pns hermes install-plugin` writes and enables
the shim; on dresden an apply runs it, removes the Hermes shell hook pair, keeps `moshi-hooks` in
Hermes's `plugins.disabled`, and uninstalls moshi's own Hermes plugin.

**Tech Stack:** Rust 2024 (the `pns` workspace), Python 3 for the shim and its test driver, bashunit and
chezmoi templates for the dotfiles pull request. Gates: `just test-rust`, `just test-unit`,
`just lint-check`.

**Spec:** `docs/superpowers/specs/2026-09-22-pns-hermes-support-design.md`

## Global Constraints

- The producer value is `hermes`. The shim sets `PNS_PRODUCER=hermes` in each child's environment, and
  pns matches the literal `"hermes"`.
- The plugin is named `pns-hooks` and lives in `$HERMES_HOME/plugins/pns-hooks/`, default
  `~/.hermes/plugins/pns-hooks/`. Its files are `__init__.py` and `plugin.yaml`.
- The shim registers exactly nine hooks: `on_session_start`, `pre_llm_call`, `post_llm_call`,
  `on_session_end`, `on_session_finalize`, `pre_tool_call`, `post_tool_call`, `pre_approval_request`,
  `post_approval_response`. The two session hooks feed moshi only; the approval pair reports to pns only;
  the rest do both.
- `blocked` is always sent as `hook blocked --remind=5m`. No other verb carries a flag.
- A Hermes event is recorded only when `platform` is a non-empty value other than `cli` and `tui` (the
  classic CLI and the TUI (terminal user interface)), or when `platform` is empty and `surface` is
  `gateway`, or when a kanban worker (`kanban_task` set) sends `prompt`, `stop` or `blocked`. Everything
  else, including both fields missing, takes the ordinary arms.
- moshi receives every non-approval event from the shim with moshi's own plugin's payload, except inside
  a run pns's own summarizer started, where the shim registers nothing. It receives the approval pair
  only from pns, only when `surface` is `cli` (and, from Task 10, when `platform` is `tui`), and never
  from a kanban worker: the request through `blocked`, presence gated; the answer through `resolved`,
  never gated, and only for an `action_id` whose request pns forwarded.
- The shim reports nothing to pns from a thread named `bg-review` (moshi is still fed from it, as its own
  plugin fed it), and registers no hook when `PNS_SUMMARIZING` is set. Every summarizer child pns starts
  carries `PNS_SUMMARIZING=1`.
- pns ships no bash. The shim is Python, embedded with `include_str!` and written by the binary. Test
  stand-ins may be bash (`support::write_script`) or Python fixtures. The shim runs under Hermes's own
  Python 3.11 (`~/.hermes/hermes-agent/venv`), so it uses nothing newer.
- No test reaches the real Hermes, moshi-hook or Codex: the sandbox fences `PNS_MOSHI_HOOK_BIN` and
  `PNS_CODEX_BIN` by default and, from Task 7, `PNS_HERMES_BIN`; tests point them at stand-ins, and
  every Hermes home is a scratch directory.
- Nothing reads `~/.hermes/config.yaml`, `~/.hermes/.env` or `~/.config/pns/config.toml`.
- Every `.rs` file stays at 300 lines ideal and 500 hard cap, unit tests included.
- Comments say what the code does or why it is that way. None explains an absence, a rejected option,
  or this plan.
- No em-dashes in any comment, message, commit or document.
- Conventional commits, `SKIP_AI_COMMIT=1`, no co-author trailer.
- Each task is one pull request on its own branch. A task ends at its commit: pushing, opening the pull
  request, review and merging are not part of it.
- Create each worktree with
  `herdr worktree create --cwd /Users/stephen/workspaces/Ivy/webdavis/dotfiles --branch <branch> --no-focus`
  and run every command from that worktree's root (`~/.herdr/worktrees/dotfiles/<branch with / as ->`).
  Rebase on `origin/main` first. Open pull requests touch the same files, and the code below depends on
  none of them landing, but its anchors can move: #917 and #919 touch `hook_dispatch.rs`, `routing.rs`,
  `moshi_submission.rs` and `legacy/usage.rs`; #919 deletes `HookPayload::file_path` and its
  `parse_payload` line; #917 and #919 each remove a numbered behavior from
  `pns/docs/specs/hook-compatibility.md`; #915 edits `command_codex.rs`. So new struct fields go at the
  END of `HookPayload` and of `parse_payload`'s literal, and a new spec behavior takes the next free
  number on the rebased file, whatever those anchors are called by then.
- The Rust blocks below are not pre-formatted: run `cargo fmt --all --manifest-path pns/Cargo.toml`
  before the gates.
- Every pns task ends with `just test-rust` and `just lint-check` green. `just test-rust` needs `chord`
  installed (`cargo install --git https://github.com/webdavis/chord chord`) and `python3` on `PATH`.

## Order

Task 1 first. Tasks 2 and 3 follow Task 1, in any order. Task 4 follows Task 3: it calls the
two-argument `moshi_subcommand` Task 3 introduces and appends to the test file Task 3 extends. Tasks 5
and 6 are independent of every other task. Task 7 follows Tasks 5 and 6, so no installed shim exists
before summarizer runs are marked. Task 8 follows all seven merges. Task 9 follows the apply of Task 8.
Task 10 follows Task 9, and its pull request opens only after its own drill passes.

## File Structure

| File | Task | Responsibility |
| --- | --- | --- |
| `pns/crates/pns-adapters/src/harness/payload.rs` | 1, 2, 4 | `platform`, `surface`, `kanban_task` and `action_id` fields; the approval's card text in the `message` chain |
| `pns/crates/pns-adapters/src/harness/hermes.rs` (new) | 1 | `hermes_records_only`, the terminal-or-recorded rule |
| `pns/crates/pns-adapters/src/harness/message.rs` | 2 | `guarded_command`, a Hermes approval's text |
| `pns/crates/pns-adapters/src/harness/routing.rs` | 3, 10 | `moshi_subcommand(agent, payload)`; the TUI arm |
| `pns/crates/pns-adapters/src/hermes_plugin.rs` (new) | 5, 7 | render, install and enable the plugin |
| `pns/crates/pns-adapters/src/hermes_plugin/pns_hooks.py` (new) | 5 | the shim Hermes loads |
| `pns/crates/pns-adapters/src/hermes_plugin/tests.rs`, `tests/moshi.rs` (new) | 5 | the shim's pns reports; its moshi feed |
| `pns/crates/pns-adapters/src/hermes_plugin/fixtures/{drive,record}.py` (new) | 5 | the shim's test driver and pns and moshi stand-in |
| `pns/crates/pns-adapters/src/recap/summarizer.rs` | 6 | `PNS_SUMMARIZING=1` on every summarizer child |
| `pns/crates/pns/src/hermes_session.rs` (new) | 1 | the recorded-only activity row |
| `pns/crates/pns/src/hook_dispatch.rs` | 1, 4 | the recorded-only branch; the answer forward call |
| `pns/crates/pns/src/moshi_submission.rs` | 3, 4 | the surface at the call site; the forwarded-request marker; `forward_resolution` |
| `pns/crates/pns/src/command_hermes.rs` (new) | 7 | `pns hermes install-plugin` |
| `pns/crates/pns/tests/hooks/hermes_sessions.rs` (new) | 1, 3, 4, 10 | end-to-end hook behavior for producer `hermes` |
| `pns/crates/pns/tests/hermes_commands.rs` (new) | 7 | end-to-end `pns hermes install-plugin` |
| `pns/crates/pns/tests/support/sandbox/commands.rs` | 7 | `PNS_HERMES_BIN` fenced off by default |
| `.chezmoiscripts/run_after_72-pns-hermes-plugin.sh.tmpl` (new) | 8 | runs `install-plugin` on every apply |
| `private_dot_hermes/modify_private_config.yaml` | 8 | removes the Hermes shell hook pair; keeps `moshi-hooks` in `plugins.disabled` |
| `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` | 8 | uninstalls moshi's Hermes plugin |

---

### Task 1: A Hermes event nobody at a pane is watching for is recorded and delivered nowhere

Branch: `feat/pns-hermes-recorded-only-sessions`.

**Files:**
- Modify: `pns/crates/pns-adapters/src/harness/payload.rs` (struct fields and `parse_payload`)
- Create: `pns/crates/pns-adapters/src/harness/hermes.rs`
- Modify: `pns/crates/pns-adapters/src/harness.rs`, `pns/crates/pns-adapters/src/lib.rs` (exports)
- Create: `pns/crates/pns-adapters/src/harness/tests/hermes.rs`; modify `harness/tests.rs`
- Create: `pns/crates/pns/src/hermes_session.rs`; modify `pns/crates/pns/src/lib.rs`
- Modify: `pns/crates/pns/src/hook_dispatch.rs` (`hook_mode`, `session_only_event`)
- Create: `pns/crates/pns/tests/hooks/hermes_sessions.rs`; modify `pns/crates/pns/tests/hooks.rs`
- Modify: `pns/docs/specs/hook-compatibility.md` (intro, one new behavior at the next free number)

**Interfaces:**
- Produces: `HookPayload::platform`, `HookPayload::surface` and `HookPayload::kanban_task`, all
  `String`; `pub fn hermes_records_only(event: &str, payload: &HookPayload) -> bool` exported from
  `pns_adapters`;
  `pub(crate) fn hermes_session::record_only(event: &str, payload: &HookPayload, agent: &str)`;
  `hook_dispatch::session_only_event` becomes `pub(crate)`. The test file
  `tests/hooks/hermes_sessions.rs` with helpers `hermes(&Sandbox) -> Command`,
  `recorded(&Sandbox) -> Vec<(String, String)>` and `const SESSION: &str`, which Tasks 3 and 4 extend.

- [ ] **Step 1: Write the failing unit tests**

Create `pns/crates/pns-adapters/src/harness/tests/hermes.rs`:

```rust
use super::*;

/// A payload with these three keys, the rest absent.
fn stamped(platform: &str, surface: &str, kanban_task: &str) -> HookPayload {
    parse_payload(
        &serde_json::json!({"platform": platform, "surface": surface, "kanban_task": kanban_task})
            .to_string(),
    )
}

#[test]
fn a_terminal_platform_notifies_whatever_the_surface_says() {
    for surface in ["", "cli", "gateway", "mcp-elicitation"] {
        for event in ["prompt", "stop", "blocked", "asked"] {
            assert!(!hermes_records_only(event, &stamped("cli", surface, "")), "cli/{surface}/{event}");
            assert!(!hermes_records_only(event, &stamped("tui", surface, "")), "tui/{surface}/{event}");
        }
    }
}

#[test]
fn every_other_named_platform_is_recorded_only() {
    for platform in ["discord", "telegram", "webhook", "api_server", "cron", "acp", "curator", "subagent"] {
        assert!(hermes_records_only("stop", &stamped(platform, "cli", "")), "{platform}/cli");
        assert!(hermes_records_only("blocked", &stamped(platform, "", "")), "{platform}/none");
    }
}

#[test]
fn with_no_platform_only_a_gateway_surface_is_recorded_only() {
    assert!(hermes_records_only("blocked", &stamped("", "gateway", "")));
    for surface in ["", "cli", "mcp-elicitation"] {
        assert!(!hermes_records_only("blocked", &stamped("", surface, "")), "none/{surface}");
    }
}

#[test]
fn a_kanban_worker_records_its_turns_and_approvals_and_notifies_the_rest() {
    let worker = stamped("cli", "cli", "t_42");
    for event in ["prompt", "stop", "blocked"] {
        assert!(hermes_records_only(event, &worker), "{event}");
    }
    for event in ["stop-failure", "asked", "resolved"] {
        assert!(!hermes_records_only(event, &worker), "{event}");
    }
}

#[test]
fn a_payload_yields_its_platform_surface_and_kanban_task_and_misses_none_as_an_error() {
    let payload = parse_payload(r#"{"session_id":"s1","platform":"tui","surface":"gateway","kanban_task":"t_42"}"#);
    assert_eq!(
        (payload.platform.as_str(), payload.surface.as_str(), payload.kanban_task.as_str()),
        ("tui", "gateway", "t_42")
    );
    let bare = parse_payload(r#"{"session_id":"s1"}"#);
    assert_eq!((bare.platform.as_str(), bare.surface.as_str(), bare.kanban_task.as_str()), ("", "", ""));
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

#[test]
fn a_kanban_worker_turn_and_approval_are_recorded_while_its_question_is_carded_until_the_worker_ends() {
    let sandbox = Sandbox::new("hermes-kanban-worker");
    sandbox.write_config(&format!("{}{LAMPS_ON}", support::STUB_CHANNELS));
    let worker = |event: &str, fields: &str| {
        let mut command = hermes(&sandbox);
        sandbox.stub_moshi(&mut command, 0);
        hook_with(
            command,
            &sandbox,
            event,
            &format!(r#"{{"session_id":"{SESSION}","cwd":"/tmp","platform":"cli","kanban_task":"t_42",{fields}}}"#),
        )
    };
    worker("prompt", r#""hook_event_name":"UserPromptSubmit","prompt":"work kanban task t_42""#);
    assert!(!marker(&sandbox, SESSION).exists(), "a worker's turn starts no turn clock");
    worker(
        "blocked",
        r#""hook_event_name":"PermissionRequest","action_id":"a1","command":"rm -rf /tmp/x","surface":"cli""#,
    );
    // Nothing can answer a worker's approval, so it raises nothing anywhere.
    assert_eq!(submissions(&sandbox), Vec::<String>::new());
    assert_eq!(waiting_sessions(&sandbox), Vec::<String>::new());
    for channel in ["banner", "phone", "hermes"] {
        assert!(!sandbox.fired(channel), "{channel} fired for a worker's approval");
    }
    worker(
        "asked",
        r#""hook_event_name":"PreToolUse","tool_name":"kanban_block","tool_input":{"reason":"Which region?"}"#,
    );
    // Only the ordinary `asked` arm starts a wait: the recorded path never does.
    assert_eq!(waiting_sessions(&sandbox), vec![SESSION.to_string()]);
    worker("stop", r#""hook_event_name":"AgentEnd","last_assistant_message":"blocked on the region""#);
    assert_eq!(waiting_sessions(&sandbox), Vec::<String>::new());
    assert_eq!(sandbox.event("hermes")["state"], "asked", "the worker's turn end delivered nothing");
    assert_eq!(
        recorded(&sandbox),
        vec![
            ("prompt".to_string(), SESSION.to_string()),
            ("blocked".to_string(), SESSION.to_string()),
            ("asked".to_string(), SESSION.to_string()),
            ("done".to_string(), SESSION.to_string()),
        ]
    );
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
Expected: `a_chat_gateway_turn...`, `a_gateway_prompt_and_approval...` and `a_kanban_worker_turn...`
FAIL (the gateway turn fires `hermes`, the prompts write a turn marker); the other two pass already.

- [ ] **Step 4: Add the payload fields**

In `pns/crates/pns-adapters/src/harness/payload.rs`, append at the end of `HookPayload` (after whatever
field is last on the rebased file; `file_path` today):

```rust
    /// Which Hermes front end sent the event: `cli`, `tui`, a gateway
    /// platform's name, `cron`, `acp`, `curator`, `subagent`. The pns-hooks
    /// plugin stamps it on every event it sends.
    pub platform: String,
    /// Which approval surface a Hermes approval came through: `cli`, `gateway`
    /// or `mcp-elicitation`. Empty on every other event.
    pub surface: String,
    /// The kanban task a Hermes worker process was started for, from its
    /// `HERMES_KANBAN_TASK`. Empty outside a kanban worker.
    pub kanban_task: String,
```

and append at the end of the struct literal in `parse_payload` (after `file_path: text("file_path"),`
today):

```rust
        platform: text("platform"),
        surface: text("surface"),
        kanban_task: text("kanban_task"),
```

- [ ] **Step 5: Add the rule**

Create `pns/crates/pns-adapters/src/harness/hermes.rs`:

```rust
use super::payload::HookPayload;

/// Whether a Hermes event is recorded for the recap and delivered nowhere.
///
/// THE PLATFORM DECIDES FIRST. `cli` and `tui` are Hermes's terminal front
/// ends, and every other platform it names (a chat gateway, `webhook`,
/// `cron`, `acp`, `curator`, `subagent`) has no operator at a pane. The surface
/// decides only when no platform arrived, because the TUI's approvals report
/// `gateway` as well. An event that states neither notifies.
///
/// A KANBAN WORKER runs on `cli` with nobody watching its turns and no
/// terminal to answer an approval at, so its turn start and end and its
/// approval requests are recorded while its failures and questions take the
/// ordinary arms.
pub fn hermes_records_only(event: &str, payload: &HookPayload) -> bool {
    match payload.platform.as_str() {
        "cli" | "tui" => {
            !payload.kanban_task.is_empty() && matches!(event, "prompt" | "stop" | "blocked")
        }
        "" => payload.surface == "gateway",
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
Expected: 5 passed.

- [ ] **Step 7: Record the row and return**

Create `pns/crates/pns/src/hermes_session.rs`:

```rust
//! A Hermes event nobody at a pane is watching for: one activity row for the
//! recap.

use crate::*;

/// Writes the activity row `event` would have written, with the state word
/// its delivering arm raises.
pub(crate) fn record_only(event: &str, payload: &HookPayload, agent: &str) {
    // A turn's start and end close the session's wait, as the delivering arms
    // do: a kanban worker's question is carded by its ordinary arm, and its
    // lamp ends with the worker's turn.
    if matches!(event, "prompt" | "stop") {
        end_blocked_wait(&payload.session_id, now_secs());
    }
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
    // A Hermes event nobody at a pane is watching for writes its activity row
    // and returns.
    if agent == "hermes" && pns_adapters::hermes_records_only(event, &payload) {
        crate::hermes_session::record_only(event, &payload, &agent);
        return 0;
    }
```

- [ ] **Step 8: Run the end-to-end tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hooks hermes_sessions`
Expected: 5 passed.

- [ ] **Step 9: State the behavior in the pns spec**

In `pns/docs/specs/hook-compatibility.md`, change the intro's "a coding harness (Claude Code or Codex)"
to "a coding harness (Claude Code, Codex or Hermes Agent)". Append, before "## Environment inputs this
path reads", one behavior numbered one past the last numbered heading on the rebased file (`N` below;
29 on today's main, lower once #917 or #919 lands):

```markdown
## N. A Hermes event nobody at a pane is watching for is recorded and delivered nowhere

Given a payload whose producer is `hermes` and whose `platform` is a non-empty value other than `cli`
and `tui`, or whose `platform` is empty and whose `surface` is `gateway`, or whose `platform` is `cli`
or `tui` with a non-empty `kanban_task` and whose event is `prompt`, `stop` or `blocked`

When `pns hook <event>` runs

Then one activity row is written with the event's state word (`stop` as `done`, `stop-failure` as
`failed`, every other word as itself), a `prompt` or `stop` ends the session's wait, and the hook exits
0 without reaching any arm.

- Success: a Discord turn end writes one `done` row and fires no channel
  (`tests/hooks/hermes_sessions.rs:a_chat_gateway_turn_is_recorded_as_done_and_delivered_nowhere`).
- Forbidden side effects: no turn marker, no reminder, no summarizer, no moshi spawn
  (`a_gateway_prompt_and_approval_leave_no_marker_and_start_no_round_trip`).
- Kanban workers: a worker's approval raises nothing and is never handed to moshi, its question takes
  the ordinary arm, and that wait ends with the worker's turn
  (`a_kanban_worker_turn_and_approval_are_recorded_while_its_question_is_carded_until_the_worker_ends`).
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
SKIP_AI_COMMIT=1 git commit -m "feat(pns): record the Hermes events nobody at a pane is watching for"
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

In `pns/docs/specs/hook-compatibility.md` behavior 6 ("One `message` is composed from four payload
roads, in a fixed order"), change the heading to "One `message` is composed
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
- Produces: `pub fn moshi_subcommand(agent: &str, payload: &HookPayload) -> Option<String>`;
  `Some("hermes-hook")` for `hermes` only when `payload.surface` is `cli`. It takes the payload so Task
  10 can admit the TUI by its `platform` without another signature change. Task 4 calls it.

- [ ] **Step 1: Write the failing unit tests**

In `pns/crates/pns-adapters/src/harness/tests/routing.rs`, replace
`only_the_harnesses_pns_registers_for_are_forwarded_to_moshi` with:

```rust
/// A Hermes approval from `surface` on `platform`, the rest absent.
fn approval(surface: &str, platform: &str) -> HookPayload {
    parse_payload(&serde_json::json!({"surface": surface, "platform": platform}).to_string())
}

#[test]
fn only_the_harnesses_pns_registers_for_are_forwarded_to_moshi() {
    let any = parse_payload("{}");
    assert_eq!(moshi_subcommand("claude", &any).as_deref(), Some("claude-hook"));
    assert_eq!(moshi_subcommand("codex", &any).as_deref(), Some("codex-hook"));
    assert_eq!(moshi_subcommand("pi", &approval("cli", "cli")), None);
    assert_eq!(moshi_subcommand("", &any), None);
    assert_eq!(moshi_subcommand("claude; rm -rf /", &any), None);
}

#[test]
fn a_hermes_approval_goes_to_moshi_only_from_the_cli_prompt() {
    assert_eq!(moshi_subcommand("hermes", &approval("cli", "cli")).as_deref(), Some("hermes-hook"));
    for (surface, platform) in [("", "cli"), ("gateway", "discord"), ("gateway", "tui"), ("mcp-elicitation", "cli")] {
        assert_eq!(moshi_subcommand("hermes", &approval(surface, platform)), None, "{surface}/{platform}");
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
fn a_tui_approval_is_carded_by_pns_rather_than_moshi() {
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
pub fn moshi_subcommand(agent: &str, payload: &HookPayload) -> Option<String> {
    match agent {
        "claude" | "codex" => Some(format!("{agent}-hook")),
        "hermes" if payload.surface == "cli" => Some("hermes-hook".to_string()),
        _ => None,
    }
}
```

Add `use super::payload::HookPayload;` at the top of `routing.rs` if the file does not already reach
it. In `pns/crates/pns/src/moshi_submission.rs` `blocking_event`, change `moshi_subcommand(agent)` to
`moshi_subcommand(agent, payload)`.

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

### Task 4: A Hermes CLI answer clears the card moshi raised for it

Branch: `feat/pns-hermes-answer-forward`. Needs Task 3 merged.

**Files:**
- Modify: `pns/crates/pns-adapters/src/harness/payload.rs` (`action_id` field)
- Test: `pns/crates/pns-adapters/src/harness/tests/hermes.rs`
- Modify: `pns/crates/pns/src/moshi_submission.rs` (the forwarded-request marker, `forward_resolution`)
- Modify: `pns/crates/pns/src/hook_dispatch.rs` (the `resolved` arm and its comment)
- Test: `pns/crates/pns/tests/hooks/hermes_sessions.rs`
- Modify: `pns/docs/specs/hook-compatibility.md` (behavior 19),
  `pns/docs/specs/persistence-and-process-lifecycle.md` (Table 1)

**Interfaces:**
- Consumes: `moshi_subcommand(agent, payload)` and `CLI_APPROVAL` from Task 3; the `hermes` test helper
  from Task 1.
- Produces: `HookPayload::action_id: String`;
  `pub(crate) fn forward_resolution(agent: &str, payload: &HookPayload, payload_json: &str)`; the marker
  `<state dir>/moshi-forwards/<action_id>`, written when `blocking_event`'s spawn of moshi-hook started
  and removed by the answer it admits.

- [ ] **Step 1: Write the failing unit test**

Append to `pns/crates/pns-adapters/src/harness/tests/hermes.rs`:

```rust
#[test]
fn a_payload_yields_its_action_id() {
    assert_eq!(parse_payload(r#"{"action_id":"a1"}"#).action_id, "a1");
    assert_eq!(parse_payload("{}").action_id, "");
}
```

- [ ] **Step 2: Write the failing end-to-end tests**

Append to `pns/crates/pns/tests/hooks/hermes_sessions.rs`:

```rust
const CLI_ANSWER: &str = r#"{"hook_event_name":"PermissionResolved","session_id":"20260922_120000_abc123","cwd":"/tmp","action_id":"a1","command":"rm -rf /tmp/drill","description":"recursive delete","choice":"once","surface":"cli","platform":"cli"}"#;

/// `CLI_APPROVAL` raised while the operator is away, so pns hands it to moshi.
fn forwarded_request(sandbox: &Sandbox) {
    let mut command = hermes(sandbox);
    sandbox.stub_moshi(&mut command, 0);
    hook_with(command, sandbox, "blocked", CLI_APPROVAL);
}

/// An answer given at the desk.
fn desk_answer(sandbox: &Sandbox, payload: &str) -> std::process::Output {
    let mut command = hermes(sandbox);
    command.env("PNS_SCREEN_IDLE", "0");
    sandbox.stub_moshi(&mut command, 0);
    hook_with(command, sandbox, "resolved", payload)
}

#[test]
fn the_answer_to_a_request_moshi_carded_is_handed_to_moshi_even_at_the_desk() {
    let sandbox = Sandbox::new("hermes-cli-answer");
    forwarded_request(&sandbox);
    let output = desk_answer(&sandbox, CLI_ANSWER);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        submissions(&sandbox),
        vec!["hermes-hook".to_string(), "hermes-hook".to_string()]
    );
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the answer"),
        CLI_ANSWER
    );
}

#[test]
fn a_desk_answer_to_a_request_moshi_never_saw_is_kept_from_moshi() {
    let sandbox = Sandbox::new("hermes-cli-answer-uncarded");
    let mut request = hermes(&sandbox);
    request.env("PNS_SCREEN_IDLE", "0");
    sandbox.stub_moshi(&mut request, 0);
    hook_with(request, &sandbox, "blocked", CLI_APPROVAL);
    desk_answer(&sandbox, CLI_ANSWER);
    assert_eq!(submissions(&sandbox), Vec::<String>::new());
}

#[test]
fn one_forwarded_request_hands_moshi_one_answer() {
    let sandbox = Sandbox::new("hermes-cli-answer-once");
    forwarded_request(&sandbox);
    desk_answer(&sandbox, CLI_ANSWER);
    desk_answer(&sandbox, CLI_ANSWER);
    assert_eq!(submissions(&sandbox).len(), 2, "the request and one answer");
}

#[test]
fn only_a_hermes_cli_answer_is_handed_to_moshi() {
    let tui = CLI_ANSWER.replace(
        r#""surface":"cli","platform":"cli""#,
        r#""surface":"gateway","platform":"tui""#,
    );
    for (producer, payload) in [("hermes", tui.as_str()), ("claude", CLI_ANSWER)] {
        let sandbox = Sandbox::new(&format!("hermes-answer-kept-{producer}"));
        forwarded_request(&sandbox);
        let mut command = with_state_dir(&sandbox);
        command.env("PNS_PRODUCER", producer);
        sandbox.stub_moshi(&mut command, 0);
        hook_with(command, &sandbox, "resolved", payload);
        assert_eq!(
            submissions(&sandbox),
            vec!["hermes-hook".to_string()],
            "{producer}: {payload}"
        );
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters harness::tests::hermes`
Expected: compile error, `no field action_id`.

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hooks hermes_sessions`
Expected: `the_answer_to_a_request_moshi_carded...` FAILS with `left: ["hermes-hook"]` and
`one_forwarded_request_hands_moshi_one_answer` FAILS with `left: 1`; the other two pass already.

- [ ] **Step 4: Add the payload field**

In `pns/crates/pns-adapters/src/harness/payload.rs`, append at the end of `HookPayload`:

```rust
    /// The id the pns-hooks plugin gives one Hermes approval, carried by its
    /// request and by its answer.
    pub action_id: String,
```

and at the end of the struct literal in `parse_payload`:

```rust
        action_id: text("action_id"),
```

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters harness::tests::hermes`
Expected: all pass.

- [ ] **Step 5: Mark a forwarded request and forward only its answer**

In `pns/crates/pns/src/moshi_submission.rs`, replace `MoshiRaiseNotification`'s `forward` with:

```rust
    fn forward(&self, subcommand: &str, payload_json: &str) -> Option<Self::Forwarded> {
        let child = pns_application::ApprovalForwarder::forward(
            &MoshiApprovalForwarder,
            subcommand,
            payload_json,
        )?;
        // THE SPAWN STARTED, so moshi is carding this request and is owed its
        // answer.
        mark_forwarded(&self.payload.action_id);
        Some(child)
    }
```

and append at the end of the file:

```rust
/// Where a request handed to moshi leaves its `action_id`, or `None` for an
/// id that cannot be a filename.
fn forwarded_marker(action_id: &str) -> Option<std::path::PathBuf> {
    pns_domain::safety::session_id_is_safe(action_id)
        .then(|| pns_adapters::state_dir().join("moshi-forwards").join(action_id))
}

/// Records that the request carrying `action_id` was handed to moshi.
fn mark_forwarded(action_id: &str) {
    if let Some(marker) = forwarded_marker(action_id) {
        // A marker that cannot be written costs the card its clearing and
        // never the request.
        let _ = pns_adapters::publish_state_line(&marker, "");
    }
}

/// A Hermes CLI approval's answer, handed to moshi so the card it raised for
/// that approval clears.
///
/// NOT PRESENCE-GATED: an operator who left the desk after the card went out
/// can come back and answer at the pane, and that card still has to clear.
/// ONLY FOR A REQUEST THIS PNS HANDED MOSHI, which removing its marker proves,
/// so every `action_id` moshi is handed is one it carded. Bounded by the same
/// acknowledgement deadline as every forward.
pub(crate) fn forward_resolution(agent: &str, payload: &HookPayload, payload_json: &str) {
    if agent != "hermes"
        || payload.hook_event_name != "PermissionResolved"
        || !payload_is_whole(payload_json)
    {
        return;
    }
    let Some(subcommand) = moshi_subcommand(agent, payload) else {
        return;
    };
    let Some(marker) = forwarded_marker(&payload.action_id) else {
        return;
    };
    if std::fs::remove_file(marker).is_err() {
        return;
    }
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
        // per-batch path to a payload read, a parse and two file operations. The
        // answer to a Hermes CLI request pns handed moshi goes to moshi too, whose
        // bounded wait reads its deadline from the config: see
        // `forward_resolution`.
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hooks hermes_sessions`
Expected: all pass.

- [ ] **Step 7: State the behavior in the pns specs**

In `pns/docs/specs/hook-compatibility.md` behavior 19 ("`resolved` clears and delivers nothing"),
change the heading to "`resolved` clears and delivers nothing, and hands moshi the answer to a Hermes
request it carded" and add a bullet: "Required side effects: a payload from producer `hermes` with
`hook_event_name` `PermissionResolved` and `surface` `cli` whose `action_id` names a request pns handed
moshi (the `moshi-forwards/<action_id>` marker `blocking_event` wrote when its spawn started) is handed
byte for byte to `moshi-hook hermes-hook`, at the desk as well, and the marker is removed
(`tests/hooks/hermes_sessions.rs:the_answer_to_a_request_moshi_carded_is_handed_to_moshi_even_at_the_desk`,
`one_forwarded_request_hands_moshi_one_answer`); without the marker nothing is handed on
(`a_desk_answer_to_a_request_moshi_never_saw_is_kept_from_moshi`)."

In `pns/docs/specs/persistence-and-process-lifecycle.md` Table 1, add after the `remind/fire.lock` row:

```markdown
| `moshi-forwards/<action_id>` | file | 0600 | the `blocked` hook, once its spawn of `moshi-hook hermes-hook` started (`src/moshi_submission.rs:mark_forwarded` via `publish_state_line`) | the `resolved` hook (`src/moshi_submission.rs:forward_resolution`) | consumed by `remove_file`; only the answer whose removal succeeds is handed on | removed by the answer; one stays per request whose answer never arrived | internal persistence detail |
```

Then run `just m`, which realigns the table.

- [ ] **Step 8: Run the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 9: Commit**

```bash
git add pns/crates/pns-adapters/src/harness/payload.rs pns/crates/pns-adapters/src/harness/tests/hermes.rs \
  pns/crates/pns/src/moshi_submission.rs pns/crates/pns/src/hook_dispatch.rs \
  pns/crates/pns/tests/hooks/hermes_sessions.rs pns/docs/specs/hook-compatibility.md \
  pns/docs/specs/persistence-and-process-lifecycle.md
SKIP_AI_COMMIT=1 git commit -m "feat(pns): hand moshi the answer to a Hermes CLI request it carded"
```

---

### Task 5: The embedded shim reports each Hermes hook to pns and keeps moshi's screens fed

Branch: `feat/pns-hermes-plugin-shim`. Independent of every other task.

**Files:**
- Create: `pns/crates/pns-adapters/src/hermes_plugin.rs`
- Create: `pns/crates/pns-adapters/src/hermes_plugin/pns_hooks.py`
- Create: `pns/crates/pns-adapters/src/hermes_plugin/tests.rs` and `hermes_plugin/tests/moshi.rs`
- Create: `pns/crates/pns-adapters/src/hermes_plugin/fixtures/drive.py`
- Create: `pns/crates/pns-adapters/src/hermes_plugin/fixtures/record.py`
- Modify: `pns/crates/pns-adapters/src/lib.rs`

**Interfaces:**
- Produces: `pub const HERMES_PLUGIN_NAME: &str = "pns-hooks"`;
  `pub struct HermesPlugin { pub init_py: String, pub manifest: String }`;
  `pub fn render_hermes_plugin(binary: &str, moshi: &str) -> HermesPlugin`, where `binary` is the pns
  the shim calls and `moshi` the moshi-hook it feeds. All exported from `pns_adapters`. Task 7 consumes
  them.

- [ ] **Step 1: Write the test fixtures**

Create `pns/crates/pns-adapters/src/hermes_plugin/fixtures/record.py`:

```python
#!/usr/bin/env python3
"""Stands in for pns and for moshi-hook: appends each call's argv, producer and
payload to $PNS_HOOKS_RECORD.<first argument>, so `hook` calls and `hermes-hook`
calls land in separate files."""

import json
import os
import sys

call = {
    "argv": sys.argv[1:],
    "producer": os.environ.get("PNS_PRODUCER"),
    "payload": json.load(sys.stdin),
}
with open(f"{os.environ['PNS_HOOKS_RECORD']}.{sys.argv[1]}", "a", encoding="utf-8") as record:
    record.write(json.dumps(call) + "\n")
```

Create `pns/crates/pns-adapters/src/hermes_plugin/fixtures/drive.py`:

```python
"""Loads a rendered pns-hooks plugin, fires one scenario of Hermes hooks at it,
waits for every delivery, and prints what registered and what each call returned.

A step is `[hook, kwargs]`, or `[hook, kwargs, thread]` to fire it on a thread of
that name, the way Hermes fires a background review's hooks."""

import importlib.util
import json
import sys
import threading

spec = importlib.util.spec_from_file_location("pns_hooks", sys.argv[1])
plugin = importlib.util.module_from_spec(spec)
spec.loader.exec_module(plugin)
hooks = {}


class Context:
    def register_hook(self, name, callback):
        hooks[name] = callback


def fire(name, kwargs, thread=None):
    if thread is None:
        return hooks[name](**kwargs)
    returned = []
    worker = threading.Thread(target=lambda: returned.append(hooks[name](**kwargs)), name=thread)
    worker.start()
    worker.join()
    return returned[0]


plugin.register(Context())
returned = [fire(*step) for step in json.loads(sys.argv[2])]
plugin._delivery.shutdown(wait=True)
plugin._moshi_delivery.shutdown(wait=True)
print(json.dumps({"registered": sorted(hooks), "returned": returned}))
```

- [ ] **Step 2: Write the failing tests**

Create `pns/crates/pns-adapters/src/hermes_plugin/tests.rs`:

```rust
mod moshi;

use super::render_hermes_plugin;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;

const DRIVER: &str = include_str!("fixtures/drive.py");
const RECORDER: &str = include_str!("fixtures/record.py");
const SESSION: &str = "20260922_120000_abc123";
const CURATOR: &str = "20260922_120001_cur001";

/// What one scenario produced: the calls the stand-in pns and the stand-in
/// moshi-hook each received, the driver's report of what registered and what
/// each callback returned, and the directory the driver ran in.
struct Fired {
    calls: Vec<Value>,
    moshi: Vec<Value>,
    report: Value,
    cwd: String,
}

/// Fires `scenario`, a list of `[hook, kwargs]` or `[hook, kwargs, thread]`
/// steps, at the rendered plugin through `python3`, with one stand-in playing
/// both pns and moshi-hook.
fn fire(name: &str, scenario: Value) -> Fired {
    fire_in(name, scenario, &[])
}

/// `fire`, with `env` added to the driver's environment.
fn fire_in(name: &str, scenario: Value, env: &[(&str, &str)]) -> Fired {
    let root = crate::state_fixtures::scratch(name);
    let recorder = root.join("stand-in");
    std::fs::write(&recorder, RECORDER).expect("the stand-in");
    std::fs::set_permissions(&recorder, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let path = recorder.to_str().expect("a UTF-8 path");
    let plugin = render_hermes_plugin(path, path);
    std::fs::write(root.join("__init__.py"), plugin.init_py).expect("the plugin");
    std::fs::write(root.join("drive.py"), DRIVER).expect("the driver");
    let record = root.join("record.jsonl");
    let output = std::process::Command::new("python3")
        .arg(root.join("drive.py"))
        .arg(root.join("__init__.py"))
        .arg(scenario.to_string())
        .current_dir(&root)
        .env_remove("PNS_SUMMARIZING")
        .env_remove("HERMES_KANBAN_TASK")
        .envs(env.iter().copied())
        .env("PNS_HOOKS_RECORD", &record)
        .output()
        .expect("python3 on PATH");
    assert!(output.status.success(), "{output:?}");
    let recorded = |callee: &str| -> Vec<Value> {
        std::fs::read_to_string(format!("{}.{callee}", record.display()))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).expect("one JSON call per line"))
            .collect()
    };
    Fired {
        calls: recorded("hook"),
        moshi: recorded("hermes-hook"),
        report: serde_json::from_slice(&output.stdout).expect("the driver's report"),
        cwd: std::fs::canonicalize(&root).expect("the scratch dir").display().to_string(),
    }
}

/// Each pns call's words after the binary.
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
fn both_binary_paths_are_python_string_literals_and_the_manifest_names_the_plugin() {
    let plugin = render_hermes_plugin(r#"/opt/odd "dir"\pns"#, "/opt/homebrew/bin/moshi-hook");
    assert!(plugin.init_py.contains(r#"PNS = "/opt/odd \"dir\"\\pns""#), "{}", plugin.init_py);
    assert!(plugin.init_py.contains(r#"MOSHI = "/opt/homebrew/bin/moshi-hook""#), "{}", plugin.init_py);
    assert!(!plugin.init_py.contains("__PNS_BINARY__") && !plugin.init_py.contains("__MOSHI_BINARY__"));
    assert_eq!(
        plugin.manifest,
        "name: pns-hooks\nversion: \"1\"\ndescription: Hands Hermes Agent session events to pns and moshi\n"
    );
}

#[test]
fn the_plugin_registers_the_nine_hooks_pns_and_moshi_are_fed_from() {
    let fired = fire("hermes-plugin-registered", json!([]));
    assert_eq!(
        fired.report["registered"],
        json!([
            "on_session_end", "on_session_finalize", "on_session_start", "post_approval_response",
            "post_llm_call", "post_tool_call", "pre_approval_request", "pre_llm_call", "pre_tool_call"
        ])
    );
}

#[test]
fn a_hermes_run_started_by_the_pns_summarizer_registers_no_hook() {
    let fired = fire_in("hermes-plugin-summarizing", json!([]), &[("PNS_SUMMARIZING", "1")]);
    assert_eq!(fired.report["registered"], json!([]));
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
fn the_background_review_replaying_a_turn_never_reaches_pns() {
    let fired = fire(
        "hermes-plugin-background-review",
        json!([
            ["pre_llm_call", {"session_id": SESSION, "user_message": "tidy the notes", "platform": "cli"}],
            ["post_llm_call", {"session_id": SESSION, "assistant_response": "notes tidied", "platform": "cli"}],
            ["pre_llm_call", {"session_id": SESSION, "user_message": "Review the conversation above", "platform": "cli"}, "bg-review"],
            ["pre_tool_call", {"tool_name": "clarify", "session_id": SESSION}, "bg-review"],
            ["post_llm_call", {"session_id": SESSION, "assistant_response": "memory updated", "platform": "cli"}, "bg-review"],
            ["on_session_end", {"session_id": SESSION, "completed": true, "interrupted": false, "platform": "cli"}, "bg-review"],
            ["on_session_end", {"session_id": SESSION, "completed": true, "interrupted": false, "platform": "cli"}]
        ]),
    );
    assert_eq!(argv(&fired), vec![json!(["hook", "prompt"]), json!(["hook", "stop"])]);
    assert_eq!(fired.calls[1]["payload"]["last_assistant_message"], "notes tidied");
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
fn an_approval_carries_the_platform_its_session_named() {
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
fn a_curator_run_in_the_same_process_leaves_a_cli_approval_stamped_cli() {
    let fired = fire(
        "hermes-plugin-curator",
        json!([
            ["pre_llm_call", {"session_id": SESSION, "user_message": "clean up", "platform": "cli"}],
            ["pre_llm_call", {"session_id": CURATOR, "user_message": "curate the skills", "platform": "curator"}],
            ["on_session_end", {"session_id": CURATOR, "completed": true, "interrupted": false, "platform": "curator"}],
            ["pre_approval_request", {"command": "rm -rf /tmp/drill", "session_key": SESSION, "surface": "cli"}]
        ]),
    );
    assert_eq!(fired.calls[1]["payload"]["platform"], "curator");
    assert_eq!(fired.calls[3]["payload"]["platform"], "cli");
}

#[test]
fn a_session_no_hook_named_takes_the_terminal_front_end_its_process_named() {
    let fired = fire(
        "hermes-plugin-stamp-terminal-miss",
        json!([
            ["pre_llm_call", {"session_id": SESSION, "user_message": "clean up", "platform": "cli"}],
            ["pre_llm_call", {"session_id": CURATOR, "user_message": "curate the skills", "platform": "curator"}],
            ["pre_approval_request", {"command": "rm -rf /tmp/drill", "session_key": "default", "surface": "cli"}]
        ]),
    );
    assert_eq!(fired.calls[2]["payload"]["platform"], "cli");
}

#[test]
fn a_gateway_approval_carries_no_platform_so_its_surface_decides() {
    let fired = fire(
        "hermes-plugin-stamp-gateway-miss",
        json!([
            ["pre_llm_call", {"session_id": SESSION, "user_message": "summarize the thread", "platform": "discord"}],
            ["pre_approval_request", {"command": "rm -rf /tmp/drill", "session_key": "agent:main:discord:dm:1", "surface": "gateway"}]
        ]),
    );
    assert_eq!(fired.calls[1]["payload"]["platform"], "");
    assert_eq!(fired.calls[1]["payload"]["surface"], "gateway");
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
fn a_kanban_worker_stamps_its_task_and_its_block_asks_the_operator() {
    let fired = fire_in(
        "hermes-plugin-kanban",
        json!([
            ["pre_llm_call", {"session_id": SESSION, "user_message": "work kanban task t_42", "platform": "cli"}],
            ["pre_tool_call", {"tool_name": "kanban_block", "args": {"reason": "Which region?"}, "tool_call_id": "k1", "session_id": SESSION}],
            ["post_tool_call", {"tool_name": "kanban_block", "tool_call_id": "k1", "session_id": SESSION}]
        ]),
        &[("HERMES_KANBAN_TASK", "t_42")],
    );
    assert_eq!(argv(&fired), vec![json!(["hook", "prompt"]), json!(["hook", "asked"])]);
    assert!(fired.calls.iter().all(|call| call["payload"]["kanban_task"] == "t_42"));
    assert_eq!(fired.calls[1]["payload"]["tool_input"], json!({"reason": "Which region?"}));
    // moshi is fed its own plugin's payload, which never carried the task.
    assert_eq!(fired.moshi.len(), 3);
    assert!(fired.moshi.iter().all(|call| call["payload"].get("kanban_task").is_none()));
}

#[test]
fn a_kanban_block_outside_a_worker_stays_in_process() {
    let fired = fire(
        "hermes-plugin-kanban-outside",
        json!([["pre_tool_call", {"tool_name": "kanban_block", "args": {"reason": "later"}, "session_id": SESSION}]]),
    );
    assert_eq!(argv(&fired), Vec::<Value>::new());
}

#[test]
fn no_callback_returns_anything_hermes_would_act_on() {
    let fired = fire(
        "hermes-plugin-returns",
        json!([
            ["on_session_start", {"session_id": SESSION, "platform": "cli"}],
            ["pre_llm_call", {"session_id": SESSION, "platform": "cli"}],
            ["pre_tool_call", {"tool_name": "clarify", "session_id": SESSION}],
            ["post_tool_call", {"tool_name": "clarify", "session_id": SESSION}],
            ["post_llm_call", {"session_id": SESSION, "assistant_response": "done"}],
            ["pre_approval_request", {"session_key": SESSION, "surface": "cli"}],
            ["post_approval_response", {"session_key": SESSION, "surface": "cli", "choice": "deny"}],
            ["on_session_end", {"session_id": SESSION, "completed": true}],
            ["on_session_finalize", {"session_id": SESSION, "platform": "cli"}],
            ["pre_llm_call", {"session_id": SESSION, "platform": "cli"}, "bg-review"]
        ]),
    );
    assert_eq!(fired.report["returned"], json!([null, null, null, null, null, null, null, null, null, null]));
}
```

Create `pns/crates/pns-adapters/src/hermes_plugin/tests/moshi.rs`:

```rust
//! What moshi-hook receives from the plugin: every event moshi's own Hermes
//! plugin sent it outside the approval pair, with the payload that plugin
//! built. The approval pair reaches moshi only through pns.

use super::*;

/// One moshi payload, the way moshi's own plugin built it.
fn sent(fired: &Fired, event: &str, extra: Value) -> Value {
    let mut payload = json!({"hook_event_name": event, "session_id": SESSION, "cwd": fired.cwd});
    for (key, value) in extra.as_object().expect("an object") {
        payload[key] = value.clone();
    }
    payload
}

/// Each moshi call's payload.
fn payloads(fired: &Fired) -> Vec<Value> {
    fired.moshi.iter().map(|call| call["payload"].clone()).collect()
}

#[test]
fn every_non_approval_event_reaches_moshi_with_the_payload_its_own_plugin_sent() {
    let fired = fire(
        "hermes-plugin-moshi-feed",
        json!([
            ["on_session_start", {"session_id": SESSION, "model": "m1", "platform": "cli"}],
            ["pre_llm_call", {"session_id": SESSION, "user_message": "list the files", "model": "m1", "platform": "cli"}],
            ["pre_tool_call", {"tool_name": "terminal", "args": {"command": "ls"}, "tool_call_id": "t1", "session_id": SESSION}],
            ["post_tool_call", {"tool_name": "terminal", "args": {"command": "ls"}, "result": "a b", "tool_call_id": "t1", "session_id": SESSION}],
            ["post_llm_call", {"session_id": SESSION, "assistant_response": "two files", "model": "m1", "platform": "cli"}],
            ["on_session_end", {"session_id": SESSION, "completed": true, "interrupted": false, "model": "m1", "platform": "cli"}],
            ["on_session_finalize", {"session_id": null, "platform": "cli"}],
            ["on_session_finalize", {"session_id": SESSION, "platform": "cli"}]
        ]),
    );
    assert!(fired.moshi.iter().all(|call| call["argv"] == json!(["hermes-hook"])));
    assert!(fired.moshi.iter().all(|call| call["producer"].is_null()));
    assert_eq!(
        payloads(&fired),
        vec![
            sent(&fired, "SessionStart", json!({"model": "m1", "platform": "cli"})),
            sent(&fired, "UserPromptSubmit", json!({"prompt": "list the files", "model": "m1", "platform": "cli"})),
            sent(&fired, "PreToolUse", json!({"tool_name": "terminal", "tool_call_id": "t1"})),
            sent(&fired, "PostToolUse", json!({"tool_name": "terminal", "tool_call_id": "t1"})),
            sent(&fired, "AgentEnd", json!({"last_assistant_message": "two files", "model": "m1", "platform": "cli"})),
            sent(&fired, "SessionEnd", json!({"platform": "cli"})),
        ]
    );
}

#[test]
fn a_turn_that_did_not_complete_reaches_moshi_as_turn_interrupted() {
    let fired = fire(
        "hermes-plugin-moshi-interrupted",
        json!([
            ["on_session_end", {"session_id": SESSION, "completed": false, "interrupted": false, "model": "m1", "platform": "cli"}],
            ["on_session_end", {"session_id": SESSION, "completed": false, "interrupted": true, "model": "m1", "platform": "cli"}]
        ]),
    );
    assert_eq!(
        payloads(&fired),
        vec![
            sent(&fired, "TurnInterrupted", json!({"interrupted": false, "model": "m1", "platform": "cli"})),
            sent(&fired, "TurnInterrupted", json!({"interrupted": true, "model": "m1", "platform": "cli"})),
        ]
    );
}

#[test]
fn a_clarify_call_reaches_moshi_with_its_question() {
    let fired = fire(
        "hermes-plugin-moshi-clarify",
        json!([["pre_tool_call", {"tool_name": "clarify", "args": {"question": "Red or blue?"}, "tool_call_id": "c1", "session_id": SESSION}]]),
    );
    assert_eq!(
        payloads(&fired),
        vec![sent(
            &fired,
            "PreToolUse",
            json!({"tool_name": "clarify", "tool_call_id": "c1", "tool_input": {"question": "Red or blue?"}})
        )]
    );
}

#[test]
fn the_approval_pair_reaches_moshi_only_through_pns() {
    let approval = json!({"command": "rm -rf /tmp/drill", "session_key": SESSION, "surface": "cli"});
    let mut answer = approval.clone();
    answer["choice"] = json!("once");
    let fired = fire(
        "hermes-plugin-moshi-approval",
        json!([["pre_approval_request", approval], ["post_approval_response", answer]]),
    );
    assert_eq!(fired.moshi, Vec::<Value>::new());
    assert_eq!(fired.calls.len(), 2, "both went to pns, which gates the request");
}

#[test]
fn the_background_review_and_a_gateway_session_feed_moshi_as_any_session_does() {
    let fired = fire(
        "hermes-plugin-moshi-unconditional",
        json!([
            ["pre_llm_call", {"session_id": SESSION, "user_message": "Review the conversation above", "platform": "cli"}, "bg-review"],
            ["pre_llm_call", {"session_id": SESSION, "user_message": "summarize the thread", "platform": "discord"}]
        ]),
    );
    assert_eq!(
        payloads(&fired),
        vec![
            sent(&fired, "UserPromptSubmit", json!({"prompt": "Review the conversation above", "model": "", "platform": "cli"})),
            sent(&fired, "UserPromptSubmit", json!({"prompt": "summarize the thread", "model": "", "platform": "discord"})),
        ]
    );
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
"""pns-hooks: Hermes Agent's session events, handed to pns and to moshi.

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
MOSHI = __MOSHI_BINARY__
# pns bounds its own waits; this only stops one wedged call from holding the queue.
CALL_DEADLINE_SECS = 120
# The bound moshi's own Hermes plugin gave each of its calls.
MOSHI_DEADLINE_SECS = 10
REMIND = "--remind=5m"
# Hermes's memory and skill review replays a finished turn under the operator's
# own session id and platform, on a thread of this name (run_agent.py,
# `_spawn_background_review`).
BACKGROUND_REVIEW_THREAD = "bg-review"
TERMINAL_PLATFORMS = ("cli", "tui")

# One worker per destination keeps Hermes's order for each: a prompt before its
# approval, an approval before its answer, and a turn's end after both.
_delivery = ThreadPoolExecutor(max_workers=1, thread_name_prefix="pns-hooks")
_moshi_delivery = ThreadPoolExecutor(max_workers=1, thread_name_prefix="pns-hooks-moshi")
_pending = defaultdict(deque)
_pending_lock = threading.Lock()
_replies = {}
_platforms = {}
_terminal = ""


def _submit(pool, call, *args):
    try:
        pool.submit(call, *args)
    except RuntimeError:
        # Interpreter shutdown has closed the pool, so the last event goes inline.
        call(*args)


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
    kanban_task = os.environ.get("HERMES_KANBAN_TASK", "")
    if kanban_task:
        payload["kanban_task"] = kanban_task
    _submit(_delivery, _deliver, verb, list(flags), payload)


def _deliver_to_moshi(payload):
    try:
        subprocess.run(
            [MOSHI, "hermes-hook"],
            input=json.dumps(payload, ensure_ascii=False),
            text=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=MOSHI_DEADLINE_SECS,
            check=False,
        )
    except Exception:
        pass


def _send_to_moshi(event_name, session_id, **extra):
    payload = {"hook_event_name": event_name, "session_id": session_id, "cwd": os.getcwd(), **extra}
    _submit(_moshi_delivery, _deliver_to_moshi, payload)


def _session(primary="", kwargs=None):
    kwargs = kwargs or {}
    return str(kwargs.get("session_id") or primary or kwargs.get("task_id") or "hermes")


def _platform(session, stated=""):
    """The event's own platform, else the one its session named, else the
    terminal front end this process last named."""
    global _terminal
    if stated:
        _platforms[session] = stated
        if stated in TERMINAL_PLATFORMS:
            _terminal = stated
        return stated
    return _platforms.get(session) or _terminal


def _approval_key(session_key, command, pattern_key):
    return (str(session_key or ""), str(command or ""), str(pattern_key or ""))


def _is_clarify(tool_name):
    return str(tool_name).strip().lower() == "clarify"


def _is_kanban_block(tool_name):
    # A kanban worker blocks its task on a question for the operator.
    return str(tool_name).strip().lower() == "kanban_block" and bool(os.environ.get("HERMES_KANBAN_TASK"))


def on_pre_llm_call(session_id="", user_message="", model="", platform="", **kwargs):
    session = _session(session_id, kwargs)
    _send(
        "prompt", "UserPromptSubmit", session,
        prompt=user_message, model=model, platform=_platform(session, platform),
    )


def on_post_llm_call(session_id="", assistant_response="", **kwargs):
    _replies[_session(session_id, kwargs)] = assistant_response


def on_session_end(session_id="", completed=False, interrupted=False, model="", platform="", **kwargs):
    session = _session(session_id, kwargs)
    reply = _replies.pop(session, "")
    stated = _platform(session, platform)
    if interrupted:
        _send("resolved", "TurnInterrupted", session, interrupted=True, platform=stated)
    elif completed:
        _send("stop", "AgentEnd", session, last_assistant_message=reply, model=model, platform=stated)
    else:
        _send("stop-failure", "TurnFailed", session, error=reply, model=model, platform=stated)


def on_pre_tool_call(tool_name="", args=None, tool_call_id="", **kwargs):
    if _is_clarify(tool_name) or _is_kanban_block(tool_name):
        session = _session(kwargs=kwargs)
        _send(
            "asked", "PreToolUse", session,
            tool_name=tool_name, tool_call_id=tool_call_id, tool_input=args or {},
            platform=_platform(session),
        )


def on_post_tool_call(tool_name="", tool_call_id="", **kwargs):
    if _is_clarify(tool_name):
        session = _session(kwargs=kwargs)
        _send(
            "resolved", "PostToolUse", session,
            tool_name=tool_name, tool_call_id=tool_call_id, platform=_platform(session),
        )


def on_pre_approval_request(
    command="", description="", pattern_key="", pattern_keys=None, session_key="", surface="", **kwargs
):
    action_id = uuid.uuid4().hex
    with _pending_lock:
        _pending[_approval_key(session_key, command, pattern_key)].append(action_id)
    asks = surface == "mcp-elicitation"
    session = _session(session_key, kwargs)
    _send(
        "asked" if asks else "blocked",
        "Elicitation" if asks else "PermissionRequest",
        session,
        () if asks else (REMIND,),
        action_id=action_id,
        command=command,
        description=description,
        pattern_key=pattern_key,
        pattern_keys=pattern_keys or [],
        surface=surface,
        platform=_platform(session),
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
    session = _session(session_key, kwargs)
    _send(
        "resolved", "PermissionResolved", session,
        action_id=action_id,
        command=command,
        description=description,
        choice=choice,
        surface=surface,
        platform=_platform(session),
    )


# moshi's screens, fed with the payloads moshi's own Hermes plugin built.


def moshi_session_start(session_id="", model="", platform="", **kwargs):
    _send_to_moshi("SessionStart", _session(session_id, kwargs), model=model, platform=platform)


def moshi_prompt(session_id="", user_message="", model="", platform="", **kwargs):
    _send_to_moshi("UserPromptSubmit", _session(session_id, kwargs), prompt=user_message, model=model, platform=platform)


def moshi_reply(session_id="", assistant_response="", model="", platform="", **kwargs):
    _send_to_moshi(
        "AgentEnd", _session(session_id, kwargs),
        last_assistant_message=assistant_response, model=model, platform=platform,
    )


def moshi_turn_end(session_id="", completed=False, interrupted=False, model="", platform="", **kwargs):
    if interrupted or not completed:
        _send_to_moshi(
            "TurnInterrupted", _session(session_id, kwargs),
            interrupted=bool(interrupted), model=model, platform=platform,
        )


def moshi_session_finalize(session_id=None, platform="", **kwargs):
    if session_id:
        _send_to_moshi("SessionEnd", _session(session_id, kwargs), platform=platform)


def moshi_tool_start(tool_name="", args=None, tool_call_id="", **kwargs):
    extra = {"tool_name": tool_name, "tool_call_id": tool_call_id}
    if _is_clarify(tool_name):
        extra["tool_input"] = args or {}
    _send_to_moshi("PreToolUse", _session(kwargs=kwargs), **extra)


def moshi_tool_end(tool_name="", tool_call_id="", **kwargs):
    _send_to_moshi("PostToolUse", _session(kwargs=kwargs), tool_name=tool_name, tool_call_id=tool_call_id)


# Each Hermes hook, the pns report it makes and the moshi event it feeds. The
# approval pair reaches moshi through pns, which gates the request.
HOOKS = (
    ("on_session_start", None, moshi_session_start),
    ("pre_llm_call", on_pre_llm_call, moshi_prompt),
    ("post_llm_call", on_post_llm_call, moshi_reply),
    ("on_session_end", on_session_end, moshi_turn_end),
    ("on_session_finalize", None, moshi_session_finalize),
    ("pre_tool_call", on_pre_tool_call, moshi_tool_start),
    ("post_tool_call", on_post_tool_call, moshi_tool_end),
    ("pre_approval_request", on_pre_approval_request, None),
    ("post_approval_response", on_post_approval_response, None),
)


def _observer(report, feed):
    # Each destination fails on its own, so a lost moshi event never costs pns's.
    def observe(**kwargs):
        if feed is not None:
            try:
                feed(**kwargs)
            except Exception:
                pass
        if report is not None and threading.current_thread().name != BACKGROUND_REVIEW_THREAD:
            try:
                report(**kwargs)
            except Exception:
                pass

    return observe


def register(ctx):
    # pns's own summarizer sets this on the Hermes run it starts, which is never a session.
    if os.environ.get("PNS_SUMMARIZING"):
        return
    for name, report, feed in HOOKS:
        ctx.register_hook(name, _observer(report, feed))
```

- [ ] **Step 5: Write the renderer**

Replace the contents of `pns/crates/pns-adapters/src/hermes_plugin.rs` with:

```rust
//! pns's Hermes Agent plugin: the Python shim Hermes loads from
//! `$HERMES_HOME/plugins/pns-hooks/`, rendered for one pns binary and one
//! moshi-hook.
//!
//! THE SHIM CALLS THE BINARY THAT RENDERED IT, by absolute path, the way the
//! Codex hooks do, so a plugin never reaches whatever later answers to `pns`.

#[cfg(test)]
mod tests;

/// The name Hermes registers the plugin under, which is also its directory.
pub const HERMES_PLUGIN_NAME: &str = "pns-hooks";

/// The shim's source, with `__PNS_BINARY__` and `__MOSHI_BINARY__` where the
/// two paths go.
const SHIM: &str = include_str!("hermes_plugin/pns_hooks.py");

/// `plugin.yaml`. Hermes shows the version and reads nothing from it, so it
/// stays fixed and a release that leaves the shim alone rewrites nothing.
const MANIFEST: &str =
    "name: pns-hooks\nversion: \"1\"\ndescription: Hands Hermes Agent session events to pns and moshi\n";

/// The plugin's two files, as Hermes reads them.
#[derive(Debug, PartialEq, Eq)]
pub struct HermesPlugin {
    /// `__init__.py`, the shim.
    pub init_py: String,
    /// `plugin.yaml`, the manifest.
    pub manifest: String,
}

/// Both files, for the pns at `binary` and the moshi-hook at `moshi`.
///
/// Each path goes in as a JSON string literal, which Python reads as the same
/// string whatever quotes or backslashes the path holds.
pub fn render_hermes_plugin(binary: &str, moshi: &str) -> HermesPlugin {
    let literal = |path: &str| serde_json::Value::String(path.to_string()).to_string();
    HermesPlugin {
        init_py: SHIM
            .replace("__PNS_BINARY__", &literal(binary))
            .replace("__MOSHI_BINARY__", &literal(moshi)),
        manifest: MANIFEST.to_string(),
    }
}
```

In `pns/crates/pns-adapters/src/lib.rs`, beside the `pub use codex_hooks::{...}` block add:

```rust
pub use hermes_plugin::{HERMES_PLUGIN_NAME, HermesPlugin, render_hermes_plugin};
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters hermes_plugin`
Expected: 23 passed, each well under a second.

Then load the shim under the interpreter Hermes itself runs (Python 3.11, not whatever `python3` is on
`PATH`), with the driver and an empty scenario:

```bash
scratch="$(mktemp -d)"
sed -e 's|__PNS_BINARY__|"/usr/bin/true"|' -e 's|__MOSHI_BINARY__|"/usr/bin/true"|' \
  pns/crates/pns-adapters/src/hermes_plugin/pns_hooks.py > "$scratch/__init__.py"
~/.hermes/hermes-agent/venv/bin/python pns/crates/pns-adapters/src/hermes_plugin/fixtures/drive.py \
  "$scratch/__init__.py" '[]'
trash "$scratch"
```

Expected: one JSON line listing the nine hooks under `registered`.

- [ ] **Step 7: Run the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 8: Commit**

```bash
git add pns/crates/pns-adapters/src/hermes_plugin.rs pns/crates/pns-adapters/src/hermes_plugin \
  pns/crates/pns-adapters/src/lib.rs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): embed a Hermes plugin that reports to pns and feeds moshi"
```

---

### Task 6: Every summarizer run pns starts is marked, so no hooked harness reports it

Branch: `feat/pns-summarizer-run-marked`. Independent of every other task. If the pull request isolating
the Codex and Claude summarizer runs has already put a marker on `run_summarizer`'s child, check that it
is `PNS_SUMMARIZING=1` (the variable the shim reads), keep its test, and skip Steps 1 to 4.

**Files:**
- Modify: `pns/crates/pns-adapters/src/recap/summarizer.rs` (`run_summarizer`)
- Test: `pns/crates/pns-adapters/src/recap/summarizer/tests.rs`
- Modify: `pns/docs/specs/return-recap.md` (behavior 8)

**Interfaces:**
- Produces: every child `run_summarizer` spawns (the recap's and `pns doctor`'s, every backend) has
  `PNS_SUMMARIZING=1` in its environment. Task 5's shim registers nothing under it.

- [ ] **Step 1: Write the failing test**

Append to `pns/crates/pns-adapters/src/recap/summarizer/tests.rs`:

```rust
/// A SUMMARIZER RUN IS MARKED, so a harness pns hooks into reports it as no
/// session: Hermes's pns-hooks plugin registers nothing under the marker.
#[test]
fn every_summarizer_run_is_marked_as_pns_summarizing() {
    let directory = temporary("marked");
    let invocation = scripted(&directory, "marked", "cat >/dev/null; printf '%s' \"$PNS_SUMMARIZING\"");
    assert_eq!(
        run_summarizer(&invocation, Duration::from_secs(5), "what moved").unwrap(),
        "1"
    );
    std::fs::remove_dir_all(&directory).ok();
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters recap::summarizer`
Expected: `every_summarizer_run_is_marked_as_pns_summarizing` FAILS, its `unwrap` meeting `Silent`
because the variable it printed is empty.

- [ ] **Step 3: Implement**

In `pns/crates/pns-adapters/src/recap/summarizer.rs` `run_summarizer`, change `command.args(arguments);`
to:

```rust
    // MARKED AS PNS'S OWN RUN, so a harness pns hooks into reports it as no
    // session: Hermes's pns-hooks plugin registers nothing under it, and the
    // Codex turn summarizer already guards on the same variable.
    command.args(arguments).env("PNS_SUMMARIZING", "1");
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters recap::summarizer`
Expected: all pass.

- [ ] **Step 5: State the behavior in the pns spec**

In `pns/docs/specs/return-recap.md` behavior 8 ("One recap spends one summarizer budget across up to
three questions"), add a bullet: "Required side effects: every summarizer child runs with
`PNS_SUMMARIZING=1`, so a harness pns hooks into reports the run as no session
(`pns-adapters/src/recap/summarizer/tests.rs:every_summarizer_run_is_marked_as_pns_summarizing`)."

- [ ] **Step 6: Run the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 7: Commit**

```bash
git add pns/crates/pns-adapters/src/recap/summarizer.rs pns/crates/pns-adapters/src/recap/summarizer/tests.rs \
  pns/docs/specs/return-recap.md
SKIP_AI_COMMIT=1 git commit -m "feat(pns): mark every summarizer run as pns's own"
```

---

### Task 7: `pns hermes install-plugin` writes and enables the plugin

Branch: `feat/pns-hermes-install-plugin`. Needs Tasks 5 and 6 merged.

**Files:**
- Modify: `pns/crates/pns-adapters/src/hermes_plugin.rs` (install and enable)
- Modify: `pns/crates/pns-adapters/src/lib.rs` (exports)
- Create: `pns/crates/pns/src/command_hermes.rs`
- Modify: `pns/crates/pns/src/command_codex.rs` (`running_binary` becomes `pub(crate)`)
- Modify: `pns/crates/pns/src/lib.rs`, `invocation.rs`, `subcommand_usage.rs`, `legacy/usage.rs`
- Modify: `pns/crates/pns/tests/support/sandbox/commands.rs` (`bare` fences `PNS_HERMES_BIN`)
- Create: `pns/crates/pns/tests/hermes_commands.rs`

**Interfaces:**
- Consumes: `HERMES_PLUGIN_NAME`, `render_hermes_plugin` from Task 5.
- Produces: `pub enum HermesPluginInstall { Changed, Unchanged }`;
  `pub fn hermes_plugin_dir(hermes_home: &Path) -> PathBuf`;
  `pub fn install_hermes_plugin(hermes_home: &Path, binary: &str, moshi: &str) -> Result<HermesPluginInstall, String>`;
  `pub fn enable_hermes_plugin(hermes: &str) -> Result<(), String>`; the CLI verb
  `pns hermes install-plugin`, which Task 8's script runs.

- [ ] **Step 1: Fence Hermes off in the test sandbox**

In `pns/crates/pns/tests/support/sandbox/commands.rs` `bare`, directly after the
`PNS_MOSHI_HOOK_BIN` line, add:

```rust
        // HERMES IS FENCED OFF THE SAME WAY: `pns hermes install-plugin` runs
        // `hermes plugins enable`, and the `hermes` on PATH is the operator's
        // own, which would write their live config.
        command.env("PNS_HERMES_BIN", self.root.join("no-hermes-here"));
```

- [ ] **Step 2: Write the failing tests**

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
fn the_plugin_calls_the_binary_that_wrote_it_and_the_moshi_hook_pns_forwards_to() {
    let sandbox = Sandbox::without_config("hermes-install-plugin-binary");
    run(&mut install(&sandbox, 0));
    let literal = |path: &std::path::Path| serde_json::Value::String(path.display().to_string()).to_string();
    let engine = std::fs::canonicalize(ENGINE).expect("the engine");
    let init = std::fs::read_to_string(plugin_dir(&hermes_home(&sandbox)).join("__init__.py"))
        .expect("the shim");
    assert!(init.contains(&format!("PNS = {}\n", literal(&engine))), "{init}");
    // The sandbox fences PNS_MOSHI_HOOK_BIN to this path, and the shim feeds moshi through it.
    let moshi = sandbox.root.join("no-moshi-hook-here");
    assert!(init.contains(&format!("MOSHI = {}\n", literal(&moshi))), "{init}");
}

#[test]
fn a_hermes_that_cannot_enable_it_is_an_exit_two_naming_the_cost_and_the_command() {
    let sandbox = Sandbox::without_config("hermes-install-plugin-refused");
    let output = run_expecting(2, &mut install(&sandbox, 1));
    assert!(stderr(&output).contains("do not reach pns"), "{output:?}");
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

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hermes_commands`
Expected: all FAIL; `pns hermes` prints the tool-wide usage and exits 2.

- [ ] **Step 4: Implement install and enable**

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
    moshi: &str,
) -> Result<HermesPluginInstall, String> {
    let directory = hermes_plugin_dir(hermes_home);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("pns: cannot create {}: {error}", directory.display()))?;
    let plugin = render_hermes_plugin(binary, moshi);
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
            "pns: Hermes did not enable the plugin, so Hermes sessions do not reach pns; run \
`{hermes} plugins enable {HERMES_PLUGIN_NAME}`"
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

- [ ] **Step 5: Implement the verb**

Create `pns/crates/pns/src/command_hermes.rs`:

```rust
//! `pns hermes install-plugin`: pns's Hermes plugin, written and enabled.
//!
//! THE PLUGIN NAMES THIS BINARY, taken from `current_exe` and canonicalized
//! the way `pns codex install-hooks` does, so the plugin a run writes calls
//! the engine that wrote it. It feeds the moshi-hook every other pns forward
//! reaches, from `moshi_hook_bin`.

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
    let moshi = pns_adapters::moshi_hook_bin();
    let installed = match pns_adapters::install_hermes_plugin(&home, &binary, &moshi) {
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

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hermes_commands`
Expected: 5 passed.

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns subcommand_usage`
Expected: the listing tests pass with the new row.

- [ ] **Step 7: Run the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 8: Commit**

```bash
git add pns/crates/pns-adapters/src/hermes_plugin.rs pns/crates/pns-adapters/src/lib.rs \
  pns/crates/pns/src/command_hermes.rs pns/crates/pns/src/command_codex.rs pns/crates/pns/src/lib.rs \
  pns/crates/pns/src/invocation.rs pns/crates/pns/src/subcommand_usage.rs \
  pns/crates/pns/src/legacy/usage.rs pns/crates/pns/tests/hermes_commands.rs \
  pns/crates/pns/tests/support/sandbox/commands.rs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): add pns hermes install-plugin"
```

---

### Task 8: This machine runs pns's Hermes plugin instead of moshi's and the shell hook pair

Branch: `feat/hermes-pns-plugin-cutover`. Needs Tasks 1 to 7 merged. This is the only task that changes
what an apply does.

**Files:**
- Create: `.chezmoiscripts/run_after_72-pns-hermes-plugin.sh.tmpl`
- Modify: `private_dot_hermes/modify_private_config.yaml` (header list, the approval-pair block, the
  `plugins.disabled` guard)
- Modify: `test/unit/hermes-config-modify-template.test.sh`
- Modify: `.chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl` (header comment, line 63)
- Modify: `CLAUDE.md` (the two `run_after_72` sentences)

**Interfaces:**
- Consumes: `pns hermes install-plugin` from Task 7.

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

function test_moshi_hooks_joins_the_disabled_plugins_once_and_hermes_entries_survive() {
  local rendered
  rendered="$(
    hermes_config_render <<'LIVE'
plugins:
  enabled:
    - pns-hooks
    - moshi-hooks
  disabled:
    - disk-cleanup
LIVE
  )"
  assert_same 'disk-cleanup,moshi-hooks' "$(yq '.plugins.disabled | join(",")' <<<"$rendered")"
  assert_same 'pns-hooks,moshi-hooks' "$(yq '.plugins.enabled | join(",")' <<<"$rendered")"
  rendered="$(printf 'plugins:\n  disabled:\n    - moshi-hooks\n' | hermes_config_render)"
  assert_same 'moshi-hooks' "$(yq '.plugins.disabled | join(",")' <<<"$rendered")"
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `just test-bashunit`
Expected: `test_the_approval_pair_is_removed...` FAILS on the first `assert_not_contains`, and
`test_moshi_hooks_joins_the_disabled_plugins...` FAILS on its first `assert_same` with `disk-cleanup`.

- [ ] **Step 3: Remove the pair in the template**

In `private_dot_hermes/modify_private_config.yaml`:

- In the header's "WHAT THIS OWNS" list, replace the two `hooks.*` rows with:

```text
    hooks.pre_approval_request       removed
    hooks.post_approval_response     removed
```

  add, after the `platform_toolsets.webhook` row:

```text
    plugins.disabled                 one member, moshi-hooks, appended when missing
```

  and change "overlays the five things above" to "overlays the four things above and removes the two".
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
{{- /* MOSHI'S OWN HERMES PLUGIN STAYS DISABLED. pns's plugin feeds moshi and
       hands it the approvals, so moshi's plugin loading beside it would send
       moshi every event twice and card each approval twice. `moshi-hooks` is
       one MEMBER of plugins.disabled, appended when missing; the rest of the
       list is hermes's. Hermes's deny list wins over plugins.enabled
       (hermes_cli/plugins.py, the discovery loop). The phone app's
       Integrations install and a bare `moshi-hook install` put the plugin
       back and may take the entry out, and the next apply puts the entry
       back. */ -}}
{{- $disabled := list -}}
{{- $plugins := get $merged "plugins" -}}
{{- if eq (kindOf $plugins) "map" -}}
{{-   $live := get $plugins "disabled" -}}
{{-   if eq (kindOf $live) "slice" -}}
{{-     $disabled = $live -}}
{{-   end -}}
{{- end -}}
{{- if not (has "moshi-hooks" $disabled) -}}
{{-   $merged = setValueAtPath (list "plugins" "disabled") (append $disabled "moshi-hooks") $merged -}}
{{- end -}}
```

This render was checked against sample live files in a scratch source state: the entry is appended once,
`plugins.enabled` and the other `disabled` entries survive, a file already carrying it is not
duplicated, and the byte-for-byte no-op holds.

In `test/unit/hermes-config-modify-template.test.sh`, the template no longer reads `.rust_tools`:
delete the `cp "$repo/.chezmoidata/rust_tools.yaml" ...` line and its comment in both helpers (keep the
`mkdir -p` of `.chezmoidata`), change the helper comment "`$1` is the HOME the render resolves the pns
hook path against" to "`$1` is the HOME the render runs under", and change the file header's "owns four
things" sentence to "owns four things, one of them a single member of plugins.disabled, and removes the
two approval hooks".

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
# uninstalls moshi's own Hermes plugin, so moshi is fed once and an approval
# raises one phone card.
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
# Hermes is uninstalled for the same reason: pns's own Hermes plugin feeds
# `moshi-hook hermes-hook` every event moshi's plugin sent and hands it the
# approvals, so moshi's plugin would send each one twice. `install --target`
# below already leaves hermes out, and the Hermes config template keeps
# moshi-hooks in plugins.disabled.
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
HERMES_HOME="$scratch/hermes" bash "$scratch/72-local.sh"; echo "exit $?"
HERMES_HOME="$scratch/hermes" bash "$scratch/72-local.sh"; echo "exit $?"
ls "$scratch/hermes/plugins/pns-hooks"
HERMES_HOME="$scratch/hermes" hermes plugins list --user
~/.hermes/hermes-agent/venv/bin/python pns/crates/pns-adapters/src/hermes_plugin/fixtures/drive.py \
  "$scratch/hermes/plugins/pns-hooks/__init__.py" '[]'
sed 's|(keepassxc "moshi-hook :: Device Token").Password|"stub-token"|' \
  .chezmoiscripts/run_once_after_60-moshi-hook-setup.sh.tmpl \
  | CI=1 chezmoi --source "$PWD" execute-template --no-tty > "$scratch/60.sh"
bash -n "$scratch/60.sh" && shellcheck "$scratch/60.sh"
grep -n 'uninstall --target codex,hermes' "$scratch/60.sh"
```

The two script runs reach the REAL `hermes`, confined to the scratch home: a `HERMES_HOME` outside
`~/.hermes` is its own root, so Hermes reads no `active_profile` and writes only under it
(`hermes_constants.py:112-149`, `hermes_cli/main.py:440-452`). That is what proves Hermes discovers the
manifest and enables it. The driver line imports the written shim under Hermes's own Python 3.11.

Expected: shellcheck silent; the first run prints the "wrote the Hermes plugin" line and `exit 0`; the
second prints only `exit 0`; the listing shows `__init__.py` and `plugin.yaml`; `hermes plugins list`
shows `pns-hooks` enabled; the driver prints the nine hooks under `registered`; the rendered 60 script
parses, lints clean and shows the new uninstall line. Never run the rendered 60 script: it pairs and
installs moshi for real, and never point `HERMES_HOME` at `~/.hermes` here. Clear the scratch directory
with `trash "$scratch"`.

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

The pull request body must tell the operator:

- the next apply re-fires `run_once_after_60` once; its pairing guard skips `pair` on a paired host
  (as the 2026-09-20 re-fire logged), so what re-runs is the install line, tap trust and a no-op service
  start;
- KeePassXC must be unlocked for it and for the config template;
- `hermes gateway restart` follows the apply;
- the same apply removes moshi's Hermes plugin before `run_after_72-pns-hermes-plugin` installs pns's,
  so a transcript line "Hermes did not enable the plugin, so Hermes sessions do not reach pns" means
  Hermes sessions reach neither pns nor moshi's screens until the command it names is run; the
  preflight drill checks it;
- the config template's two changes (the hook pair removed, `moshi-hooks` added to `plugins.disabled`)
  re-serialize `~/.hermes/config.yaml`, sorted and without Hermes's comments, as its header describes:
  once, or twice if moshi's uninstall in that same apply takes the entry out and the next apply puts it
  back; compare the parsed documents, not the texts.

---

### Task 9: The drills

No branch and no pull request. The operator runs these after applying Task 8 and running
`hermes gateway restart`. Record each result under "pns validation follow-ups" in
`docs/remaining-work.md`.

- [ ] **Preflight.** `hermes plugins list --user` shows `pns-hooks` enabled and `moshi-hooks` absent or
  disabled. `hermes hooks list` shows no `pns hook` command.
- [ ] **H1, desk approval.** In a herdr pane: `hermes chat --cli`, then ask it to run
  `rm -rf /tmp/pns-hermes-drill`. Expect a pns banner naming the command, no phone card, the blocked
  lamp if lamps are on. Answer once at the pane: the lamp clears, no moshi approval card appears (pns
  never forwarded the request, so it forwards no answer), the turn ends with a `hermes` done banner.
- [ ] **H2, phone approval.** Same prompt with the surface Mobile or Away, set the way drill D8 set it.
  Expect exactly one moshi card. Approve on the phone: moshi types the answer, the command runs, the card
  clears. Repeat with Deny and record the result.
- [ ] **H3, answered at the desk after the card.** Go away, let the card arrive, answer at the pane.
  Expect the phone card to clear.
- [ ] **H4, question.** Ask Hermes to use its clarify tool to ask which of two colors to use. Expect an
  `asked` notification, cleared when answered.
- [ ] **H5, failure.** `hermes chat --cli --max-turns 1` with a request that needs two tool calls.
  Expect a `failed` notification.
- [ ] **H6, gateway.** Message the Hermes Discord bot with a request that runs a tool. Expect no pns
  banner, no pns phone card, no lamp change and no `#pns-events` line; `pns recap --since 15m` lists the
  session's `hermes` rows; moshi's inbox shows the Discord session as it did under moshi's own plugin.
- [ ] **H7, moshi's screens.** In `hermes chat --cli`, run two turns, then `/exit`. On the phone, expect
  moshi's inbox to show the session running during each turn and complete after it, Chat View to open it
  with both turns, and the row to clear at the exit, as before the cutover.
- [ ] **H8, the TUI as a terminal session.** In a herdr pane: `hermes chat --tui`. Run a turn: expect a
  `hermes` done banner. Ask it to run `rm -rf /tmp/pns-hermes-drill` at the desk: expect a pns banner
  and the blocked lamp, cleared by the answer. Away, the same approval gets pns's own phone card, which
  cannot answer, until Task 10 lands.
- [ ] **H9, a kanban worker.** Create three kanban tasks for the default profile with `hermes kanban
  create`: one it can finish on its own, one that tells it to block on a question with `kanban_block`,
  and one that tells it to run `rm -rf /tmp/pns-kanban-drill`. Expect no pns banner and no pns card for
  the first worker's turns and a `done` row for it in `pns recap --since 30m`; expect one `asked`
  notification naming the second worker's question. For the third, expect no card of any kind (neither
  pns's nor moshi's) and no blocked lamp, its approval recorded as one `blocked` row in
  `pns recap --since 30m`, and Hermes's timeout deny after `approvals.timeout` (60 s by default) in
  `hermes kanban log <task id>`; if that deny fails the worker's turn, one `failed` notification.

---

### Task 10: A Hermes TUI approval is handed to moshi

Branch: `feat/pns-hermes-tui-approval-forward`. Needs Task 9's drills run. The operator's ruling (A2)
makes the TUI a terminal session, approvals included, so an away TUI approval goes to moshi as a CLI one
does. moshi's own plugin never sent a TUI approval, and its docs say gateway decisions "are not exposed
as terminal actions" (`docs/hooks.md:170-173`) while the TUI's approvals report `surface: "gateway"`, so
only the drill in Step 6 can say whether moshi answers one. By the spec's decision 24, THIS PULL REQUEST
OPENS ONLY AFTER THE OPERATOR REPORTS STEP 6 PASSED. A failed Step 6 neither closes the task nor drops
the requirement: the result goes under "pns validation follow-ups" in `docs/remaining-work.md` and back
to the operator with two options, pns keeps carding TUI approvals itself or they go to moshi anyway, and
the task waits on that answer. Until the task lands, the TUI keeps pns's own card.

**Files:**
- Modify: `pns/crates/pns-adapters/src/harness/routing.rs` (`moshi_subcommand`)
- Test: `pns/crates/pns-adapters/src/harness/tests/routing.rs`
- Test: `pns/crates/pns/tests/hooks/hermes_sessions.rs`
- Modify: `pns/docs/specs/blocking-approval.md` and `pns/docs/specs/hook-compatibility.md` (the roster)

**Interfaces:**
- Consumes: `moshi_subcommand(agent, payload)` from Task 3, `CLI_APPROVAL` and the Task 4 answer
  helpers.
- Produces: `moshi_subcommand` answers `Some("hermes-hook")` for a Hermes approval whose `platform` is
  `tui`, whatever its surface.

- [ ] **Step 1: Write the failing tests**

In `pns/crates/pns-adapters/src/harness/tests/routing.rs`, replace
`a_hermes_approval_goes_to_moshi_only_from_the_cli_prompt` with:

```rust
#[test]
fn a_hermes_approval_goes_to_moshi_from_the_cli_prompt_and_from_the_tui() {
    for (surface, platform) in [("cli", "cli"), ("gateway", "tui")] {
        assert_eq!(
            moshi_subcommand("hermes", &approval(surface, platform)).as_deref(),
            Some("hermes-hook"),
            "{surface}/{platform}"
        );
    }
    for (surface, platform) in [("", "cli"), ("gateway", "discord"), ("mcp-elicitation", "cli")] {
        assert_eq!(moshi_subcommand("hermes", &approval(surface, platform)), None, "{surface}/{platform}");
    }
}
```

In `pns/crates/pns/tests/hooks/hermes_sessions.rs`, replace
`a_tui_approval_is_carded_by_pns_rather_than_moshi` with:

```rust
#[test]
fn an_away_tui_approval_is_handed_to_moshi_byte_for_byte() {
    let sandbox = Sandbox::new("hermes-tui-approval-forward");
    let mut command = hermes(&sandbox);
    sandbox.stub_moshi(&mut command, 0);
    let tui = CLI_APPROVAL.replace(
        r#""surface":"cli","platform":"cli""#,
        r#""surface":"gateway","platform":"tui""#,
    );
    hook_with(command, &sandbox, "blocked", &tui);
    assert_eq!(submissions(&sandbox), vec!["hermes-hook".to_string()]);
    assert_eq!(
        std::fs::read_to_string(sandbox.path("moshi.stdin")).expect("moshi read the payload"),
        tui
    );
}
```

and in `only_a_hermes_cli_answer_is_handed_to_moshi`, rename it to
`only_a_hermes_terminal_answer_is_handed_to_moshi` and change its TUI case to a Discord one: replace
`r#""surface":"gateway","platform":"tui""#` there with `r#""surface":"gateway","platform":"discord""#`
and the variable `tui` with `discord`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns-adapters harness::tests::routing`
Expected: `a_hermes_approval_goes_to_moshi_from_the_cli_prompt_and_from_the_tui` FAILS on
`gateway/tui`.

Run: `cargo test --locked --manifest-path pns/Cargo.toml -p pns --test hooks hermes_sessions`
Expected: `an_away_tui_approval_is_handed_to_moshi_byte_for_byte` FAILS with `left: []`.

- [ ] **Step 3: Implement**

In `pns/crates/pns-adapters/src/harness/routing.rs`, change the Hermes arm and the last doc sentence of
`moshi_subcommand`:

```rust
/// third-party binary. A Hermes approval goes from the two terminal front
/// ends moshi's bridge answers: the classic CLI's own prompt and the TUI,
/// whose approvals report the `gateway` surface.
pub fn moshi_subcommand(agent: &str, payload: &HookPayload) -> Option<String> {
    match agent {
        "claude" | "codex" => Some(format!("{agent}-hook")),
        "hermes" if payload.surface == "cli" || payload.platform == "tui" => {
            Some("hermes-hook".to_string())
        }
        _ => None,
    }
}
```

In `pns/docs/specs/blocking-approval.md` and `pns/docs/specs/hook-compatibility.md`, change the roster
sentence Task 3 wrote to "admits `claude`, `codex`, and `hermes` for an approval whose `surface` is
`cli` or whose `platform` is `tui`".

- [ ] **Step 4: Run the tests and the gates**

Run: `just test-rust && just lint-check`
Expected: both exit 0.

- [ ] **Step 5: Commit**

```bash
git add pns/crates/pns-adapters/src/harness/routing.rs pns/crates/pns-adapters/src/harness/tests/routing.rs \
  pns/crates/pns/tests/hooks/hermes_sessions.rs pns/docs/specs
SKIP_AI_COMMIT=1 git commit -m "feat(pns): hand a Hermes TUI approval to moshi-hook hermes-hook"
```

- [ ] **Step 6: The operator's drill (H10), which gates the pull request**

The shim calls the binary that wrote it, so the branch build is drilled by pointing the live plugin at
it and back. The operator runs, from this worktree:

```bash
cargo build --locked --release --manifest-path pns/Cargo.toml -p pns
./pns/target/release/pns hermes install-plugin
```

Then, with the surface Mobile or Away, in a herdr pane: `hermes chat --tui`, ask it to run
`rm -rf /tmp/pns-hermes-drill`. Pass: exactly one moshi card; approving on the phone types the answer
into the TUI and the command runs; the card clears. Repeat with Deny. Then point the plugin back at the
installed engine:

```bash
~/.cargo/bin/pns hermes install-plugin
```

A Hermes process that started while the plugin pointed at the branch build keeps calling that build
until it restarts, and loses every pns event once this worktree is removed. So quit the drill's TUI and
any other Hermes session started during the drill, let any kanban worker it started finish, and run
`hermes gateway restart` if the gateway was restarted during the drill.

Record pass or fail. A pass opens the pull request; a fail goes back to the operator as the paragraph at
the top of this task says.
