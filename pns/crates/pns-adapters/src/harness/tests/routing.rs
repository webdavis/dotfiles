use super::*;
#[test]
fn only_the_harnesses_pns_registers_for_are_forwarded_to_moshi() {
    assert_eq!(moshi_subcommand("claude").as_deref(), Some("claude-hook"));
    assert_eq!(moshi_subcommand("codex").as_deref(), Some("codex-hook"));
    assert_eq!(moshi_subcommand("pi"), None);
    assert_eq!(moshi_subcommand(""), None);
    assert_eq!(moshi_subcommand("claude; rm -rf /"), None);
}
