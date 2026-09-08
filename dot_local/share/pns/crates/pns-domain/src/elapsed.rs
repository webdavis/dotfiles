use crate::EventArgs;

pub fn elapsed_event(mut event: EventArgs, seconds: u64) -> Option<EventArgs> {
    if seconds < 30 {
        return None;
    }
    event.long_running = seconds >= 300;
    event.detail = if event.detail.is_empty() {
        format!("{seconds}s")
    } else {
        format!("{} ({seconds}s)", event.detail)
    };
    Some(event)
}
