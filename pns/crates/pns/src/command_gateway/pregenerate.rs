//! The tick's pregenerate pass: each named window's summary, written in the
//! background at the window's end.
//!
//! THE MODEL RUNS IN A CHILD, NEVER IN THE TICK. The clock ticks once a second
//! and a summarizer may take minutes, so this starts `pns recap <window>
//! --pregenerate` as a supervised job and returns at once. Its job id carries
//! `RECAP_PREGENERATE_PREFIX`, which the daemon's own child bound reads as
//! unbounded: the summarizer's own `deadline` kills a wedged backend, so the
//! tick's generic bound would only kill a slow but honest one early.
//!
//! ONE CHILD PER WINDOW INSTANCE. The job id carries the window's name, so
//! `running` keeps a second copy from starting, and the instance's own end
//! stamp is remembered so a window is pregenerated once rather than on every
//! pass for as long as it stays ended.

use pns_application::JobChildren;
use std::collections::HashMap;

/// Start whatever windows are due, if this pass is due at all.
///
/// THE CONFIG IS READ AT THE PASS, like the prune's own read, because the
/// gateway outlives an edit: a window added to `pregenerate` is picked up
/// without a bounce.
pub(crate) fn pregenerate(
    now: u64,
    children: &mut impl JobChildren,
    next_pass: &mut u64,
    started: &mut HashMap<String, u64>,
) -> Result<(), String> {
    if now < *next_pass {
        return Ok(());
    }
    *next_pass = now.saturating_add(PASS_INTERVAL_SECS);
    let home = std::env::var("HOME").unwrap_or_default();
    // AN UNREADABLE CONFIG PREGENERATES NOTHING, which is this file's own
    // fail direction: the summary is an extra, and a file nobody can read
    // must not start a model process the operator never named.
    let Ok(pns_adapters::LoadOutcome::Loaded(config)) =
        pns_adapters::load_config(&pns_adapters::config_path(&home))
    else {
        return Ok(());
    };
    if !config.recap.summarizer.configured() {
        return Ok(());
    }
    for window in &config.recap.pregenerate {
        let Some(ended) = ended_at(window, &config.recap, now) else {
            continue;
        };
        if started.get(window) == Some(&ended) {
            continue;
        }
        let id = format!("{}{window}", pns_domain::jobs::RECAP_PREGENERATE_PREFIX);
        if children.running(&id) {
            continue;
        }
        started.insert(window.clone(), ended);
        children.start(&pns_domain::jobs::Job {
            id,
            due: now,
            until: now,
            every: None,
            unless_marker: None,
            args: vec![
                "recap".to_string(),
                window.clone(),
                crate::command_recap::PREGENERATE.to_string(),
            ],
        })?;
    }
    Ok(())
}

/// When this window's most recent instance ended, or None while it is still
/// running or its bounds cannot be stated.
///
/// A WINDOW IN PROGRESS IS NOT PREGENERATED, because its last event has not
/// happened yet: the paragraph would be written over half a window and then
/// shown as the whole of it.
fn ended_at(window: &str, recap: &pns_adapters::Recap, now: u64) -> Option<u64> {
    let (parsed, implied) = pns_domain::recap::window::parse_window(window)?;
    let resolved = crate::command_recap::window::named(parsed, implied, recap, now).ok()?;
    (resolved.until <= now).then_some(resolved.until)
}

/// How long between two passes. ONCE A MINUTE, not once a tick: a window ends
/// on a minute boundary, so a pass a second would read the config sixty times
/// to find the same nothing.
const PASS_INTERVAL_SECS: u64 = 60;
