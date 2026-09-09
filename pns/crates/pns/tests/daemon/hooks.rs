use super::*;

/// A GUARD, not a red-first behavior: it passes on the first commit and its
/// job is to keep passing.
///
/// THE SLICE'S ENTIRE SAFETY CLAIM. Every notification pns delivers today must
/// be delivered identically with a daemon running: the same stdout byte for
/// byte, the same stderr, the same exit code, AND THE SAME CHANNELS FIRED.
/// Kept over the usual objection to guard tests because its failure is silent
/// and lands in the harness, where a changed byte on a hook's stdout is a
/// permission prompt that stops being drawn.
///
/// THE FIRED SET IS PART OF THE CLAIM. A hook whose stdout and exit code were
/// untouched while a delivery quietly stopped going out has failed the
/// fail-open property exactly as badly, and only the channel legs can see it.
#[test]
fn the_daemon_changes_nothing_about_a_hook() {
    let sandbox = Sandbox::new("daemon-changes-no-hook");
    sandbox.write_config(ONE_CHANNEL);

    // The same two hooks, run first with no daemon anywhere.
    let quiet = [run_hook(&sandbox, "stop"), run_hook(&sandbox, "blocked")];

    let guard = DaemonGuard::start(&sandbox, TICK_MS);
    // Past a few ticks, so the daemon is unmistakably up and beating.
    std::thread::sleep(Duration::from_millis(TICK_MS * 4));
    let noisy = [run_hook(&sandbox, "stop"), run_hook(&sandbox, "blocked")];

    for (event, (before, after)) in ["stop", "blocked"]
        .iter()
        .zip(quiet.iter().zip(noisy.iter()))
    {
        assert_eq!(
            before.0.stdout,
            after.0.stdout,
            "{event}: a daemon changed the hook's stdout; it said: {}",
            guard.said()
        );
        assert_eq!(
            String::from_utf8_lossy(&before.0.stderr),
            String::from_utf8_lossy(&after.0.stderr),
            "{event}: a daemon changed the hook's stderr; it said: {}",
            guard.said()
        );
        assert_eq!(
            before.0.status.code(),
            after.0.status.code(),
            "{event}: a daemon changed the hook's exit code"
        );
        assert_eq!(
            before.1, after.1,
            "{event}: a daemon changed which channels the hook delivered to"
        );
    }
}

/// The channels this sandbox's stubs recorded a delivery on, cleared first so
/// the answer belongs to ONE hook run rather than to every run before it.
const RECORDING: [&str; 3] = ["mobile", "hermes", "macos-banner"];

fn run_hook(sandbox: &Sandbox, event: &str) -> (std::process::Output, Vec<&'static str>) {
    for channel in RECORDING {
        let _ = std::fs::remove_file(sandbox.path(&format!("{channel}.event")));
    }
    let payload = match event {
        "stop" => r#"{"session_id":"s1","last_assistant_message":"done here"}"#,
        _ => r#"{"session_id":"s1","message":"needs approval"}"#,
    };
    let output = hook(sandbox, event, payload);
    let fired = RECORDING
        .into_iter()
        .filter(|channel| sandbox.fired(channel))
        .collect();
    (output, fired)
}

/// One hook run, payload on stdin, in `tests/hooks.rs`'s own shape.
fn hook(sandbox: &Sandbox, event: &str, payload: &str) -> std::process::Output {
    use std::io::Write;
    let mut child = sandbox
        .pns_stateful()
        .args(["hook", event])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
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
