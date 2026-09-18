//! Which sources the page is made of, and what each one says when it cannot be
//! read.

use morning_adapters::{CommandSource, Config, LedgerSource, capture, files};
use morning_application::{Source, SourceOutcome, gather};
use morning_domain::{apply_log, ledger, page};
use std::path::Path;
use std::time::Duration;

/// The heading the frame carries.
const HEADING: &str = "morning";

/// Reads every configured source and renders the page.
pub fn compose(config: &Config, home: &Path) -> String {
    let timeout = config.timeout();
    let sources = vec![
        Source::new("Last apply", || last_apply(config, home)),
        Source::new("Applies owed", || {
            ledger_section(config.ledger.as_ref(), home, Owed::Applies)
        }),
        Source::new("Pull requests", || {
            command(config.pull_requests.as_ref(), timeout)
        }),
        Source::new("Overnight recap", || recap(config, home)),
        Source::new("Operator's own items", || {
            ledger_section(config.ledger.as_ref(), home, Owed::Operator)
        }),
        Source::new("Today", || command(config.tasks.as_ref(), timeout)),
    ];
    page::render(HEADING, &gather(sources), config.rows_per_section())
}

/// What the source was not given.
fn unconfigured() -> SourceOutcome {
    SourceOutcome::Unavailable("not configured".to_string())
}

fn last_apply(config: &Config, home: &Path) -> SourceOutcome {
    let Some(path) = config
        .apply_log
        .as_ref()
        .and_then(|source| source.resolve(home))
    else {
        return unconfigured();
    };
    match files::read(&path) {
        Err(reason) => SourceOutcome::Unavailable(reason),
        Ok(transcript) => match apply_log::parse(&transcript) {
            Some(apply) => SourceOutcome::Lines(vec![apply.summary()]),
            None => SourceOutcome::Unavailable(format!(
                "{} records no result, so an apply may still be running",
                path.display()
            )),
        },
    }
}

/// Which half of the ledger a section wants.
enum Owed {
    Applies,
    Operator,
}

fn ledger_section(source: Option<&LedgerSource>, home: &Path, owed: Owed) -> SourceOutcome {
    let Some(source) = source else {
        return unconfigured();
    };
    let Some(path) = source.resolve(home) else {
        return unconfigured();
    };
    match files::read(&path) {
        Err(reason) => SourceOutcome::Unavailable(reason),
        Ok(text) => {
            let found = ledger::parse(&text, &source.apply_markers, &source.operator_markers);
            SourceOutcome::Lines(match owed {
                Owed::Applies => found.owed_applies,
                Owed::Operator => found.operator_items,
            })
        }
    }
}

fn recap(config: &Config, home: &Path) -> SourceOutcome {
    let Some(source) = config.recap.as_ref() else {
        return unconfigured();
    };
    let Some(directory) = source.resolve(home) else {
        return unconfigured();
    };
    match files::newest(&directory).and_then(|path| files::read(&path)) {
        Err(reason) => SourceOutcome::Unavailable(reason),
        Ok(text) => SourceOutcome::Lines(files::head(&text, source.lines)),
    }
}

fn command(source: Option<&CommandSource>, timeout: Duration) -> SourceOutcome {
    let Some(argv) = source.and_then(CommandSource::resolve) else {
        return unconfigured();
    };
    match capture(argv, timeout) {
        Ok(output) => SourceOutcome::Lines(output.lines().map(str::to_string).collect()),
        Err(error) => SourceOutcome::Unavailable(error.to_string()),
    }
}
