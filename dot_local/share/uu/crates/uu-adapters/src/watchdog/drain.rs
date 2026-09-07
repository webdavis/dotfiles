//! One of the child's pipes, drained on a thread of its own.

use std::io::Read;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

/// One of the child's pipes, drained on a thread of its own into a buffer the
/// watchdog can take at any moment.
///
/// NEVER JOINED. The read is exactly what blocks when something the child left
/// behind still holds the pipe, so a watchdog that joined to collect the
/// output would inherit the hang it exists to bound. The group kill is what
/// closes the last write end, and the thread then ends on its own.
pub(super) struct Drain {
    collected: Arc<Mutex<Vec<u8>>>,
    reader: JoinHandle<()>,
}

impl Drain {
    pub(super) fn new<R: Read + Send + 'static>(pipe: Option<R>) -> Self {
        let collected = Arc::new(Mutex::new(Vec::new()));
        let into = Arc::clone(&collected);
        let reader = std::thread::spawn(move || {
            let Some(mut pipe) = pipe else { return };
            let mut chunk = [0u8; 8192];
            while let Ok(read) = pipe.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                lock(&into).extend_from_slice(&chunk[..read]);
            }
        });
        Drain { collected, reader }
    }

    pub(super) fn at_eof(&self) -> bool {
        self.reader.is_finished()
    }

    pub(super) fn taken(&self) -> Vec<u8> {
        lock(&self.collected).clone()
    }
}

/// The buffer, whatever a panicking reader left in it: a poisoned lock still
/// holds the output this child produced, and dropping it would cost the record
/// the only line that says how far the lane got.
fn lock(buffer: &Mutex<Vec<u8>>) -> MutexGuard<'_, Vec<u8>> {
    buffer.lock().unwrap_or_else(PoisonError::into_inner)
}
