use std::num::NonZeroU32;

pub const DEFAULT_ESCALATE_AFTER_RUNS: NonZeroU32 = NonZeroU32::new(3).unwrap();

pub fn next_pending_streak(previous: u32, pending: bool, threshold: NonZeroU32) -> (u32, bool) {
    if !pending {
        return (0, false);
    }
    let next = previous.saturating_add(1);
    (next, previous < threshold.get() && next == threshold.get())
}

#[cfg(test)]
mod tests;
