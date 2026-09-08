use super::FileNagRecords;
use crate::marker_files::marker_path;
use crate::nag_records::{claim_path, claim_record, nag_dir, parse, record_entries, record_path};
use pns_application::Claimed;
use pns_domain::nag::{Dropped, marker_name, session_of};

impl FileNagRecords {
    pub(super) fn claimed_records(&self) -> Vec<Claimed> {
        let mut held = Vec::new();
        for record in record_entries(&nag_dir(&self.state)) {
            // SOMEBODY ELSE OWNS IT, or it is not a regular file: either way this
            // process never opened it and never counts it.
            let Some(claim) = claim_record(&record) else {
                continue;
            };
            // A NAME THAT IS NOT A SESSION IS DROPPED, LOUDLY, AND ONLY ONCE.
            // Nothing can be resolved from it: no marker, job or card has a name
            // to be written under, so there is nothing to degrade to.
            let Some(session) = record
                .file_name()
                .and_then(|name| session_of(&name.to_string_lossy()))
            else {
                eprintln!(
                    "pns nag: {} is not named for a session this can act on; it is dropped",
                    record.display()
                );
                let _ = std::fs::remove_file(&claim);
                continue;
            };
            let parsed = std::fs::read_to_string(&claim)
                .ok()
                .as_deref()
                .and_then(parse);
            let answered = marker_name(&session)
                .is_some_and(|marker| marker_path(&self.state, &marker).exists());
            held.push(Claimed {
                session_id: session,
                record: parsed,
                answered,
            });
        }
        held
    }

    pub(super) fn retire_claim(&self, session_id: &str, reason: Option<Dropped>) {
        let Some(record) = record_path(&self.state, session_id) else {
            return;
        };
        let claim = claim_path(&record, std::process::id());
        // AN ACTION THAT SUPPRESSED ITS OWN ERROR HAS ONLY BEEN ATTEMPTED:
        // unreadable content is somebody else's write and must be named.
        if reason == Some(Dropped::Unreadable) {
            eprintln!(
                "pns nag: {} is not a record this can read; it is dropped",
                record.display()
            );
        }
        let removed = std::fs::remove_file(&claim);
        if reason.is_none()
            && let Err(error) = removed
        {
            eprintln!(
                "pns nag: the working file {} could not be removed ({error}); it is left behind",
                claim.display()
            );
        }
    }
}
