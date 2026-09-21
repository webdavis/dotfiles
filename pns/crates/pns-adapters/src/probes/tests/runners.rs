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

/// A runner whose only scripted call is the one `pgrep` the phone chain
/// still spawns.
pub(super) struct GateRunner {
    pub(super) release: std::sync::mpsc::Sender<()>,
}

impl CommandRunner for GateRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        match format!("{program} {}", args.join(" ")).as_str() {
            "/usr/bin/pgrep -x mosh-server" => {
                // A CONCURRENT desk thread is waiting on this; a
                // sequential, phone-only or join-at-start mutant never
                // reaches it before the registry's own deadline.
                let _ = self.release.send(());
                Some("14362\n".to_string())
            }
            _ => None,
        }
    }
}

/// The registry answering from a script, counting every read, so a test pins
/// the fail direction and the one-reading rule without the machine it runs
/// on.
pub(super) struct FakeRegistry {
    pub(super) idle_nanoseconds: Option<u64>,
    pub(super) console_locked: Option<bool>,
    pub(super) idle_reads: Arc<std::sync::atomic::AtomicU32>,
    pub(super) lock_reads: Arc<std::sync::atomic::AtomicU32>,
}

impl FakeRegistry {
    pub(super) fn answering(idle_nanoseconds: u64, console_locked: bool) -> Self {
        Self {
            idle_nanoseconds: Some(idle_nanoseconds),
            console_locked: Some(console_locked),
            idle_reads: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            lock_reads: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    /// A device that refuses both readings, which is the unreadable case.
    pub(super) fn unreadable() -> Self {
        Self {
            idle_nanoseconds: None,
            console_locked: None,
            idle_reads: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            lock_reads: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }
}

impl ConsoleRegistry for FakeRegistry {
    fn idle_nanoseconds(&self) -> Option<u64> {
        self.idle_reads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.idle_nanoseconds
    }

    fn console_locked(&self) -> Option<bool> {
        self.lock_reads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.console_locked
    }
}

/// A registry whose idle read blocks until the phone chain's `pgrep`
/// releases it, or 2 s pass: see `a_slow_probe_does_not_hold_up_a_fast_one`.
pub(super) struct GateRegistry {
    // MUTEX-WRAPPED SOLELY FOR `Sync`: an `mpsc::Receiver` is `Send` but
    // never `Sync`, and the seam needs both.
    pub(super) wait: Mutex<std::sync::mpsc::Receiver<()>>,
    pub(super) idle_nanoseconds: u64,
}

impl ConsoleRegistry for GateRegistry {
    fn idle_nanoseconds(&self) -> Option<u64> {
        self.wait
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(2))
            .ok()?;
        Some(self.idle_nanoseconds)
    }

    fn console_locked(&self) -> Option<bool> {
        Some(true)
    }
}

/// A registry whose reads never come back inside any deadline a test is
/// willing to wait for.
pub(super) struct StalledRegistry;

impl ConsoleRegistry for StalledRegistry {
    fn idle_nanoseconds(&self) -> Option<u64> {
        std::thread::sleep(Duration::from_secs(30));
        Some(0)
    }

    fn console_locked(&self) -> Option<bool> {
        std::thread::sleep(Duration::from_secs(30));
        Some(false)
    }
}

/// The process table answering a fixed terminal list, recording the parents
/// it was asked about.
pub(super) struct FakeTable {
    pub(super) terminals: Vec<String>,
    pub(super) asked: Arc<Mutex<Vec<Vec<i32>>>>,
}

impl FakeTable {
    pub(super) fn naming(terminals: &[&str]) -> Self {
        Self {
            terminals: terminals.iter().map(|name| (*name).to_string()).collect(),
            asked: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ProcessTable for FakeTable {
    fn child_terminals(&self, parents: &[i32]) -> Vec<String> {
        self.asked.lock().unwrap().push(parents.to_vec());
        self.terminals.clone()
    }
}

/// The phone twin of `StalledRegistry`.
pub(super) struct StalledTable;

impl ProcessTable for StalledTable {
    fn child_terminals(&self, _parents: &[i32]) -> Vec<String> {
        std::thread::sleep(Duration::from_secs(30));
        Vec::new()
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
