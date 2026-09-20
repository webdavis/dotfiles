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
        .env("PNS_SCREEN_IDLE", "99999")
        .env("PNS_PHONE_INPUT_MAX_AGE", "24h");
    command
}

/// The capture server, already bound to its ephemeral port.
pub struct Capture {
    server: Child,
    port: u16,
    captured: PathBuf,
}

/// The status http-capture answers when no caller chose one.
const DEFAULT_STATUS: u16 = 200;

/// Refuses a status no HTTP response can carry, at the call site that named it.
fn checked_status(status: u16) -> u16 {
    assert!(
        (100..=599).contains(&status),
        "{status} is not an HTTP status a capture can answer"
    );
    status
}

/// Names the status and the request count, so neither can take the other's place.
pub struct CaptureBuilder<'a> {
    sandbox: &'a Sandbox,
    name: &'a str,
    status: u16,
    requests: usize,
}

impl<'a> CaptureBuilder<'a> {
    /// The status the server answers every request with.
    pub fn status(mut self, status: u16) -> Self {
        self.status = checked_status(status);
        self
    }

    /// How many requests the server serves before it exits, which only a leg
    /// that posts more than once has any use for.
    pub fn requests(mut self, requests: usize) -> Self {
        self.requests = requests;
        self
    }

    pub fn start(self) -> Capture {
        let port_file = self.sandbox.path(&format!("{}.port", self.name));
        let captured = self.sandbox.path(&format!("{}.capture", self.name));
        let mut command = Command::new(CAPTURE);
        // Both are passed on every start, so http-capture's positional pair is
        // never read one argument short.
        command
            .arg(&port_file)
            .arg(&captured)
            .arg(self.status.to_string())
            .arg(self.requests.to_string());
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
}

impl Capture {
    /// A capture that answers 200 to one request until told otherwise.
    pub fn builder<'a>(sandbox: &'a Sandbox, name: &'a str) -> CaptureBuilder<'a> {
        CaptureBuilder {
            sandbox,
            name,
            status: DEFAULT_STATUS,
            requests: 1,
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

#[cfg(test)]
mod tests {
    use super::checked_status;

    #[test]
    fn a_real_http_status_is_kept() {
        assert_eq!(checked_status(503), 503);
    }

    #[test]
    #[should_panic(expected = "3 is not an HTTP status")]
    fn a_request_count_passed_as_a_status_is_refused() {
        checked_status(3);
    }
}
