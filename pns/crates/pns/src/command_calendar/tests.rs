use super::*;

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

#[test]
fn a_stated_client_takes_both_flags_or_neither() {
    assert_eq!(
        stated_client(&strings(&["--client-id", "an-id", "--client-secret-stdin"])),
        Ok(Some("an-id".to_string()))
    );
    assert_eq!(stated_client(&[]), Ok(None));
}

/// THE MUTANT THIS PINS: a `--client-secret <value>` flag quietly accepted,
/// which writes the secret into a command line every process can read.
#[test]
fn an_argv_secret_is_refused_by_name_and_told_where_the_secret_goes() {
    for arguments in [
        strings(&["--client-secret", "a-secret"]),
        strings(&["--client-secret=a-secret"]),
    ] {
        let complaint = stated_client(&arguments).expect_err("an argv secret is refused");
        assert!(
            complaint.contains("--client-secret-stdin") && complaint.contains("standard input"),
            "{complaint}"
        );
        assert!(!complaint.contains("a-secret"), "{complaint}");
    }
}

#[test]
fn half_a_stated_client_is_refused_rather_than_half_read() {
    for arguments in [
        strings(&["--client-id", "an-id"]),
        strings(&["--client-secret-stdin"]),
    ] {
        assert!(stated_client(&arguments).is_err(), "{arguments:?}");
    }
}

#[test]
fn a_flag_this_walk_does_not_take_is_refused_by_name() {
    for arguments in [strings(&["--client-id"]), strings(&["--force"])] {
        assert!(stated_client(&arguments).is_err(), "{arguments:?}");
    }
}

#[test]
fn a_verb_this_subcommand_does_not_have_earns_the_usage_and_exit_two() {
    for verb in ["", "mint", "--json"] {
        assert_eq!(calendar_mode(verb), 2, "{verb}");
    }
}

/// The sentence the operator reads once, and the token above it.
#[test]
fn the_minted_token_is_printed_with_the_sentence_that_says_where_it_goes() {
    let printed = minted("not-a-real-refresh-token");
    assert_eq!(
        printed.first().map(String::as_str),
        Some("not-a-real-refresh-token")
    );
    assert_eq!(
        printed.last().map(String::as_str),
        Some(
            "pns calendar consent: Store that refresh token in KeePassXC now: it is printed once, \
             pns writes it to no file, and `[quiet.calendar] refresh_token` reads it from the \
             vault."
        )
    );
}
