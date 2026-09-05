//! The surface and visibility model: WHICH display the operator is looking at
//! picks the notifier, and WHETHER the origin pane is visible there decides
//! suppression. Confirmed by the operator on 2026-08-11 after live testing,
//! with newest-signal-wins added 2026-08-12; the full model and its drill
//! ladder live in the drill ledger.
//!
//! Architecture fact the model rests on: herdr is a server, and ghostty
//! (desk) and moshi (phone) are both clients presenting the same session, so
//! pane visibility is ONE session-level fact; only the surface differs.

/// Where the operator's eyes are. Picks the notifier: Desk = banner,
/// Mobile = phone card, Away = phone card. A banner NEVER fires while the
/// surface is Mobile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Desk,
    Mobile,
    Away,
}

/// Whether the origin pane can be seen on whatever client shows the session.
/// Unknown never suppresses: a notification wrongly delivered costs a glance,
/// a notification wrongly suppressed is the product failing silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Hidden,
    Unknown,
}

/// One reading of the session's display state, from herdr's own CLI:
/// `workspace list` for the focused workspace's active tab, and `pane layout`
/// on the origin pane for that tab's id, focused pane and zoom.
///
/// EVERY FIELD HERE IS SESSION-GLOBAL, and building one from a caller-relative
/// answer is the bug class this type keeps inviting. `herdr pane current`
/// resolves against the CALLER'S `HERDR_PANE_ID`, and the caller is always the
/// pane the event fired from, so it reports the origin as focused no matter
/// what is on screen: the view then says Visible for the very pane that fired,
/// and every desk notification suppresses itself. Drill D4 found exactly that
/// on 2026-08-13. Anything addressed by an explicit pane id is safe; anything
/// meaning "mine" is not.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionView {
    /// The tab the ORIGIN pane belongs to.
    pub origin_tab: String,
    /// The tab the session is currently showing: the focused workspace's
    /// active tab.
    pub focused_tab: String,
    /// The focused pane inside the ORIGIN's tab, which is the pane on screen
    /// exactly when that tab is also the focused one, and that is the only
    /// case visibility consults it.
    pub focused_pane: String,
    /// Tab-level zoom: true means the focused pane fills the window and
    /// every sibling is hidden (operator-confirmed herdr semantics).
    pub zoomed: bool,
}

/// What one event should do, given surface, visibility and tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryPlan {
    pub banner: bool,
    pub phone_card: bool,
    /// The lights signal rides on top in EVERY >=300s scenario.
    pub pulse: bool,
}

/// Whether the origin pane can be seen on the client showing the session.
///
/// One session-level fact, because herdr is the server and every client
/// presents the same panes. Hidden needs PROOF: a different tab, or a zoom
/// that is covering this pane. Anything unreadable is Unknown, and Unknown
/// never suppresses.
pub fn visibility(origin: &str, view: &SessionView) -> Visibility {
    if origin.is_empty() || view.origin_tab.is_empty() || view.focused_tab.is_empty() {
        return Visibility::Unknown;
    }
    if view.origin_tab != view.focused_tab {
        return Visibility::Hidden;
    }
    // Zoom hides every sibling, so only the focused pane survives it.
    if view.zoomed && view.focused_pane != origin {
        return Visibility::Hidden;
    }
    Visibility::Visible
}

/// A reading still inside the freshness window, or nothing at all.
fn fresh_age(age: Option<u64>, fresh_secs: u64) -> Option<u64> {
    age.filter(|seconds| *seconds < fresh_secs)
}

/// Whether a reading is recent enough to speak for its surface.
///
/// ONE definition of fresh, exported so there is only ever one: the
/// arbitration below and the mobile-visibility rule beside it must not be able
/// to disagree about whether the phone was just used.
pub fn is_fresh(age: Option<u64>, fresh_secs: u64) -> bool {
    fresh_age(age, fresh_secs).is_some()
}

/// Where the operator's eyes are, by NEWEST SIGNAL WINS.
///
/// TWO CLOCKS OF THE SAME KIND, which is the whole amendment (operator
/// confirmed 2026-08-15). The desk reports when its keyboard was last touched
/// and the phone reports when the mosh client's pty was last WRITTEN TO by its
/// reader; both answer "how long since a human last did something here", so
/// the fresher one is where the operator is. A signal older than the freshness
/// window counts for nothing, which is what retired the marker's fixed TTL: a
/// signal holds its surface for as long as nothing newer contradicts it.
///
/// WHY THE PHONE NEEDED ITS OWN CLOCK. The reading it replaces was a
/// one-second sample of bytes moving over moshi, and passive viewing moves
/// almost none: drill D5(i) had the operator reading the session on the phone
/// while the sample came in under the floor, fresh desk input won on a desk
/// nobody was at, and the banner fired into an empty room. Input is what the
/// desk was always measuring, so measuring it on the phone too puts the two on
/// one comparable footing.
///
/// THE TAP AND THE PTY ARE ONE CLASS. A Back Tap is manual phone input by
/// another route, so it does not outrank the client's own clock and is not
/// outranked by it; the fresher of the two speaks for the phone, and that
/// combined reading is what meets the desk.
///
/// A missing reading is never fresh, so every unknown falls toward Away rather
/// than Desk: getting a card while at the desk costs a glance, missing one
/// while away costs the event.
///
/// A LOCKED SCREEN DISQUALIFIES THE DESK CLOCK and nothing else. That is
/// newest-signal-wins rather than an exception to it: locking necessarily
/// postdates the last desk input, because typing again means unlocking first,
/// so the lock is the newest fact about the desk. It is deliberately NOT a
/// blanket Away, because it says nothing about the phone: locking the laptop
/// and picking it up is the canonical case, and Away always cards while
/// Mobile lets a watched pane suppress.
///
/// ONLY `Some(true)` LOCKS. `Some(false)` and `None` leave every clock exactly
/// as it was, so a reading nobody could take costs one freshness window of the
/// behavior that shipped before this, where inventing a lock would kill the
/// desk banner permanently wherever the reading stops working.
pub fn surface(
    desk_input_age: Option<u64>,
    phone_input_age: Option<u64>,
    marker_age: Option<u64>,
    desk_fresh_secs: u64,
    screen_locked: Option<bool>,
) -> Surface {
    let fresh = |age: Option<u64>| fresh_age(age, desk_fresh_secs);
    // Smallest age is the most recent, and an unreadable one simply does not
    // compete: two ways of touching the phone, one verdict for the phone.
    let phone = [fresh(phone_input_age), fresh(marker_age)]
        .into_iter()
        .flatten()
        .min();
    let desk = fresh(desk_input_age).filter(|_| screen_locked != Some(true));
    match (desk, phone) {
        // The tie goes to the desk, where the operator has to be sitting for
        // the reading to exist at all.
        (Some(desk), Some(phone)) => {
            if desk <= phone {
                Surface::Desk
            } else {
                Surface::Mobile
            }
        }
        (Some(_), None) => Surface::Desk,
        (None, Some(_)) => Surface::Mobile,
        (None, None) => Surface::Away,
    }
}

/// The visibility the DELIVERY decision runs on, which is not always the one
/// the session reports.
///
/// A MOBILE SURFACE REACHED BY THE BACK TAP ALONE IS WATCHING NOTHING. Two
/// different things put the operator on mobile, and only one of them means a
/// screen is in front of them: the phone's pty clock says moshi is open and
/// taking input, while the tap says only that they reached for the phone. When
/// the tap is the fresher signal and the pty clock is not fresh at all, moshi
/// is not open in their hand, so the session is on screen nowhere they can see
/// it. The desk display showing the origin pane is showing it to an empty
/// chair.
///
/// Drill D6 caught exactly that on 2026-08-19: a Back Tap with moshi closed
/// produced NOTHING, because the session view answered Visible for a pane
/// focused on the unattended desk display and mobile-plus-visible suppresses.
/// The operator's confirmed mobile matrix has that row firing the card.
///
/// When the pty clock IS fresh the session view governs unchanged, because
/// moshi really is open and what it shows is what the operator sees. That is
/// the D5 behavior and this rule must never reach it.
pub fn effective_visibility(
    surface: Surface,
    phone_input_fresh: bool,
    session: Visibility,
) -> Visibility {
    if surface == Surface::Mobile && !phone_input_fresh {
        // Nothing is on screen for them, so nothing can suppress.
        return Visibility::Hidden;
    }
    session
}

/// What one event should do. The operator-confirmed matrix, as three rules.
///
/// The pulse rides on top of every long-running event, whatever else is
/// decided. The banner belongs to the desk alone and fires only when the
/// origin pane is not already on screen. The card belongs to the phone: always
/// when away, and on mobile unless the operator is watching the pane already,
/// where it takes the opt-in toggle to say anything at all.
pub fn plan(
    surface: Surface,
    visibility: Visibility,
    long_running: bool,
    mobile_watch_card: bool,
) -> DeliveryPlan {
    let watching = visibility == Visibility::Visible;
    DeliveryPlan {
        banner: surface == Surface::Desk && !watching,
        phone_card: match surface {
            Surface::Desk => false,
            Surface::Mobile => !watching || (long_running && mobile_watch_card),
            Surface::Away => true,
        },
        pulse: long_running,
    }
}

#[cfg(test)]
mod matrix;

#[cfg(test)]
mod rules;
