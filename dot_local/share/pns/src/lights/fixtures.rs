//! What every lamp test builds from: the module's own items, the recorded
//! shapes, and the fixture clocks. One copy, because these rows were one test
//! module before the file outgrew the size rule.

#![allow(unused_imports)]

pub use crate::config::Behaviour;
pub use crate::lights::{
    Action, FADE_LEAD_MS, Fade, Held, HeldEntry, House, LOOP_USAGE, Loop, LoopCommand,
    MAX_MUTED_PLACES, Muted, News, Phase, QuietCommand, Resume, Say, Streak, Unread, WORKING,
    active_held, any_blocked, any_working, bare_mute_secs, blocked_marker, blocked_marker_action,
    breath_cycle, breath_fades, breathe_then_flare_cycle, last_interaction, lease_marker,
    loop_command, loop_running, muted_after, muted_entries, muted_places, muted_report, news_after,
    next_streak, parse_held_token, parse_news, parse_streak, pulse_fires, quiet_command,
    render_held_token, render_muted, render_news, render_streak, resume_from, say, shown, step_ms,
    unread_arming, workspace_agent_statuses,
};

/// herdr 0.8.2's own answer, captured live on 2026-09-01: three workspaces
/// carrying three of the four status words.
pub const HERDR_WORKSPACES: &str = r#"{"result":{"workspaces":[
  {"active_tab_id":"t1","agent_status":"working","focused":true,"workspace_id":"w1"},
  {"active_tab_id":"t4","agent_status":"idle","focused":false,"workspace_id":"w2"},
  {"active_tab_id":"t7","agent_status":"unknown","focused":false,"workspace_id":"w3"}
]}}"#;

/// The answer the suite's SHIPPED stub gives, which carries no
/// `agent_status` at all.
pub const NO_STATUS_FIELD: &str =
    r#"{"result":{"workspaces":[{"active_tab_id":"t1","focused":true,"workspace_id":"w1"}]}}"#;

pub const NOW: u64 = 10_000;

/// A run of work that started `ago` seconds before now.
pub fn streak_from(ago: u64) -> Streak {
    Streak {
        since: NOW - ago,
        last_seen: NOW,
    }
}

/// The locked blocked shape: two-second fades between 100 and 30.
pub const BLOCKED: crate::config::Breath = crate::config::Breath {
    duration_ms: 2000,
    high: 100,
    low: 30,
};

/// The locked unread shape: four-second fades between 60 and 10.
pub const SLOW: crate::config::Breath = crate::config::Breath {
    duration_ms: 4000,
    high: 60,
    low: 10,
};

/// The locked loop motion: four-second fades from 10 up to 80, with a two
/// hundred millisecond flash to 100 at the peak.
pub const LOOP_MOTION: crate::config::BreatheThenFlare = crate::config::BreatheThenFlare {
    breath: crate::config::Breath {
        duration_ms: 4000,
        high: 80,
        low: 10,
    },
    flare: 100,
    flare_ms: 200,
};

pub fn muted(entries: &[(u64, &str)]) -> Vec<Muted> {
    entries
        .iter()
        .map(|(expiry, place)| Muted {
            expiry: *expiry,
            place: (*place).to_string(),
        })
        .collect()
}
