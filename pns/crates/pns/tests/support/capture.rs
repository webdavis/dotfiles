//! The one-shot HTTP capture server, and the command that posts at it.
//!
//! SHARED BY TWO BINARIES. `native.rs` proves the compiled-in plugins'
//! own wire format; `recap_commands.rs` proves the route an agent's recap
//! takes. Both need a gateway that answers a chosen status and records
//! what arrived, and a second copy of one would be a second thing to keep
//! in step.

use super::{CAPTURE, Sandbox};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

pub fn plugin_command(sandbox: &Sandbox) -> Command {
    let mut command = sandbox.bare();
    // These tests exercise delivery, not the operator's live idle and mosh sessions.
    command
        .env("PNS_IDLE_SECS", "99999")
        .env("PNS_PHONE_INPUT_AGE", "99999");
    command
}

/// The capture server, already bound to its ephemeral port.
pub struct Capture {
    server: Child,
    port: u16,
    captured: PathBuf,
}

impl Capture {
    /// `requests` is how many the server serves before it exits, which only a
    /// leg that posts more than once has any use for.
    pub fn start(
        sandbox: &Sandbox,
        name: &str,
        status: Option<&str>,
        requests: Option<&str>,
    ) -> Self {
        let port_file = sandbox.path(&format!("{name}.port"));
        let captured = sandbox.path(&format!("{name}.capture"));
        let mut command = Command::new(CAPTURE);
        command.arg(&port_file).arg(&captured);
        // The count is positional behind the status, so a caller naming one
        // names both.
        if let Some(status) = status {
            command.arg(status);
        }
        if let Some(requests) = requests {
            command.arg(requests);
        }
        let server = command.spawn().expect("the capture server starts");

        let deadline = Instant::now() + Duration::from_secs(30);
        let port = loop {
            if let Ok(text) = std::fs::read_to_string(&port_file)
                && let Ok(port) = text.trim().parse::<u16>()
            {
                break port;
            }
            assert!(Instant::now() < deadline, "the capture server never bound");
            std::thread::sleep(Duration::from_millis(25));
        };
        Capture {
            server,
            port,
            captured,
        }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The raw request, once the server has answered and exited.
    ///
    /// The join is bounded by the CHILD, not by a deadline here: http-capture
    /// exits 1 if no connection arrives within thirty seconds, and every read
    /// after accept runs under a ten second socket timeout whose expiry ends
    /// its loop. There is no path on which it waits forever.
    pub fn finish(mut self) -> String {
        let _ = self.server.wait();
        std::fs::read_to_string(&self.captured).unwrap_or_default()
    }
}
