use super::*;

// --- the loop lease -----------------------------------------------------

#[test]
fn a_lease_is_keyed_to_the_pane_it_was_typed_in_and_refused_when_there_is_none() {
    assert_eq!(
        loop_command("begin", &[], Some("wW:p21")),
        Ok(LoopCommand::Begin("wW:p21".to_string())),
        "the ordinary case takes the pane out of the environment and needs no \
         argument at all"
    );
    assert_eq!(
        loop_command("end", &[], Some("wW:p21")),
        Ok(LoopCommand::End("wW:p21".to_string())),
    );
    assert_eq!(
        loop_command(
            "begin",
            &["--pane".to_string(), "wW:p9".to_string()],
            Some("wW:p21")
        ),
        Ok(LoopCommand::Begin("wW:p9".to_string())),
        "and an explicit pane beats the environment, which is how a lease is \
         taken for a pane other than this one"
    );
    // REFUSED, NEVER GUESSED. A lease keyed to a pane whose ordinary traffic
    // will never renew it breathes for the whole timeout with nothing behind
    // it, which is the opposite of a liveness signal.
    for absent in [None, Some("")] {
        assert_eq!(
            loop_command("begin", &[], absent),
            Err(
                "pns: loop: no HERDR_PANE_ID in this environment, so there is no \
                 pane to key the lease to; run it inside the pane, or name one \
                 with --pane"
                    .to_string()
            ),
            "env pane {absent:?}"
        );
    }
}

#[test]
fn a_pane_that_cannot_name_a_file_and_an_argument_this_does_not_know_are_refused() {
    assert_eq!(
        loop_command("begin", &["--pane".to_string(), "../x".to_string()], None),
        Err("pns: loop: \"../x\" is not a pane id this can key a lease to".to_string()),
        "the path-escape guard, through the predicate that backs the filename"
    );
    assert_eq!(
        loop_command(
            "begin",
            &["--pane".to_string(), "abc.new.1".to_string()],
            None
        ),
        Err("pns: loop: \"abc.new.1\" is not a pane id this can key a lease to".to_string()),
        "the working grammar guard, through the same predicate: without it \
         this prints 'the clock cannot be read' instead of refusing the pane"
    );
    // EVERY ROAD TO A PANE, not the one the case above happens to take.
    // The guard sits between the pane is resolved and the verb is read, so
    // `end` is judged as `begin` is and the ENVIRONMENT pane is judged as
    // an explicit one. A guard moved into the `--pane` arm, or into the
    // `begin` arm, refuses nothing on the other road: `HERDR_PANE_ID` is a
    // value from another program, which is the reason the predicate exists
    // at all.
    for (verb, arguments, env_pane) in [
        (
            "end",
            vec!["--pane".to_string(), "abc.new.1".to_string()],
            None,
        ),
        ("begin", vec![], Some("abc.new.1")),
        ("end", vec![], Some("abc.sweep.7")),
    ] {
        let refused = loop_command(verb, &arguments, env_pane);
        let pane = env_pane.unwrap_or("abc.new.1");
        assert_eq!(
            refused,
            Err(format!(
                "pns: loop: {pane:?} is not a pane id this can key a lease to"
            )),
            "{verb} with arguments {arguments:?} and env pane {env_pane:?}"
        );
    }
    for arguments in [
        vec!["--pain".to_string(), "wW:p9".to_string()],
        vec!["wW:p9".to_string()],
        vec![],
    ] {
        let refused = if arguments.is_empty() {
            loop_command("resume", &arguments, Some("wW:p21"))
        } else {
            loop_command("begin", &arguments, Some("wW:p21"))
        };
        assert_eq!(
            refused,
            Err(LOOP_USAGE.to_string()),
            "arguments: {arguments:?}"
        );
    }
}
