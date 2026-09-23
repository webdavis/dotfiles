//! The sandbox's fence: what a test that stubs nothing still cannot reach.
//!
//! The URLs are literals, so a fence moved onto a real host fails its twin.

mod support;

use support::{Sandbox, plugin_command, run};

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
