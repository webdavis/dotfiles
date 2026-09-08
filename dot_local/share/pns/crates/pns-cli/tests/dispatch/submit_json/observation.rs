use super::*;

#[test]
fn json_observations_banner_across_surfaces_without_phone_or_replay() {
    for (label, desk, phone, visible) in [
        ("away", "99999", "99999", false),
        ("desk-visible", "0", "99999", true),
        ("mobile-visible", "90", "0", true),
        ("desk-hidden", "0", "99999", false),
    ] {
        let sandbox = Sandbox::new(&format!("observation-{label}"));
        let mut request = request();
        request.elapsed_secs = Some(301);
        request.context.pane = Some("t1:p1".into());
        let mut command = sandbox.pns_stateful();
        command
            .env("PNS_IDLE_SECS", desk)
            .env("PNS_PHONE_INPUT_AGE", phone)
            .env("PNS_FORCE_PHONE", "1");
        sandbox.stub_herdr(&mut command, visible);
        let output = invoke_command(&sandbox, command, &request.encode().unwrap());
        assert_eq!(
            result(&output).status,
            Status::Accepted,
            "{label}: {output:?}"
        );
        assert!(
            sandbox.fired("macos-banner"),
            "{label}: observation banner missing"
        );
        assert!(sandbox.fired("hermes"), "{label}: observation log missing");
        assert!(
            !sandbox.fired("mobile"),
            "{label}: observation must not card"
        );
        let connection = database(&sandbox);
        let destinations: Vec<String> = connection
            .prepare("SELECT destination FROM ledger_legs ORDER BY position")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(destinations, ["macos-banner", "hermes"]);
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM journal", [], |row| row
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
    }
}

#[test]
fn json_progress_and_attention_keep_presence_driven_phone_cards() {
    use pns_protocol::Signal;
    for signal in [Signal::Progress, Signal::NeedsAttention] {
        for (label, idle, visible, banner, phone) in [
            ("away", "99999", false, false, true),
            ("desk-hidden", "0", false, true, false),
            ("desk-visible", "0", true, false, false),
        ] {
            let sandbox = Sandbox::new(&format!("ordinary-{signal:?}-{label}"));
            let mut request = request();
            request.signal = signal;
            request.context.pane = Some("t1:p1".into());
            let mut command = sandbox.pns_stateful();
            command.env("PNS_IDLE_SECS", idle);
            sandbox.stub_herdr(&mut command, visible);
            let output = invoke_command(&sandbox, command, &request.encode().unwrap());
            assert_eq!(
                result(&output).status,
                Status::Accepted,
                "{signal:?}/{label}: {output:?}"
            );
            assert_eq!(sandbox.fired("macos-banner"), banner, "{signal:?}/{label}");
            assert_eq!(sandbox.fired("mobile"), phone, "{signal:?}/{label}");
            assert!(sandbox.fired("hermes"));
        }
    }
}
