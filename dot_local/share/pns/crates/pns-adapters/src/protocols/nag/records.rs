use super::{FIRE_LOCK, nag_dir, record_entries, record_path, render};
use crate::marker_files::{marker_path, write_marker};
use pns_application::{Claimed, NagRecords};
use pns_domain::nag::{Dropped, Record, marker_name};
use std::path::PathBuf;

pub struct FileNagRecords {
    state: PathBuf,
}

impl FileNagRecords {
    pub fn new(state: PathBuf) -> Self {
        Self { state }
    }
}

impl NagRecords for FileNagRecords {
    fn claim_fire(&self, now: u64) -> bool {
        let directory = nag_dir(&self.state);
        // THE DIRECTORY BEFORE THE LOCK THAT LIVES IN IT. The arm makes this
        // directory, but an operator running the fire by hand before anything has
        // ever armed has no directory to take a lock in.
        let _ = std::fs::create_dir_all(&directory);
        super::claim_fire(&directory, now).is_some()
    }

    fn claim_due(&self, _now: u64) -> Vec<Claimed> {
        self.claimed_records()
    }

    fn drop_claim(&self, session_id: &str, reason: Option<Dropped>) {
        self.retire_claim(session_id, reason);
    }

    fn release_fire(&self) {
        super::release_fire(&nag_dir(&self.state).join(FIRE_LOCK));
    }

    fn mark_answered(&self, session_id: &str) -> Result<(), String> {
        let marker = marker_name(session_id).ok_or("invalid nag session")?;
        write_marker(&self.state, &marker).map_err(|error| error.to_string())
    }

    fn clear_answered(&self, session_id: &str) -> Result<(), String> {
        let marker = marker_name(session_id).ok_or("invalid nag session")?;
        match std::fs::remove_file(marker_path(&self.state, &marker)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    fn publish(&self, session_id: &str, record: &Record) -> Result<(), String> {
        let path = record_path(&self.state, session_id).ok_or("invalid nag session")?;
        crate::publish_state_line(&path, &render(record)).map_err(|error| error.to_string())
    }

    fn drop_record(&self, session_id: &str) -> Result<(), String> {
        let path = record_path(&self.state, session_id).ok_or("invalid nag session")?;
        std::fs::remove_file(path).map_err(|error| error.to_string())
    }

    fn clear_pending(&self) -> usize {
        record_entries(&nag_dir(&self.state))
            .iter()
            .filter(|record| std::fs::remove_file(record).is_ok())
            .count()
    }
}

mod claimed;
