//! The shared harness: a private HOME per test, stub channels, and the
//! engine spawned against them.
//!
//! Env NEVER goes through std::env::set_var: the test binary is threaded, so
//! a process-wide mutation would leak into whatever else is running. Every
//! variable rides on the Command instead.

#![allow(dead_code)] // each test binary uses its own subset of this harness.

mod budget;
mod daemon_guard;
mod process;
mod router;
mod sandbox;

// Each integration binary compiles this module and uses its own subset.
#[allow(unused_imports)]
pub use {
    daemon_guard::DaemonGuard,
    process::{poll_until, run, stderr, stdout},
    router::{KEYS_DISAGREE, RouterStub, router_table},
    sandbox::Sandbox,
};

use std::path::Path;

pub const ENGINE: &str = env!("CARGO_BIN_EXE_pns");
pub const CAPTURE: &str = env!("CARGO_BIN_EXE_http-capture");

/// The config every sandbox starts with: the three stub channels switched on,
/// and the mobile table naming the one backend compiled in. A test that needs
/// something else writes over it with `write_config`.
pub const STUB_CHANNELS: &str = "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
                                 [plugins.hermes]\nenabled = true\n\
                                 [plugins.macos-banner]\nenabled = true\n\
                                 [failures]\nserve = false\n";

pub fn write_script(path: &Path, body: &str) {
    std::fs::write(path, format!("#!/usr/bin/env bash\n{body}\n")).expect("write script");
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("chmod");
}

#[cfg(test)]
mod guard_tests;
