use super::support::Sandbox;
use std::io;
use std::time::{Duration, Instant};

/// Every event one channel was handed, in the order it got them.
pub(super) fn events(sandbox: &Sandbox, channel: &str) -> Vec<serde_json::Value> {
    let path = sandbox.path(&format!("{channel}.events"));
    read_events(|| std::fs::read(&path), channel)
}

fn read_events(
    mut read: impl FnMut() -> io::Result<Vec<u8>>,
    channel: &str,
) -> Vec<serde_json::Value> {
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        let bytes = match read() {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
            Err(error) => panic!("{channel}: cannot read the capture: {error}"),
        };
        if bytes.is_empty() {
            return Vec::new();
        }
        // Stub writers append newline-delimited records. A snapshot taken
        // mid-append may end inside JSON or UTF-8; neither means no events.
        if let Some(complete) = bytes.strip_suffix(b"\n") {
            return complete
                .split(|byte| *byte == b'\n')
                .map(|line| {
                    serde_json::from_slice(line).unwrap_or_else(|error| {
                        panic!("{channel}: {error}: {}", String::from_utf8_lossy(line))
                    })
                })
                .collect();
        }
        assert!(
            Instant::now() < deadline,
            "{channel}: capture did not finish its final record within 250 ms"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(test)]
#[path = "captured_events/tests.rs"]
mod tests;
