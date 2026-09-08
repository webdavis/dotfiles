mod support;

use support::{Sandbox, run, stderr, stdout};

#[test]
fn home_keeps_its_diagnostic_with_extra_arguments() {
    let sandbox = Sandbox::without_config("home-extra-arguments");
    let bare = run(sandbox.pns_stateful().arg("home"));
    let extra = run(sandbox.pns_stateful().args(["home", "extra", "--unknown"]));
    assert_eq!(bare.status.code(), Some(0));
    assert!(
        stdout(&bare).starts_with("home: not configured (no config file)"),
        "{}",
        stdout(&bare)
    );
    assert_eq!(extra.status.code(), Some(0));
    assert_eq!(extra.stdout, bare.stdout);
    assert_eq!(extra.stderr, bare.stderr);
}

#[test]
fn daemon_cancel_reports_the_removed_job_and_then_its_absence() {
    let sandbox = Sandbox::new("application-cancel-job");
    let scheduled = run(sandbox.pns_stateful().args([
        "daemon", "schedule", "--id", "owned", "--in", "60", "--", "--state", "done",
    ]));
    assert_eq!(scheduled.status.code(), Some(0), "{}", stderr(&scheduled));
    assert!(sandbox.path("state/daemon/owned").is_file());
    let cancelled = run(sandbox
        .pns_stateful()
        .args(["daemon", "cancel", "--id", "owned"]));
    assert_eq!(cancelled.status.code(), Some(0));
    assert_eq!(stdout(&cancelled), "pns daemon: cancelled `owned`\n");
    assert!(!sandbox.path("state/daemon/owned").exists());
    let absent = run(sandbox
        .pns_stateful()
        .args(["daemon", "cancel", "--id", "owned"]));
    assert_eq!(absent.status.code(), Some(0));
    assert_eq!(
        stdout(&absent),
        "pns daemon: no job named `owned` was scheduled\n"
    );
}

#[test]
fn recap_posts_unreadable_wall_clocks_as_placeholders() {
    let sandbox = Sandbox::new("recap-clock-placeholder");
    sandbox.write_config("[plugins.hermes]\nenabled = true\n[recap]\ndigest_as_thread = false\n");
    std::fs::create_dir_all(sandbox.state()).unwrap();
    std::fs::write(
        sandbox.path("state/activity"),
        format!(
            "{{\"at\":{},\"agent\":\"claude\",\"state\":\"done\",\"project\":\"owned\",\"branch\":\"b\",\"detail\":\"clock fixture\"}}\n",
            i64::MAX
        ),
    ).unwrap();
    let output = run(sandbox.pns_stateful().args([
        "recap",
        "--since",
        &(i64::MAX - 1).to_string(),
        "--until",
        &i64::MAX.to_string(),
    ]));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let event = sandbox.event("hermes");
    assert_eq!(event["state"], "recap");
    let body = event["detail"].as_str().unwrap();
    assert!(body.contains("--:--"), "{body}");
    assert!(body.contains("clock fixture"), "{body}");
    assert!(body.contains("1 event"), "{body}");
}

#[test]
fn tick_clears_a_held_lamp_despite_notification_quiet_and_focus() {
    use std::net::TcpListener;
    use std::time::{Duration, Instant};
    let sandbox = Sandbox::new("tick-ignores-notification-mutes");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"owned\"\n\
         [focus]\nsilence = [\"Fixture Focus\"]\n"
    ));
    sandbox.write_focus_store("com.apple.donotdisturb.mode.fixture", "Fixture Focus");
    std::fs::create_dir_all(sandbox.state()).unwrap();
    let quiet = run(sandbox.pns_stateful().args(["quiet", "1h"]));
    assert!(stdout(&quiet).starts_with("pns: quiet for another"));
    std::fs::write(sandbox.path("state/lights-held"), "light/owned\n").unwrap();
    let mut command = sandbox.pns_stateful();
    sandbox.stub_herdr(&mut command, false);
    let mut child = command.args(["lights", "tick"]).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_millis(650);
    let mut dialled = false;
    let status = loop {
        if listener.accept().is_ok() {
            dialled = true;
        }
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("tick did not finish within the owned fixture budget");
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(status.success());
    assert!(dialled, "the held lamp was not addressed");
    assert!(!sandbox.path("state/lights-held").exists());
    assert!(!sandbox.fired("hermes") && !sandbox.fired("mobile"));
}
