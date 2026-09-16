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
