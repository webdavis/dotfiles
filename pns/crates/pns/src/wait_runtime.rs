use crate::*;
use pns_adapters::SqliteStore;

/// This session's wait on the operator, recorded where the blocked marker is
/// written: the row the return card and the stale-block escalation read, and
/// the job that wakes its fire.
///
/// THE SAME CALL AS THE MARKER, deliberately. The two are one fact stated to
/// two readers (a lamp and a page), so they are written at one seam; the row
/// is NOT gated on the lamps, because a machine with no `[lights]` table
/// writes no marker at all and an escalation built on that marker would be
/// silently dead there.
pub(crate) fn track_wait(session_id: &str, event_state: &str, window: u64, now: Option<u64>) {
    let state = state_dir();
    pns_application::track_wait(
        &SqliteStore::new(state.clone()),
        &pns_adapters::FileJobSpool::new(state),
        session_id,
        event_state,
        window,
        now,
        |warning| eprintln!("{warning}"),
    );
}

/// End this session's wait: the marker and the row, in one call.
///
/// IT SHADOWS THE ADAPTER'S OWN `end_blocked_wait` ON PURPOSE, in `clear_remind`'s
/// style. Both hook arms that end a wait directly (`prompt`, `resolved`) reach
/// this one name, so a caller cannot end half of it, and a third caller
/// arriving later gets both without knowing there were two.
///
/// `now` IS THE MOMENT BEING CLEARED FOR, handed down so the marker's End can
/// refuse a wait armed after it; the row has no such compare, because it is
/// keyed by session and rewritten by the next wait rather than raced for.
pub(crate) fn end_blocked_wait(session_id: &str, now: Option<u64>) {
    pns_adapters::marker_files::end_blocked_wait(session_id, now);
    if let Err(error) = pns_application::end_wait(&SqliteStore::new(state_dir()), session_id) {
        eprintln!("pns: state error (this session's wait could not be cleared: {error})");
    }
}

/// What one fire reads off config: how long a block stands before it is
/// escalated, and the route its page takes.
pub(crate) struct StaleSettings {
    pub window: u64,
    pub route: String,
}

/// Both of them, from ONE load: the window and the route are two answers about
/// one page, and reading the file twice is how they come back from different
/// files.
///
/// AN UNREADABLE CONFIG MEANS OFF, which is `remind_delay_secs`'s reading and
/// for its reason: a file nobody can parse asked for nothing, and a feature
/// that PAGES must not be switched on by a parse failure.
///
/// AND THE ROUTE FALLS BACK TO THE SHIPPED URGENT NAME, which is the one the
/// health kind would resolve to anyway, so the line a suppressed fire prints
/// still names somewhere rather than nothing.
pub(crate) fn stale_settings() -> StaleSettings {
    let home = std::env::var("HOME").unwrap_or_default();
    let shipped = || {
        pns_domain::routes::Routes::default()
            .urgent_route()
            .to_string()
    };
    match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => StaleSettings {
            window: config.stale_window_secs(),
            route: config
                .stale_route
                .clone()
                .unwrap_or_else(|| config.routes.urgent_route().to_string()),
        },
        _ => StaleSettings {
            window: WINDOW_OFF,
            route: shipped(),
        },
    }
}

/// The window that means the escalation is off, in the composition root's own
/// spelling of `config`'s default.
pub(crate) const WINDOW_OFF: u64 = 0;
