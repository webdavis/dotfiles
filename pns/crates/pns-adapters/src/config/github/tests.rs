use super::super::{ConfigError, parse_config, parse_github};

/// An obviously fake token. The real one lives in the vault and is never read
/// by a test.
const FAKE_TOKEN: &str = "ghp-not-a-real-token";

fn loaded(text: &str) -> Result<Option<super::GithubSource>, ConfigError> {
    let config = parse_config(text).expect("the file loads");
    parse_github(&config)
}

fn armed(extra: &str) -> String {
    format!("[plugins.github]\nenabled = true\ntoken = \"{FAKE_TOKEN}\"\n{extra}")
}

#[test]
fn an_armed_table_carries_the_token_and_the_shipped_interval() {
    let source = loaded(&armed("")).expect("it reads").expect("it is armed");
    assert_eq!(source.token, FAKE_TOKEN);
    assert_eq!(source.poll_secs, super::DEFAULT_POLL_SECS);
}

#[test]
fn an_absent_or_switched_off_table_is_inert_rather_than_a_refusal() {
    // THE MUTANT THIS PINS: a refusal on a table the operator switched off,
    // which would take the whole config down over a source nobody armed.
    for text in [
        "",
        "[plugins.github]\nenabled = false\n",
        &format!("[plugins.github]\nenabled = false\ntoken = \"{FAKE_TOKEN}\"\n"),
    ] {
        assert!(
            matches!(loaded(text), Ok(None)),
            "case {text:?} is not inert"
        );
    }
}

#[test]
fn an_armed_table_with_no_usable_token_is_refused_by_name() {
    // THE MUTANT THIS PINS: an empty token defaulted through, which the API
    // answers 401 to: the poll would then report a configuration problem this
    // file could have named itself.
    for text in [
        "[plugins.github]\nenabled = true\n",
        "[plugins.github]\nenabled = true\ntoken = \"\"\n",
        "[plugins.github]\nenabled = true\ntoken = 5\n",
    ] {
        let Err(ConfigError::Invalid(said)) = loaded(text) else {
            panic!("case {text:?} was not refused");
        };
        assert!(said.contains("token"), "{said}");
    }
}

#[test]
fn an_interval_outside_the_range_is_refused_by_name_and_the_ends_are_not() {
    for stated in ["59", "3601", "0", "-1"] {
        let Err(ConfigError::Invalid(said)) = loaded(&armed(&format!("poll_secs = {stated}\n")))
        else {
            panic!("case {stated} was not refused");
        };
        assert!(said.contains("poll_secs"), "{said}");
    }
    // The positive control: both ends are settings, not refusals.
    for stated in ["60", "3600"] {
        let source = loaded(&armed(&format!("poll_secs = {stated}\n")))
            .expect("it reads")
            .expect("it is armed");
        assert_eq!(source.poll_secs, stated.parse::<u64>().unwrap());
    }
}

#[test]
fn a_key_this_table_does_not_serve_is_refused_by_name_at_load() {
    // The schema roster is what makes `tokens` a refusal rather than a
    // setting the source silently never reads.
    let Err(ConfigError::Invalid(said)) =
        parse_config(&armed("tokens = \"another\"\n")).map(|_| ())
    else {
        panic!("a near miss was accepted");
    };
    assert!(said.contains("tokens"), "{said}");
}

#[test]
fn the_table_name_is_a_plugin_the_registry_knows() {
    // A config naming a plugin nothing registered is refused at load, so this
    // is what proves the roster entry exists: without it, an armed table
    // takes the whole file down.
    assert!(parse_config(&armed("")).is_ok());
}

#[test]
fn the_interval_the_server_asked_for_beats_the_key_and_is_held_to_its_bounds() {
    // THE MUTANT THIS PINS: the header handed to the scheduler unchecked. A
    // `1` there polls sixty times an hour against a documented floor of 60,
    // which is a request to be rate-limited, and a header past the ceiling
    // leaves the source registered, alive and silent for as long as it says.
    for (asked_for, registered) in [(0, 300), (60, 60), (1, 60), (900, 900), (36_000, 3600)] {
        assert_eq!(
            super::job_interval(300, asked_for),
            registered,
            "a server asking for {asked_for} runs the job at {registered}"
        );
    }
}

#[test]
fn a_table_with_no_webhook_secret_polls_with_the_receiver_inert() {
    // THE MUTANT THIS PINS: a receiver whose absence is a refusal, which
    // would take the whole source down over the transport that only makes it
    // faster. The poll is the floor and reads none of these keys.
    let source = loaded(&armed("")).expect("it reads").expect("it is armed");
    assert!(source.webhook.is_none());
    assert_eq!(source.poll_secs, super::DEFAULT_POLL_SECS);
    let armed_receiver = loaded(&armed("webhook_secret = \"a-secret\"\n"))
        .expect("it reads")
        .expect("it is armed");
    assert_eq!(
        armed_receiver.poll_secs,
        super::DEFAULT_POLL_SECS,
        "arming the receiver moved the poll"
    );
    let webhook = armed_receiver.webhook.expect("the receiver is armed");
    assert_eq!(webhook.secret, "a-secret");
    assert_eq!(webhook.port, super::DEFAULT_WEBHOOK_PORT);
}

#[test]
fn a_webhook_port_outside_the_range_is_refused_by_name_and_the_ends_are_not() {
    for stated in ["1023", "65536", "0", "-1"] {
        let Err(ConfigError::Invalid(said)) = loaded(&armed(&format!(
            "webhook_secret = \"a-secret\"\nwebhook_port = {stated}\n"
        ))) else {
            panic!("port {stated} was accepted");
        };
        assert!(said.contains("webhook_port"), "{said}");
    }
    for stated in ["1024", "65535"] {
        let source = loaded(&armed(&format!(
            "webhook_secret = \"a-secret\"\nwebhook_port = {stated}\n"
        )))
        .expect("it reads")
        .expect("it is armed");
        assert_eq!(
            source.webhook.expect("armed").port,
            stated.parse::<u64>().expect("a count")
        );
    }
}

#[test]
fn a_webhook_port_with_no_secret_beside_it_arms_nothing() {
    // A PORT IS NOT AN ARMING. Only the secret decides, because a receiver
    // with no secret would have nothing to verify a request with.
    let source = loaded(&armed("webhook_port = 9001\n"))
        .expect("it reads")
        .expect("it is armed");
    assert!(source.webhook.is_none());
}
