#[test]
fn both_narrowing_flags_plan_nothing_at_all() {
    for flags in [
        ["--local-only", "--remote-only"],
        ["--remote-only", "--local-only"],
    ] {
        let argv = flags.map(str::to_owned);
        assert_eq!(
            super::run(&argv, |_| panic!("invalid scope reached submission")),
            0
        );
    }
}
