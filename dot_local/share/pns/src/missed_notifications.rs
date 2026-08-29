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

/// How many missed notifications the journal keeps.
///
/// TWENTY FIVE RATHER THAN THE RING'S FIVE. Five is argued from one
/// intervening Stop hook, which is a scale of seconds; this file has to
/// survive an absence of hours, and twenty five covers an evening at a few
/// notifiable events an hour. Unbounded is wrong for the other reason: this
/// is state, not a log stream, and nothing rotates it.
///
/// RAISING IT IS THIS ONE NUMBER ONLY UP TO A CEILING, and the ceiling is
/// near enough to state. Each of the five text fields is capped at
/// `render::PREVIEW_MAX_CHARS` characters, and one character can cost six
/// bytes escaped (a control byte is written `\u001b`), so a worst-case entry
/// MEASURES 7,876 bytes and a full journal 196,900, which is 75% of the 256
/// KiB the composition root reads any of these state files back through.
/// Past a depth of 33 a full journal no longer reads back at all, and the
/// append answers a file it cannot read by republishing the one line it just
/// wrote: the journal would collapse to a single entry exactly when it is
/// fullest, and silently. Raising this past 33 means raising that read cap in
/// the same change.
///
/// ORDINARY ENTRIES ARE NOWHERE NEAR THAT, a few hundred bytes of plain text,
/// so the ceiling is reached only by fields that are all escape bytes. It is
/// stated because the collapse is silent, not because it is likely.
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

/// One journal entry read back: the six values `entry` wrote, and nothing
/// else.
///
/// THE READ SIDE OF `entry`, kept beside it so the pair changes together. It
/// is a struct rather than a `serde_json::Value` because the replay renders
/// from it and a caller holding a `Value` would be free to reach for a key
/// nobody wrote.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    /// The epoch the miss was journaled at, absent when the writer had no
    /// readable clock.
    pub at: Option<u64>,
    pub agent: String,
    pub state: String,
    pub project: String,
    pub branch: String,
    pub detail: String,
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

/// The one card a replay delivers, whatever the count: the true count, then
/// as many entries as fit, NEWEST FIRST.
///
/// ONE SHAPE AND NO SPECIAL CASE. A summary for many plus the real card for
/// exactly one would be two code paths, two sets of tests and a seam where
/// the two can disagree about what a replayed card looks like; and a
/// one-entry summary carries the same content the real card would, because
/// an entry holds exactly the values `render::title` and `render::message`
/// consume.
///
/// `waiting` ARRIVES IN THE FILE'S OWN ORDER, oldest first, and is rendered
/// newest first here, because `render::preview` cuts from the START: what
/// survives a cut has to be what matters most.
///
/// THE COUNT IS ALWAYS THE REAL COUNT, even when the body stopped early, so
/// the card never claims a number it did not show and never shows a number it
/// cannot back. The body stops at `render::PREVIEW_MAX_CHARS` rather than
/// leaving the cut to `preview`, so the operator is told how many are behind
/// the ones they can read; the full text of every entry already reached the
/// durable log when it happened.
///
/// THE NEWEST ENTRY GOES IN WHATEVER ITS LENGTH, and only the ones behind it
/// have to fit. MEASURED: a single missed notification with a 209-character
/// detail took the body one character past the cap, so the loop stopped
/// before appending anything and the card read "1 missed notification" with
/// no content at all, which is precisely the notification it exists to
/// deliver. The cut for that one entry is `render::preview`'s, on the way
/// out, which is where every other over-long body is already cut.
pub fn summary(waiting: &[Entry]) -> String {
    let mut body = match waiting.len() {
        1 => "1 missed notification".to_string(),
        many => format!("{many} missed notifications"),
    };
    for (shown, entry) in waiting.iter().rev().enumerate() {
        let separator = if shown == 0 { ". " } else { "; " };
        let extended = format!("{body}{separator}{}", rendered(entry));
        // STOPPED RATHER THAN SKIPPED, which is also what lets the index above
        // stand in for how many were shown: the entries left out are the
        // oldest, and a body that skipped a long one to reach an older short
        // one would read as though the newest were missing.
        //
        // AND NEVER BEFORE THE FIRST ONE. `shown == 0` is the newest entry
        // with nothing appended yet, and stopping there leaves the count
        // standing alone as the whole card.
        if shown > 0 && extended.chars().count() > crate::render::PREVIEW_MAX_CHARS {
            break;
        }
        body = extended;
    }
    body
}

/// One entry as a line of the summary: the card's own title, and its text
/// where there is any.
///
/// THE TITLE ALONE FOR AN EMPTY DETAIL, because the title already carries the
/// state a bare `done` turn would otherwise repeat after a colon.
fn rendered(entry: &Entry) -> String {
    let title = crate::render::title(&entry.agent, &entry.state, &entry.project);
    if entry.detail.is_empty() {
        title
    } else {
        format!("{title}: {}", entry.detail)
    }
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
/// IT NAMES WHAT DELIVERS THEM, which is a promise the binary keeps, and it
/// names it EXACTLY. The sentence used to end "nothing replays them yet",
/// which the replay made false the moment it shipped, and then "the next
/// event the operator is present for", which promises more than the binary
/// does: presence alone delivers nothing. Three things have to be true at
/// once, and the sentence says all three. The operator is not away; the event
/// earned a banner or a card (a muted one earns neither, and neither does one
/// on a pane they are watching); and a leg was there to raise it (a machine
/// with only a durable channel raises nothing). The zero case says nothing
/// about replaying, because there is nothing waiting to promise anything
/// about.
pub fn waiting_line(contents: Option<&str>) -> String {
    let waiting = contents
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    match waiting {
        0 => NONE_WAITING.to_string(),
        1 => "pns doctor: 1 missed notification is waiting to be replayed; \
             the next event that raises a banner or a card while the operator \
             is not away delivers it."
            .to_string(),
        many => {
            format!(
                "pns doctor: {many} missed notifications are waiting to be replayed; \
                 the next event that raises a banner or a card while the operator \
                 is not away delivers them."
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
    use super::{Entry, entries, entry, should_replay, summary, waiting_line, was_missed};
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

    // --- reading the entries back ------------------------------------------

    #[test]
    fn an_entry_reads_back_into_the_six_values_the_writer_put_there() {
        // FED FROM `entry`'S OWN OUTPUT rather than a hand-written literal, so
        // the writer and the reader can never drift apart in a fixture. BY
        // KEY and never by position, which is what makes the writer's key
        // order (`serde_json`'s business, not this module's) invisible here.
        let written = entry(&event("a private summary"), Some(1_756_500_000));
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
        assert_eq!(entries(&entry(&event("x"), None))[0].at, None);
    }

    #[test]
    fn a_short_entry_reads_its_absent_fields_as_empty_and_a_junk_line_costs_the_batch_nothing() {
        // THE FILE IS A PLAIN FILE anything can reach, and the append's own
        // heal can republish a single line over it. One line nobody can parse
        // must not throw away the notifications around it, and a short object
        // must degrade to a thinner card rather than to no card.
        let read = entries(&format!(
            "{}\nnot JSON at all\n{{\"agent\":\"codex\"}}\n\"a bare string\"\n[1,2]\n\n{}\n",
            entry(&event("the first"), Some(1_756_500_000)),
            entry(&event("the last"), Some(1_756_500_001)),
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
    fn a_bare_entry_renders_its_title_alone_without_a_dangling_colon() {
        // The common bare `done` turn carries no detail, and a card that read
        // "claude \u{b7} done \u{b7} dotfiles:" would look truncated rather
        // than complete. Pins the title-only arm of `rendered`.
        let bare = Entry {
            at: Some(1_000),
            agent: "claude".to_string(),
            state: "done".to_string(),
            project: "dotfiles".to_string(),
            branch: String::new(),
            detail: String::new(),
        };
        let line = summary(&[bare]);
        assert!(
            !line.trim_end().ends_with(':') && !line.contains(": ;") && !line.contains(":;"),
            "a bare entry must not dangle a colon: {line}"
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

    /// One entry of a stated length, carrying a phrase no other entry in its
    /// fixture holds, so an assertion names WHICH entry reached the body.
    fn sized(phrase: &str, padding: usize) -> Entry {
        Entry {
            agent: "claude".to_string(),
            state: "done".to_string(),
            detail: format!("{phrase}{}", "x".repeat(padding)),
            ..Entry::default()
        }
    }

    #[test]
    fn a_summary_too_long_for_the_card_stops_early_and_still_names_the_true_count() {
        // THE COUNT NEVER LIES EITHER WAY: it is the real number even when the
        // body ran out of room, and the body never runs past what a card
        // renders without a cut.
        //
        // THE TWO NEWEST ARE LONG AND THE TWO OLDEST SHORT, which is what makes
        // STOPPING distinguishable from SKIPPING at all. Four entries of ONE
        // length cannot tell the two apart: what does not fit for the newest
        // does not fit for an older one either, so a build that skipped and
        // carried on would render exactly the same body. With a short entry
        // waiting behind a long one, skipping reaches it and prints the oldest
        // news as though the newest were missing.
        let cap = crate::render::PREVIEW_MAX_CHARS;
        let waiting = [
            sized("the oldest short one", 0),
            sized("the second short one", 0),
            sized("the older long one", cap / 2),
            sized("the newest long one", cap / 2),
        ];
        let body = summary(&waiting);
        assert!(
            body.starts_with("4 missed notifications. "),
            "the true count leads: {body}"
        );
        assert!(
            body.chars().count() <= cap,
            "the body is inside what a card renders whole: {} chars",
            body.chars().count()
        );
        assert!(
            body.contains("the newest long one"),
            "the newest entry is what the body spends its room on: {body}"
        );
        // AND NOTHING BEHIND THE ONE THAT DID NOT FIT, which is the assertion a
        // `continue` in place of the `break` fails: it would reach past the
        // older long entry to both short ones.
        for skipped in [
            "the older long one",
            "the second short one",
            "the oldest short one",
        ] {
            assert!(
                !body.contains(skipped),
                "{skipped} rode past the stop: {body}"
            );
        }
    }

    #[test]
    fn a_single_entry_past_the_cap_is_still_delivered_rather_than_becoming_a_bare_count() {
        // MEASURED ON THE SHIPPED BUILD: a 209-character detail put the body
        // one character past the cap, the loop broke before appending
        // anything, and the whole card read "1 missed notification" with no
        // content at all. The one notification the operator missed is exactly
        // what a card is for, so the newest entry goes in whatever its length
        // and `render::preview` takes the cut on the way out, which is the
        // existing, tested path.
        //
        // THE TWO FIXTURES STRADDLE THE CAP BY ONE, and each asserts where it
        // landed rather than trusting a length: 208 characters of detail is the
        // last that fits whole, 209 the first that does not. A cap that moves
        // fails these two lines and names the new number.
        //
        // THE MEASURED SHAPE, card and all: a title of `claude, blocked,
        // dotfiles` is what puts the boundary at 208 characters of detail.
        let carded = |detail: usize| Entry {
            agent: "claude".to_string(),
            state: "blocked".to_string(),
            project: "dotfiles".to_string(),
            detail: "x".repeat(detail),
            ..Entry::default()
        };
        let cap = crate::render::PREVIEW_MAX_CHARS;
        let fits = summary(&[carded(208)]);
        assert_eq!(
            fits.chars().count(),
            cap,
            "the fixture has to land ON the cap: {fits}"
        );
        assert!(fits.contains(&"x".repeat(208)), "{fits}");
        let over = summary(&[carded(209)]);
        assert_eq!(
            over.chars().count(),
            cap + 1,
            "the fixture has to land one PAST the cap: {over}"
        );
        assert!(
            over.starts_with("1 missed notification. "),
            "the count still leads: {over}"
        );
        assert!(
            over.contains(&"x".repeat(209)),
            "the only entry there was never reached the card: {over}"
        );
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
    fn the_waiting_line_counts_the_journal_and_says_the_entries_wait_to_be_replayed() {
        // IT SAYS WHAT IS WAITING, never "you missed N": the prune drops the
        // oldest, so a count of what was truly missed over a long absence is a
        // number this file cannot back.
        //
        // AND IT NAMES WHAT DELIVERS THEM, because this is a promise the
        // binary now keeps: the old sentence ended "nothing replays them yet",
        // which the replay made false the moment it shipped.
        assert_eq!(
            waiting_line(Some(&journal(3))),
            "pns doctor: 3 missed notifications are waiting to be replayed; \
             the next event that raises a banner or a card while the operator \
             is not away delivers them."
        );
        assert_eq!(
            waiting_line(Some(&journal(1))),
            "pns doctor: 1 missed notification is waiting to be replayed; \
             the next event that raises a banner or a card while the operator \
             is not away delivers it."
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
