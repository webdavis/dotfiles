use crate::*;

/// The turn's final text: the harness's own copy first, the transcript tail
/// as the fallback.
///
/// THE FALLBACK IS RE-READ inside a bounded window. The harness has not always
/// flushed the assistant's final text when the Stop hook runs (live capture
/// 2026-08-12: one read came back empty and the notification shipped with no
/// detail at all). An expired window proves only that nothing readable arrived
/// in time; a turn that said nothing, an unreadable transcript and an
/// unparseable one all leave the same empty string and are reported the same.
///
/// Emptiness is judged on the FLATTENED reply, because a block carrying only
/// whitespace is non-empty raw and empty once flattened, which is the same
/// missing-summary symptom through another door.
pub(crate) fn turn_reply(payload: &HookPayload) -> String {
    let flatten = |text: &str| pns::render::flatten_reply(text, REPLY_MAX_CHARS);
    let from_payload = flatten(&payload.last_assistant_message);
    if !from_payload.is_empty() || payload.transcript_path.is_empty() {
        return from_payload;
    }
    for attempt in 0..=reread_attempts() {
        if attempt > 0 {
            std::thread::sleep(reread_interval());
        }
        let reply = flatten(&transcript_reply(&transcript_tail(
            &payload.transcript_path,
        )));
        if !reply.is_empty() {
            return reply;
        }
    }
    String::new()
}
/// The tail of a transcript, never the whole file: a long session grows it
/// past 200MB, and the extraction only ever needs the last turn. Measured
/// 2026-08-05: slurping the whole file held ~33MB resident and minutes of CPU.
fn transcript_tail(path: &str) -> String {
    use std::io::{Read, Seek, SeekFrom};
    // CHECKED BEFORE OPENING, and on the link itself. Opening a FIFO blocks
    // until a writer appears and /dev/zero never ends; both hang a hook whose
    // whole contract is answering promptly. A transcript is a regular file.
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return String::new();
    };
    if !metadata.is_file() {
        return String::new();
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let _ = file.seek(SeekFrom::Start(
        metadata.len().saturating_sub(TRANSCRIPT_TAIL_BYTES),
    ));
    let mut tail = Vec::new();
    // Capped as well as sought: the file can grow between the two calls, and
    // a seek that failed would otherwise read all of it.
    let _ = file.take(TRANSCRIPT_TAIL_BYTES).read_to_end(&mut tail);
    String::from_utf8_lossy(&tail).into_owned()
}
/// How many extra times the transcript is re-read while the harness flushes.
/// VALIDATED before it is believed, and falling back to the default rather
/// than to no retries.
fn reread_attempts() -> u32 {
    reread_attempts_from(std::env::var("PNS_REPLY_REREAD_ATTEMPTS").ok().as_deref())
}
fn reread_interval() -> Duration {
    reread_interval_from(std::env::var("PNS_REPLY_REREAD_INTERVAL").ok().as_deref())
}
/// The count, clamped. See `MAX_REREAD_ATTEMPTS`.
fn reread_attempts_from(raw: Option<&str>) -> u32 {
    raw.and_then(|raw| raw.parse().ok())
        .unwrap_or(DEFAULT_REREAD_ATTEMPTS)
        .min(MAX_REREAD_ATTEMPTS)
}
/// The interval, clamped.
///
/// `try_from_secs_f64` IS the validation, and it replaced a hand-written one
/// that looked complete: NaN, infinity and negatives were refused, but a
/// finite oversized value like `1e300` passed and panicked the constructor
/// anyway, exiting 101 on a path whose whole contract is exiting 0.
fn reread_interval_from(raw: Option<&str>) -> Duration {
    raw.and_then(|raw| raw.parse::<f64>().ok())
        .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
        .unwrap_or(DEFAULT_REREAD_INTERVAL)
        .min(MAX_REREAD_INTERVAL)
}
/// At most this much of a turn reaches the condenser or the notification.
const REPLY_MAX_CHARS: usize = 8000;
/// The last few megabytes of a transcript parse in well under a second, and
/// carry far more than one turn.
const TRANSCRIPT_TAIL_BYTES: u64 = 4_000_000;
/// Four extra reads at 150ms: enough for the harness to finish flushing,
/// short enough that a turn which really said nothing is reported promptly.
const DEFAULT_REREAD_ATTEMPTS: u32 = 4;
const DEFAULT_REREAD_INTERVAL: Duration = Duration::from_millis(150);
/// The ceilings on those two knobs. Their PRODUCT is how long a Stop hook can
/// sit re-reading a transcript that is never going to fill, so each is capped
/// rather than believed: a stray zero in either costs seconds, never hours.
const MAX_REREAD_ATTEMPTS: u32 = 10;
const MAX_REREAD_INTERVAL: Duration = Duration::from_secs(5);

#[cfg(test)]
#[path = "turn_text/tests.rs"]
mod turn_text_tests;
