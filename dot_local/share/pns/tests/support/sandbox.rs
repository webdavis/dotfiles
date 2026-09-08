use super::STUB_CHANNELS;
use super::budget::{TEST_BUDGET_MS, live_ceiling_ms, over_budget, over_ceiling};
use std::cell::Cell;
use std::path::PathBuf;
use std::time::Instant;

mod commands;
mod files;
mod stubs;

/// Everything one test owns: a private HOME, its stub channels, and the
/// event files those stubs record into. Removed on drop.
pub struct Sandbox {
    pub root: PathBuf,
    pub(super) created: Instant,
    excused: Cell<bool>,
}

impl Sandbox {
    /// Named for its test, so parallel tests cannot collide and a failure
    /// leaves an identifiable directory behind.
    ///
    /// IT ARRIVES WITH A CONFIG, `STUB_CHANNELS`, because a machine with NO
    /// config runs the CORE alone (`registry::CORE`) and a test that wrote
    /// nothing would be measuring that fallback rather than the routing it was
    /// written for. It is the shape the template ships: every stub channel
    /// enabled, and the mobile table naming its backend. `without_config` is
    /// how the absence itself is tested.
    pub fn new(name: &str) -> Self {
        let sandbox = Sandbox::without_config(name);
        sandbox.write_config(STUB_CHANNELS);
        sandbox
    }

    /// A sandbox with NO config file, for the one question that is about the
    /// absence of one.
    pub fn without_config(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("pns-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("channels")).expect("sandbox");
        let sandbox = Sandbox {
            root,
            created: Instant::now(),
            excused: Cell::new(false),
        };
        for channel in ["mobile", "hermes", "macos-banner"] {
            sandbox.stub_channel(
                channel,
                &format!("cat >\"{}/{channel}.event\"", sandbox.display()),
            );
        }
        sandbox
    }

    /// Excuse THIS sandbox from the ceiling because its cost is
    /// structural rather than a regression (an epoch-second lease that
    /// cannot lapse faster, say). `&self`: tests hold their sandbox
    /// immutably, so the excuse is a `Cell`. Never silences the WARNING at
    /// `TEST_BUDGET_MS`, only the failure at the ceiling: a test that KNOWS
    /// it is slow still deserves the same review flag as one that got slow
    /// by accident.
    pub fn allow_slow(&self, reason: &'static str) {
        debug_assert!(!reason.is_empty(), "allow_slow needs a real reason");
        self.excused.set(true);
    }

    /// The engine's state directory for this test, INSIDE the sandbox.
    ///
    /// Named here rather than left to `$HOME/.local/state/pns` so the daemon
    /// guard has something to assert against: a supervised loop that outlived
    /// a test and ticked over the developer's own state directory would be
    /// invisible and would keep firing.
    pub fn state(&self) -> PathBuf {
        self.path("state")
    }

    pub fn display(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
        let elapsed = self.created.elapsed().as_millis();
        if over_budget(elapsed) {
            // THE PROCESS'S OWN STDERR, not `eprintln!`: libtest captures the
            // print macros of a passing test and shows them only on failure or
            // under `--show-output`, which would swallow the one line the
            // review rule exists to print.
            use std::io::Write as _;
            let _ = writeln!(
                std::io::stderr(),
                "test budget: sandbox {:?} took {elapsed} ms, over the {TEST_BUDGET_MS} ms budget",
                self.root
            );
        }
        let ceiling = live_ceiling_ms();
        if over_ceiling(
            elapsed,
            ceiling,
            self.excused.get(),
            std::thread::panicking(),
        ) {
            panic!(
                "test budget: sandbox {:?} took {elapsed} ms, over the {ceiling} ms \
                 ceiling (call allow_slow(\"reason\") if this is structural)",
                self.root
            );
        }
    }
}
