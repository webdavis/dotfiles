use super::*;

#[test]
fn the_doctor_sends_its_labelled_payload_to_every_enabled_channel_and_reports_each_one() {
    let sandbox = Sandbox::new("doctor-sends");
    // The sensor is switched ON here and the lights are not, so the one report
    // carries both skip reasons: a plugin that cannot be a destination, and a
    // plugin the config declined.
    sandbox.write_config(&format!(
        "[plugins.router]\nenabled = true\n{EVERY_DISPATCHED_CHANNEL}"
    ));
    let output = doctor_command(&sandbox).output().expect("the engine runs");

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    for channel in ["mobile", "macos-banner", "hermes"] {
        let event = sandbox.event(channel);
        assert_eq!(event["agent"], "pns", "channel: {channel}");
        assert_eq!(event["state"], "doctor", "channel: {channel}");
        assert_eq!(
            event["detail"], "test send from pns doctor; nothing is wrong and nothing needs doing",
            "the payload says at once that nothing is wrong: channel {channel}"
        );
        assert_eq!(event["title"], "pns · doctor", "channel: {channel}");
        assert_eq!(
            event["pane"], "",
            "the doctor carries no pane, because no test can watch a click land"
        );
        assert_eq!(
            event["mode"], "sync",
            "the operator is standing here waiting for the answer: channel {channel}"
        );
    }
    assert!(
        !stderr(&output).contains("dropped a pane id"),
        "the doctor hands over no pane, so it has none to scrub: {}",
        stderr(&output)
    );

    let reported = stdout(&output);
    let printed: Vec<&str> = reported.lines().collect();
    assert_eq!(printed[0], DOCTOR_OPENING);
    assert_eq!(
        &printed[1..],
        [
            "router: skipped, a sensor and never a delivery destination",
            "presence: skipped, not enabled in the config",
            "mobile: sent, this channel reports no outcome",
            "macos-banner: sent, this channel reports no outcome",
            "hermes: sent, this channel reports no outcome",
            "hue: skipped, not enabled in the config",
            "pns doctor: 3 sent, 0 failed, 3 skipped",
            NO_MOSHI_HOOK_LINE,
            FOCUS_OFF_LINE,
            DAEMON_NEVER_RAN_LINE,
            NAG_OFF_LINE,
            LIGHTS_OFF_LINE,
            "pns doctor: delivery ledger unreadable; backlog and deadletters unknown",
            NO_DECISION_RECORDED,
            NONE_WAITING,
        ],
        "one line per REGISTERED plugin, in registration order: a report that \
         walked the selection would answer what is on when the operator asked \
         what will reach them"
    );
}

#[test]
fn a_mobile_table_naming_no_compiled_in_backend_pushes_no_card_through_either_seam() {
    // "NO CARD IS PUSHED" IS PRINTED, so it has to be true wherever the leg is
    // dispatched. The gate used to sit on the TOKEN, which only feeds the
    // native channel: with an executable channel of the same name installed,
    // the card went out under a backend nobody named while stderr said it had
    // not.
    let sandbox = Sandbox::new("mobile-type-refused-leg");
    sandbox.write_config(
        "[plugins.mobile]\nenabled = true\ntype = \"pushover\"\ntoken = \"tok-real\"\n\
         [plugins.hermes]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n",
    );
    let output = run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "done"])
        .args(["--project", "dotfiles", "--detail", "a summary"]));

    assert!(
        !sandbox.fired("mobile"),
        "the card went out under a backend nobody named: {:?}",
        sandbox.event("mobile")
    );
    assert!(
        sandbox.fired("hermes"),
        "and one refused table costs no sibling its leg: {}",
        stderr(&output)
    );
    let said = stderr(&output);
    assert_eq!(
        said.matches("no card is pushed").count(),
        1,
        "one fault, one complaint: {said}"
    );
    assert!(said.contains("\"pushover\""), "quoting the type: {said}");
}

#[test]
fn the_doctor_names_the_type_when_the_type_is_the_fault_and_never_the_token() {
    // ONE FAULT, ONE LINE, AND IT NAMES THE RIGHT KEY. The complaint used to
    // be consumed at the read and the value collapsed into `None`, which is
    // indistinguishable from a missing token by the time the leg runs: the
    // operator with a perfectly good token in the file was sent to `token`.
    let sandbox = Sandbox::new("doctor-type-fault");
    sandbox.write_config(
        "[plugins.mobile]\nenabled = true\ntype = \"pushover\"\ntoken = \"tok-real\"\n\
         [plugins.macos-banner]\nenabled = true\n[plugins.hermes]\nenabled = true\n",
    );
    let output = doctor_command(&sandbox).output().expect("the engine runs");

    let reported = stdout(&output);
    let mobile = reported
        .lines()
        .find(|line| line.starts_with("mobile:"))
        .unwrap_or_else(|| panic!("the census names every plugin: {reported}"));
    assert!(
        mobile.starts_with("mobile: FAILED,"),
        "a card that was never pushed is not a send: {mobile}"
    );
    assert!(
        mobile.contains("\"pushover\"") && mobile.contains("type"),
        "the line names the key that is wrong: {mobile}"
    );
    assert!(
        !mobile.contains("token"),
        "and never the key that is right: {mobile}"
    );
    assert_eq!(
        reported
            .lines()
            .filter(|line| line.starts_with("mobile:"))
            .count(),
        1,
        "one plugin, one line: {reported}"
    );
    assert_eq!(output.status.code(), Some(1), "a failed leg is a failure");
}

#[test]
fn the_doctor_tells_a_machine_with_no_config_that_there_is_no_config() {
    // "NOT ENABLED IN THE CONFIG" POINTS AT A FILE THAT DOES NOT EXIST. It was
    // unreachable in this state until the fallback narrowed to the core; it is
    // now the ordinary report on a fresh machine, and it sends the operator to
    // edit nothing.
    let sandbox = Sandbox::without_config("doctor-no-config");
    let output = doctor_command(&sandbox).output().expect("the engine runs");

    let reported = stdout(&output);
    for plugin in ["router", "hermes", "hue"] {
        let line = reported
            .lines()
            .find(|line| line.starts_with(&format!("{plugin}:")))
            .unwrap_or_else(|| panic!("the census names every plugin: {reported}"));
        assert_eq!(
            line,
            format!("{plugin}: skipped, no config file, so only the core runs"),
            "the skip reason has to be true of this machine"
        );
    }
}

#[test]
fn the_doctor_says_a_switched_off_table_names_no_backend_and_an_event_never_does() {
    // A DISABLED TABLE IS INERT (operator ruling 2026-08-31): nothing on the
    // event path refuses it, because complaining about a channel the operator
    // switched off on every event is noise. It is still a misconfiguration
    // waiting for the moment the switch flips, so the DIAGNOSTIC says it,
    // which is where diagnostics belong.
    let sandbox = Sandbox::new("disabled-table-type");
    sandbox.write_config(&format!(
        "[plugins.router]\nenabled = false\ntype = \"asus\"\n{EVERY_DISPATCHED_CHANNEL}"
    ));

    let checked = doctor_command(&sandbox).output().expect("the engine runs");
    assert!(
        stderr(&checked).contains("[plugins.router]") && stderr(&checked).contains("switched off"),
        "the doctor is where a switched-off misconfiguration is visible: {}",
        stderr(&checked)
    );

    let fired = run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "done"])
        .args(["--project", "dotfiles", "--detail", "a summary"]));
    assert!(
        !stderr(&fired).contains("switched off"),
        "and the event path stays silent about a table nobody switched on: {}",
        stderr(&fired)
    );
}
