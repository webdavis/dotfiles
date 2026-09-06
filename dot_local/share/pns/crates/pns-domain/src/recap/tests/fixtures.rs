//! What the recap tests build from: one entry, one clock, one window.

#![allow(unused_imports)]

use crate::missed::Entry;

/// A fixed clock, so the fixtures state a time rather than reading one.
pub(super) fn clock(at: Option<u64>) -> String {
    match at {
        Some(epoch) => format!("{:02}:{:02}", (epoch / 3600) % 24, (epoch / 60) % 60),
        None => "--:--".to_string(),
    }
}

/// One event in the window: an epoch, a state, and text naming its place.
pub(super) fn acted(at: u64, state: &str, detail: &str) -> Entry {
    Entry {
        at: Some(at),
        agent: "claude".to_string(),
        state: state.to_string(),
        project: "dotfiles".to_string(),
        branch: String::new(),
        detail: detail.to_string(),
    }
}

/// A window of `count` finished turns, each naming its own index.
pub(super) fn window(count: usize) -> Vec<Entry> {
    (0..count)
        .map(|which| {
            acted(
                1_756_500_000 + which as u64 * 60,
                "done",
                &format!("turn {which}"),
            )
        })
        .collect()
}

/// The ring's own field cap, stated here so the fixture below is the
/// widest line the engine can actually write rather than an invented one.
pub(super) const ACTIVITY_MAX_CHARS: usize = 120;
