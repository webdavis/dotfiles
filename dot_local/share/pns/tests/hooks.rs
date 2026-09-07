//! The harness hooks, end to end: a payload on stdin becomes the same event
//! any other caller would produce, and a blocking one becomes the operator's
//! decision. These are the twins of the bats suites the bash hooks carried.

#[path = "hooks/captured_child.rs"]
mod captured_child;
mod support;

use std::io::Write;
use std::process::{Command, Stdio};
use support::{Sandbox, write_script};

/// One hook run: the payload on stdin, the output back.
fn hook(sandbox: &Sandbox, event: &str, payload: &str) -> std::process::Output {
    hook_with(sandbox.pns(), sandbox, event, payload)
}

fn hook_with(
    mut command: Command,
    _sandbox: &Sandbox,
    event: &str,
    payload: &str,
) -> std::process::Output {
    let mut child = command
        .args(["hook", event])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the engine runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload.as_bytes())
        .expect("payload");
    child.wait_with_output().expect("output")
}

fn marker(sandbox: &Sandbox, session: &str) -> std::path::PathBuf {
    sandbox.path(&format!("state/session-{session}.start"))
}

fn with_state_dir(sandbox: &Sandbox) -> Command {
    let mut command = sandbox.pns();
    command.env("PNS_STATE_DIR", sandbox.path("state"));
    command
}

// --- the approval contract ---------------------------------------------------
//
// THE GATE THAT BOUNDS THE SECOND APPROVAL SURFACE. Everything below is green
// on today's build by construction and each row was proved killable by a named
// mutation of the engine, because a characterization test nobody proved can
// fail is a line of green that guards nothing.
//
// WHAT IS DELIBERATELY NOT HERE, so nobody adds it back, and the test that
// covers each. Every one was written for this gate, measured against its
// mutation, found already killed by a test that exists, and dropped: a second
// copy of a guard is not a second guard.
//
//   the single-submitter rule, hook entry point
//     `one_prompt_is_submitted_exactly_once_and_a_zero_answer_from_it_is_an_approve`
//   the single-submitter rule, gate entry point
//     `the_gate_submits_one_prompt_exactly_once`
//   a submission that died without answering
//     `a_submission_that_dies_without_answering_is_not_a_decision`
//   the gate declining at the desk
//     `at_the_desk_the_gate_submits_nothing_and_exits_zero`
//   the gate refusing an over-cap payload
//     `the_gate_refuses_an_over_cap_payload_as_firmly_as_the_hook_does`
//   a watched pane is still forwarded
//     `an_approval_is_forwarded_even_with_the_pane_in_plain_sight`
//
// THREE MORE WERE STRUCK AND ARE BACK, at the end of this section, because in
// each case the test said to cover them drives a DIFFERENT arm and a
// blocked-only regression walks past it: the desk banner and the watched pane
// (`plan`'s matrix is a unit on the plan, and nothing composed a blocked hook
// at the desk), a payload nobody finishes writing (the deadline test beside it
// drives `stop`), and a presence reading nobody can parse (a unit on
// `operator_surface`, never across the forward).
//
// ONE BEHAVIOR IS DROPPED ON SCOPE AND IS PINNED NOWHERE END TO END: the
// locked screen. `screen_locked` spawns `/usr/sbin/ioreg` by absolute path, so
// no PATH stub reaches it, and it is read only where `PNS_IDLE_SECS` is
// unstated while every sandbox here states it. It has a unit pin on
// `operator_surface` and buying the composition would need a production
// override that exists for no other reason.
//
// THE EXIT CODE IS NOT HOW CLAUDE CODE ANSWERS, and the rows that pin one say
// so themselves. Claude Code 2.1.241 decides a PermissionRequest from the
// hook's STDOUT alone, off `hookSpecificOutput.decision`, and reads the exit
// code on that event nowhere; the answer to a phone tap travels moshi's own
// bridge, which screen-reads the pane and sends keys. What the exit code IS is
// a pns-side contract the gate's direct callers read, and whose reading by
// Codex is unverified. The corollary is the load-bearing one and
// `the_blocked_hook_writes_nothing_the_harness_would_read_as_a_decision` is
// its guard: stdout is a live channel on this event, and pns writes NOTHING to
// it on this path (measured, a real blocked run prints zero bytes), which is
// exactly why anything starting with `{` there is an object nobody meant to
// print.

/// A `PermissionRequest` payload, the binary's own field set (2.1.241):
/// `tool_name` and `tool_input` required and `permission_suggestions`
/// optional, spread over the base every event carries, in the emitter's own
/// key order.
///
/// IT STATES NO `message`, WHICH IS THE WHOLE POINT. The card's detail
/// resolves through `parse_payload`'s fallthrough to the tool request, exactly
/// as a Codex approval does, and every approval test written before this one
/// used a `{"message":...}` shape the harness has never sent.
///
/// ALL EIGHT BASE FIELDS ARE HERE (`session_id`, `transcript_path`, `cwd`,
/// `prompt_id`, `permission_mode`, `agent_id`, `agent_type`, `effort`), and so
/// is `permission_suggestions`, though pns reads three of the eight and none
/// of the suggestions. They are carried because the harness carries them: the
/// point of this fixture is that a future reader sees the real thing rather
/// than a reduction of it, and four base fields would have been a reduction.
/// The NAMES are the emitter's; the values of the four pns never reads are the
/// suite's own, because nothing measured what the harness puts in them.
const CLAUDE_APPROVAL: &str = r#"{"session_id":"s1","transcript_path":"/dev/null","cwd":"/a/dotfiles","prompt_id":"prompt_01","permission_mode":"default","agent_id":"agent_01","agent_type":"main","effort":"medium","hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"rm -rf /tmp/x"},"permission_suggestions":[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"rm:*"}],"behavior":"allow","destination":"localSettings"}]}"#;

/// A Codex `PermissionRequest`, the shape measured off 0.147: `tool_name` and
/// `tool_input` and neither `message` nor `detail`, which is the same two keys
/// Claude Code sends.
const CODEX_APPROVAL: &str = r#"{"hook_event_name":"PermissionRequest","session_id":"s1","cwd":"/a/dotfiles","tool_name":"shell","tool_input":{"command":["bash","-lc","rm -rf build"]}}"#;

/// Stubs live here rather than in the shared harness: only this suite spawns
/// a condenser or an approval round trip.
trait HookStubs {
    fn stub_codex(&self, command: &mut Command, line: &str);
    fn stub_moshi(&self, command: &mut Command, exit_code: i32);
}

impl HookStubs for Sandbox {
    fn stub_codex(&self, command: &mut Command, line: &str) {
        let bin = self.path("bin");
        std::fs::create_dir_all(&bin).expect("stub bin");
        write_script(
            &bin.join("codex"),
            &format!("cat >/dev/null; printf '%s\\n' '{line}'"),
        );
        prepend_path(command, &bin);
        command.env("CODEX_BIN", bin.join("codex"));
        command.env("PNS_CODEX_HOME", self.path("codex-home"));
    }

    /// THE ARGV FILE APPENDS, one line per spawn, so a SECOND submission of the
    /// same prompt is observable at all. A truncating record answered "what was
    /// the last argv"; the single-submitter rule needs "how many were there".
    /// Of the fifteen readers, FOUR compare contents and all four trim, so one
    /// spawn still yields exactly `claude-hook`; the other eleven only ask
    /// whether anything was recorded at all, and the ones this gate adds ask
    /// through `submissions` below.
    fn stub_moshi(&self, command: &mut Command, exit_code: i32) {
        let bin = self.path("bin");
        std::fs::create_dir_all(&bin).expect("stub bin");
        write_script(
            &bin.join("moshi-hook"),
            &format!(
                "printf '%s\\n' \"$*\" >>\"{sandbox}/moshi.argv\"; cat >\"{sandbox}/moshi.stdin\"; exit {exit_code}",
                sandbox = self.display()
            ),
        );
        command.env("MOSHI_HOOK_BIN", bin.join("moshi-hook"));
    }
}

/// Every submission `stub_moshi` recorded, one per line, in the order they
/// were made, and EMPTY when nothing was recorded at all.
///
/// THE EMPTY CASE IS THE LOAD-BEARING ONE. Every "never submitted" assertion
/// in this file reads through here rather than through the record's filename,
/// so the day the submission stops being a child process there is ONE place
/// to re-point at whatever the new transport records. Spelled as a filename,
/// those guards answer "no file, so nothing was submitted" for a build that
/// submits over something else, which is the single regression this gate
/// exists to catch. `tests/dispatch.rs`'s `moshi_hook_argv` reads its own
/// record the same way, for the same reason.
///
/// NO SETTLE, AND THE RESIDUAL IS STATED RATHER THAN SLEPT ON. Every
/// submission the crate makes today is waited on by the process under test,
/// so the record has landed by the time that process exits; this counts what
/// the exiting process left behind. A duplicate that was DETACHED instead of
/// waited on could land after the read, and no sleep short enough for this
/// suite would close that (`tests/support/mod.rs` refuses fixed sleeps for
/// exactly that reason, and a 100ms one here was measured to change nothing).
fn submissions(sandbox: &Sandbox) -> Vec<String> {
    std::fs::read_to_string(sandbox.path("moshi.argv"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// The engine with a moshi stub ALWAYS installed, whatever else the caller
/// overrides afterwards.
///
/// Every test in the approval section spawns the blocked path, and
/// `Sandbox::pns` points `MOSHI_HOOK_BIN` nowhere, so a test that forgets to
/// stub reaches the OPERATOR'S OWN moshi-hook and can raise a real card on
/// their phone. That is not hypothetical: it happened during slice 11, seven
/// tests deep. One helper is cheaper than remembering.
fn approval(sandbox: &Sandbox, exit_code: i32) -> Command {
    let mut command = sandbox.pns();
    sandbox.stub_moshi(&mut command, exit_code);
    command
}

fn prepend_path(command: &mut Command, directory: &std::path::Path) {
    let mut path = std::ffi::OsString::from(directory);
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap_or_default());
    command.env("PATH", path);
}

// --- nothing may hang -------------------------------------------------------

/// Every bound below is proved the same way: run the thing against input that
/// would block forever, with a tight injected deadline, and require an answer.
const HANG_LIMIT: std::time::Duration = std::time::Duration::from_secs(5);

fn spawn_hook(mut command: Command, event: &str) -> std::process::Child {
    command
        .args(["hook", event])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the engine runs")
}

/// Write the payload and CLOSE the pipe: the reader waits for EOF, so a
/// handle left open is the test hanging itself rather than the hook.
fn write_payload(child: &mut std::process::Child, payload: &[u8]) {
    let mut stdin = child.stdin.take().expect("stdin");
    let _ = stdin.write_all(payload);
}

fn finished_within(mut child: std::process::Child, limit: std::time::Duration) -> Option<i32> {
    let deadline = std::time::Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status.code();
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[path = "hooks/approval_exemptions.rs"]
mod approval_exemptions;
#[path = "hooks/approval_forwarding.rs"]
mod approval_forwarding;
#[path = "hooks/approval_payload.rs"]
mod approval_payload;
#[path = "hooks/approval_presence.rs"]
mod approval_presence;
#[path = "hooks/approval_reporting.rs"]
mod approval_reporting;
#[path = "hooks/config_change.rs"]
mod config_change;
#[path = "hooks/config_change_state.rs"]
mod config_change_state;
#[path = "hooks/deadlines.rs"]
mod deadlines;
#[path = "hooks/denied_tools.rs"]
mod denied_tools;
#[path = "hooks/elicitation.rs"]
mod elicitation;
#[path = "hooks/failed_turns.rs"]
mod failed_turns;
#[path = "hooks/gate.rs"]
mod gate;
#[path = "hooks/hook_contract.rs"]
mod hook_contract;
#[path = "hooks/lights_waits.rs"]
mod lights_waits;
#[path = "hooks/model_switch.rs"]
mod model_switch;
#[path = "hooks/model_switch_state.rs"]
mod model_switch_state;
#[path = "hooks/nag_arming.rs"]
mod nag_arming;
#[path = "hooks/nag_clearing.rs"]
mod nag_clearing;
#[path = "hooks/nag_delivery.rs"]
mod nag_delivery;
#[path = "hooks/nag_observations.rs"]
mod nag_observations;
#[path = "hooks/nag_refusals.rs"]
mod nag_refusals;
#[path = "hooks/nag_state.rs"]
mod nag_state;
#[path = "hooks/policy_audit.rs"]
mod policy_audit;
#[path = "hooks/quota_messages.rs"]
mod quota_messages;
#[path = "hooks/quota_state.rs"]
mod quota_state;
#[path = "hooks/quota_waits.rs"]
mod quota_waits;
#[path = "hooks/turn_markers.rs"]
mod turn_markers;
#[path = "hooks/turn_reply.rs"]
mod turn_reply;
#[path = "hooks/turn_tier.rs"]
mod turn_tier;

use config_change::config_change_payload;
use lights_waits::{LAMPS_ON, waiting_sessions};
use model_switch::model_switch_payload;
use nag_state::{
    counted_channels, deliveries, epoch_now, nag, nag_config, nag_directory_names, nag_marker,
    nag_record, spool_entries, spool_entry, state_lines, write_marker, write_record,
    write_record_at,
};
use quota_messages::{QUOTA_TYPES, quota_payload};
