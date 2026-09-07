use crate::*;

/// The harness payload from stdin, bounded in SIZE and in TIME.
///
/// Neither bound is theoretical: a pipe nobody closes hangs the hook before
/// the exit contract can run, and a payload nobody caps can exhaust memory
/// long before the reply's own character cap applies.
pub(crate) fn read_payload() -> Option<String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut payload = String::new();
        // ONE BYTE PAST the cap, so a payload that hit it is distinguishable
        // from one that merely reached it: see `payload_is_whole`.
        let read = std::io::Read::read_to_string(
            &mut std::io::Read::take(std::io::stdin(), MAX_PAYLOAD_BYTES + 1),
            &mut payload,
        );
        let _ = sender.send(read.ok().map(|_| payload));
    });
    // The reader thread outlives a refusal, which is fine: the process is
    // about to exit, and it holds nothing but its own buffer.
    receiver.recv_timeout(payload_deadline()).ok().flatten()
}
/// A harness payload is a small JSON object; anything larger is not one.
const MAX_PAYLOAD_BYTES: u64 = 1_000_000;
/// Whether the payload is the bytes the harness actually sent.
///
/// A payload that reached the cap was CUT MID-OBJECT, so it is no longer
/// JSON and no longer what anybody wrote. Forwarding it hands moshi an empty
/// parse, which is the exact failure the byte-for-byte rule exists to
/// prevent; measured 2026-08-19, a 1.2MB payload forwarded as exactly
/// 1,000,000 bytes. The notification still goes out, carrying whatever an
/// unparseable payload yields, because something IS blocked either way.
pub(crate) fn payload_is_whole(payload_json: &str) -> bool {
    payload_json.len() <= MAX_PAYLOAD_BYTES as usize
}
/// How long the payload may take to arrive. Generous, because a harness
/// writing a large transcript path is normal and a hang is not.
fn payload_deadline() -> Duration {
    env_deadline("PNS_PAYLOAD_DEADLINE_MS").unwrap_or(Duration::from_secs(5))
}
