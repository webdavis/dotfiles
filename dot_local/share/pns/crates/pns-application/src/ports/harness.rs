//! The harness on the other side of a hook.

/// Read a bounded UTF-8 payload, including a capped prefix when oversized.
///
/// `None` means a read failure, invalid UTF-8 or a deadline. Empty input can
/// return an empty string. Forwarding separately checks that the payload is
/// whole; an oversized prefix can still produce a fallback notification.
/// Checked against `read_payload` and `payload_is_whole` in `src/hook_payload.rs`.
/// Statements: S047, S048.
pub trait HarnessPayload {
    fn read(&self) -> Option<String>;
}
