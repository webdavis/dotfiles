//! What every lamp test builds from: the module's own items, the recorded
//! shapes, and the fixture clocks. One copy, because these rows were one test
//! module before the file outgrew the size rule.

#![allow(unused_imports)]

pub use pns_adapters::workspace_agent_statuses;
pub use pns_domain::lamps::config::Behaviour;
pub use pns_domain::lights::breath::{
    FADE_LEAD_MS, Fade, Resume, breath_cycle, breath_fades, breathe_then_flare_cycle, step_ms,
};
pub use pns_domain::lights::held::{Held, House, active_held, any_blocked, pulse_fires, shown};
pub use pns_domain::lights::looping::{Loop, loop_running};
pub use pns_domain::lights::mute::{
    MAX_MUTED_PLACES, Muted, QuietCommand, bare_mute_secs, muted_after, muted_places, muted_report,
};
pub use pns_domain::lights::phase::{
    Action, HeldEntry, Phase, Say, blocked_marker_action, resume_from, say,
};
pub use pns_domain::lights::streak::{Streak, WORKING, any_working, next_streak};
pub use pns_domain::lights::unread::{News, Unread, last_interaction, news_after, unread_arming};

pub const NOW: u64 = 10_000;

/// A run of work that started `ago` seconds before now.
pub fn streak_from(ago: u64) -> Streak {
    Streak {
        since: NOW - ago,
        last_seen: NOW,
    }
}

/// The locked blocked shape: two-second fades between 100 and 30.
pub const BLOCKED: pns_domain::lamps::config::Breath = pns_domain::lamps::config::Breath {
    duration_ms: 2000,
    high: 100,
    low: 30,
};

/// The locked unread shape: four-second fades between 60 and 10.
pub const SLOW: pns_domain::lamps::config::Breath = pns_domain::lamps::config::Breath {
    duration_ms: 4000,
    high: 60,
    low: 10,
};

/// The locked loop motion: four-second fades from 10 up to 80, with a two
/// hundred millisecond flash to 100 at the peak.
pub const LOOP_MOTION: pns_domain::lamps::config::BreatheThenFlare =
    pns_domain::lamps::config::BreatheThenFlare {
        breath: pns_domain::lamps::config::Breath {
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

pub use pns_adapters::lights_codec::{
    muted_entries, parse_held_token, parse_news, parse_streak, render_held_token, render_muted,
    render_news, render_streak,
};

pub use pns_adapters::marker_files::{
    blocked_dir, blocked_marker, lease_dir, lease_marker, sweep_claim,
};
