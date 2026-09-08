#[test]
fn the_stub_refuses_a_secret_action_that_forgot_totoml() {
    // THE MUTANT THIS PINS: a template secret line with `| toToml`
    // dropped. Chezmoi would then splice the raw vault bytes in unquoted
    // and the deployed file would not parse, but a stub that swaps ANY
    // action for a quoted placeholder would keep every template test
    // green. So the stub only stands in for the one action grammar the
    // renderer writes, and refuses the rest out loud.
    let error = super::strip_chezmoi_actions(
        "token = {{ (keepassxc \"Moshi :: Webhook Secret\").Password }}",
        |_, _| "\"from-the-vault\"".to_string(),
    )
    .expect_err("a bare action with no `| toToml` is not a secret action");
    assert!(error.contains("not a `| toToml` secret action"), "{error}");
}
