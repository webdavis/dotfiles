use super::*;

pub(super) fn remember_phase(
    held: &impl HeldLamps,
    held_now: &[String],
    breathing: &[Breathing],
    landings: &[(String, u8, u64)],
    now_ms: u64,
    spent_ms: u64,
) {
    // THE PHASE, WRITTEN ONLY IF THE PRE-ARM LIST IS STILL THIS TICK'S OWN. A
    // return that cleared every held lamp during the breath already emptied
    // the record; resurrecting it here with a phase would hold a lamp the
    // operator just put out. A lamp whose schedule came back empty (a budget
    // too short to fit even one fade) keeps its bare, phaseless entry.
    if bare_held(held).as_deref() == Some(held_now) {
        // WALKED OVER `breathing` AND NOT OVER THE BARE PATHS, because a phase
        // carries the STATE it belongs to and that is the one place still
        // holding it. The two lists are the same paths in the same order:
        // `held_now` is this one, mapped.
        let phased: Vec<pns_domain::lights::phase::HeldEntry> = breathing
            .iter()
            .map(|entry| {
                landings
                    .iter()
                    .find(|(landed_path, _, _)| *landed_path == entry.path)
                    // THE RESOLVE IS PART OF THE OFFSET. A landing is reported
                    // from the DRIVER's own start, which is `spent_ms` after
                    // this tick's, so a record written without that term would
                    // put every end a whole resolve early and the next tick
                    // would take the breath over before this one finished it.
                    .map(|(path, landed_on, end_relative_ms)| {
                        pns_domain::lights::phase::HeldEntry {
                            path: path.clone(),
                            resume: Some(pns_domain::lights::phase::Phase {
                                end_unix_ms: now_ms + spent_ms + end_relative_ms,
                                landed_on: *landed_on,
                                held: entry.held,
                            }),
                        }
                    })
                    .unwrap_or_else(|| {
                        pns_domain::lights::phase::HeldEntry::bare(entry.path.clone())
                    })
            })
            .collect();
        let _ = held.remember(&phased);
    }
}
