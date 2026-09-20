use super::*;
use crate::herdr::view::{parse_focused_tab, parse_layout};
use crate::macos::desk::{idle_reading_within, lock_reading_within};
use crate::macos::phone::{newest_terminal_atime, parse_pids, phone_reading_within};
use crate::macos::proc_table::{ProcessTable, plain_device_name};
use crate::macos::registry::ConsoleRegistry;
use pns_application::{IdleProbe, PhoneInputProbe, PhoneMarkerProbe, SessionViewProbe};
use pns_domain::surface::{Visibility, visibility};
use std::sync::Mutex;
use std::time::Duration;
mod runners;
use runners::*;
mod captures;
use captures::*;
mod files;
use files::*;
mod views;
use views::*;
mod desk_read;
mod herdr;
mod marker;
mod memoized;
mod phone;
mod pid_parse;
mod presence;
mod selective_start;
mod start;
mod terminal;

impl<R: CommandRunner> SystemProbes<R> {
    /// Seeds the presence reading for a test, so a suite can drive a line
    /// without a file. `cfg(test)`-ONLY and never from the environment, the
    /// same rule `with_clock` below states.
    pub fn with_presence_line(self, line: &str) -> Self {
        let _ = self.presence_line.set(Some(line.to_string()));
        self
    }

    /// Seeds the clock reading for a test, so a suite can pin "now" to an
    /// exact second instead of racing the real one.
    ///
    /// `cfg(test)`-ONLY AND NEVER FROM THE ENVIRONMENT. The four probes
    /// beside this one stay live in production no matter what a test does,
    /// because nothing here reads an override out of `std::env`: unlike the
    /// desk and phone thresholds, the wall clock has no operator-facing knob
    /// to seed it by accident.
    pub fn with_clock(self, now: u64) -> Self {
        let _ = self.now.set(Some(now));
        self
    }

    /// Points the phone chain's terminal lookup at a fixture directory
    /// instead of `TTY_DIR`, so a test's canned `ps` output can name a
    /// device the test itself created and stamped, rather than a real
    /// `/dev` entry whose presence varies by machine.
    ///
    /// `cfg(test)`-ONLY, same as `with_clock` beside it: production always
    /// takes the `TTY_DIR` default set in `new`.
    pub fn with_tty_dir(mut self, dir: String) -> Self {
        self.tty_dir = dir;
        self
    }

    /// Points the desk pair at a registry of the test's own, so a suite can
    /// drive a locked console, an unreadable property or a read that never
    /// returns without touching the machine it runs on.
    ///
    /// `cfg(test)`-ONLY, same as the seeds above: production always takes the
    /// `IoKitRegistry` default set in `new`.
    pub fn with_registry(mut self, registry: Arc<dyn ConsoleRegistry>) -> Self {
        self.registry = registry;
        self
    }

    /// The phone twin of `with_registry`.
    pub fn with_table(mut self, table: Arc<dyn ProcessTable>) -> Self {
        self.table = table;
        self
    }
}
