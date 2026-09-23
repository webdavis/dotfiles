//! The sandbox's fence: what a test that stubs nothing still cannot reach.
//!
//! The URLs are literals, so a fence moved onto a real host fails its twin.

mod support;

use std::process::Command;
use support::{Sandbox, plugin_command, run, stdout, write_script};

#[test]
fn a_desk_banner_from_a_test_that_stubbed_nothing_lands_in_the_sandbox() {
    let sandbox = Sandbox::new("fence-desk-banner");
    let mut command = plugin_command(&sandbox);
    command.env("PNS_SCREEN_IDLE", "0");
    run(command
        .args(["send", "--producer", "claude", "--state", "done"])
        .args(["--detail", "x", "--scope", "local_only"]));

    let recorded = std::fs::read_to_string(sandbox.path("notifier.args"))
        .expect("the desk banner reached the sandbox's own notifier");
    assert!(recorded.contains("-title"), "{recorded}");
}

/// Names the planted directory in the copy of the test below that runs with it
/// on its own PATH.
const PLANTED: &str = "PNS_FENCE_PLANTED_BIN";

/// A detached recap child can outlive its sandbox, and `Drop` takes `bin` with
/// it. Its banner must then fail to spawn rather than find a notifier anywhere
/// else.
///
/// The test runs itself again with a recording `terminal-notifier` planted
/// first on that copy's own PATH, so a sandbox that inherited the test
/// process's PATH reaches the plant on any machine, whatever is installed.
#[test]
fn with_the_sandbox_bin_gone_a_banner_reaches_no_notifier_at_all() {
    if std::env::var_os(PLANTED).is_some() {
        let sandbox = Sandbox::new("fence-bin-gone");
        std::fs::remove_dir_all(sandbox.path("bin")).expect("the sandbox bin");
        let mut command = sandbox.bare();
        command
            .env("PNS_STATE_DIR", sandbox.state())
            .env("PNS_HERMES_URL", "http://127.0.0.1:1/");
        let output = command.arg("doctor").output().expect("the engine runs");
        let printed = stdout(&output);
        assert!(
            printed.contains("banner: FAILED"),
            "the banner found a notifier outside the sandbox: {printed}"
        );
        return;
    }
    let host = Sandbox::without_config("fence-planted-notifier");
    host.allow_slow("runs this test again in a child process");
    let planted = host.path("planted");
    std::fs::create_dir_all(&planted).expect("the planted directory");
    write_script(
        &planted.join("terminal-notifier"),
        &format!("printf '%s\\n' \"$*\" >\"{}/reached\"", planted.display()),
    );
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "with_the_sandbox_bin_gone_a_banner_reaches_no_notifier_at_all",
        ])
        .env("PATH", format!("{}:/usr/bin:/bin", planted.display()))
        .env(PLANTED, &planted)
        .output()
        .expect("the copy runs");

    assert!(
        output.status.success() && stdout(&output).contains("1 passed"),
        "{output:?}"
    );
    assert!(
        !planted.join("reached").exists(),
        "the banner reached the notifier on the test process's own PATH"
    );
}

#[test]
fn a_stub_on_a_command_with_no_path_searches_only_the_sandbox_path() {
    let sandbox = Sandbox::new("fence-stub-no-path");
    let mut command = Command::new("/usr/bin/true");
    sandbox.stub_on_path(&mut command, "herdr", "exit 0");
    let path = command
        .get_envs()
        .find(|(key, _)| *key == "PATH")
        .and_then(|(_, value)| value)
        .and_then(|value| value.to_str())
        .map(str::to_owned);
    assert_eq!(
        path,
        Some(format!(
            "{}:/usr/bin:/bin:/usr/sbin:/sbin",
            sandbox.path("bin").display()
        ))
    );
}

#[test]
fn bare_points_both_moshi_urls_at_loopback_port_one() {
    let sandbox = Sandbox::new("fence-moshi-url");
    let command = sandbox.bare();
    for variable in ["PNS_MOSHI_URL", "PNS_MOSHI_UPLOAD_URL"] {
        let seen = command
            .get_envs()
            .find(|(key, _)| *key == variable)
            .and_then(|(_, value)| value)
            .and_then(|value| value.to_str());
        assert_eq!(seen, Some("http://127.0.0.1:1/"), "{variable}");
    }
}
