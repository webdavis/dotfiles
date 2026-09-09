use super::*;
#[test]
fn the_gate_vouches_for_the_shape_of_a_subcommand_it_did_not_choose() {
    assert!(super::is_harness_subcommand("pi-hook"));
    assert!(super::is_harness_subcommand("claude-hook"));
    assert!(!super::is_harness_subcommand("hook"));
    assert!(!super::is_harness_subcommand("-hook"));
    assert!(!super::is_harness_subcommand("Pi-hook"));
    assert!(!super::is_harness_subcommand("pi-hook; rm -rf /"));
    assert!(!super::is_harness_subcommand("../../etc/passwd"));
    assert!(!super::is_harness_subcommand(""));
}

#[test]
fn only_the_harnesses_pns_registers_for_are_forwarded_to_moshi() {
    assert_eq!(moshi_subcommand("claude").as_deref(), Some("claude-hook"));
    assert_eq!(moshi_subcommand("codex").as_deref(), Some("codex-hook"));
    assert_eq!(moshi_subcommand("pi"), None);
    assert_eq!(moshi_subcommand(""), None);
    assert_eq!(moshi_subcommand("claude; rm -rf /"), None);
}
