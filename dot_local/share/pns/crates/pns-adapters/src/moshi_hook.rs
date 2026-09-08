use crate::moshi_hook_bin;
use std::io::Write;
use std::process::{Command, Stdio};
mod settings;
mod wait;
use settings::submit_deadline;
use wait::answer_within;

pub struct MoshiApprovalForwarder;

impl pns_application::ApprovalForwarder for MoshiApprovalForwarder {
    type Forwarded = std::process::Child;

    fn forward(&self, subcommand: &str, payload_json: &str) -> Option<Self::Forwarded> {
        spawn_moshi_hook(subcommand, payload_json)
    }
    fn answer(&self, child: Self::Forwarded) -> i32 {
        // THE CONFIG IS READ A SECOND TIME HERE, after the notification and
        // immediately before the wait, as it always was: threading it out of
        // `run_event` would change that function's signature for one duration,
        // and a view torn between the two reads costs at most this event.
        answer_within(child, submit_deadline())
    }
}

/// Start moshi on the stream. `None` is "not installed", which is the
/// harness's "no opinion": it prompts as usual.
///
/// THE WRITE HAPPENS OFF THIS THREAD. A child that does not read its stdin
/// blocks the writer as soon as the pipe buffer fills, and a payload larger
/// than that buffer is ordinary. Writing here would put that block in front
/// of the notification and in front of the wait below, which is supposed to
/// be the only place this waits on anybody. The thread outlives a caller that
/// stops waiting, which is fine: it holds a pipe and a copy of the payload,
/// and the process is on its way out.
fn spawn_moshi_hook(subcommand: &str, payload_json: &str) -> Option<std::process::Child> {
    let moshi = moshi_hook_bin();
    let mut child = Command::new(&moshi)
        .arg(subcommand)
        .stdin(Stdio::piped())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let payload = payload_json.to_string();
        // Dropping the pipe when the write finishes is what gives the child
        // its EOF; a child waiting on one would otherwise never start.
        std::thread::spawn(move || {
            let _ = stdin.write_all(payload.as_bytes());
        });
    }
    Some(child)
}
