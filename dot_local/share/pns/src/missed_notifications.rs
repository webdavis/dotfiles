//! The journal of notifications the operator could not have perceived.
//!
//! POLICY ONLY, in `decision_log`'s style: every function here is a total
//! function of its arguments, with no config, no clock, no environment, no
//! file and no printing. The composition root reads the world, decides where
//! the file is, appends what comes back and prints what the doctor asks for.
//! This module never learns where the journal lives.
//!
//! THE PRIVACY RULE, in one sentence: the journal holds what a CARD would
//! have shown, no pns command ever prints an entry, and the only thing that
//! reads an entry back is the replayer, which delivers it to the same
//! channels the live event would have reached. `waiting_line` is where that
//! rule is STRUCTURAL rather than promised: it counts non-empty lines and has
//! no parse, so there is no code path in it that can emit a field.
//!
//! WHY THIS IS NOT THE DECISION RING. The two files have different readers.
//! The ring is read by a human through `pns doctor` and therefore admits no
//! free text at all; the journal is read by the replayer and is useless
//! without the event's own text. Fusing them would mean either printing
//! content to a terminal or journaling nothing worth replaying.

use crate::args::EventArgs;
use crate::engine::{Decision, Overrides};
use crate::surface::{Surface, Visibility};

/// How many missed notifications the journal keeps. Raising it is this one
/// number.
///
/// TWENTY FIVE RATHER THAN THE RING'S FIVE. Five is argued from one
/// intervening Stop hook, which is a scale of seconds; this file has to
/// survive an absence of hours, and twenty five covers an evening at a few
/// notifiable events an hour. Unbounded is wrong for the other reason: this
/// is state, not a log stream, and nothing rotates it.
pub const KEPT: usize = 25;

/// Whether the operator COULD NOT HAVE PERCEIVED this event.
///
/// Three clauses, all over values the record site already holds and none of
/// them a second reading. The plan said nothing, nobody was looking at the
/// origin pane, and the card was not skipped because another route already
/// carried it.
///
/// THE SURFACE HALF OF THE WATCHING CLAUSE is what saves the Away row: an
/// away operator is watching nothing, and a desk display showing the origin
/// pane to an empty chair is exactly the reading that must not suppress.
/// `surface::plan` reads `watching` the same way (it consults visibility only
/// in the Desk and Mobile arms), so this is that rule restated over the same
/// two values rather than a second rule.
///
/// `plan.pulse` IS DELIBERATELY NOT READ: the lights are decoration, and the
/// quiet window suppresses only them.
///
/// IT IS PLAN-LEVEL AND NOT DELIVERY-LEVEL, and that is a decision. A card
/// the plan called for and a channel failed to deliver is a truer miss than a
/// muted one, and it is still out of scope: `routing` derives legs FROM the
/// plan and drops the decoration on the way, so asking "did the leg carrying
/// the card fail" means re-deriving that policy here, which is the second
/// copy of a rule that then drifts. Two limits follow and are named rather
/// than left to be discovered. An event narrowed with both `--local-only` and
/// `--remote-only` reaches no channel while its plan still says banner, so it
/// is not journaled (it prints its own line and the decision log records it).
/// An event whose plan called for a card on a machine with no phone channel
/// configured is not journaled either.
pub fn was_missed(decision: &Decision, overrides: &Overrides) -> bool {
    let watching = decision.inputs.visibility == Visibility::Visible
        && decision.inputs.surface != Surface::Away;
    !overrides.skip_phone && !watching && !decision.plan.banner && !decision.plan.phone_card
}

/// One journal entry: a single JSON object, on one line.
///
/// IT CARRIES THE FIVE VALUES `render::title` and `render::message` consume,
/// plus the epoch, and nothing else. Raw fields rather than a pre-rendered
/// string, because the replay may need to shape them differently from the
/// live card (one card per event, or one summary of several) and a frozen
/// string cannot be reshaped. Deliberately absent: the pane (an id from an
/// hour ago may name a pane that no longer exists, so a replayed card's click
/// would do nothing), the channel (the durable route already has the event),
/// the tier (it drove a pulse for work that is now over) and every leg
/// verdict (the decision ring is where delivery outcomes live).
///
/// JSON AND NOT THE RING'S key=value, because of the free text. A detail can
/// contain a newline, a quote or an escape byte, and one entry must stay one
/// line or an append forges a second entry. The ring solves that by refusing
/// free text; this cannot, so the escaping is taken from the library that is
/// already a dependency. BUILT WITH `json!` AND NEVER WITH `format!`, which
/// is the Rust spelling of this repo's "build JSON with `jq -n --arg`" rule:
/// interpolation is exactly how a newline in a detail would forge an entry.
///
/// `at` IS THE DECISION'S OWN CLOCK READ, never a second `SystemTime` call at
/// the record site, for the reason the decision log takes its epoch from
/// there: two readings of one moment can disagree. An unreadable clock writes
/// `null`, which is honest and which a reader can tell from an absent field.
pub fn entry(event: &EventArgs, at: Option<u64>) -> String {
    serde_json::json!({
        "at": at,
        "agent": capped(&event.agent),
        "state": capped(&event.state),
        "project": capped(&event.project),
        "branch": capped(&event.branch),
        "detail": capped(&event.detail),
    })
    .to_string()
}

/// One text field as the journal holds it.
///
/// THE CAP IS THE CARD'S OWN, reused rather than invented a second time: the
/// journal holds what a card would have shown, so what a card renders without
/// a cut is exactly what a replay needs. It costs nothing, because the
/// durable hermes log is not gated by the plan and is exempt from the mute,
/// so every journaled event already reached the full-text record. The TAIL is
/// what survives, following `flatten_reply`'s own reasoning: a turn states its
/// conclusion at the end.
fn capped(text: &str) -> String {
    crate::render::flatten_reply(text, crate::render::PREVIEW_MAX_CHARS)
}

/// The doctor's one line about the journal, from the file's contents.
/// `contents` is `None` when there is no journal at all.
///
/// IT COUNTS AND NEVER PARSES, and that is the privacy rule made structural
/// rather than promised: there is no code path in here that could emit a
/// field, because nothing in here ever looks inside a line. Anyone tempted to
/// make this "more helpful" by rendering the newest entry is about to print
/// the operator's own text to a terminal, which is exactly what the decision
/// ring refuses free text to avoid.
///
/// IT SAYS WHAT IS WAITING, never "you missed N". The prune drops the oldest,
/// so over a long absence the file under-reports what was truly missed, and no
/// line here claims a number the file cannot back.
///
/// IT NAMES THE GAP rather than promising the replayer, because nothing
/// replays yet and a line that implied otherwise would be a promise the
/// binary does not keep.
pub fn waiting_line(contents: Option<&str>) -> String {
    let waiting = contents
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    match waiting {
        0 => NONE_WAITING.to_string(),
        1 => "pns doctor: 1 missed notification is recorded; nothing replays it yet.".to_string(),
        many => {
            format!(
                "pns doctor: {many} missed notifications are recorded; nothing replays them yet."
            )
        }
    }
}

/// An empty journal, which is honestly ambiguous: either nothing was missed or
/// a write did not land. It says what is RECORDED for that reason, and claims
/// neither reading.
const NONE_WAITING: &str = "pns doctor: no missed notification is recorded.";

#[cfg(test)]
mod tests {
    use super::{entry, waiting_line, was_missed};
    use crate::args::EventArgs;
    use crate::engine::{Decision, GateInputs, Overrides};
    use crate::surface::{DeliveryPlan, Surface, Visibility};

    /// A decision over the three values the predicate reads, with every other
    /// reading absent. NOTHING HERE IS A DOUBLE: `Decision` and `GateInputs`
    /// are the crate's own value types and the predicate is a total function
    /// of them, so a test states the values rather than driving a probe to
    /// produce them.
    fn decided(surface: Surface, visibility: Visibility, plan: DeliveryPlan) -> Decision {
        Decision {
            legs: Vec::new(),
            plan,
            pane_dropped: false,
            inputs: GateInputs {
                desk_input_age: None,
                phone_input_age: None,
                marker_age: None,
                screen_locked: None,
                desk_fresh_secs: None,
                surface,
                // The two agree everywhere except the Back Tap row, which
                // states its own disagreement below.
                session_visibility: visibility,
                visibility,
                now_secs: Some(1_756_500_000),
                long_running: false,
                mobile_watch_card: false,
                local_only: false,
                remote_only: false,
                pane_present: true,
            },
        }
    }

    /// A plan that decorated nothing, which is what a mute leaves behind.
    const NOTHING: DeliveryPlan = DeliveryPlan {
        banner: false,
        phone_card: false,
        pulse: false,
    };

    #[test]
    fn a_plan_that_decorated_nothing_with_nobody_watching_the_pane_is_missed() {
        // THE CASE THE JOURNAL EXISTS FOR: the plan said nothing and the
        // operator was not looking at the pane, so the event reached them
        // through no surface at all.
        assert!(was_missed(
            &decided(Surface::Desk, Visibility::Hidden, NOTHING),
            &Overrides::default()
        ));
    }

    #[test]
    fn a_plan_that_decorated_something_is_not_missed_whichever_decoration_it_was() {
        // THE PLAN AFTER ARBITRATION, never the matrix underneath it: the
        // banner and the card are two separate ways the operator was told,
        // and either one on its own is a delivery.
        let banner = DeliveryPlan {
            banner: true,
            ..NOTHING
        };
        assert!(!was_missed(
            &decided(Surface::Desk, Visibility::Hidden, banner),
            &Overrides::default()
        ));
        let card = DeliveryPlan {
            phone_card: true,
            ..NOTHING
        };
        assert!(!was_missed(
            &decided(Surface::Away, Visibility::Hidden, card),
            &Overrides::default()
        ));
    }

    #[test]
    fn an_event_suppressed_while_the_pane_was_on_screen_is_not_missed() {
        // THE ROW THAT KILLS THE NAIVE PREDICATE. Nothing was decorated here
        // either, but the operator was looking straight at the pane the event
        // came from, which is why the matrix suppressed it in the first place.
        for surface in [Surface::Desk, Surface::Mobile] {
            assert!(
                !was_missed(
                    &decided(surface, Visibility::Visible, NOTHING),
                    &Overrides::default()
                ),
                "{surface:?} watching the origin pane"
            );
        }
    }

    #[test]
    fn an_away_event_is_missed_even_when_the_session_reported_the_pane_visible() {
        // A DESK DISPLAY SHOWING THE ORIGIN PANE TO AN EMPTY CHAIR is exactly
        // the reading that must not suppress. `surface::plan` consults
        // visibility only in its Desk and Mobile arms, so the surface half of
        // this clause is that rule restated rather than a second rule.
        assert!(was_missed(
            &decided(Surface::Away, Visibility::Visible, NOTHING),
            &Overrides::default()
        ));
    }

    #[test]
    fn a_card_skipped_because_another_route_already_raised_one_is_not_missed() {
        // DELIVERED BY ANOTHER ROUTE, never missed: the environment sets
        // `PNS_SKIP_PHONE` exactly when the moshi approval forward really
        // happened, so an approval is already sitting on the phone. Replaying
        // a stale approval card later would be actively wrong.
        let skipped = Overrides {
            skip_phone: true,
            ..Overrides::default()
        };
        for surface in [Surface::Desk, Surface::Mobile, Surface::Away] {
            for visibility in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
                assert!(
                    !was_missed(&decided(surface, visibility, NOTHING), &skipped),
                    "{surface:?} / {visibility:?}"
                );
            }
        }
    }

    #[test]
    fn a_muted_event_the_surface_would_have_decorated_is_the_journals_queue() {
        // THE MUTE'S QUEUE, which is what this file mostly holds: the mute
        // zeroes the plan LAST, after the matrix already decided to decorate,
        // so the predicate never reads `muted` itself and reads the plan the
        // mute left behind instead.
        let muted = Overrides {
            muted: true,
            ..Overrides::default()
        };
        // A desk with the pane out of sight would have had a banner.
        assert!(was_missed(
            &decided(Surface::Desk, Visibility::Hidden, NOTHING),
            &muted
        ));
        // Away would have had a card.
        assert!(was_missed(
            &decided(Surface::Away, Visibility::Hidden, NOTHING),
            &muted
        ));
        // THE BACK TAP ROW, and the reason the predicate reads `visibility`
        // rather than `session_visibility`: the operator tapped the phone with
        // moshi closed, so the session still reports the pane Visible while
        // nothing is on screen for them. `effective_visibility` has already
        // rewritten that to Hidden, and reading the session's own answer here
        // would call an empty screen a watched one.
        let back_tap = Decision {
            inputs: crate::engine::GateInputs {
                session_visibility: Visibility::Visible,
                visibility: Visibility::Hidden,
                ..decided(Surface::Mobile, Visibility::Hidden, NOTHING).inputs
            },
            ..decided(Surface::Mobile, Visibility::Hidden, NOTHING)
        };
        assert!(was_missed(&back_tap, &muted));
    }

    // --- the entry ---------------------------------------------------------

    /// The five values `render::title` and `render::message` consume, as an
    /// event. Everything else on `EventArgs` defaults, because nothing else
    /// reaches an entry.
    fn event(detail: &str) -> EventArgs {
        EventArgs {
            agent: "claude".to_string(),
            state: "blocked".to_string(),
            project: "dotfiles".to_string(),
            branch: "main".to_string(),
            detail: detail.to_string(),
            ..EventArgs::default()
        }
    }

    #[test]
    fn an_entry_carries_the_epoch_and_the_five_values_a_card_is_rebuilt_from() {
        // RAW FIELDS AND NOT A PRE-RENDERED STRING: the replay may need to
        // shape them differently from the live card, and a frozen string
        // cannot be reshaped. AND NO OTHER FIELD: the pane, the channel and
        // the tier are all deliberately absent, so the key set is asserted
        // whole rather than one key at a time.
        let written = entry(&event("a summary"), Some(1_756_500_000));
        let parsed: serde_json::Value = serde_json::from_str(&written).expect("one JSON object");
        let object = parsed.as_object().expect("an object");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["agent", "at", "branch", "detail", "project", "state"]
        );
        assert_eq!(parsed["at"], 1_756_500_000_u64);
        assert_eq!(parsed["agent"], "claude");
        assert_eq!(parsed["state"], "blocked");
        assert_eq!(parsed["project"], "dotfiles");
        assert_eq!(parsed["branch"], "main");
        assert_eq!(parsed["detail"], "a summary");
    }

    #[test]
    fn an_entry_written_with_no_readable_clock_records_a_null_rather_than_a_zero() {
        // A ZERO IS A CLAIM about January 1970; null is the honest reading,
        // and a reader can tell it from an absent field.
        let written = entry(&event("a summary"), None);
        let parsed: serde_json::Value = serde_json::from_str(&written).expect("one JSON object");
        assert!(parsed["at"].is_null(), "got {written}");
        assert!(
            parsed.as_object().expect("an object").contains_key("at"),
            "the field is still there to be read: {written}"
        );
    }

    #[test]
    fn a_hostile_detail_still_produces_exactly_one_entry_on_one_line() {
        // A NEWLINE IN A DETAIL WOULD FORGE A SECOND ENTRY, and a quote or a
        // control byte would leave a line no reader can parse back. The
        // library escaping is what prevents both; interpolating the value
        // into a JSON-shaped string is what would not.
        let hostile = "he said \"stop\"\nthen a literal \\n and an escape \u{1b}[0m";
        let written = entry(&event(hostile), Some(1_756_500_000));
        assert_eq!(written.lines().count(), 1, "got {written:?}");
        assert!(!written.contains('\n'), "got {written:?}");
        let parsed: serde_json::Value = serde_json::from_str(&written).expect("one JSON object");
        assert_eq!(
            parsed["detail"],
            serde_json::Value::from(hostile.replace('\n', " ")),
            "the detail survives the escaping unchanged but for the flatten"
        );
    }

    #[test]
    fn every_text_field_is_flattened_and_cut_to_the_cap_a_card_renders() {
        // THE TAIL SURVIVES, following `flatten_reply`'s own reasoning: a turn
        // states its conclusion at the end. EVERY field, not the detail alone,
        // because a branch or a project is free text too.
        let cap = crate::render::PREVIEW_MAX_CHARS;
        let long = format!("{}\n\n  END", "x ".repeat(cap));
        let written = entry(
            &EventArgs {
                project: long.clone(),
                ..event(&long)
            },
            Some(1_756_500_000),
        );
        let parsed: serde_json::Value = serde_json::from_str(&written).expect("one JSON object");
        for field in ["detail", "project"] {
            let held = parsed[field].as_str().expect("a string");
            assert_eq!(held.chars().count(), cap, "{field}: {held:?}");
            assert!(held.ends_with("END"), "{field} kept its tail: {held:?}");
            assert!(!held.contains('\n'), "{field} was flattened: {held:?}");
        }
    }

    // --- the doctor's one line ---------------------------------------------

    /// A journal holding `count` real entries, written the way the append
    /// leaves it: one per line, oldest first, trailing newline present.
    fn journal(count: usize) -> String {
        (0..count)
            .map(|which| {
                entry(
                    &event(&format!("summary {which}")),
                    Some(1_756_500_000 + which as u64),
                )
            })
            .map(|line| format!("{line}\n"))
            .collect()
    }

    #[test]
    fn the_waiting_line_counts_the_journal_and_says_that_nothing_replays_it_yet() {
        // IT SAYS WHAT IS WAITING, never "you missed N": the prune drops the
        // oldest, so a count of what was truly missed over a long absence is a
        // number this file cannot back.
        assert_eq!(
            waiting_line(Some(&journal(3))),
            "pns doctor: 3 missed notifications are recorded; nothing replays them yet."
        );
        assert_eq!(
            waiting_line(Some(&journal(1))),
            "pns doctor: 1 missed notification is recorded; nothing replays it yet."
        );
    }

    #[test]
    fn an_absent_or_blank_journal_says_nothing_is_recorded_and_names_nothing() {
        // AN EMPTY JOURNAL IS AMBIGUOUS: either nothing was missed or a write
        // did not land. The line claims neither, and it never says a number.
        let none = "pns doctor: no missed notification is recorded.";
        assert_eq!(waiting_line(None), none);
        assert_eq!(waiting_line(Some("")), none);
        assert_eq!(waiting_line(Some("\n")), none);
        assert_eq!(waiting_line(Some("\n   \n\t\n")), none);
    }

    #[test]
    fn the_waiting_line_cannot_emit_an_entrys_content() {
        // THE PRIVACY RULE, pinned. Every value in this fixture is
        // unmistakable, so a line that leaked any part of an entry, in any
        // arm, cannot pass by coincidence.
        let secret = EventArgs {
            agent: "zzagentzz".to_string(),
            state: "zzstatezz".to_string(),
            project: "zzprojectzz".to_string(),
            branch: "zzbranchzz".to_string(),
            detail: "zzthe-operators-own-private-summaryzz".to_string(),
            ..EventArgs::default()
        };
        for count in [1, 3] {
            let contents: String = (0..count)
                .map(|_| format!("{}\n", entry(&secret, Some(1_756_500_000))))
                .collect();
            let line = waiting_line(Some(&contents));
            for leaked in [
                &secret.agent,
                &secret.state,
                &secret.project,
                &secret.branch,
                &secret.detail,
            ] {
                assert!(
                    !line.contains(leaked.as_str()),
                    "{leaked:?} reached the doctor's line: {line}"
                );
            }
            // AND NOT THE EPOCH EITHER, which is the one field that would look
            // harmless enough to print.
            assert!(!line.contains("1756500000"), "{line}");
        }
    }
}
