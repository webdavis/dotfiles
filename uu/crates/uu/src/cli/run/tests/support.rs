use std::cell::{Cell, RefCell};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use uu_adapters::{Config, LaneRegistration, LoadOutcome, load_config};
use uu_application::{ClockFailure, MarkerSnapshot, Notice, RunClock, RunHeader, RunPresentation};
use uu_domain::LaneReport;

static NEXT_HOME: AtomicUsize = AtomicUsize::new(0);

pub(super) struct Fixture {
    pub dir: PathBuf,
    pub started: Instant,
}

impl Fixture {
    pub fn new(name: &str) -> Self {
        let serial = NEXT_HOME.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "uu-composition-{name}-{}-{serial}",
            std::process::id()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .expect("private fixture");
        Self {
            dir,
            started: Instant::now(),
        }
    }

    pub fn home(&self) -> &str {
        self.dir.to_str().expect("fixture path")
    }

    pub fn stub(&self, body: &str) -> PathBuf {
        let path = self.dir.join("updater");
        std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}")).expect("stub");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).expect("stub mode");
        path
    }

    pub fn load(
        &self,
        text: &str,
        registrations: &[LaneRegistration],
    ) -> Result<Config, uu_adapters::ConfigError> {
        let path = self.dir.join("config.toml");
        std::fs::write(&path, text).expect("config");
        match load_config(&path, registrations)? {
            LoadOutcome::Loaded(config) => Ok(config),
            LoadOutcome::Missing => panic!("the fixture just wrote its config"),
        }
    }

    pub fn marker(&self) -> PathBuf {
        uu_adapters::marker_path(self.home())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let elapsed = self.started.elapsed();
        eprintln!("composition fixture {}: {elapsed:?}", self.dir.display());
        if !std::thread::panicking() {
            assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
        }
    }
}

pub(super) struct Clock {
    epoch: Cell<i64>,
    elapsed: Duration,
}

impl Clock {
    pub fn new(elapsed: Duration) -> Self {
        Self {
            epoch: Cell::new(1_000),
            elapsed,
        }
    }
}

impl RunClock for Clock {
    type Tick = ();
    fn epoch(&self) -> Result<i64, ClockFailure> {
        Ok(self.epoch.replace(2_000))
    }
    fn start(&self) {}
    fn elapsed(&self, _: &()) -> Duration {
        self.elapsed
    }
}

#[derive(Default)]
pub(super) struct Observed {
    pub reports: RefCell<Vec<LaneReport>>,
    pub epochs: RefCell<Vec<i64>>,
}

impl RunPresentation for &Observed {
    fn header(&self, epoch: i64, _: &MarkerSnapshot) -> RunHeader {
        self.epochs.borrow_mut().push(epoch);
        RunHeader {
            host: "fixture-host".to_string(),
            started_iso: "1970-01-01T00:16:40Z".to_string(),
            gap: "never recorded".to_string(),
        }
    }
    fn write_record(&self, _: &RunHeader, reports: &[LaneReport]) -> String {
        self.reports.replace(reports.to_vec());
        String::new()
    }
    fn notice(&self, _: Notice<'_>) {}
}
