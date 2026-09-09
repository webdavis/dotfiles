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
    // `PNS_IDLE_SECS` in production (measured repo-wide 2026-08-28); a future
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
    // arbitration rather than making it infinitely fresh.
    let age_of =
        |taken_at: Option<u64>| now_secs.and_then(|now| Some(now.saturating_sub(taken_at?)));
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
