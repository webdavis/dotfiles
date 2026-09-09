use super::*;

/// Records what it was asked to run and answers from a script, so a test
/// pins both the parsing and the exact argv a probe uses.
pub(super) struct FakeRunner {
    pub(super) answers: Vec<(String, Option<String>)>,
    pub(super) calls: Mutex<Vec<String>>,
}

impl FakeRunner {
    pub(super) fn answering(answer: &str) -> Self {
        // The empty key matches every program, so one answer serves any
        // single-command probe.
        Self {
            answers: vec![(String::new(), Some(answer.to_string()))],
            calls: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn failing() -> Self {
        Self {
            answers: Vec::new(),
            calls: Mutex::new(Vec::new()),
        }
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{program} {}", args.join(" ")));
        self.answers
            .iter()
            .find(|(key, _)| program.contains(key.as_str()))
            .and_then(|(_, answer)| answer.clone())
    }
}

// --- one reading per probe set ------------------------------------------

/// Counts what it was asked to run, and keeps the counter reachable after
/// the runner is handed to the probe set that owns it.
///
/// `Arc<AtomicU32>`, not `Rc<Cell<u32>>`: `start` hands a clone of this
/// runner to a spawned thread, which needs `Send + Sync`, and neither
/// `Rc` nor `Cell` is either.
pub(super) struct CountingRunner {
    pub(super) answer: String,
    pub(super) calls: Arc<std::sync::atomic::AtomicU32>,
}

impl CommandRunner for CountingRunner {
    fn run(&self, _program: &str, _args: &[&str]) -> Option<String> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Some(self.answer.clone())
    }
}

/// A runner whose `ioreg -c IOHIDSystem` blocks until the phone chain's
/// first `pgrep` releases it, or 2 s pass: see
/// `a_slow_probe_does_not_hold_up_a_fast_one` (C4).
pub(super) struct GateRunner {
    pub(super) release: std::sync::mpsc::Sender<()>,
    // MUTEX-WRAPPED SOLELY FOR `Sync`: an `mpsc::Receiver` is `Send` but
    // never `Sync` (it has exactly one consumer by design), and `Arc<R>`
    // needs `R: Sync` to cross into the spawned thread. Only the desk
    // thread ever locks this, once.
    pub(super) wait: Mutex<std::sync::mpsc::Receiver<()>>,
    pub(super) idle_answer: String,
}

impl CommandRunner for GateRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        let call = format!("{program} {}", args.join(" "));
        match call.as_str() {
            "/usr/sbin/ioreg -c IOHIDSystem" => {
                // A CONCURRENT phone thread's own `pgrep` releases this;
                // a sequential, desk-only, or join-at-start mutant never
                // reaches that `pgrep` before this deadline. GREEN NEVER
                // WAITS ON IT, so it is sized for a starved test thread on
                // a loaded runner rather than for a fast red.
                self.wait
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(2))
                    .ok()?;
                Some(self.idle_answer.clone())
            }
            "/usr/bin/pgrep -x mosh-server" => {
                let _ = self.release.send(());
                Some("14362\n".to_string())
            }
            "/usr/sbin/ioreg -n Root -d1" => Some(ROOT_LOCKED.to_string()),
            "/usr/bin/pgrep -P 14362" => Some("14363\n".to_string()),
            "/bin/ps -o tty= -p 14363" => Some("ttys000 \n".to_string()),
            _ => None,
        }
    }
}

/// Answers exact argv and records every call, so an unscripted call reads
/// as that herdr subcommand failing rather than as a silent default.
pub(super) struct ExactArgvRunner {
    pub(super) answers: Vec<(String, String)>,
    pub(super) calls: Mutex<Vec<String>>,
}

impl CommandRunner for ExactArgvRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        let call = format!("{program} {}", args.join(" "));
        self.calls.lock().unwrap().push(call.clone());
        self.answers
            .iter()
            .find(|(scripted, _)| *scripted == call)
            .map(|(_, answer)| answer.clone())
    }
}
