//! pns's CLI contract, ported verbatim: lenient, warning, never fatal.
//!
//! The engine sits on an always-exit-0 notification path, so argument
//! problems WARN and degrade rather than abort. Three rules carry the
//! contract: a value-taking flag whose next token is missing or is itself a
//! RECOGNIZED flag is warned about and ignored WITHOUT consuming that token
//! (consuming it would silently drop the real flag, e.g. leak an event a
//! caller narrowed with `--pane --local-only`); an unrecognized next token
//! IS taken as the value, the leniency the bash deliberately retained; and
//! any other unknown argument is skipped silently.

/// The parsed event arguments. Every field defaults to empty or false, so a
/// bare invocation is valid and renders an empty event.
#[derive(Debug, Default, PartialEq)]
pub struct EventArgs {
    pub agent: String,
    pub state: String,
    pub project: String,
    pub branch: String,
    pub detail: String,
    pub pane: String,
    /// The named hermes route this event posts to, resolved through the
    /// config's `[plugins.hermes]` channels table; empty means the default
    /// (alert) route. Names, not URLs: the caller says WHERE, the config
    /// says HOW to get there.
    pub channel: String,
    pub local_only: bool,
    pub remote_only: bool,
    /// The >=300s tier: the lights signal rides on top of whatever else the
    /// plan decides.
    pub long_running: bool,
}

/// Parse argv (without the program name). Returns the arguments plus the
/// warnings to print to stderr, one per ignored flag.
pub fn parse_args<I>(argv: I) -> (EventArgs, Vec<String>)
where
    I: IntoIterator<Item = String>,
{
    const VALUE_FLAGS: [&str; 7] = [
        "--agent",
        "--state",
        "--project",
        "--branch",
        "--detail",
        "--pane",
        "--channel",
    ];
    // Every flag that takes no value. It is a LIST rather than a chain of
    // comparisons because the chain is what went stale: `--long-running` was
    // handled below and never added here, so a value flag in front of it ate
    // it as its value and the tier vanished without a warning.
    const BARE_FLAGS: [&str; 3] = ["--long-running", "--local-only", "--remote-only"];
    let recognized = |token: &str| VALUE_FLAGS.contains(&token) || BARE_FLAGS.contains(&token);

    let mut parsed = EventArgs::default();
    let mut warnings = Vec::new();
    let mut tokens = argv.into_iter().peekable();
    while let Some(token) = tokens.next() {
        match token.as_str() {
            "--long-running" => parsed.long_running = true,
            "--local-only" => parsed.local_only = true,
            "--remote-only" => parsed.remote_only = true,
            flag if VALUE_FLAGS.contains(&flag) => {
                // Missing, or a recognized flag standing where the value
                // should be: warn and leave the token for its own arm.
                if tokens.peek().is_none_or(|next| recognized(next)) {
                    warnings.push(format!("{flag} given without a value; ignoring"));
                    continue;
                }
                let Some(value) = tokens.next() else { continue };
                match flag {
                    "--agent" => parsed.agent = value,
                    "--state" => parsed.state = value,
                    "--project" => parsed.project = value,
                    "--branch" => parsed.branch = value,
                    "--detail" => parsed.detail = value,
                    "--channel" => parsed.channel = value,
                    _ => parsed.pane = value,
                }
            }
            _ => {}
        }
    }
    (parsed, warnings)
}

#[cfg(test)]
mod tests {
    use super::parse_args;

    fn args(tokens: &[&str]) -> (super::EventArgs, Vec<String>) {
        parse_args(tokens.iter().map(|t| t.to_string()))
    }

    #[test]
    fn every_value_flag_lands_in_its_field() {
        let (parsed, warnings) = args(&[
            "--agent",
            "claude",
            "--state",
            "done",
            "--project",
            "dotfiles",
            "--branch",
            "main",
            "--detail",
            "a summary",
            "--pane",
            "wW:p21",
            "--local-only",
        ]);
        assert_eq!(parsed.agent, "claude");
        assert_eq!(parsed.state, "done");
        assert_eq!(parsed.project, "dotfiles");
        assert_eq!(parsed.branch, "main");
        assert_eq!(parsed.detail, "a summary");
        assert_eq!(parsed.pane, "wW:p21");
        assert!(parsed.local_only);
        assert!(!parsed.remote_only);
        assert!(warnings.is_empty());
    }

    #[test]
    fn the_channel_flag_names_a_route_and_is_protected_like_every_value_flag() {
        let (parsed, warnings) = args(&["--channel", "log", "--agent", "brew"]);
        assert_eq!(parsed.channel, "log");
        assert_eq!(parsed.agent, "brew");
        assert!(warnings.is_empty());
        // And it is never eaten as another flag's value.
        let (parsed, warnings) = args(&["--detail", "--channel", "log"]);
        assert_eq!(parsed.detail, "");
        assert_eq!(parsed.channel, "log");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn a_recognized_flag_is_never_consumed_as_a_value() {
        // `--pane --local-only`: eating the narrowing flag as the pane value
        // would deliver an event the caller asked to keep local.
        let (parsed, warnings) = args(&["--pane", "--local-only", "--agent", "claude"]);
        assert_eq!(parsed.pane, "");
        assert!(parsed.local_only, "the narrowing flag must still apply");
        assert_eq!(parsed.agent, "claude");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("--pane"), "the warning names the flag");
    }

    #[test]
    fn the_long_running_flag_is_protected_from_being_eaten_like_every_other_one() {
        // It was handled but left out of the predicate, so `--detail
        // --long-running` swallowed it as the detail text: the notification
        // carried a flag name as its summary AND lost the tier that decides
        // the lights, both in silence.
        let (parsed, warnings) = args(&["--detail", "--long-running"]);
        assert_eq!(parsed.detail, "");
        assert!(parsed.long_running, "the tier must still apply");
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].contains("--detail"),
            "the warning names the flag"
        );
    }

    #[test]
    fn a_trailing_value_flag_is_warned_and_ignored() {
        let (parsed, warnings) = args(&["--agent", "claude", "--detail"]);
        assert_eq!(parsed.agent, "claude");
        assert_eq!(parsed.detail, "");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("--detail"));
    }

    #[test]
    fn an_unrecognized_token_is_still_taken_as_a_value() {
        // The bash deliberately kept this leniency: only RECOGNIZED flags are
        // protected from being eaten.
        let (parsed, warnings) = args(&["--agent", "--bogus"]);
        assert_eq!(parsed.agent, "--bogus");
        assert!(warnings.is_empty());
    }

    #[test]
    fn unknown_arguments_are_skipped_in_silence() {
        let (parsed, warnings) = args(&["stray", "--agent", "claude", "--wat"]);
        assert_eq!(parsed.agent, "claude");
        assert!(warnings.is_empty());
    }
}
