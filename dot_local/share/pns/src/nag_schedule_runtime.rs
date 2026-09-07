use crate::*;

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
pub(crate) fn clear_nag(session_id: &str) {
    let state = state_dir();
    let (Some(record), Some(marker)) = (
        pns::nag::record_path(&state, session_id),
        pns::nag::marker_name(session_id),
    ) else {
        return;
    };
    if let Err(error) = write_marker(&state, &marker) {
        // ON STDERR AND NEVER ON STDOUT: this runs on a harness hook whose
        // output the harness reads.
        eprintln!("pns: an answered marker could not be written ({error})");
    }
    // BEST EFFORT, PRESENT OR NOT. Nothing here has to exist: the ordinary case
    // is a session that never armed, and the racing case is a record another
    // process is holding under a name this one does not know.
    let _ = std::fs::remove_file(&record);
}

/// One nudge armed for a blocked approval: the record, the marker clear, the
/// job.
///
/// EACH STEP'S FAILURE LEAVES A STATE THE NEXT FIRE RESOLVES, which is why any
/// order is safe and this one is stated: a crash after the record leaves a
/// record with no job, which the next fire enumerates and drops as stale, and a
/// failed registration leaves a record nothing will read.
///
/// EVERY FAILURE IS A LINE ON STDERR, NEVER ON STDOUT, and none of them changes
/// the exit code. Claude Code parses this hook's stdout as `let t = e.trim();
/// if (!t.startsWith("{")) return { plainText: e }`, so one stray line in front
/// of moshi's object turns an Allow into no decision at all. Bug class 19 is why
/// they are SAID rather than swallowed: the read-back here is deliberately weak,
/// so the honest move is a line naming what did not get armed.
///
/// WHAT IT COSTS THE BLOCKED PATH, BOUNDED AND MEASURED. Every step is local
/// filesystem work: one config open and TOML parse, one marker unlink, one
/// record published by write-then-rename, and one spool entry published the
/// same way. NO NETWORK, NO SUBPROCESS, NO SPAWN AND NO WAIT ON ANY OF THEM,
/// which is what makes it safe to sit in front of a notification the operator
/// is waiting on: nothing here can block on something that is not this
/// machine's own disk.
///
/// MEASURED ON DRESDEN, 500 runs of the blocked hook each way, one HOME with
/// `[nag] after_secs = 300` and one with no `[nag]` table and everything else
/// identical: 134.7ms +/- 14.1ms armed against 134.8ms +/- 13.3ms unarmed. The
/// arm is not separable from the hook's own run-to-run variation, which is the
/// bound worth stating: it is smaller than the noise of the thing it sits in.
pub(crate) fn arm_nag(session_id: &str, event: &pns::args::EventArgs) {
    // NO NAG ON CODEX, and the gate is POSITIVE rather than a `!= "codex"`, so
    // an empty or unknown `PNS_AGENT` arms nothing either (bug class 16:
    // set-but-empty is not unset). Codex wires exactly Stop and
    // PermissionRequest, so it has a turn-end clear and no batch-level one, and
    // agent turns in this repo routinely run tens of minutes: a Codex nag would
    // be wrong in the COMMON case rather than at an edge.
    if event.agent != CLAUDE_AGENT {
        return;
    }
    let after_secs = nag_after_secs();
    if after_secs == NAG_OFF {
        return;
    }
    let state = state_dir();
    let (Some(record), Some(marker), Some(id)) = (
        pns::nag::record_path(&state, session_id),
        pns::nag::marker_name(session_id),
        pns::nag::job_id(session_id),
    ) else {
        return;
    };
    // NO CLOCK IS NO ARM. A record whose `armed` nothing could read would be
    // judged stale on the first fire anyway; not writing it is the same answer
    // one step earlier.
    let Some(now) = now_secs() else {
        return;
    };
    // THE MARKER GOES FIRST, AND THE ORDER IS LOAD BEARING TWICE OVER.
    //
    // CLEARING IT AT ALL is required for correctness rather than hygiene: the
    // marker name is constant PER SESSION, so one left by the PREVIOUS approval
    // in this session would make the new job drop silently and this approval
    // would never be nudged. That is bug class 14 wearing this feature's
    // clothes, since the marker's identity is not the approval's presence.
    //
    // CLEARING IT BEFORE THE RECORD closes a window a concurrent fire can walk
    // into. Published first, the new record can be claimed by a fire that then
    // finds the PREVIOUS approval's marker still on disk and drops it as
    // answered, which costs this approval its nudge. Cleared first, the worst a
    // fire in the window can find is the previous approval's own record with no
    // marker, which is an outstanding approval being nudged about correctly.
    if let Err(error) = std::fs::remove_file(marker_path(&state, &marker))
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!(
            "pns: a previous approval's answered marker could not be cleared ({error}); \
             this approval will not be nudged"
        );
    }
    let written = publish_state_line(
        &record,
        &pns::nag::render(&pns::nag::Record {
            agent: event.agent.clone(),
            project: event.project.clone(),
            branch: event.branch.clone(),
            detail: event.detail.clone(),
            pane: event.pane.clone(),
            armed: now,
        }),
    );
    if let Err(error) = written {
        eprintln!(
            "pns: the nag record could not be written ({error}); this approval will not be nudged"
        );
        return;
    }
    let due = now.saturating_add(after_secs);
    let job = pns::daemon::Job {
        id,
        due,
        // THE LEASE IS ONE MORE SCHEDULE PAST THE DUE SECOND, which resolves to
        // the same instant as the fire-time staleness cap. The two are not
        // redundant: this drops the JOB, so a machine that slept through the
        // window never spawns at all, while the cap judges RECORDS, which is a
        // different set because a fire enumerates siblings whose own jobs have
        // not fired yet.
        until: due.saturating_add(after_secs),
        every: None,
        unless_marker: Some(marker),
        // NO FREE TEXT REACHES THE SPOOL. `args` are visible in the spool file
        // and in whatever the daemon logs, and the detail is the operator's own
        // question, so it lives in the record and `pns nag` takes no argument.
        args: vec![NAG_MODE_WORD.to_string()],
    };
    if let Err(refusal) = pns::daemon::schedule(&state, &job, now) {
        // AND THE RECORD GOES WITH IT, which is what makes the sentence true. A
        // record with no job wakes no fire of its own, but it stays ENUMERABLE:
        // a sibling approval's fire, or the operator running `pns nag` by hand,
        // counts it and cards about it. Leaving it would be this line saying
        // one thing while the state on disk said another.
        let dropped = match std::fs::remove_file(&record) {
            Ok(()) => "its record is dropped",
            Err(_) => "and its record could not be dropped either",
        };
        eprintln!(
            "pns: the nag could not be scheduled ({refusal}); this approval will not be nudged, {dropped}"
        );
    }
}

/// The one agent a nag is armed for. See `arm_nag`.
const CLAUDE_AGENT: &str = "claude";

/// The word the daemon re-executes this binary with.
const NAG_MODE_WORD: &str = "nag";
/// The state word a blocked approval and its nudge both carry.
pub(crate) const BLOCKED_STATE: &str = "blocked";

/// The schedule that means the nag is off, in the composition root's own
/// spelling of `config`'s default.
pub(crate) const NAG_OFF: u64 = 0;
/// Where one answered marker lives. The daemon owns the directory and resolves
/// the NAME inside it; this is the same resolution for the two writers that are
/// not the daemon.
pub(crate) fn marker_path(state: &Path, marker: &str) -> std::path::PathBuf {
    pns::daemon::marker_dir(state).join(marker)
}

/// One answered marker written: empty, 0600, and present is the whole message.
pub(crate) fn write_marker(state: &Path, marker: &str) -> std::io::Result<()> {
    let path = marker_path(state, marker);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(STATE_FILE_MODE)
        .open(&path)?;
    // AND AGAIN AFTER THE OPEN, for `publish_state_line`'s reason: `mode`
    // applies only when the open CREATES the file, and a marker left by an
    // earlier arm in this session is reused rather than made.
    file.set_permissions(std::fs::Permissions::from_mode(STATE_FILE_MODE))
}

/// How long an unanswered approval waits before it is carded again, or
/// `NAG_OFF`.
///
/// AN UNREADABLE CONFIG MEANS OFF, which is `focus_silence`'s reading and for
/// the same reason: a file nobody can parse asked for nothing, and a feature
/// that INTERRUPTS must not be switched on by a parse failure. This
/// deliberately differs from `[recap]`, whose fallback is on because it
/// delivers something the operator is owed.
pub(crate) fn nag_after_secs() -> u64 {
    let home = std::env::var("HOME").unwrap_or_default();
    match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config.nag_after_secs,
        _ => NAG_OFF,
    }
}
