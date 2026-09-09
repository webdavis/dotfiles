/// A whole twelve-second interval, in the milliseconds the driver budgets
/// in: the shipped refresh with nothing yet spent resolving the map.
pub(super) const FULL_INTERVAL_MS: u64 = 12_000;

/// The locked blocked shape: two-second fades between 100 and 30.
pub(super) const BLOCKED: crate::lamps::config::Breath = crate::lamps::config::Breath {
    duration_ms: 2000,
    high: 100,
    low: 30,
};

/// The locked unread shape: four-second fades between 60 and 10.
pub(super) const SLOW: crate::lamps::config::Breath = crate::lamps::config::Breath {
    duration_ms: 4000,
    high: 60,
    low: 10,
};

/// The locked loop motion: four-second fades from 10 up to 80, with a two
/// hundred millisecond flash to 100 at the peak.
pub(super) const LOOP_MOTION: crate::lamps::config::BreatheThenFlare =
    crate::lamps::config::BreatheThenFlare {
        breath: crate::lamps::config::Breath {
            duration_ms: 4000,
            high: 80,
            low: 10,
        },
        flare: 100,
        flare_ms: 200,
    };
