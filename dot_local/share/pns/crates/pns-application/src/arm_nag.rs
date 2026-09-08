use crate::{JobSpool, NagRecords};
use pns_domain::EventArgs;

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
pub struct ArmNag<'a, R, J> {
    pub records: &'a R,
    pub jobs: &'a J,
}

impl<R: NagRecords, J: JobSpool> ArmNag<'_, R, J> {
    pub fn run(
        &self,
        session_id: &str,
        event: &EventArgs,
        after_secs: impl FnOnce() -> u64,
        clock: impl FnOnce() -> Option<u64>,
        mut warn: impl FnMut(&str),
    ) {
        // NO NAG ON CODEX, and the gate is POSITIVE rather than a `!= "codex"`, so
        // an empty or unknown `PNS_AGENT` arms nothing either (bug class 16:
        // set-but-empty is not unset). Codex wires exactly Stop and
        // PermissionRequest, so it has a turn-end clear and no batch-level one, and
        // agent turns in this repo routinely run tens of minutes: a Codex nag would
        // be wrong in the COMMON case rather than at an edge.
        if event.agent != CLAUDE_AGENT {
            return;
        }
        let after_secs = after_secs();
        if after_secs == 0 {
            return;
        }
        let (Some(marker), Some(id)) = (
            pns_domain::nag::marker_name(session_id),
            pns_domain::nag::job_id(session_id),
        ) else {
            return;
        };
        // NO CLOCK IS NO ARM. A record whose `armed` nothing could read would be
        // judged stale on the first fire anyway; not writing it is the same answer
        // one step earlier.
        let Some(now) = clock() else {
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
        if let Err(error) = self.records.clear_answered(session_id) {
            warn(&format!(
                "pns: a previous approval's answered marker could not be cleared ({error}); \
             this approval will not be nudged"
            ));
        }
        let written = self.records.publish(
            session_id,
            &pns_domain::nag::Record {
                agent: event.agent.clone(),
                project: event.project.clone(),
                branch: event.branch.clone(),
                detail: event.detail.clone(),
                pane: event.pane.clone(),
                armed: now,
            },
        );
        if let Err(error) = written {
            warn(&format!(
                "pns: the nag record could not be written ({error}); this approval will not be nudged"
            ));
            return;
        }
        let due = now.saturating_add(after_secs);
        let job = pns_domain::jobs::Job {
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
        if let Err(refusal) = self.jobs.schedule(&job, now) {
            // AND THE RECORD GOES WITH IT, which is what makes the sentence true. A
            // record with no job wakes no fire of its own, but it stays ENUMERABLE:
            // a sibling approval's fire, or the operator running `pns nag` by hand,
            // counts it and cards about it. Leaving it would be this line saying
            // one thing while the state on disk said another.
            let dropped = match self.records.drop_record(session_id) {
                Ok(()) => "its record is dropped",
                Err(_) => "and its record could not be dropped either",
            };
            warn(&format!(
                "pns: the nag could not be scheduled ({refusal}); this approval will not be nudged, {dropped}"
            ));
        }
    }
}

/// The one agent a nag is armed for. See `arm_nag`.
const CLAUDE_AGENT: &str = "claude";

/// The word the daemon re-executes this binary with.
const NAG_MODE_WORD: &str = "nag";

#[cfg(test)]
mod tests;
