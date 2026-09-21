//! The composition root's read of the active profile: config, store and
//! probes in, one resolved answer out.
//!
//! EVERY PROBE IS READ ONCE HERE and handed to the resolver as a plain value,
//! which is what keeps the resolver pure and makes the whole rule matrix
//! testable with no daemon.

use crate::*;
use pns_adapters::SqliteStore;
use pns_domain::profiles::{Inputs, Profile, Resolved, resolve};

/// What one read answered: the resolution, the profile it names, and the
/// local clock time a bounded override ends at.
pub(crate) struct Reading {
    pub resolved: Resolved,
    pub profile: Profile,
    pub until_clock: Option<String>,
}

pub(crate) fn active(records: &SqliteStore) -> Reading {
    let config = loaded_config();
    let now = now_secs();
    let standing = records.profile_override().ok().flatten();
    let resolved = resolve(&config.profile_rules, &inputs(now), standing.as_ref(), now);
    // A NAME NO TABLE DEFINES RUNS THE DEFAULT PROFILE rather than nothing: a
    // stored override can outlive the table it named, and the fail-open rule
    // is that not knowing costs the narrowing, never the notification.
    let profile = config
        .profiles
        .get(&resolved.profile)
        .cloned()
        .unwrap_or_default();
    let until_clock = match &resolved.chose {
        pns_domain::profiles::Chose::Manual { until: Some(until) } => {
            pns_adapters::local_minutes_since_midnight(*until).map(clock)
        }
        _ => None,
    };
    Reading {
        resolved,
        profile,
        until_clock,
    }
}

/// Every profile the config defines, in sorted order, which is what a refusal
/// lists back at an operator who typed a name it does not serve.
pub(crate) fn defined_profiles() -> Vec<String> {
    loaded_config().profiles.keys().cloned().collect()
}

pub(crate) fn minutes_now() -> Option<u16> {
    now_secs().and_then(pns_adapters::local_minutes_since_midnight)
}

/// The five inputs, each read once. An unreadable one stays `None`, which
/// matches no rule that names it.
///
/// `location` AND `focus` ARE FILLED IN THE NEXT SLICE and are `None` here,
/// so a rule naming either does not match yet. `calendar_busy` is read as
/// `None` until the calendar poll supplies it.
fn inputs(now: Option<u64>) -> Inputs {
    Inputs {
        weekday: now
            .and_then(pns_adapters::local_civil)
            .map(|(_, weekday)| weekday),
        minutes_of_day: now.and_then(pns_adapters::local_minutes_since_midnight),
        location: None,
        focus: None,
        calendar_busy: None,
    }
}

/// A config that will not load names no profile and no rule, which resolves
/// `default` and runs `Profile::default()`: today's behaviour.
fn loaded_config() -> Box<pns_adapters::Config> {
    let home = std::env::var("HOME").unwrap_or_default();
    match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config,
        _ => Box::default(),
    }
}

/// Minutes since local midnight as `HH:MM`.
fn clock(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}
