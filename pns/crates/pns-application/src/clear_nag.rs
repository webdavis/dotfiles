use crate::NagRecords;

/// The ONE clearing rule, and both signals go through it.
///
/// THE MARKER FIRST, THEN THE RECORD. A crash between the two leaves an
/// approval that is never nudged rather than one nudged after being answered,
/// which is the safe direction; and a marker whose write FAILED still removes
/// the record, because the record's absence already carries the same fact and
/// the marker is only what saves the daemon a no-op spawn.
///
/// THE MARKER IS WRITTEN WHETHER OR NOT A RECORD IS THERE, and that is a
/// correctness requirement rather than a simplification. The fire owns a record
/// by RENAMING it out of its own name, so between that rename and the fire's
/// marker check there is no `.pending` file for the session at all; a clear
/// gated on the record's presence does nothing in that window and the fire
/// cards an approval that has just been dealt with. The marker is the only
/// signal that reaches a record somebody else is holding.
///
/// WHAT THAT COSTS, NAMED: one marker file per session that ever resolves a
/// tool batch or ends a turn, rather than one per session that armed a nag.
/// They are empty, they are 0600, and one session writes one (the name is
/// constant per session, so a second batch rewrites the same file). That is the
/// accumulation the turn-start markers have carried since the turn clock
/// shipped, and it is accepted on the same terms (Risks 6, and the
/// no-removal-mechanisms ruling).
///
/// IT DOES NOT SILENCE A LATER APPROVAL. The arm clears this session's marker
/// BEFORE it publishes the new record, so a marker left by a batch that
/// resolved long ago cannot make the next approval's job drop.
///
/// NO COMMENT HERE MAY SAY THE MARKER RECORDS THE OPERATOR'S ANSWER. It records
/// the BATCH'S RESOLUTION, which is the only per-batch fact the harness's hook
/// vocabulary carries: an approval answered at ten seconds whose tool then runs
/// past the schedule is nudged about anyway. That cost is named in the template
/// rather than papered over here.
pub fn clear_nag<R: NagRecords>(records: &R, session_id: &str, mut warn: impl FnMut(&str)) {
    if pns_domain::nag::usable(session_id).is_none() {
        return;
    }
    if let Err(error) = records.mark_answered(session_id) {
        // ON STDERR AND NEVER ON STDOUT: this runs on a harness hook whose
        // output the harness reads.
        warn(&format!(
            "pns: an answered marker could not be written ({error})"
        ));
    }
    // BEST EFFORT, PRESENT OR NOT. Nothing here has to exist: the ordinary case
    // is a session that never armed, and the racing case is a record another
    // process is holding under a name this one does not know.
    let _ = records.drop_record(session_id);
}
