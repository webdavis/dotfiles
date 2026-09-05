//! The journal of notifications the operator could not have perceived: the
//! JSON codec that writes and reads one entry, and the three predicates that
//! decide whether an event was missed.
//!
//! POLICY ONLY, in `decision_log`'s style: every function here is a total
//! function of its arguments, with no config, no clock, no environment, no
//! file and no printing. The composition root reads the world, decides where
//! the file is, appends what comes back and prints what the doctor asks for.
//! This module never learns where the journal lives.
//!
//! WHY THIS IS NOT THE DECISION RING. The two files have different readers.
//! The ring is read by a human through `pns doctor` and therefore admits no
//! free text at all; the journal is read by the replayer and is useless
//! without the event's own text. Fusing them would mean either printing
//! content to a terminal or journaling nothing worth replaying.
//!
//! What a miss COMPOSES INTO moved to `pns-domain`. The codec stays because
//! this crate is where `serde_json` lives, and the three predicates stay
//! because they answer over the engine's `Decision`.

use crate::args::EventArgs;
use crate::engine::{Decision, Overrides};
use crate::surface::{Surface, Visibility};

pub use pns_domain::missed::{
    Entry, KEPT, NEEDS_YOU, event_count, needing_you, recap_card, summary, waiting_line,
};

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

/// Whether this event is the operator's RETURN, and so the moment a queued
/// notification can be put in front of them.
///
/// THE RETURN TRANSITION IS THE NEXT EVENT, and the engine has already
/// computed it. Nothing schedules a probe, so nothing OBSERVES a transition;
/// what the engine does do is read presence per event, at the last moment
/// before delivery, and publish the answer as the plan and the surface it
/// decided on. Both clauses below are values the record site already holds, so
/// this is no new probe, no second reading and no new trigger, and it inherits
/// the timing ruling for free.
///
/// AWAY IS WHERE MISSES ARE MADE AND NEVER WHERE THEY ARE DELIVERED. The Away
/// row always cards, so without this clause the journal would be flushed at
/// the phone of an operator who has not come back, which is the opposite of
/// what "return" means.
///
/// THE DECORATION CLAUSE BUYS TWO PROPERTIES AND CODES NEITHER. A mute zeroes
/// the plan, so a muted run cannot flush the queue it is filling, and the
/// replay fires on the first event AFTER the mute lapses that earns the
/// operator something; nothing here reads `overrides.muted`, for the reason
/// `was_missed` reads the arbitrated plan rather than the matrix underneath
/// it. And a run whose plan decorated nothing is exactly a run that JOURNALS,
/// so a miss and a replay are mutually exclusive by construction: no event can
/// deliver the entry it just wrote.
///
/// IT IS THE ENGINE'S OWN PERCEPTION RULE RESTATED, not a second one. An
/// operator at the desk watching the origin pane earns nothing, live or
/// replayed, so the queue waits for an event on a pane they are not watching.
pub fn should_replay(decision: &Decision) -> bool {
    decision.inputs.surface != Surface::Away && (decision.plan.banner || decision.plan.phone_card)
}

/// Whether this event PROVES the operator was here, and so moves the recap
/// window's near edge forward.
///
/// AWAY IS THE ONLY THING THAT DOES NOT COUNT. Desk and Mobile are both a
/// human within reach of a screen; Away is the state the whole recap exists to
/// bracket, and the window it brackets runs from the last event that was not
/// one to now.
///
/// VISIBILITY IS DELIBERATELY NOT READ, unlike `was_missed`'s watching clause.
/// An operator at the desk looking at a different pane is still present, and
/// reading visibility here would make the window's near edge depend on which
/// pane happened to fire.
///
/// IT READS A VALUE THE DECISION ALREADY HOLDS, so it is no new probe and no
/// second reading, exactly as `was_missed` and `should_replay` are argued
/// above.
pub fn is_present(decision: &Decision) -> bool {
    decision.inputs.surface != Surface::Away
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
///
/// `max_chars` IS THE CALLER'S, because two files now hold this shape and they
/// hold it for different readers. The journal passes the card's own cap, since
/// what a card renders without a cut is exactly what a replay needs; the
/// activity ring passes a timeline's cap, which is much shorter, because a
/// recap line is one line among a hundred and the full text of every event
/// already reached the durable log the recap points at. Neither number lives
/// here: this writes what it is given.
pub fn entry(event: &EventArgs, at: Option<u64>, max_chars: usize) -> String {
    let capped = |text: &str| crate::render::flatten_reply(text, max_chars);
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

/// The journal's contents read back into entries, oldest first, which is the
/// order the append leaves the file in.
///
/// PARSED BY KEY, never by position, which is what makes the writer's key
/// order (`serde_json`'s business, not this module's) invisible to the reader.
///
/// A LINE THAT IS NOT A JSON OBJECT IS SKIPPED, and it costs the rest of the
/// batch nothing. The file is a plain file in a directory an operator, a
/// backup tool or another program can reach, and the append's own heal can
/// republish a single line over it; one unparseable line must not throw away
/// the notifications around it. An object MISSING a field reads that field as
/// empty for the same reason: `render::title` and `render::message` already
/// have an answer for every empty value, so a short entry degrades to a
/// thinner card rather than to no card at all.
pub fn entries(contents: &str) -> Vec<Entry> {
    contents
        .lines()
        .filter_map(|line| {
            serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(line).ok()
        })
        .map(|fields| Entry {
            at: fields.get("at").and_then(serde_json::Value::as_u64),
            agent: text(&fields, "agent"),
            state: text(&fields, "state"),
            project: text(&fields, "project"),
            branch: text(&fields, "branch"),
            detail: text(&fields, "detail"),
        })
        .collect()
}

/// One text field off a parsed entry, absent and non-string alike reading as
/// empty.
fn text(fields: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    fields
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        Entry, entries, entry, is_present, should_replay, summary, waiting_line, was_missed,
    };
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

    // --- the replay condition ----------------------------------------------

    /// A plan that decorated the desk.
    const BANNER: DeliveryPlan = DeliveryPlan {
        banner: true,
        phone_card: false,
        pulse: false,
    };

    /// A plan that decorated the phone.
    const CARD: DeliveryPlan = DeliveryPlan {
        banner: false,
        phone_card: true,
        pulse: false,
    };

    #[test]
    fn a_decision_that_earned_a_banner_at_the_desk_says_replay() {
        // THE RETURN TRANSITION IS THIS EVENT. A banner fired means the
        // operator is at the desk with something on screen for them, which is
        // the moment a queued notification can be perceived.
        assert!(should_replay(&decided(
            Surface::Desk,
            Visibility::Hidden,
            BANNER
        )));
    }

    #[test]
    fn a_decision_that_earned_a_card_on_mobile_says_replay() {
        // THE SAME RULE ON THE OTHER SURFACE. A card fired means the phone in
        // the operator's hand just lit up, so the queue can ride along.
        assert!(should_replay(&decided(
            Surface::Mobile,
            Visibility::Hidden,
            CARD
        )));
    }

    #[test]
    fn an_away_decision_never_says_replay_however_much_it_carded() {
        // AWAY IS WHERE MISSES ARE MADE AND NEVER WHERE THEY ARE DELIVERED.
        // The Away row always cards, so without this clause the journal is
        // flushed at the phone of an operator who has not come back, which is
        // the opposite of what "return" means. Every visibility, because an
        // away operator is watching nothing whatever the session reported.
        for visibility in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
            assert!(
                !should_replay(&decided(Surface::Away, visibility, CARD)),
                "{visibility:?}"
            );
            assert!(
                !should_replay(&decided(Surface::Away, visibility, BANNER)),
                "{visibility:?}"
            );
        }
    }

    #[test]
    fn a_decision_that_decorated_nothing_says_no_replay() {
        // ONE CLAUSE, TWO PROPERTIES. A mute zeroes the plan, so a muted run
        // cannot flush the queue it is filling; and a run whose plan decorated
        // nothing is exactly a run that JOURNALS, so no event can ever replay
        // its own miss. The two are mutually exclusive by construction rather
        // than by an ordering rule at the record site.
        for surface in [Surface::Desk, Surface::Mobile] {
            for visibility in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
                assert!(
                    !should_replay(&decided(surface, visibility, NOTHING)),
                    "{surface:?} / {visibility:?}"
                );
            }
        }
    }

    // --- the presence marker's own predicate --------------------------------

    #[test]
    fn every_surface_but_away_proves_the_operator_was_here() {
        // AWAY IS THE ONLY THING THAT DOES NOT COUNT. Desk and Mobile are both
        // a human within reach of a screen, and Away is the state the whole
        // recap exists to bracket.
        for surface in [Surface::Desk, Surface::Mobile] {
            for visibility in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
                assert!(
                    is_present(&decided(surface, visibility, NOTHING)),
                    "{surface:?} / {visibility:?}"
                );
            }
        }
    }

    #[test]
    fn an_away_decision_never_moves_the_windows_near_edge() {
        // VISIBILITY IS DELIBERATELY NOT READ, on either side of this. An
        // operator at the desk looking at a different pane is still present,
        // and reading visibility here would make the window's edge depend on
        // which pane happened to fire.
        for visibility in [Visibility::Visible, Visibility::Hidden, Visibility::Unknown] {
            assert!(
                !is_present(&decided(Surface::Away, visibility, CARD)),
                "{visibility:?}"
            );
        }
    }

    // --- the entry ---------------------------------------------------------

    /// One entry at the JOURNAL'S own cap, which is what every test but the
    /// cap test itself is about. The cap travels with the caller now, so the
    /// journal's number is stated here rather than assumed inside `entry`.
    fn journaled(event: &EventArgs, at: Option<u64>) -> String {
        entry(event, at, crate::render::PREVIEW_MAX_CHARS)
    }

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
        let written = journaled(&event("a summary"), Some(1_756_500_000));
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
        let written = journaled(&event("a summary"), None);
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
        let written = journaled(&event(hostile), Some(1_756_500_000));
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
            cap,
        );
        let parsed: serde_json::Value = serde_json::from_str(&written).expect("one JSON object");
        for field in ["detail", "project"] {
            let held = parsed[field].as_str().expect("a string");
            assert_eq!(held.chars().count(), cap, "{field}: {held:?}");
            assert!(held.ends_with("END"), "{field} kept its tail: {held:?}");
            assert!(!held.contains('\n'), "{field} was flattened: {held:?}");
        }
    }

    // --- reading the entries back ------------------------------------------

    #[test]
    fn an_entry_reads_back_into_the_six_values_the_writer_put_there() {
        // FED FROM `entry`'S OWN OUTPUT rather than a hand-written literal, so
        // the writer and the reader can never drift apart in a fixture. BY
        // KEY and never by position, which is what makes the writer's key
        // order (`serde_json`'s business, not this module's) invisible here.
        let written = journaled(&event("a private summary"), Some(1_756_500_000));
        let read = entries(&format!("{written}\n"));
        assert_eq!(
            read,
            vec![Entry {
                at: Some(1_756_500_000),
                agent: "claude".to_string(),
                state: "blocked".to_string(),
                project: "dotfiles".to_string(),
                branch: "main".to_string(),
                detail: "a private summary".to_string(),
            }]
        );
        // AND THE CLOCK'S OTHER STATE, which is the one field that is not a
        // string: a null reads back as an unknown epoch, not as 1970.
        assert_eq!(entries(&journaled(&event("x"), None))[0].at, None);
    }

    #[test]
    fn a_short_entry_reads_its_absent_fields_as_empty_and_a_junk_line_costs_the_batch_nothing() {
        // THE FILE IS A PLAIN FILE anything can reach, and the append's own
        // heal can republish a single line over it. One line nobody can parse
        // must not throw away the notifications around it, and a short object
        // must degrade to a thinner card rather than to no card.
        let read = entries(&format!(
            "{}\nnot JSON at all\n{{\"agent\":\"codex\"}}\n\"a bare string\"\n[1,2]\n\n{}\n",
            journaled(&event("the first"), Some(1_756_500_000)),
            journaled(&event("the last"), Some(1_756_500_001)),
        ));
        assert_eq!(
            read.len(),
            3,
            "the two whole entries and the short one survived: {read:?}"
        );
        assert_eq!(read[0].detail, "the first");
        assert_eq!(read[2].detail, "the last", "{read:?}");
        assert_eq!(
            read[1],
            Entry {
                agent: "codex".to_string(),
                ..Entry::default()
            },
            "every absent field read as empty, the epoch included"
        );
    }

    #[test]
    fn an_entry_whose_keys_arrive_in_another_order_reads_back_the_same() {
        // HAND-BUILT AND DELIBERATELY NOT FROM `entry`, which is the only way
        // this says anything at all: `serde_json` writes the keys in one order,
        // and a reader taking fields by POSITION would agree with every fixture
        // the writer produced. The file is a plain file another hand can
        // rewrite, so the reader has to be keyed.
        let read = entries(
            "{\"detail\":\"a private summary\",\"branch\":\"main\",\"at\":1756500000,\
             \"project\":\"dotfiles\",\"state\":\"blocked\",\"agent\":\"claude\"}\n",
        );
        assert_eq!(
            read,
            vec![Entry {
                at: Some(1_756_500_000),
                agent: "claude".to_string(),
                state: "blocked".to_string(),
                project: "dotfiles".to_string(),
                branch: "main".to_string(),
                detail: "a private summary".to_string(),
            }]
        );
    }

    // --- the summary one card carries --------------------------------------

    /// The journal as the replay receives it: oldest first, each entry naming
    /// its own place, which is what makes an order assertion unambiguous.
    fn waiting(count: usize) -> Vec<Entry> {
        entries(&journal(count))
    }
    #[test]
    fn a_summary_of_three_names_three_and_puts_the_newest_first() {
        // NEWEST FIRST because `render::preview` cuts from the START, so what
        // survives a cut has to be what matters most. The count leads, so a
        // card that stopped early still says how many are behind it.
        let body = summary(&waiting(3));
        assert_eq!(
            body,
            "3 missed notifications. claude · blocked · dotfiles: summary 2; \
             claude · blocked · dotfiles: summary 1; \
             claude · blocked · dotfiles: summary 0"
        );
    }

    #[test]
    fn a_summary_of_one_reads_as_a_single_notification_in_the_singular() {
        // ONE SHAPE FOR EVERY COUNT: a single entry gets the same card the
        // batch does, carrying the same values the live card would have, and
        // only the wording follows the count the way `waiting_line`'s does.
        assert_eq!(
            summary(&waiting(1)),
            "1 missed notification. claude · blocked · dotfiles: summary 0"
        );
    }

    // --- the doctor's one line ---------------------------------------------

    /// A journal holding `count` real entries, written the way the append
    /// leaves it: one per line, oldest first, trailing newline present.
    fn journal(count: usize) -> String {
        (0..count)
            .map(|which| {
                journaled(
                    &event(&format!("summary {which}")),
                    Some(1_756_500_000 + which as u64),
                )
            })
            .map(|line| format!("{line}\n"))
            .collect()
    }

    #[test]
    fn the_waiting_line_counts_the_journal_and_says_the_entries_wait_to_be_replayed() {
        // IT SAYS WHAT IS WAITING, never "you missed N": the prune drops the
        // oldest, so a count of what was truly missed over a long absence is a
        // number this file cannot back.
        //
        // AND IT NAMES WHAT DELIVERS THEM, because this is a promise the
        // binary now keeps: the old sentence ended "nothing replays them yet",
        // which the replay made false the moment it shipped.
        assert_eq!(
            waiting_line(Some(&journal(3)), true),
            "pns doctor: 3 missed notifications are waiting to be replayed; \
             the next event that raises a banner or a card while the operator \
             is not away delivers them."
        );
        assert_eq!(
            waiting_line(Some(&journal(1)), true),
            "pns doctor: 1 missed notification is waiting to be replayed; \
             the next event that raises a banner or a card while the operator \
             is not away delivers it."
        );
    }

    #[test]
    fn a_switched_off_card_says_the_misses_are_recorded_and_that_nothing_delivers_them() {
        // THE PROMISE BELONGS TO THE SWITCH. `[recap] replay_card = false`
        // means no event will ever deliver these, so a line that still named
        // "the next event" would be a lie the operator's own setting makes
        // permanent, and the doctor would be the thing telling it. It says
        // what is true instead: the misses are recorded, the card is off, and
        // nothing moves them until the card is back on.
        assert_eq!(
            waiting_line(Some(&journal(3)), false),
            "pns doctor: 3 missed notifications are recorded; the catch-up card \
             is switched off (`[recap] replay_card = false`), so nothing delivers \
             them until the card is switched back on."
        );
        assert_eq!(
            waiting_line(Some(&journal(1)), false),
            "pns doctor: 1 missed notification is recorded; the catch-up card \
             is switched off (`[recap] replay_card = false`), so nothing delivers \
             it until the card is switched back on."
        );
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
        for (count, replay_card) in [(1, true), (3, true), (1, false), (3, false)] {
            let contents: String = (0..count)
                .map(|_| format!("{}\n", journaled(&secret, Some(1_756_500_000))))
                .collect();
            let line = waiting_line(Some(&contents), replay_card);
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
