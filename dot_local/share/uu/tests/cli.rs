//! The run wiring, pinned end to end: exit codes, the marker policy and the
//! doctor's key hygiene are decided in main, where no unit test reaches, and a
//! mutation there (the marker written on a failed run, a lane failure leaking
//! into the exit code, a configless machine exiting non-zero) survived the
//! whole unit suite. Each test here spawns the real binary once against its
//! own scratch HOME, with the lane pointed at a stub herdr by absolute path so
//! nothing touches PATH, the network or the machine's own config.

use std::path::PathBuf;
use std::process::{Command, Output};

struct Home {
    dir: PathBuf,
}

impl Home {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("uu-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch HOME");
        Home { dir }
    }

    fn with_config(self, text: &str) -> Self {
        let config = self.dir.join(".config/uu");
        std::fs::create_dir_all(&config).expect("config dir");
        std::fs::write(config.join("config.toml"), text).expect("config file");
        self
    }

    /// A herdr that answers every call with `exit_code`, and a lane block
    /// pointing straight at it.
    fn with_herdr_lane(self, exit_code: i32) -> Self {
        self.with_herdr_lane_and(&format!("exit {exit_code}\n"), "")
    }

    /// The same lane, with `body` as the stub herdr's whole script and `extra`
    /// appended to the config file.
    fn with_herdr_lane_and(self, body: &str, extra: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let stub = self.dir.join("herdr-stub");
        std::fs::write(&stub, format!("#!/bin/sh\n{body}")).expect("stub");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let text = format!("[lanes.herdr]\nbinary = \"{}\"\n{extra}", stub.display());
        self.with_config(&text)
    }

    fn uu(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_uu"))
            .args(args)
            .env("HOME", &self.dir)
            .output()
            .expect("spawn uu")
    }

    fn marker(&self) -> PathBuf {
        self.dir.join(".local/state/uu/last-success")
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// A loopback port with nothing behind it: bound only to learn a number the
/// kernel says is free, then released. A connection there is REFUSED at once,
/// so the record path's failure arm is exercised without waiting out a
/// deadline or reaching the network.
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    listener.local_addr().expect("its number").port()
}

#[test]
fn a_machine_with_no_config_updates_nothing_and_exits_clean() {
    let home = Home::new("no-config");
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(stdout(&output).contains("no config"), "{output:?}");
    assert!(
        !home.marker().exists(),
        "a run that did nothing must not invent a success"
    );
}

#[test]
fn a_clean_run_exits_zero_and_advances_the_marker() {
    let home = Home::new("clean").with_herdr_lane(0);
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(home.marker().exists(), "{output:?}");
}

#[test]
fn a_failed_lane_leaves_the_exit_at_zero_and_the_marker_unmoved() {
    // CONTINUE ON FAILURE is the run's contract: the record is what reports
    // the failure, and the next entry's gap has to measure the last time
    // everything actually worked.
    let home = Home::new("failed").with_herdr_lane(1);
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(stdout(&output).contains("failure"), "{output:?}");
    assert!(!home.marker().exists(), "{output:?}");
}

#[test]
fn a_record_the_gateway_never_received_leaves_the_marker_unmoved() {
    // The marker is what the NEXT record measures its gap from, so a run
    // stamped successful after its entry was refused makes the following entry
    // claim a gap from a week nothing can read. The lanes themselves pass
    // here: the record path alone decides.
    let home = Home::new("record-refused").with_herdr_lane_and(
        "exit 0\n",
        &format!(
            "\n[records]\nurl = \"http://127.0.0.1:{}/uu\"\nkey = \"k\"\n",
            closed_port()
        ),
    );
    let output = home.uu(&["run"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        stdout(&output).contains("post FAILED"),
        "the refused delivery is said out loud: {}",
        stdout(&output)
    );
    assert!(
        !home.marker().exists(),
        "a run whose record never landed must not stamp a success: {output:?}"
    );
}

#[test]
fn the_doctor_never_prints_the_records_signing_key() {
    let home = Home::new("doctor").with_config("[records]\nkey = \"s3cr3t-value\"\n");
    let output = home.uu(&["doctor"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let everything = format!(
        "{}{}",
        stdout(&output),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!everything.contains("s3cr3t-value"), "{everything}");
    assert!(everything.contains("key set"), "{everything}");
}

#[test]
fn an_unknown_command_is_usage_on_stderr_and_exit_two() {
    let home = Home::new("usage");
    let output = home.uu(&["bogus"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("usage"),
        "{output:?}"
    );
}
