//! `pns doctor`: what one test send through every configured channel found.
//!
//! POLICY ONLY, and every function here is a total function of its arguments:
//! no config, no clock, no environment, no network, no printing. The binary
//! reads the world, sends through the engine's own wiring, and hands what came
//! back to these to shape.
//!
//! THE CENSUS IS THE WHOLE ROSTER, never the selection. A plugin the config
//! left off has to be visibly absent BY CHOICE, or the report answers "what is
//! on" when the operator asked "what will reach me", which is the narrower
//! predicate this project keeps re-finding.

use crate::registry::{PluginKind, Registration, Selection};

/// One registered plugin and what checking it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Check {
    /// The plugin's config-table name, which is also how its line is labelled.
    pub plugin: &'static str,
    pub kind: CheckKind,
}

/// What a check does, decided from the registration and the selection alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// One test event through this channel's own delivery path.
    Send,
    /// A signal to the lights, which no event dispatches: counted in rooms,
    /// because the bridge acknowledges no write.
    Pulse,
    /// Nothing to check, and why.
    Skipped(&'static str),
}

/// Why a registered plugin was not checked: the config never switched it on.
const NOT_ENABLED: &str = "not enabled in the config";

/// Why a selected plugin was not checked: it is an input, and no leg can reach
/// it whatever the config says.
const A_SENSOR: &str = "a sensor and never a delivery destination";

/// What one check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It arrived, and the channel said this about it.
    Sent(String),
    /// It arrived, and the channel had nothing to say. An executable channel
    /// is silent by design, so claiming success for it would be claiming what
    /// the code does not provide.
    SentUnreported,
    /// It did not arrive, and the channel said this about that.
    Failed(String),
    /// The lights, and how many rooms were signalled.
    Signalled(usize),
    /// Nothing was checked, and why.
    Skipped(&'static str),
}

/// One check per registration, in registration order, whatever the config
/// selected.
pub fn checks(registered: &Selection, selected: &Selection) -> Vec<Check> {
    registered
        .iter()
        .map(|entry| Check {
            plugin: entry.name,
            kind: kind_of(entry, selected),
        })
        .collect()
}

/// What checking one registration means, given what the config selected.
///
/// NOT ENABLED IS ASKED FIRST, so a sensor the config never switched on reads
/// as absent by choice rather than as the kind it would have been.
fn kind_of(entry: &Registration, selected: &Selection) -> CheckKind {
    if !selected.iter().any(|chosen| chosen.name == entry.name) {
        return CheckKind::Skipped(NOT_ENABLED);
    }
    match entry.kind {
        PluginKind::Sensor => CheckKind::Skipped(A_SENSOR),
        // A channel the binary drives in its own mode is checkable, just not
        // as a leg: no event routes to it, so a send would never happen and
        // reporting it as skipped would hide the one destination hardest to
        // verify any other way.
        PluginKind::Channel(routing) if !routing.event_dispatched => CheckKind::Pulse,
        PluginKind::Channel(_) => CheckKind::Send,
    }
}

/// The one line this check earned.
pub fn line(check: &Check, outcome: &Outcome) -> String {
    let plugin = check.plugin;
    match outcome {
        Outcome::Sent(said) => format!("{plugin}: sent, {said}"),
        Outcome::SentUnreported => format!("{plugin}: sent, this channel reports no outcome"),
        Outcome::Failed(said) => format!("{plugin}: FAILED, {said}"),
        // NEITHER CLAIM IS MADE. Zero rooms is a bridge that answered no
        // listing OR a configured name nothing matched, and the line names
        // both rather than picking one; a count above zero says the rooms were
        // addressed and stops there, because the bridge acknowledges no write.
        Outcome::Signalled(0) => format!(
            "{plugin}: FAILED, signalled no rooms \
             (no room listing from the bridge, or no configured room name matched)"
        ),
        Outcome::Signalled(1) => format!("{plugin}: signalled 1 room ({WATCH_FOR_IT})"),
        Outcome::Signalled(rooms) => format!("{plugin}: signalled {rooms} rooms ({WATCH_FOR_IT})"),
        Outcome::Skipped(reason) => format!("{plugin}: skipped, {reason}"),
    }
}

/// What the operator has to do to confirm a pulse, since nothing else can.
const WATCH_FOR_IT: &str = "watch for the flash; the bridge acknowledges no write";

/// The last line: how the whole run went.
pub fn summary(outcomes: &[Outcome]) -> String {
    let count = |wanted: Verdict| outcomes.iter().filter(|o| verdict(o) == wanted).count();
    format!(
        "pns doctor: {} sent, {} failed, {} skipped",
        count(Verdict::Sent),
        count(Verdict::Failed),
        count(Verdict::Skipped)
    )
}

/// What the shell learns.
///
/// NOT THE ALWAYS-EXIT-0 CONTRACT'S TERRITORY: that covers the hook and
/// notification paths, where a non-zero exit fails the turn being reported on.
/// This is hand typed and is never a hook.
pub fn exit_code(outcomes: &[Outcome]) -> i32 {
    if outcomes
        .iter()
        .any(|outcome| verdict(outcome) == Verdict::Failed)
    {
        return 1;
    }
    // A CHECK WITH NOTHING TO CHECK MUST NEVER REPORT GREEN, which is the same
    // ruling the mute took: reporting success for something that is not in
    // effect is the worst outcome available.
    i32::from(
        !outcomes
            .iter()
            .any(|outcome| verdict(outcome) == Verdict::Sent),
    )
}

/// The three buckets every outcome falls into, decided ONCE so the summary's
/// counts and the exit code cannot read the same run differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Sent,
    Failed,
    Skipped,
}

fn verdict(outcome: &Outcome) -> Verdict {
    match outcome {
        Outcome::Sent(_) | Outcome::SentUnreported => Verdict::Sent,
        Outcome::Failed(_) => Verdict::Failed,
        // A PULSE THAT REACHED NO ROOM REACHED NOTHING. It is the shape every
        // hue misconfiguration takes, and an enabled channel that could not be
        // attempted is exactly what the exit contract calls a failure.
        Outcome::Signalled(0) => Verdict::Failed,
        Outcome::Signalled(_) => Verdict::Sent,
        Outcome::Skipped(_) => Verdict::Skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        A_SENSOR, Check, CheckKind, NOT_ENABLED, Outcome, checks, exit_code, line, summary,
    };
    use crate::config::parse_config;
    use crate::registry::{Registry, Selection, roster};

    /// The roster's own selection for a config, both halves the census takes.
    fn census(config_text: &str) -> (Registry, Selection, Selection) {
        let registry = roster();
        let selected = registry
            .enabled(&parse_config(config_text).unwrap())
            .unwrap();
        let registered = registry.all();
        (registry, registered, selected)
    }

    fn kind_for(config_text: &str, plugin: &str) -> CheckKind {
        let (_, registered, selected) = census(config_text);
        checks(&registered, &selected)
            .into_iter()
            .find(|check| check.plugin == plugin)
            .unwrap_or_else(|| panic!("{plugin} is registered"))
            .kind
    }

    // --- the census ----------------------------------------------------------

    #[test]
    fn the_check_list_holds_one_entry_per_registration_in_registration_order() {
        // WITH NOTHING ENABLED, so a census that walked the SELECTION would
        // return an empty report and lose every plugin at once. Registration
        // order is delivery order, and the report is read against the config.
        let (registry, registered, selected) = census("");
        assert_eq!(
            checks(&registered, &selected)
                .iter()
                .map(|check| check.plugin)
                .collect::<Vec<_>>(),
            registry.names(),
            "a report cannot silently omit a plugin"
        );
    }

    #[test]
    fn a_registered_plugin_the_config_did_not_enable_is_a_skip_that_says_which() {
        // BOTH WAYS a config declines a plugin: never naming it, and naming it
        // switched off. Neither is an error and both have to be visible, or
        // the operator reads a short report as a complete one.
        assert_eq!(
            kind_for("[plugins.hermes]\nenabled = true\n", "moshi"),
            CheckKind::Skipped(NOT_ENABLED)
        );
        assert_eq!(
            kind_for("[plugins.moshi]\nenabled = false\n", "moshi"),
            CheckKind::Skipped(NOT_ENABLED)
        );
    }

    #[test]
    fn a_selected_sensor_is_a_skip_because_no_leg_can_ever_reach_one() {
        assert_eq!(
            kind_for("[plugins.router]\nenabled = true\n", "router"),
            CheckKind::Skipped(A_SENSOR)
        );
    }

    #[test]
    fn a_selected_channel_no_event_dispatches_is_a_pulse_rather_than_a_send() {
        assert_eq!(
            kind_for("[plugins.hue]\nenabled = true\n", "hue"),
            CheckKind::Pulse
        );
    }

    #[test]
    fn a_selected_event_dispatched_channel_is_a_send() {
        for plugin in ["moshi", "macos-banner", "hermes"] {
            assert_eq!(
                kind_for(
                    "[plugins.moshi]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n\
                     [plugins.hermes]\nenabled = true\n",
                    plugin
                ),
                CheckKind::Send,
                "plugin: {plugin}"
            );
        }
    }

    // --- the report ----------------------------------------------------------

    #[test]
    fn a_line_names_its_plugin_and_its_outcome_and_a_failure_quotes_the_channel() {
        let hermes = Check {
            plugin: "hermes",
            kind: CheckKind::Send,
        };
        assert_eq!(
            line(&hermes, &Outcome::Sent("posted HTTP 200".to_string())),
            "hermes: sent, posted HTTP 200"
        );
        assert_eq!(
            line(
                &hermes,
                &Outcome::Failed("post FAILED HTTP 401".to_string())
            ),
            "hermes: FAILED, post FAILED HTTP 401",
            "the channel's own sentence, verbatim: a doctor that paraphrased \
             would be a second wording of one answer"
        );
        assert_eq!(
            line(&hermes, &Outcome::SentUnreported),
            "hermes: sent, this channel reports no outcome"
        );
        let router = Check {
            plugin: "router",
            kind: CheckKind::Skipped(A_SENSOR),
        };
        assert_eq!(
            line(&router, &Outcome::Skipped(A_SENSOR)),
            "router: skipped, a sensor and never a delivery destination"
        );
    }

    #[test]
    fn the_pulse_line_claims_neither_a_flash_nor_a_cause_it_cannot_know() {
        let hue = Check {
            plugin: "hue",
            kind: CheckKind::Pulse,
        };
        assert_eq!(
            line(&hue, &Outcome::Signalled(2)),
            "hue: signalled 2 rooms (watch for the flash; the bridge acknowledges no write)"
        );
        assert_eq!(
            line(&hue, &Outcome::Signalled(1)),
            "hue: signalled 1 room (watch for the flash; the bridge acknowledges no write)"
        );
        assert_eq!(
            line(&hue, &Outcome::Signalled(0)),
            "hue: FAILED, signalled no rooms \
             (no room listing from the bridge, or no configured room name matched)",
            "zero names both causes rather than choosing one, and no count claims the \
             lights actually flashed"
        );
    }

    #[test]
    fn the_summary_counts_every_check_exactly_once() {
        let outcomes = [
            Outcome::Skipped(A_SENSOR),
            Outcome::Sent("posted HTTP 200".to_string()),
            Outcome::SentUnreported,
            Outcome::Failed("post FAILED HTTP 401".to_string()),
            Outcome::Signalled(2),
            Outcome::Signalled(0),
            Outcome::Skipped(NOT_ENABLED),
        ];
        let summarized = summary(&outcomes);
        assert_eq!(summarized, "pns doctor: 3 sent, 2 failed, 2 skipped");
        let counted: usize = summarized
            .split_whitespace()
            .filter_map(|word| word.parse::<usize>().ok())
            .sum();
        assert_eq!(
            counted,
            outcomes.len(),
            "a check that fell into no bucket is a plugin the summary lost"
        );
    }

    // --- the exit contract ---------------------------------------------------

    #[test]
    fn only_a_run_that_sent_something_and_failed_nothing_exits_zero() {
        assert_eq!(
            exit_code(&[
                Outcome::Sent("posted HTTP 200".to_string()),
                Outcome::Skipped(NOT_ENABLED),
            ]),
            0
        );
        assert_eq!(
            exit_code(&[Outcome::SentUnreported]),
            0,
            "a channel that reports no outcome was still handed the event"
        );
        assert_eq!(exit_code(&[Outcome::Signalled(3)]), 0);
        assert_eq!(
            exit_code(&[
                Outcome::Sent("posted HTTP 200".to_string()),
                Outcome::Failed("post FAILED HTTP 401".to_string()),
            ]),
            1,
            "one failure is enough, however much else worked"
        );
        assert_eq!(
            exit_code(&[Outcome::Signalled(0)]),
            1,
            "a pulse that reached no room reached nothing"
        );
        assert_eq!(
            exit_code(&[Outcome::Skipped(NOT_ENABLED), Outcome::Skipped(A_SENSOR)]),
            1,
            "a run with nothing to check must never report green"
        );
        assert_eq!(exit_code(&[]), 1, "and neither must an empty one");
    }
}
