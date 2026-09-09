use super::*;

#[test]
fn the_first_run_walk_refuses_a_terminal_nobody_is_at_and_writes_nothing() {
    // A WIZARD NOBODY CAN ANSWER MUST NOT GUESS. Every question has a default
    // and answering them all by default would still write a file the operator
    // never agreed to, on a machine that may be about to get a real one.
    let (sandbox, mut command) = desk_with_a_native_banner("setup-no-tty");
    std::fs::remove_file(sandbox.path(".config/pns/config.toml")).expect("the config goes");
    let output = command.arg("setup").output().expect("the engine runs");

    assert_eq!(output.status.code(), Some(2), "a refusal: {output:?}");
    assert!(
        stderr(&output).contains("not a terminal"),
        "it says why: {output:?}"
    );
    assert!(
        !sandbox.path(".config/pns/config.toml").exists(),
        "and wrote no config: {output:?}"
    );
    assert_eq!(
        sandbox.spawned(),
        "",
        "a refusal spawns nothing: {output:?}"
    );
}

#[test]
fn the_first_run_walk_refuses_a_config_that_is_already_there_and_leaves_it_alone() {
    // THE FILE IS THE OPERATOR'S. Replacing one full of plugin secrets on a
    // bare `pns setup` is unrecoverable, and the refusal names the flag that
    // does it deliberately.
    let (sandbox, mut command) = desk_with_a_native_banner("setup-exists");
    let path = sandbox.path(".config/pns/config.toml");
    let before = std::fs::read_to_string(&path).expect("the config is there");
    let output = command.arg("setup").output().expect("the engine runs");

    assert_eq!(output.status.code(), Some(2), "a refusal: {output:?}");
    assert!(
        stderr(&output).contains("--force"),
        "it names the flag: {output:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("the config is still there"),
        before,
        "the config was rewritten: {output:?}"
    );
}

#[test]
fn a_setup_typed_wrong_is_refused_with_what_it_takes_rather_than_walked_anyway() {
    // ANY OTHER WORD IS A REFUSAL: a mistyped `--force` that walked anyway
    // would ask every question and then refuse over the config it was told to
    // replace.
    let (sandbox, mut command) = desk_with_a_native_banner("setup-typo");
    let output = command
        .args(["setup", "--frce"])
        .output()
        .expect("the engine runs");

    assert_eq!(output.status.code(), Some(2), "a refusal: {output:?}");
    assert!(
        stderr(&output).contains("usage: pns setup"),
        "it says what it takes: {output:?}"
    );
    assert_eq!(
        sandbox.spawned(),
        "",
        "a refusal spawns nothing: {output:?}"
    );
}
