use super::journal_claims::{HeldJournal, claim_journal};
use crate::marker_files::{owner_is_gone, read_epoch};
use crate::{publish_state_line, state_dir};
use std::path::Path;
mod window;
use window::{Moment, claim_moment};
mod marker;
pub use marker::mark_present;
use marker::{LAST_PRESENT, advance_marker};
mod adoption;
use adoption::{StrandedWindow, stranded_window_claim, window_claim_suffix};

use pns_application::{Claim, ReturnMoment};
use std::cell::RefCell;
use std::path::PathBuf;

/// One replay owner. Dropping it preserves unfinished holds for adoption.
pub struct FileReturnMoment {
    state: PathBuf,
    held: RefCell<Vec<PathBuf>>,
}

impl FileReturnMoment {
    pub fn new(state: PathBuf) -> Self {
        Self {
            state,
            held: RefCell::new(Vec::new()),
        }
    }
}

impl ReturnMoment for FileReturnMoment {
    fn claim(&self, now: Option<u64>, take_journal: bool) -> Option<Claim> {
        if !self.held.borrow().is_empty() {
            return None;
        }
        match claim_moment(&self.state, now, take_journal) {
            Moment::Busy => None,
            Moment::Owned { since, waiting } => {
                self.held.borrow_mut().extend(waiting.holds);
                Some(Claim {
                    since,
                    waiting: waiting.entries,
                })
            }
        }
    }

    fn complete(&self) {
        // Only a completed attempt releases holds. An unlink failure leaves a
        // recoverable file; a newer journal at its ordinary path is untouched.
        for held in self.held.take() {
            let _ = std::fs::remove_file(held);
        }
    }
}

#[cfg(test)]
mod tests;
