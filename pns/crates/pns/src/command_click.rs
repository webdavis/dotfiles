use crate::*;
use pns_domain::failure::ClickView;

/// The `click` mode: what a click on a failure banner runs.
///
/// IT IS NOT TYPED BY THE OPERATOR, which is what shapes everything here. The
/// banner's `-execute` string calls it from a bare launchd context with no
/// PATH, no terminal and nobody watching stderr, so every path ends in
/// something the operator can SEE: a window, or a log line naming what was
/// tried.
///
/// A MODE beside the doctor's: it opens a view and delivers nothing, so no
/// event's plan reaches it.
pub(crate) fn click_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let [word] = arguments.as_slice() else {
        eprintln!("{CLICK_USAGE}");
        return 2;
    };
    let Ok(id) = word.parse::<u64>() else {
        eprintln!("{CLICK_USAGE}");
        return 2;
    };
    open(id)
}

fn open(id: u64) -> i32 {
    let herdr = executable_in_path("herdr");
    let view = match view(herdr.is_some()) {
        Ok(view) => view,
        // A REFUSED CONFIG STILL OPENS THE RECORD. The operator clicked a
        // banner about a lost page; telling them their config is wrong and
        // showing them nothing answers a question they did not ask. The
        // complaint is printed and the fallback runs.
        Err(complaint) => {
            eprintln!("pns: {complaint}");
            ClickView::Window
        }
    };
    let outcome = pns_application::open_failure(
        &pns_adapters::SystemCommandRunner,
        &view,
        id,
        herdr.as_deref().unwrap_or("herdr"),
        &pns_path(),
    );
    if outcome.opened {
        return 0;
    }
    eprintln!("{}", pns_application::click_failure_line(id, &outcome));
    raise_last_resort_banner(id);
    1
}

/// The banner that reports a click nothing answered.
///
/// `":"` IS THE EXEC STRING, which is the no-op the channel already writes for
/// an event with no pane: the banner raises the terminal and runs nothing.
/// Clicking a failed click to be told the click failed is a loop, and this is
/// where it stops.
fn raise_last_resort_banner(id: u64) {
    let (title, message) = pns_application::click_banner(id);
    let args = pns_adapters::notifier_args(
        &title,
        &message,
        Some("default"),
        pns_adapters::DEFAULT_TERMINAL_BUNDLE_ID,
        ":",
    );
    // The outcome is DROPPED, and there is nowhere left to report it: a banner
    // that will not post is the third failure in a row, and the log line above
    // has already said everything a reader could act on.
    let _ = pns_application::CommandRunner::run(
        &pns_adapters::SystemCommandRunner,
        "terminal-notifier",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    );
}

fn view(herdr_present: bool) -> Result<ClickView, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let Ok(LoadOutcome::Loaded(config)) = load_config(&config_path(&home)) else {
        return Ok(ClickView::inferred(herdr_present));
    };
    match plugin_settings(&config, "macos-banner") {
        Some(settings) => pns_adapters::banner_click(settings, herdr_present),
        None => Ok(ClickView::inferred(herdr_present)),
    }
}

/// This binary's own path, so the view it opens runs THIS pns rather than
/// whatever a new shell's PATH resolves. A click has no PATH to resolve with,
/// and a machine mid-upgrade can have two.
pub(crate) fn pns_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.to_str().map(str::to_string))
        .unwrap_or_else(|| "pns".to_string())
}

const CLICK_USAGE: &str = "pns: usage: pns click <failure-id>";

#[cfg(test)]
#[path = "command_click/tests.rs"]
mod tests;
