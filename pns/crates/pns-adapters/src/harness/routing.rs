/// Whether a blocking event is handed to moshi for a round trip.
///
/// Only the harnesses pns registers itself for: the name arrives from a config
/// file, so it is MATCHED rather than pasted into a subcommand handed to a
/// third-party binary.
pub fn moshi_subcommand(agent: &str) -> Option<String> {
    matches!(agent, "claude" | "codex").then(|| format!("{agent}-hook"))
}
