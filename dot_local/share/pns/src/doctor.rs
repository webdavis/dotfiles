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
///
/// THE PAIRING IS AN ARGUMENT RATHER THAN A SECOND CODE THE CALLER COMBINES,
/// which is the same rule the summary and this function already share: two
/// contributors decided at one point cannot disagree, and two decided at two
/// call sites eventually will.
pub fn exit_code(outcomes: &[Outcome], pairing: &PairingReport) -> i32 {
    if outcomes
        .iter()
        .any(|outcome| verdict(outcome) == Verdict::Failed)
    {
        return 1;
    }
    // AN UNPAIRED HOST IS A DEAD APPROVAL PATH, and it is the one pairing
    // state that moves this: the check only reaches it on a machine where
    // moshi-hook is installed and answering, and there an unregistered host
    // means every card is going nowhere while the census reports the moshi
    // channel green over its webhook. The other three states could not check
    // and are inert, so a machine that does not use moshi still exits 0.
    if pairing.pairing == Pairing::Unpaired {
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

/// What `moshi-hook status` said about this host, in the only two shapes pns
/// is willing to state: a graded local fact, and moshi's own sentence about
/// the server relayed word for word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingReport {
    pub pairing: Pairing,
    /// moshi's `server:` sentence, filtered and capped, RELAYED AND NEVER
    /// GRADED. `None` when moshi printed no such line, which is what an
    /// unpaired host prints today and what a moshi that renamed the line
    /// would print: both degrade to no relay and nothing else moves.
    pub server: Option<String>,
}

/// What the LOCAL pairing material says, which is all `status --json` knows.
///
/// `Paired` PROVES LESS THAN IT SOUNDS LIKE, and the line built from it must
/// never read as "approvals work". It says this host has pairing material on
/// disk and that moshi answered about it. It does NOT prove the running daemon
/// is serving that pairing (a re-pair mints a new host id while the live
/// daemon keeps the old one, and no daemon-side evidence is readable from
/// here), and it does not prove an approval will round trip, which needs a
/// human tapping a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pairing {
    /// moshi answered `paired: true`, and named this host these two ways.
    Paired {
        host_id: String,
        display_name: String,
    },
    /// moshi answered `paired: false`.
    Unpaired,
    /// moshi answered something with no `paired` in it.
    Unreadable,
    /// moshi did not answer at all.
    NoAnswer,
}

/// The pairing, read off `status --json`, and the server sentence, read off
/// plain `status`. Either is `None` when that call gave no answer.
pub fn pairing_report(json: Option<&str>, plain: Option<&str>) -> PairingReport {
    PairingReport {
        pairing: pairing_of(json),
        server: plain.and_then(server_said),
    }
}

/// What moshi said about the server, taken off the ONE line that begins with
/// the label at column zero.
///
/// NOTHING HERE MATCHES ON THE SENTENCE. pns has no stable way to tell "Moshi
/// Pro attached" from "host does not belong to this user token", and a prefix
/// or substring rule over moshi's prose would fail in the dangerous direction
/// the day the wording changes. The operator reads moshi's own words instead.
fn server_said(plain: &str) -> Option<String> {
    plain
        .lines()
        .find_map(|line| line.strip_prefix(SERVER_LABEL))
        .map(|said| said.trim().to_string())
        .filter(|said| !said.is_empty())
}

/// The pairing check's own lines, in the order they are printed: what pns
/// graded, then what moshi said, when moshi said anything.
pub fn pairing_lines(report: &PairingReport) -> Vec<String> {
    let mut lines = vec![format!(
        "{PREFIX}moshi pairing: {}",
        said_of(&report.pairing)
    )];
    if let Some(said) = report
        .server
        .as_deref()
        .map(printable)
        .filter(|said| !said.is_empty())
    {
        // ATTRIBUTED, because pns is not making this claim and could not
        // check it: the sentence is moshi's and the label says so.
        lines.push(format!("{PREFIX}moshi says: {said}"));
    }
    lines
}

/// Somebody else's sentence, made safe to put on a terminal, and capped.
///
/// FILTERED AT THE POINT IT BECOMES A LINE, which is the only place that can
/// promise it: the report holds what moshi said, and this is what decides what
/// may be printed.
///
/// THE NEWLINE IS THE LOAD-BEARING ONE. An unfiltered newline would print a
/// second `pns doctor:` line that the operator would read as pns's own
/// verdict, and a report that can be made to lie about itself is worse than no
/// relay at all. The carriage return is the one that survives being split into
/// lines and returns a terminal's cursor to column zero for whatever follows
/// to overwrite the prefix with. Escapes, bells and every other control byte
/// go the same way, and so does anything outside ASCII, which is also what
/// makes the cap safe: a character is dropped whole, so the count can never
/// land inside a multi-byte sequence.
///
/// This does NOT reuse the decision log's identity filter, and the difference
/// is the point: that rule judges a short identity token that becomes a key's
/// value and replaces the whole thing when it fails, while this judges a
/// relayed English sentence full of spaces, parentheses, quotes and colons.
/// One predicate for both would have to be the wider of the two, which is the
/// narrower one weakened.
fn printable(said: &str) -> String {
    said.chars()
        .filter(|character| *character == ' ' || character.is_ascii_graphic())
        .take(RELAY_MAX)
        .collect()
}

/// How much of somebody else's sentence this report is willing to carry. An
/// unbounded relay is an unbounded line in a report pns is responsible for.
const RELAY_MAX: usize = 200;

/// The one sentence each state has earned. EVERY ONE OF THEM IS BOUNDED BY
/// WHAT THIS CHECK CAN SEE: the paired line says who this host is paired as
/// and stops, and the three that could not answer say so rather than reading
/// as a verdict either way.
fn said_of(pairing: &Pairing) -> String {
    match pairing {
        Pairing::Paired {
            host_id,
            display_name,
        } => format!("paired as {display_name} ({host_id})."),
        // THE REMEDY IS IN THE LINE, because this is the state the whole check
        // exists for and it is invisible everywhere else: the census reports
        // the moshi channel green over its webhook the whole time, while every
        // approval card is going nowhere.
        Pairing::Unpaired => "this host is NOT paired, so every approval card is dead \
             until `moshi-hook pair` runs."
            .to_string(),
        Pairing::Unreadable => "moshi-hook answered something this cannot read.".to_string(),
        // BOTH EXPLANATIONS AND NEITHER CLAIM. The bounded spawn cannot tell
        // an absent binary from one that hung or one that exited non-zero, and
        // a machine that simply does not use moshi must not fail its doctor
        // forever, so this costs nothing on the exit code either.
        Pairing::NoAnswer => "moshi-hook did not answer (not installed, or it did not \
             answer in time), so the approval path could not be checked."
            .to_string(),
    }
}

/// How every line the doctor prints for itself is addressed.
const PREFIX: &str = "pns doctor: ";

/// moshi's own label for the one line carrying a server verdict. A LINE
/// PREFIX, never a substring: moshi indents its detail lines, and a substring
/// rule would quote whichever of them said the word first.
const SERVER_LABEL: &str = "server:";

/// The pairing `status --json` described, and NOTHING ELSE OFF THAT OBJECT.
/// Three keys are read; `hooks` in particular is deliberately not one of them.
fn pairing_of(json: Option<&str>) -> Pairing {
    // NO ANSWER IS ITS OWN STATE AND NOTHING GUESSES PAST IT. The bounded
    // spawn answers `None` for a binary that is absent, one that hung past its
    // deadline and one that exited non-zero, and nothing downstream may claim
    // to know which of the three it was.
    let Some(json) = json else {
        return Pairing::NoAnswer;
    };
    let Ok(answer) = serde_json::from_str::<serde_json::Value>(json) else {
        return Pairing::Unreadable;
    };
    match answer.get(PAIRED).and_then(serde_json::Value::as_bool) {
        Some(true) => Pairing::Paired {
            host_id: named(&answer, "hostId"),
            display_name: named(&answer, "displayName"),
        },
        Some(false) => Pairing::Unpaired,
        // A key that is absent, or holds something other than a bool, is an
        // answer this cannot read. It is NOT read as unpaired: guessing the
        // one state that earns an exit 1 out of a shape nobody recognized is
        // how a doctor starts failing healthy machines.
        None => Pairing::Unreadable,
    }
}

/// One string moshi named, or the honest admission that it named none. The
/// measured 0.3.3 answer always carries both alongside `paired: true`, so this
/// is the shape nobody has seen rather than a case to design around.
fn named(answer: &serde_json::Value, key: &str) -> String {
    answer
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(NOT_REPORTED)
        .to_string()
}

/// The one key that moves the exit code, spelled once.
const PAIRED: &str = "paired";

/// What stands in for an identifier moshi did not name.
const NOT_REPORTED: &str = "not reported";

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
        A_SENSOR, Check, CheckKind, NOT_ENABLED, Outcome, Pairing, PairingReport, checks,
        exit_code, line, pairing_lines, pairing_report, summary,
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
        // THE SENDS ALONE, which is what the inert pairing below holds fixed:
        // a report that could not be checked moves nothing, so every case here
        // is decided by its outcomes exactly as it was before the pairing
        // check existed.
        let no_pairing_answer = pairing_report(None, None);
        assert_eq!(
            exit_code(
                &[
                    Outcome::Sent("posted HTTP 200".to_string()),
                    Outcome::Skipped(NOT_ENABLED),
                ],
                &no_pairing_answer
            ),
            0
        );
        assert_eq!(
            exit_code(&[Outcome::SentUnreported], &no_pairing_answer),
            0,
            "a channel that reports no outcome was still handed the event"
        );
        assert_eq!(exit_code(&[Outcome::Signalled(3)], &no_pairing_answer), 0);
        assert_eq!(
            exit_code(
                &[
                    Outcome::Sent("posted HTTP 200".to_string()),
                    Outcome::Failed("post FAILED HTTP 401".to_string()),
                ],
                &no_pairing_answer
            ),
            1,
            "one failure is enough, however much else worked"
        );
        assert_eq!(
            exit_code(&[Outcome::Signalled(0)], &no_pairing_answer),
            1,
            "a pulse that reached no room reached nothing"
        );
        assert_eq!(
            exit_code(
                &[Outcome::Skipped(NOT_ENABLED), Outcome::Skipped(A_SENSOR)],
                &no_pairing_answer
            ),
            1,
            "a run with nothing to check must never report green"
        );
        assert_eq!(
            exit_code(&[], &no_pairing_answer),
            1,
            "and neither must an empty one"
        );
    }

    // --- the moshi pairing check ---------------------------------------------

    #[test]
    fn a_pairing_built_from_no_answer_claims_neither_paired_nor_unpaired() {
        let report = pairing_report(None, None);
        assert_eq!(
            report.pairing,
            Pairing::NoAnswer,
            "no answer is its own state, never a guess at one"
        );
        assert_eq!(report.server, None, "and there is nothing to relay either");
    }

    /// `moshi-hook status --json` on this machine, moshi-hook 0.3.3, healthy.
    ///
    /// The three values the capture elided are elided here too (`hooks`,
    /// `logPath`, `socketPath`): NOTHING READS THEM, and `hooks` in particular
    /// is deliberately out of scope, because on this machine it reports the
    /// claude and codex hooks as stale BY DESIGN under the single-submitter
    /// rule, so a check that graded it would page a permanent false alarm.
    const PAIRED_JSON: &str = r#"{"baseUrl":"https://api.getmoshi.app/api/v1",
        "displayName":"dresden","hooks":[],
        "hostId":"host_b14dd2bb0b1f45899d9eaa81a71ff874","logPath":"...",
        "paired":true,"platform":"macos","secretStore":"keychain","socketPath":"..."}"#;

    /// The same call measured with `HOME` pointed at an empty directory: the
    /// answer is `paired: false` and carries no host id at all.
    const UNPAIRED_JSON: &str = r#"{"baseUrl":"https://api.getmoshi.app/api/v1","hooks":[],
        "logPath":"...","paired":false,"platform":"macos","secretStore":"keychain",
        "socketPath":"..."}"#;

    #[test]
    fn a_paired_answer_carries_back_the_host_id_and_display_name_moshi_named() {
        assert_eq!(
            pairing_report(Some(PAIRED_JSON), None).pairing,
            Pairing::Paired {
                host_id: "host_b14dd2bb0b1f45899d9eaa81a71ff874".to_string(),
                display_name: "dresden".to_string(),
            },
            "both come back VERBATIM: a doctor that abbreviated moshi's own \
             identifiers would be a second spelling of one answer, and the host \
             id is the thing an operator compares against the phone"
        );

        // A SHAPE NOBODY HAS SEEN. Every measured `paired: true` carries both
        // names, so this is the fallback rather than a case to design around,
        // and it says the identifier is missing instead of rendering an empty
        // parenthesis the operator would read as a host id they misread.
        assert_eq!(
            pairing_report(Some(r#"{"paired":true}"#), None).pairing,
            Pairing::Paired {
                host_id: "not reported".to_string(),
                display_name: "not reported".to_string(),
            }
        );
    }

    #[test]
    fn an_unpaired_answer_is_unpaired_rather_than_unreadable() {
        assert_eq!(
            pairing_report(Some(UNPAIRED_JSON), None).pairing,
            Pairing::Unpaired,
            "an answer naming no host is still an ANSWER: reading it as \
             unreadable would make the one state that earns an exit 1 inert"
        );
    }

    #[test]
    fn json_that_will_not_parse_or_names_no_paired_key_claims_neither() {
        for answer in [
            "",
            "not json at all",
            "{",
            r#"{"displayName":"dresden"}"#,
            r#"{"paired":"yes"}"#,
        ] {
            assert_eq!(
                pairing_report(Some(answer), None).pairing,
                Pairing::Unreadable,
                "answer: {answer:?}"
            );
        }
    }

    /// `moshi-hook status` (plain), healthy, on this machine. This shape is the
    /// only one carrying a server verdict at all: the JSON answer above is
    /// local-only and measured to perform no network I/O.
    const PAIRED_PLAIN: &str = "status:       paired\n\
         host id:      host_b14dd2bb0b1f45899d9eaa81a71ff874\n\
         display name: dresden\n\
         server:       Moshi Pro attached (usage scope: license)\n";

    #[test]
    fn the_server_line_is_relayed_as_moshis_own_words_with_the_label_removed() {
        assert_eq!(
            pairing_report(Some(PAIRED_JSON), Some(PAIRED_PLAIN))
                .server
                .as_deref(),
            Some("Moshi Pro attached (usage scope: license)"),
            "moshi's own sentence, VERBATIM. pns has no stable way to tell this \
             apart from a host that does not belong to the user token, and any \
             match on the prose would fail in the dangerous direction the day \
             moshi rewords it: a healthy machine failing its doctor, or a real \
             break going unreported"
        );
    }

    #[test]
    fn only_a_server_line_at_column_zero_is_relayed() {
        // moshi's own output indents continuation and detail lines, so a
        // relay anchored on a substring would quote whichever of them said
        // the word first and attribute it to the server.
        let indented_first = "status:       paired\n  server: an indented line\n\
             server:       the server line\n";
        assert_eq!(
            pairing_report(None, Some(indented_first)).server.as_deref(),
            Some("the server line"),
            "the label is a line PREFIX, never a substring anywhere in the line"
        );
        let only_indented = "status:       paired\n  server: an indented line\n";
        assert_eq!(
            pairing_report(None, Some(only_indented)).server,
            None,
            "and an indented line alone is no server verdict at all"
        );
    }

    /// The label the relayed line carries, which is how the report attributes
    /// the sentence to moshi rather than to pns.
    const MOSHI_SAYS: &str = "moshi says";

    #[test]
    fn plain_output_with_no_server_line_relays_nothing_rather_than_an_empty_line() {
        // AN UNPAIRED HOST PRINTS NO `server:` LINE AT ALL, measured, and a
        // future moshi that renamed or dropped the line would print none
        // either. That degradation is the SAFE direction: no relay, and
        // nothing else about the report moves.
        let unpaired_plain = "status:       unpaired\n";
        let report = pairing_report(Some(PAIRED_JSON), Some(unpaired_plain));
        assert_eq!(report.server, None);
        assert!(
            !pairing_lines(&report)
                .iter()
                .any(|line| line.contains(MOSHI_SAYS)),
            "a relay with nothing to relay is an absent line, never a labelled \
             blank one: {:?}",
            pairing_lines(&report)
        );

        // And a label with nothing after it is not a verdict either.
        let empty_value = "status:       paired\nserver:       \n";
        let report = pairing_report(Some(PAIRED_JSON), Some(empty_value));
        assert_eq!(report.server, None);
        assert!(
            !pairing_lines(&report)
                .iter()
                .any(|line| line.contains(MOSHI_SAYS)),
            "{:?}",
            pairing_lines(&report)
        );
    }

    /// What a relayed line is addressed as, so a test can strip it back off.
    const RELAY_OPENING: &str = "pns doctor: moshi says: ";

    #[test]
    fn a_relayed_value_carrying_a_newline_or_a_control_byte_cannot_forge_a_report_line() {
        // THE WHOLE POINT OF THE FILTER. This is third-party text going
        // straight to a terminal, and an unfiltered newline in it would print
        // a second `pns doctor:` line that the operator would read as pns's
        // own verdict. A report that can be made to lie about itself is worse
        // than no relay at all.
        let forged = PairingReport {
            pairing: Pairing::Unpaired,
            server: Some(
                "attached\npns doctor: 9 sent, 0 failed, 0 skipped\r\u{1b}[2Kok\u{7}".to_string(),
            ),
        };
        let lines = pairing_lines(&forged);
        assert_eq!(lines.len(), 2, "the relay forged a line: {lines:?}");
        assert_eq!(
            lines[1],
            "pns doctor: moshi says: attachedpns doctor: 9 sent, 0 failed, 0 skipped[2Kok",
            "the newline, the carriage return, the escape and the bell are all \
             gone, and what is left is visibly inside one relayed line"
        );

        // AND THE SAME THROUGH THE READING PATH. A carriage return is the one
        // that survives being split into lines, and on a terminal it returns
        // the cursor to column zero for whatever follows to overwrite the
        // report's own prefix with.
        let read = pairing_report(None, Some("server:       up\rpns doctor: forged\n"));
        let lines = pairing_lines(&read);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(
            !lines[1].contains('\r'),
            "a carriage return reached the terminal: {:?}",
            lines[1]
        );
    }

    #[test]
    fn an_over_long_relayed_value_stops_at_the_cap() {
        let report = PairingReport {
            pairing: Pairing::Unpaired,
            server: Some("x".repeat(500)),
        };
        let lines = pairing_lines(&report);
        let relayed = lines[1]
            .strip_prefix(RELAY_OPENING)
            .unwrap_or_else(|| panic!("{:?}", lines[1]));
        assert_eq!(
            relayed.chars().count(),
            200,
            "an unbounded relay is an unbounded line in somebody else's report"
        );

        // COUNTED IN CHARACTERS AND FILTERED FIRST, so the cap can never land
        // inside a multi-byte sequence: a character outside printable ASCII is
        // gone before anything is counted.
        let multibyte = PairingReport {
            pairing: Pairing::Unpaired,
            server: Some("\u{e9}".repeat(300)),
        };
        assert_eq!(
            pairing_lines(&multibyte).len(),
            1,
            "nothing printable survived, so there is nothing to relay"
        );
    }

    #[test]
    fn the_paired_line_names_the_host_and_claims_nothing_about_approvals() {
        let lines = pairing_lines(&pairing_report(Some(PAIRED_JSON), Some(PAIRED_PLAIN)));
        assert_eq!(
            lines,
            [
                "pns doctor: moshi pairing: paired as dresden \
                 (host_b14dd2bb0b1f45899d9eaa81a71ff874).",
                "pns doctor: moshi says: Moshi Pro attached (usage scope: license)",
            ]
        );
        // IT SAYS WHO THIS HOST IS PAIRED AS AND STOPS THERE. A re-pair mints
        // a new host id while the live daemon keeps serving the old one, and
        // an approval only really round trips when a human taps a card:
        // neither is visible from here, so neither may be implied.
        for overclaim in ["approvals work", "working", "will reach", "healthy"] {
            assert!(
                !lines[0].contains(overclaim),
                "the line claims {overclaim:?}, which this check cannot see: {:?}",
                lines[0]
            );
        }
    }

    #[test]
    fn the_unpaired_line_says_the_cards_are_dead_and_names_the_command_that_fixes_it() {
        let lines = pairing_lines(&pairing_report(Some(UNPAIRED_JSON), None));
        assert_eq!(
            lines,
            [
                "pns doctor: moshi pairing: this host is NOT paired, so every \
              approval card is dead until `moshi-hook pair` runs."
            ],
            "the remedy is IN THE LINE: this is the state the whole check \
             exists for, and the census reports the moshi channel green over \
             its webhook the entire time"
        );
    }

    #[test]
    fn the_no_answer_line_offers_both_explanations_and_commits_to_neither() {
        let lines = pairing_lines(&pairing_report(None, None));
        assert_eq!(
            lines,
            ["pns doctor: moshi pairing: moshi-hook did not answer (not \
              installed, or it did not answer in time), so the approval path \
              could not be checked."],
            "the bounded spawn cannot tell an absent binary from one that hung \
             or exited non-zero, so the line names two explanations and picks \
             neither"
        );

        // The fourth state, and the last one with a line of its own: moshi
        // answered, and the answer was a shape this does not recognize.
        assert_eq!(
            pairing_lines(&pairing_report(Some("{"), None)),
            [
                "pns doctor: moshi pairing: moshi-hook answered something this \
              cannot read."
            ]
        );
    }

    #[test]
    fn an_unpaired_host_alone_earns_the_exit_code_a_one() {
        // THE JUDGEMENT CALL. It only fires on a machine moshi-hook is
        // installed and answering on, which is a machine that set moshi up,
        // and on one of those an unregistered host means every approval card
        // is dead while the census reports the moshi channel green over its
        // webhook. That gap is the entire reason this check exists apart from
        // the census.
        let every_send_green = [Outcome::Sent("posted HTTP 200".to_string())];
        assert_eq!(
            exit_code(&every_send_green, &pairing_report(Some(PAIRED_JSON), None)),
            0,
            "the control: the same sends with a healthy pairing"
        );
        assert_eq!(
            exit_code(
                &every_send_green,
                &pairing_report(Some(UNPAIRED_JSON), None)
            ),
            1,
            "the pairing ALONE moved it, with nothing else changed"
        );
    }

    #[test]
    fn a_no_answer_or_unreadable_pairing_leaves_a_green_run_exiting_zero() {
        // A MACHINE THAT DOES NOT USE MOSHI MUST NOT FAIL ITS DOCTOR FOREVER,
        // and neither must one whose moshi answered a shape this cannot read:
        // both are "could not check", and a check that could not run is not a
        // failure it found.
        let every_send_green = [Outcome::Sent("posted HTTP 200".to_string())];
        for could_not_check in [pairing_report(None, None), pairing_report(Some("{"), None)] {
            assert_eq!(
                exit_code(&every_send_green, &could_not_check),
                0,
                "{could_not_check:?}"
            );
        }
    }

    #[test]
    fn a_failed_send_still_exits_one_when_the_pairing_is_healthy() {
        // NEITHER READER OVERRIDES THE OTHER. A healthy pairing cannot mask a
        // send that failed, and it cannot turn a run with nothing to check
        // green either.
        let healthy = pairing_report(Some(PAIRED_JSON), Some(PAIRED_PLAIN));
        assert_eq!(
            exit_code(
                &[
                    Outcome::Sent("posted HTTP 200".to_string()),
                    Outcome::Failed("post FAILED HTTP 401".to_string()),
                ],
                &healthy
            ),
            1
        );
        assert_eq!(
            exit_code(&[Outcome::Skipped(NOT_ENABLED)], &healthy),
            1,
            "a run with nothing to check must never report green, whatever \
             the pairing says"
        );
    }
}
