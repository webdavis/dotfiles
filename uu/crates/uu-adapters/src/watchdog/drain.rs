//! One of the child's pipes, drained on a thread of its own.

use std::fs::File;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
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
    error: mpsc::Receiver<String>,
}

impl Drain {
    pub(super) fn new<R: Read + Send + 'static>(pipe: Option<R>) -> Self {
        Self::read(pipe, None)
    }

    pub(super) fn to_file<R: Read + Send + 'static>(pipe: Option<R>, file: File) -> Self {
        Self::read(pipe, Some(file))
    }

    fn read<R: Read + Send + 'static>(pipe: Option<R>, mut file: Option<File>) -> Self {
        let collected = Arc::new(Mutex::new(Vec::new()));
        let into = Arc::clone(&collected);
        let (send, error) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let Some(mut pipe) = pipe else { return };
            let mut chunk = [0u8; 8192];
            let mut write_failed = false;
            loop {
                let read = match pipe.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => read,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        if file.is_some() {
                            let _ = send.send(format!("could not read archive output: {error}"));
                        }
                        break;
                    }
                };
                if let Some(file) = &mut file {
                    // Keep draining after a write failure so the child's pipe and lifetime
                    // remain bounded by the existing watchdog.
                    if !write_failed && let Err(error) = file.write_all(&chunk[..read]) {
                        write_failed = true;
                        let _ = send.send(format!("could not write archive output: {error}"));
                    }
                } else {
                    lock(&into).extend_from_slice(&chunk[..read]);
                }
            }
        });
        Drain {
            collected,
            reader,
            error,
        }
    }

    pub(super) fn error(&self) -> Option<String> {
        self.error.try_recv().ok()
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
