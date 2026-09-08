/// Whether a blocking event is handed to moshi for a round trip.
///
/// Only the harnesses pns registers itself for: the name arrives from a config
/// file, so it is MATCHED rather than pasted into a subcommand handed to a
/// third-party binary.
pub fn moshi_subcommand(agent: &str) -> Option<String> {
    matches!(agent, "claude" | "codex").then(|| format!("{agent}-hook"))
}

/// Whether a subcommand handed to us by moshi's OWN generated extension may be
/// passed through to moshi-hook.
///
/// pi and omp reach the gate directly (`helperBinary pi-hook`), so the word
/// arrives from a file moshi generates while moshi-hook's positional is a
/// PATH. Shape only, not a roster: the harness list is moshi's and grows. An
/// unvetted word here is this repo handing a third-party binary a filesystem
/// argument nobody chose.
pub fn is_harness_subcommand(subcommand: &str) -> bool {
    let (name, suffix) = match subcommand.split_once('-') {
        Some(parts) => parts,
        None => return false,
    };
    suffix == "hook" && !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase())
}
