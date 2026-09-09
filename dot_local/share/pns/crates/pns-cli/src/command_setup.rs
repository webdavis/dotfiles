use crate::*;
use pns_application::SETUP_USAGE;

/// The `setup` mode: the first-run walk, and the only writer of the config.
///
/// A THIN EDGE OVER A PURE COMPOSER. Everything about what lands in the file
/// is `pns_domain::setup`; this asks, reads a line, and publishes. It EXITS NON-ZERO
/// on every refusal, which the always-exit-0 contract permits for the same
/// reason `quiet` does: that contract covers the hook and notification paths,
/// where a non-zero exit fails the turn being reported on, and this is hand
/// typed and is never a hook.
///
/// IT REFUSES A WALK NOBODY CAN ANSWER. Without a terminal there is no walk,
/// and guessing every answer would write a config the operator never agreed
/// to, over one they may already have.
pub(crate) fn setup_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let force = match arguments.as_slice() {
        [] => false,
        [word] if word == "--force" => true,
        // ANY OTHER WORD IS A REFUSAL, never a silent fallthrough to the walk:
        // a mistyped `--force` that walked anyway would ask ten questions and
        // then refuse at the end, over a config it was told to replace.
        _ => {
            eprintln!("{SETUP_USAGE}");
            return 2;
        }
    };
    // AN EMPTY HOME IS REFUSED BY NAME, before the config is even located: an
    // unset or empty HOME would otherwise compose a config path relative to
    // the current directory, which is not the operator's own machine-wide
    // config no matter where this happened to be run from.
    let Some(home) = std::env::var("HOME").ok().filter(|home| !home.is_empty()) else {
        eprintln!("pns setup: HOME is unset or empty; nothing was written");
        return 2;
    };
    pns_application::RunSetup {
        terminal: &pns_adapters::ConsoleTerminal,
        renderer: &pns_adapters::SetupRenderer,
        publisher: &pns_adapters::FileConfigPublisher {
            path: config_path(&home),
        },
    }
    .run(force, |line| eprintln!("{line}"))
}
