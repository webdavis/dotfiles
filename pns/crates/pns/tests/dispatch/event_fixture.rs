use super::*;

// --- what argv[1] dispatches to ---------------------------------------------

/// A sandbox whose ONLY channel is the native banner, driven from the desk.
///
/// THE BANNER IS WHAT MAKES THE SPY BITE. The stub-channel harness dispatches
/// by absolute path inside the sandbox, so no PATH can see it; the native
/// banner resolves `terminal-notifier` through PATH, so an argv path that
/// reached the event path leaves a line in the spy log. `PNS_IDLE_SECS` puts
/// the operator at the desk, which is where the banner is the delivery.
pub(super) fn desk_with_a_native_banner(name: &str) -> (Sandbox, std::process::Command) {
    let sandbox = Sandbox::without_config(name);
    sandbox.write_config("[plugins.macos-banner]\nenabled = true\n");
    let mut command = sandbox.bare();
    command
        .env("PNS_STATE_DIR", sandbox.state())
        .env("PNS_IDLE_SECS", "0");
    sandbox.spy_path(&mut command);
    (sandbox, command)
}

/// Wait for a child with a deadline, killing it if it outlives one. The suite
/// must not be able to hang on a test whose whole point is a blocking read.
pub(super) fn wait_bounded(
    mut child: std::process::Child,
    limit: std::time::Duration,
) -> Option<i32> {
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
