//! `pns failures drain`: the verb that clears the legs nothing will deliver.

mod support;

use support::{Sandbox, run, run_expecting, stderr, stdout};

/// A machine with nothing given up on gets an answer rather than an error, so
/// the verb can be run blind after a bad night.
#[test]
fn draining_nothing_says_so_and_exits_zero() {
    let sandbox = Sandbox::new("failures-drain-empty");
    let output = run(sandbox.pns_stateful().args(["failures", "drain"]));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        stdout(&output).contains("pns: nothing to drain"),
        "{output:?}"
    );
}

/// A near miss is refused with the usage rather than read as an id, which is
/// what every other word this subcommand does not know gets.
#[test]
fn a_word_that_is_not_the_drain_verb_is_refused_with_the_usage() {
    let sandbox = Sandbox::new("failures-drain-typo");
    let output = run_expecting(2, sandbox.pns_stateful().args(["failures", "drainx"]));
    assert!(
        stderr(&output).contains("usage: pns failures"),
        "{output:?}"
    );
}
