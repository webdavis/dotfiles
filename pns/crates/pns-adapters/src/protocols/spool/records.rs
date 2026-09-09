use pns_application::{DaemonSpool, JobSpool, SpoolReading};
use pns_domain::jobs::Job;
use std::path::PathBuf;

pub struct FileJobSpool {
    state: PathBuf,
}

impl FileJobSpool {
    pub fn new(state: PathBuf) -> Self {
        Self { state }
    }
}

impl JobSpool for FileJobSpool {
    fn pending(&self, id: &str) -> Option<Job> {
        match super::peek(&super::spool_dir(&self.state).join(id), id) {
            super::Peeked::Job(job) => Some(*job),
            _ => None,
        }
    }
    fn schedule(&self, job: &Job, now: u64) -> Result<(), String> {
        super::schedule(&self.state, job, now)
    }
    fn cancel(&self, id: &str) -> Result<bool, String> {
        super::cancel(&self.state, id)
    }
}

impl DaemonSpool for FileJobSpool {
    type Entry = PathBuf;
    fn entries(&self) -> Vec<PathBuf> {
        super::spool_entries(&super::spool_dir(&self.state))
    }
    fn id(&self, entry: &PathBuf) -> Option<String> {
        entry
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }
    fn describe(&self, entry: &PathBuf) -> String {
        entry.display().to_string()
    }
    fn read(&self, entry: &PathBuf, id: &str) -> SpoolReading {
        match super::peek(entry, id) {
            super::Peeked::Job(job) => SpoolReading::Job(job),
            super::Peeked::Irregular => SpoolReading::Irregular,
            super::Peeked::Unusable(why) => SpoolReading::Unusable(why),
        }
    }
    fn claim(&self, entry: &PathBuf) -> Option<PathBuf> {
        super::claim(entry)
    }
    fn release(&self, claim: &PathBuf) -> Result<(), String> {
        std::fs::remove_file(claim).map_err(|error| error.to_string())
    }
    fn marker_exists(&self, job: &Job) -> bool {
        super::marker_exists(&self.state, job)
    }
    fn hand_back(&self, job: &Job) -> Result<bool, String> {
        super::hand_back(&super::spool_dir(&self.state), job).map_err(|error| error.to_string())
    }
    fn heartbeat(&self, now: u64) {
        let _ = super::publish_heartbeat(
            &self.state,
            &pns_domain::jobs::Heartbeat {
                pid: std::process::id(),
                at: now,
            },
        );
    }
}
