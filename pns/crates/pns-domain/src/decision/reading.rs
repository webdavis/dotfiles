use super::{DEFAULT_DESK_IDLE_SECS, EnvironmentSnapshot, Overrides, SurfaceReading};
use crate::surface::{Surface, Visibility};

pub fn surface_reading(
    snapshot: &EnvironmentSnapshot,
    overrides: &Overrides,
    now_secs: Option<u64>,
) -> SurfaceReading {
    // A garbled threshold is UNKNOWN, never the default: substituting 120
    // would read a stale desk as fresh and hold the operator at their desk.
    let desk_fresh_secs = if overrides.desk_invalid {
        None
    } else {
        Some(overrides.desk_idle_secs.unwrap_or(DEFAULT_DESK_IDLE_SECS))
    };
    let Some(desk_fresh_secs) = desk_fresh_secs else {
        // With no window to measure against, nothing can be called fresh,
        // and no reading below this point was ever taken.
        return SurfaceReading {
            surface: Surface::Away,
            phone_input_fresh: false,
            desk_input_age: None,
            phone_input_age: None,
            marker_age: None,
            screen_locked: None,
            desk_fresh_secs: None,
        };
    };

    // THE LOCK IS READ ONLY WHERE THE IDLE CLOCK ANSWERED, because its only
    // job is to disqualify what that probe reported: a desk reading the
    // caller stated, never took, or could not take leaves the lock a spawn
    // for an answer nothing can use, and the blocked path an approval waits
    // on pays that deadline serially. Nothing in this repo sets
    // `PNS_SCREEN_IDLE` in production (measured repo-wide 2026-08-28); a future
    // setter would silently disable the override with it.
    let (desk_input_age, screen_locked) = if overrides.reads_desk() {
        let idle = snapshot.idle;
        (idle, idle.and(snapshot.screen_locked))
    } else if overrides.idle_invalid {
        (None, None)
    } else {
        (overrides.idle_secs, None)
    };
    // AGES, never timestamps, and both aged against the SAME clock read: an
    // unreadable clock ages nothing, which drops a phone signal out of the
    // arbitration rather than making it infinitely fresh. A `taken_at` in
    // the future is the same kind of untrustworthy clock read, so it ages
    // nothing too, rather than saturating to age 0, the freshest reading.
    let age_of = |taken_at: Option<u64>| {
        now_secs.and_then(|now| {
            let taken_at = taken_at?;
            (taken_at <= now).then(|| now - taken_at)
        })
    };
    let phone_input_age = if overrides.reads_phone() {
        age_of(snapshot.phone_atime)
    } else if overrides.phone_invalid {
        None
    } else {
        overrides.phone_input_age
    };
    let marker_age = age_of(snapshot.marker_mtime);
    SurfaceReading {
        surface: crate::surface::surface(
            desk_input_age,
            phone_input_age,
            marker_age,
            desk_fresh_secs,
            screen_locked,
        ),
        phone_input_fresh: crate::surface::is_fresh(phone_input_age, desk_fresh_secs),
        desk_input_age,
        phone_input_age,
        marker_age,
        screen_locked,
        desk_fresh_secs: Some(desk_fresh_secs),
    }
}

pub(super) fn operator_visibility(snapshot: &EnvironmentSnapshot, pane: &str) -> Visibility {
    if pane.is_empty() {
        return Visibility::Unknown;
    }
    match snapshot.view.as_ref() {
        Some(view) => crate::surface::visibility(pane, view),
        None => Visibility::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::surface_reading;
    use crate::{EnvironmentSnapshot, Overrides};

    /// A `taken_at` one second ahead of the clock read is unknown, not the
    /// freshest possible reading.
    #[test]
    fn a_taken_at_one_second_in_the_future_ages_as_unknown() {
        let snapshot = EnvironmentSnapshot {
            marker_mtime: Some(1_000_001),
            ..EnvironmentSnapshot::default()
        };
        let reading = surface_reading(&snapshot, &Overrides::default(), Some(1_000_000));
        assert_eq!(reading.marker_age, None);
    }

    /// `taken_at` equal to `now` is the freshest real reading, age zero.
    #[test]
    fn a_taken_at_equal_to_now_ages_zero() {
        let snapshot = EnvironmentSnapshot {
            marker_mtime: Some(1_000_000),
            ..EnvironmentSnapshot::default()
        };
        let reading = surface_reading(&snapshot, &Overrides::default(), Some(1_000_000));
        assert_eq!(reading.marker_age, Some(0));
    }

    /// A `taken_at` in the past ages normally.
    #[test]
    fn a_taken_at_in_the_past_ages_by_the_difference() {
        let snapshot = EnvironmentSnapshot {
            marker_mtime: Some(999_400),
            ..EnvironmentSnapshot::default()
        };
        let reading = surface_reading(&snapshot, &Overrides::default(), Some(1_000_000));
        assert_eq!(reading.marker_age, Some(600));
    }
}
