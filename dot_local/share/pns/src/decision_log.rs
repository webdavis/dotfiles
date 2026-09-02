//! The decision log: why a card did or did not fire, in one line per event.
//!
//! POLICY ONLY, in `doctor.rs`'s style: every function here is a total
//! function of its arguments, with no config, no clock, no environment, no
//! file and no printing. The composition root reads the world, assembles a
//! `Record` out of values the decision ALREADY HAS, appends what comes back
//! and prints what the doctor asks for. This module never learns where the
//! file is.
//!
//! THE RECORD IS THE DECISION'S OWN READINGS, never a second reading taken
//! afterwards. That is why a `Record` is built from a `&Decision` rather than
//! from loose values: two readings of where the operator is can disagree, and
//! an explanation assembled from the later one describes a moment the decision
//! never saw. `engine` owns `GateInputs` for the same reason, and this module
//! depends on it rather than the other way round. The other three types it
//! names (`EventArgs`, `Leg`, `Delivery`) are the crate's own value types,
//! taken exactly as the composition root already holds them so that nothing is
//! transformed on the way in.

use crate::args::EventArgs;
use crate::channels::Delivery;
use crate::engine::{Decision, Overrides};
use crate::routing::Leg;

/// How many decisions the ring keeps, which is also how many the report
/// prints. ONE CONSTANT for both, so the file holds exactly what is read.
///
/// FIVE RATHER THAN ONE, because a single slot does not survive being looked
/// at: between the card the operator wondered about and them typing
/// `pns doctor`, the Stop hook of the session they are typing in fires its own
/// event and overwrites it. Five covers that card through a couple of
/// intervening turns. Raising it is this one number.
pub const KEPT: usize = 5;

/// One decision, as everything needed to write its line. THE STRUCT IS THE
/// SCHEMA, and every field is a value the composition root already holds.
pub struct Record<'a> {
    /// The event as it arrived. Its FREE TEXT never reaches a line: see
    /// `line`.
    pub event: &'a EventArgs,
    /// What the engine decided, carrying the readings it decided from.
    pub decision: &'a Decision,
    /// The overrides that decision ran under, parsed once at the edge.
    pub overrides: &'a Overrides,
    /// What each dispatched leg's channel had to say. Empty is a plan that
    /// reached no channel at all, which is the case this log exists for.
    pub legs: &'a [(Leg, Delivery)],
    /// Whether this line is a NUDGE about an approval already recorded rather
    /// than the approval's own first card.
    ///
    /// WITHOUT IT THE RING HOLDS TWO INDISTINGUISHABLE LINES. One prompt that
    /// went unanswered leaves two `claude/blocked` entries differing in nothing
    /// an operator can see, and "why did I get two cards for one prompt" is the
    /// exact question this log exists to answer. It is a BOOLEAN and no free
    /// text is added, so the file's privacy rule is untouched.
    pub nag: bool,
    /// The harness payload's own permission mode, empty when the event carries
    /// none: see `HookPayload::permission_mode`.
    pub permission_mode: &'a str,
    /// The harness payload's own subagent id, empty on the main thread: see
    /// `HookPayload::agent_id`.
    pub agent_id: &'a str,
    /// The harness payload's own raw tool name, empty when the event names
    /// none: see `HookPayload::tool_name`.
    pub tool_name: &'a str,
}

/// One decision as one line: `<epoch> <key=value ...>`.
///
/// NO FREE TEXT REACHES IT. The detail, the branch, the project and the pane
/// id are the operator's own content, and this file is printed to a terminal
/// by `pns doctor`, so recording them would put that content into a state file
/// and then onto a screen. The pane appears as the two booleans the decision
/// actually used it for. Every other value here is a number, a boolean, an
/// enum name or a plugin name out of the compiled roster.
///
/// NOT JSON, though the crate already carries a JSON writer. The only reader
/// is the section below, whose whole parse is one `split_once(' ')` over the
/// epoch; a JSON round trip would add a schema and an error taxonomy to render
/// a sentence that reads as it stands.
///
/// NO actionId IS RECORDED, because pns never has one: the notification seam
/// answers a bool and drops the body, and on the approval path moshi mints the
/// id inside itself and answers with an exit code.
pub fn line(record: &Record) -> String {
    let inputs = record.decision.inputs;
    let overrides = record.overrides;
    format!(
        "{epoch} {agent}/{state} \
         mode={mode} agent={payload_agent} tool={tool} \
         surface={surface:?} visibility={visibility:?} \
         session_visibility={session_visibility:?} \
         desk_age={desk_age} phone_age={phone_age} tap_age={tap_age} \
         locked={locked} fresh_window={fresh_window} long_running={long_running} \
         nag={nag} \
         local_only={local_only} remote_only={remote_only} \
         pane={pane} pane_dropped={pane_dropped} watch_card={watch_card} \
         muted={muted} focus={focus} skip_phone={skip_phone} force_phone={force_phone} \
         idle_invalid={idle_invalid} desk_invalid={desk_invalid} \
         phone_invalid={phone_invalid} \
         plan=banner:{banner},card:{card},pulse:{pulse} legs={legs}",
        epoch = inputs
            .now_secs
            .map_or_else(|| NO_CLOCK.to_string(), |now| now.to_string()),
        agent = printable(&record.event.agent),
        state = printable(&record.event.state),
        // THE PAYLOAD'S OWN IDENTITY, PRINTABLE-FILTERED LIKE `agent`/`state`
        // ABOVE: `tool_name` is remote text a connected MCP server named, so
        // it gets the same allowlist rather than a free pass into a file
        // `pns doctor` prints straight to a terminal.
        mode = printable(record.permission_mode),
        payload_agent = printable(record.agent_id),
        tool = printable(record.tool_name),
        // A FIELDLESS DERIVED `Debug` IS THE VARIANT NAME, which is exactly
        // what an enum reads as here.
        surface = inputs.surface,
        visibility = inputs.visibility,
        session_visibility = inputs.session_visibility,
        desk_age = count(inputs.desk_input_age),
        phone_age = count(inputs.phone_input_age),
        tap_age = count(inputs.marker_age),
        locked = tri(inputs.screen_locked),
        fresh_window = count(inputs.desk_fresh_secs),
        long_running = yes_no(inputs.long_running),
        nag = yes_no(record.nag),
        local_only = yes_no(inputs.local_only),
        remote_only = yes_no(inputs.remote_only),
        // THE PANE AS THE DECISION USED IT and no further: its value is a
        // multiplexer id this crate does not own, and these two booleans are
        // everything the decision read out of it.
        pane = if inputs.pane_present {
            "present"
        } else {
            ABSENT
        },
        pane_dropped = yes_no(record.decision.pane_dropped),
        watch_card = yes_no(inputs.mobile_watch_card),
        muted = yes_no(overrides.muted),
        // TWO FIELDS RATHER THAN ONE. The log exists to answer "why did no
        // card fire", and "you have a `pns quiet` running" sends the operator
        // somewhere completely different from "your Mac is in a Focus you told
        // pns to respect".
        focus = yes_no(overrides.focus_active),
        skip_phone = yes_no(overrides.skip_phone),
        force_phone = yes_no(overrides.force_phone),
        idle_invalid = yes_no(overrides.idle_invalid),
        desk_invalid = yes_no(overrides.desk_invalid),
        phone_invalid = yes_no(overrides.phone_invalid),
        banner = yes_no(record.decision.plan.banner),
        card = yes_no(record.decision.plan.phone_card),
        pulse = yes_no(record.decision.plan.pulse),
        legs = verdicts(record.legs),
    )
}

/// One `plugin:verdict` per dispatched leg, in delivery order.
///
/// THE VARIANT NAME AND NEVER THE SENTENCE. A channel's own words can carry a
/// status code or a URL, and this file is printed by `pns doctor`; the variant
/// is the verdict anyway, which is why `Delivery` keeps the two apart. The
/// plugin name comes out of the compiled roster, so nothing here can carry a
/// newline.
fn verdicts(legs: &[(Leg, Delivery)]) -> String {
    if legs.is_empty() {
        return ABSENT.to_string();
    }
    legs.iter()
        .map(|(leg, delivery)| {
            let verdict = match delivery {
                Delivery::Delivered(_) => "delivered",
                Delivery::Failed(_) => "failed",
                Delivery::Unlaunched(_) => "unlaunched",
                Delivery::Silent => "silent",
            };
            format!("{}:{verdict}", leg.name)
        })
        .collect::<Vec<String>>()
        .join(",")
}

/// The only text a line carries, filtered to what may be PRINTED.
///
/// `agent` and `state` come from argv and are what identify which card the
/// operator is asking about, so they cannot be reduced to a boolean the way
/// the rest of the event is. Everything else on a line is a number, a boolean,
/// an enum name or a plugin name out of the compiled roster.
///
/// A NEWLINE IS THE ONE THAT MATTERS: this file is one record per line, so a
/// value carrying one FORGES a second entry that the reader cannot tell from a
/// real decision. An escape sequence is the other, because `pns doctor` prints
/// these straight to a terminal.
///
/// DELIBERATELY NOT `safety::route_name_is_usable`. That predicate's doc
/// comment says it exists so ONE rule judges route names, and borrowing it for
/// "what may be printed into a report" would make it two rules wearing one
/// spelling: they would then be changed for one caller and silently applied to
/// the other. This is the new rule, and printing is what it is for.
///
/// THE WHOLE VALUE IS JUDGED BEFORE ANYTHING IS TRUNCATED, which is also what
/// makes the truncation safe: every accepted byte is ASCII, so a cut at
/// `IDENTITY_MAX` can never land inside a multi-byte character.
fn printable(text: &str) -> String {
    if text.is_empty() {
        return ABSENT.to_string();
    }
    if !text
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'))
    {
        return UNPRINTABLE.to_string();
    }
    text.chars().take(IDENTITY_MAX).collect()
}

/// What a value outside the allowlist is recorded as. The line still names
/// the decision it belonged to, which is more than dropping the entry would.
const UNPRINTABLE: &str = "unprintable";

/// The longest agent or state a line carries. Both are short names in every
/// producer this repo owns; the cap is what stops an argv nobody validated
/// from filling the ring with one entry.
const IDENTITY_MAX: usize = 32;

/// A clock nobody could read, in the field an epoch second would hold. It is
/// a RECOGNIZED value rather than epoch zero, which would parse cleanly and
/// render as 56 years ago, and rather than an empty field, which the reader
/// could not tell from a line it failed to parse.
const NO_CLOCK: &str = "-";

/// A reading nobody could take, spelled the one way everywhere in a line. It
/// is never a zero: `0` reads as "touched this instant", which is a claim
/// about a measurement that never happened.
const ABSENT: &str = "none";

fn count(reading: Option<u64>) -> String {
    reading.map_or_else(|| ABSENT.to_string(), |value| value.to_string())
}

/// A boolean reading that may also be absent, which is three states and never
/// two: an unread lock is not an unlocked one.
fn tri(reading: Option<bool>) -> &'static str {
    match reading {
        Some(locked) => yes_no(locked),
        None => ABSENT,
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

/// The decision log as the doctor's own section: a heading and one rendered
/// entry per line, newest first, capped at `KEPT`.
///
/// `contents` is the file, `None` when there is none. `now` is the reader's
/// own clock, which ages the entries and nothing else.
///
/// IT REPORTS HISTORY, NEVER HEALTH. Nothing here reaches an exit code: an
/// empty log on a fresh machine is not a failure, and neither is one this
/// could not parse.
pub fn section(contents: Option<&str>, now: Option<u64>) -> Vec<String> {
    let entries: Vec<&str> = contents
        .unwrap_or_default()
        .lines()
        .filter(|entry| !entry.trim().is_empty())
        .collect();
    if entries.is_empty() {
        return vec![NOTHING_RECORDED.to_string()];
    }
    // NEWEST FIRST, because the ring is written by APPEND and the operator
    // came to look at the card that just did or did not arrive.
    let shown: Vec<&&str> = entries.iter().rev().take(KEPT).collect();
    let mut rendered = vec![heading(shown.len())];
    rendered.extend(shown.into_iter().map(|entry| render(entry, now)));
    rendered
}

/// The section's own first line, counting what it is ABOUT TO SHOW rather than
/// the cap: a heading claiming five over one entry invents four decisions.
fn heading(shown: usize) -> String {
    let counted = if shown == 1 {
        "the last decision,".to_string()
    } else {
        format!("the last {shown} decisions,")
    };
    format!("pns doctor: {counted}{HEADING_TAIL}")
}

/// The rest of every heading. THE actionId IS TOLD HONESTLY rather than
/// printed as an empty field: pns never has one, because moshi mints it inside
/// the approval round trip and answers with an exit code.
const HEADING_TAIL: &str = " newest first (why a card did or did not fire). No actionId \
     is recorded: moshi mints it inside the approval round trip and never hands it back.";

/// One entry, as its age and the rest of the line it was written as.
///
/// THE BODY IS NEVER PARSED, only ESCAPED and printed. The whole reader is
/// the one split below, which is what keeps a format change in `line` from
/// needing a matching change here.
fn render(entry: &str, now: Option<u64>) -> String {
    let Some((stamp, rest)) = entry.split_once(' ') else {
        return complaint(entry);
    };
    let recorded = if stamp == NO_CLOCK {
        None
    } else {
        match crate::parse_count(stamp) {
            Some(recorded) => Some(recorded),
            // NOT DROPPED SILENTLY: a log that hides the one entry that
            // mattered is worse than one that says it cannot read it.
            None => return complaint(entry),
        }
    };
    format!("  {}: {}", age(recorded, now), escaped(rest))
}

/// THE ONE ESCAPE RULE for text out of the ring, and the reason it is a
/// function rather than two spellings: a parsed entry and an unparsable one
/// go to the SAME terminal, so a rule applied to only one of them is a rule
/// the other arm quietly does not have. Measured before this existed: an
/// entry whose epoch parsed printed its ESC and BEL bytes to the terminal
/// raw, while its unparsable neighbour on the line above was escaped.
///
/// Rust's own debug escaping is the rule, without the quotes `complaint`
/// wraps its half in: nothing here writes a format, it makes a control byte
/// visible as the characters that spell it.
fn escaped(text: &str) -> String {
    text.escape_debug().to_string()
}

/// How long ago, in the largest unit that still reads as a count. Absent at
/// either end means NO AGE IS INVENTED: an entry written with no clock, and a
/// reader with no clock of its own, are both unknowable rather than zero.
fn age(recorded: Option<u64>, now: Option<u64>) -> String {
    let (Some(recorded), Some(now)) = (recorded, now) else {
        return UNKNOWN_AGE.to_string();
    };
    let seconds = now.saturating_sub(recorded);
    match seconds {
        ..60 => format!("{seconds}s ago"),
        60..3_600 => format!("{}m ago", seconds / 60),
        _ => format!("{}h ago", seconds / 3_600),
    }
}

/// An entry this cannot read, QUOTED AND TRUNCATED. The quotes are this arm's
/// own, marking off a fragment of a file from the sentence around it; the
/// escaping inside them is `escaped`, the same rule the readable arm runs.
fn complaint(entry: &str) -> String {
    let held: String = entry.chars().take(QUOTED_MAX).collect();
    format!("  unreadable entry: \"{}\"", escaped(&held))
}

/// How much of an unreadable entry is quoted back. Enough to recognize, short
/// enough that a file of garbage cannot fill the report.
const QUOTED_MAX: usize = 60;

/// A reader with no clock of its own, and an entry written without one.
const UNKNOWN_AGE: &str = "age unknown";

/// What an absent log says. THE PARENTHESIS IS THE HONEST HALF: the write is
/// fail-quiet, so nothing here can tell an unused log from one that could not
/// be written, and the line must not claim the first.
const NOTHING_RECORDED: &str = "pns doctor: no decision has been recorded yet \
     (no event has run since this was installed, or none could be written).";

#[cfg(test)]
mod tests {
    use super::{KEPT, Record, line, section};
    use crate::args::EventArgs;
    use crate::channels::Delivery;
    use crate::engine::{Decision, GateInputs, Overrides};
    use crate::routing::{Leg, ReportMode};
    use crate::surface::{DeliveryPlan, Surface, Visibility};

    /// The readings behind one decision, distinct in every field so a swap
    /// between two of them cannot pass.
    fn inputs() -> GateInputs {
        GateInputs {
            now_secs: Some(1_756_500_000),
            desk_input_age: None,
            phone_input_age: Some(12),
            marker_age: None,
            screen_locked: Some(false),
            desk_fresh_secs: Some(120),
            surface: Surface::Mobile,
            session_visibility: Visibility::Visible,
            visibility: Visibility::Hidden,
            long_running: false,
            mobile_watch_card: false,
            local_only: false,
            remote_only: false,
            pane_present: true,
        }
    }

    fn decision(inputs: GateInputs) -> Decision {
        Decision {
            legs: Vec::new(),
            plan: DeliveryPlan {
                banner: false,
                phone_card: false,
                pulse: false,
            },
            pane_dropped: false,
            inputs,
        }
    }

    fn event() -> EventArgs {
        EventArgs {
            agent: "claude".to_string(),
            state: "blocked".to_string(),
            ..EventArgs::default()
        }
    }

    #[test]
    fn a_line_names_the_event_and_every_gate_input_behind_one_epoch_second() {
        // EVERY VALUE IS A NUMBER, A BOOLEAN OR AN ENUM NAME, so the only
        // reader this file has can print it without interpreting it, and a
        // reading nobody could take stays absent instead of becoming a zero.
        let plain = decision(inputs());
        let overrides = Overrides {
            skip_phone: true,
            ..Overrides::default()
        };
        assert_eq!(
            line(&Record {
                event: &event(),
                decision: &plain,
                overrides: &overrides,
                legs: &[],
                nag: false,
                permission_mode: "",
                agent_id: "",
                tool_name: "",
            }),
            "1756500000 claude/blocked mode=none agent=none tool=none surface=Mobile visibility=Hidden \
             session_visibility=Visible desk_age=none phone_age=12 tap_age=none locked=no \
             fresh_window=120 long_running=no nag=no local_only=no remote_only=no pane=present \
             pane_dropped=no watch_card=no muted=no focus=no skip_phone=yes force_phone=no \
             idle_invalid=no desk_invalid=no phone_invalid=no \
             plan=banner:no,card:no,pulse:no legs=none"
        );

        // AN UNREAD LOCK IS ITS OWN ROW, byte for byte. A `locked=no` here
        // would be the line claiming the display was awake on a reading the
        // decision never took, which is the one thing `tri` exists to stop.
        let unread_lock = decision(GateInputs {
            screen_locked: None,
            ..inputs()
        });
        assert_eq!(
            line(&Record {
                event: &event(),
                decision: &unread_lock,
                overrides: &overrides,
                legs: &[],
                nag: false,
                permission_mode: "",
                agent_id: "",
                tool_name: "",
            }),
            "1756500000 claude/blocked mode=none agent=none tool=none surface=Mobile visibility=Hidden \
             session_visibility=Visible desk_age=none phone_age=12 tap_age=none locked=none \
             fresh_window=120 long_running=no nag=no local_only=no remote_only=no pane=present \
             pane_dropped=no watch_card=no muted=no focus=no skip_phone=yes force_phone=no \
             idle_invalid=no desk_invalid=no phone_invalid=no \
             plan=banner:no,card:no,pulse:no legs=none"
        );
    }

    #[test]
    fn a_line_carries_the_payloads_mode_agent_and_tool_or_says_none() {
        // WHY: three `claude/blocked` events lined up with subagent
        // hand-offs, not with any prompt the operator saw (OBS-4), and the
        // decision log had no field that could ever tell those apart from an
        // ordinary approval.
        let plain = decision(inputs());
        let recorded = line(&Record {
            event: &event(),
            decision: &plain,
            overrides: &Overrides::default(),
            legs: &[],
            nag: false,
            permission_mode: "bypassPermissions",
            agent_id: "agent_01",
            tool_name: "Bash",
        });
        assert!(
            recorded.contains(" mode=bypassPermissions agent=agent_01 tool=Bash "),
            "got {recorded}"
        );

        // AND EVERY FIELD A PAYLOAD DID NOT STATE READS `none`, never a blank:
        // an empty field in the middle of a space-delimited line is
        // indistinguishable from a line one field short.
        let recorded = line(&Record {
            event: &event(),
            decision: &plain,
            overrides: &Overrides::default(),
            legs: &[],
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        });
        assert!(
            recorded.contains(" mode=none agent=none tool=none "),
            "got {recorded}"
        );
    }

    #[test]
    fn a_line_with_no_readable_clock_leads_with_a_dash_rather_than_epoch_zero() {
        // A RECOGNIZED VALUE, so the reader can tell it from a line it could
        // not parse. Epoch zero would parse cleanly and render as 56 years
        // ago, which is a claim nobody measured.
        let decision = decision(GateInputs {
            now_secs: None,
            ..inputs()
        });
        let recorded = line(&Record {
            event: &event(),
            decision: &decision,
            overrides: &Overrides::default(),
            legs: &[],
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        });
        assert!(
            recorded.starts_with("- claude/blocked "),
            "got {recorded:?}"
        );
    }

    #[test]
    fn no_free_text_reaches_a_line_and_the_pane_appears_only_as_two_booleans() {
        // THE OPERATOR'S OWN CONTENT: a tool call, a reply, a working
        // directory, a branch name. `pns doctor` PRINTS this file, so anything
        // recorded here lands in a state file and then on a terminal. Every
        // field of `EventArgs` outside `agent` and `state` is checked, the
        // narrowing flags included, because they reach the line through the
        // decision's own inputs and never through the event.
        let event = EventArgs {
            agent: "claude".to_string(),
            state: "blocked".to_string(),
            project: "SECRETPROJECT".to_string(),
            branch: "SECRETBRANCH".to_string(),
            detail: "SECRETDETAIL".to_string(),
            pane: "wW:pSECRETPANE".to_string(),
            channel: "SECRETCHANNEL".to_string(),
            local_only: true,
            remote_only: true,
            long_running: true,
            help: false,
        };
        let decision = decision(GateInputs {
            pane_present: true,
            ..inputs()
        });
        let recorded = line(&Record {
            event: &event,
            decision: &decision,
            overrides: &Overrides::default(),
            legs: &[],
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        });
        for content in [
            "SECRETPROJECT",
            "SECRETBRANCH",
            "SECRETDETAIL",
            "SECRETPANE",
            "wW",
            "SECRETCHANNEL",
        ] {
            assert!(
                !recorded.contains(content),
                "{content} reached the record: {recorded}"
            );
        }
        assert!(
            recorded.contains(" pane=present pane_dropped=no "),
            "{recorded}"
        );
    }

    #[test]
    fn an_agent_or_state_outside_the_printable_allowlist_is_recorded_as_unprintable() {
        // THE TWO VALUES THAT COME FROM ARGV, and the only text in a line.
        // A NEWLINE IS THE DANGEROUS ONE: this file is one record per line, so
        // a value carrying one forges a second entry. An escape sequence is the
        // other: `pns doctor` prints these to a terminal.
        let identity = |agent: &str, state: &str| {
            let event = EventArgs {
                agent: agent.to_string(),
                state: state.to_string(),
                ..EventArgs::default()
            };
            let decision = decision(inputs());
            let recorded = line(&Record {
                event: &event,
                decision: &decision,
                overrides: &Overrides::default(),
                legs: &[],
                nag: false,
                permission_mode: "",
                agent_id: "",
                tool_name: "",
            });
            recorded
                .split_whitespace()
                .nth(1)
                .unwrap_or_default()
                .to_string()
        };

        assert_eq!(identity("claude", "blocked"), "claude/blocked");
        assert_eq!(identity("codex-2.tui_1", "done"), "codex-2.tui_1/done");
        // NO SPACE IN EITHER: a space is refused on its own, which would
        // mask the newline this row is actually about.
        for forged in ["claude\n1756500000", "claude\rfake"] {
            assert_eq!(
                identity(forged, "done"),
                "unprintable/done",
                "a newline forges a second entry"
            );
        }
        assert_eq!(identity("claude", "\u{1b}[31mred"), "claude/unprintable");
        assert_eq!(identity("cl aude", "done"), "unprintable/done");
        assert_eq!(identity("clau\u{00e9}de", "done"), "unprintable/done");
        // AN EMPTY VALUE is absent, not unprintable: a bare `pns` names no
        // agent and no state, and there is nothing there to refuse.
        assert_eq!(identity("", ""), "none/none");
        // AN OVER-LONG ONE IS TRUNCATED, and the allowlist runs over the whole
        // value first, so a truncation can never land mid-character.
        assert_eq!(
            identity(&"a".repeat(40), "done"),
            format!("{}/done", "a".repeat(32))
        );
        // AND THE ORDER IS JUDGE THEN TRUNCATE, never the reverse. A clean
        // 32-character head with a newline at position 40 passes any check
        // that runs on the cut value, and the cut value is then written as a
        // real agent name while the forged entry rides in behind it. Cutting
        // first is also a panic hazard the moment a cut lands mid-character.
        assert_eq!(
            identity(
                &format!("{}\n1756500000 forged/entry", "a".repeat(40)),
                "done"
            ),
            "unprintable/done",
            "the tail is judged too, not only the 32 characters that survive"
        );
    }

    #[test]
    fn a_line_carries_the_arbitrated_plan_and_each_legs_verdict() {
        // WITHOUT THE LEGS the log says pns decided to card the operator while
        // their question is why no card appeared. THE VERDICT IS THE VARIANT
        // NAME, never the channel's sentence, which can carry a status code or
        // a URL.
        let carded = decision(inputs());
        let carded = Decision {
            plan: DeliveryPlan {
                banner: false,
                phone_card: true,
                pulse: false,
            },
            ..carded
        };
        // THE DECORATION FLAG IS THE ROSTER'S OWN: the phone and the banner
        // show the operator something, the durable log and an unknown channel
        // do not. Nothing in a ring line reads it, which is exactly why it is
        // stated honestly here rather than defaulted.
        let legs = [
            (
                Leg {
                    name: "mobile",
                    mode: ReportMode::Silent,
                    decorative: true,
                },
                Delivery::Failed("the gateway answered 502 at https://example.invalid".to_string()),
            ),
            (
                Leg {
                    name: "hermes",
                    mode: ReportMode::Silent,
                    decorative: false,
                },
                Delivery::Delivered("posted".to_string()),
            ),
            (
                Leg {
                    name: "macos-banner",
                    mode: ReportMode::Silent,
                    decorative: true,
                },
                Delivery::Silent,
            ),
            (
                Leg {
                    name: "kitchen",
                    mode: ReportMode::Silent,
                    decorative: false,
                },
                Delivery::Unlaunched("no such channel".to_string()),
            ),
        ];
        let recorded = line(&Record {
            event: &event(),
            decision: &carded,
            overrides: &Overrides::default(),
            legs: &legs,
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        });
        assert!(
            recorded.ends_with(
                " plan=banner:no,card:yes,pulse:no \
                 legs=mobile:failed,hermes:delivered,macos-banner:silent,kitchen:unlaunched"
            ),
            "got {recorded}"
        );
        assert!(
            !recorded.contains("502"),
            "the sentence stays out: {recorded}"
        );
        assert!(
            !recorded.contains("example.invalid"),
            "and its URL with it: {recorded}"
        );

        // A PLAN THAT REACHED NO CHANNEL still records, and says so.
        let recorded = line(&Record {
            event: &event(),
            decision: &decision(inputs()),
            overrides: &Overrides::default(),
            legs: &[],
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        });
        assert!(
            recorded.ends_with(" plan=banner:no,card:no,pulse:no legs=none"),
            "got {recorded}"
        );
    }

    #[test]
    fn a_section_over_no_contents_says_no_decision_has_been_recorded_and_names_nothing() {
        // THE PARENTHESIS IS THE HONEST HALF. The write is fail-quiet, so an
        // absent log cannot be told from an unused one, and a line claiming
        // "no event has run" alone would be a guess presented as a finding.
        assert_eq!(
            section(None, Some(1_756_500_000)),
            vec![
                "pns doctor: no decision has been recorded yet (no event has run since this \
                 was installed, or none could be written)."
            ]
        );
        // A FILE THAT EXISTS AND HOLDS NOTHING is the same state and says the
        // same thing rather than printing an empty heading over no entries.
        assert_eq!(section(Some(""), None), section(None, None));
        assert_eq!(section(Some("\n\n"), None), section(None, None));
    }

    /// Seven decisions, oldest first, the order an append leaves them in.
    const SEVEN: &str = "1756400000 a/one surface=Desk\n\
                         1756490000 b/two surface=Desk\n\
                         1756499000 c/three surface=Desk\n\
                         1756499900 d/four surface=Desk\n\
                         1756499970 e/five surface=Desk\n\
                         1756499990 f/six surface=Desk\n\
                         1756500000 g/seven surface=Desk\n";

    const HEADING_TAIL: &str = " newest first (why a card did or did not fire). No actionId \
         is recorded: moshi mints it inside the approval round trip and never hands it back.";

    #[test]
    fn a_section_renders_the_newest_entry_first_capped_at_the_kept_count_with_each_ones_age() {
        // NEWEST FIRST IS THE READING ORDER: the operator came to look at the
        // card that just did or did not arrive, and the ring is written by
        // append, so the file's own order is the opposite of the useful one.
        assert_eq!(
            section(Some(SEVEN), Some(1_756_500_000)),
            vec![
                format!("pns doctor: the last {KEPT} decisions,{HEADING_TAIL}"),
                "  0s ago: g/seven surface=Desk".to_string(),
                "  10s ago: f/six surface=Desk".to_string(),
                "  30s ago: e/five surface=Desk".to_string(),
                "  1m ago: d/four surface=Desk".to_string(),
                "  16m ago: c/three surface=Desk".to_string(),
            ],
            "the two oldest are gone and the newest leads"
        );
    }

    #[test]
    fn a_section_counts_the_entries_it_actually_shows_rather_than_the_cap() {
        // A heading claiming five over one entry would be the report inventing
        // four decisions nobody took.
        assert_eq!(
            section(
                Some("1756499000 c/three surface=Desk\n"),
                Some(1_756_500_000)
            ),
            vec![
                format!("pns doctor: the last decision,{HEADING_TAIL}"),
                "  16m ago: c/three surface=Desk".to_string(),
            ]
        );
        assert_eq!(
            section(Some(SEVEN), Some(1_756_500_000)).len(),
            KEPT + 1,
            "one heading over the kept count"
        );
    }

    #[test]
    fn a_section_ages_an_entry_in_the_largest_unit_that_still_reads_as_a_count() {
        // Hours, because a five-deep ring on a machine used a few times a week
        // holds day-old entries, and "4320m ago" makes the reader do the
        // arithmetic the report exists to save them.
        for (recorded, expected) in [
            (1_756_499_999_u64, "1s ago"),
            (1_756_499_941, "59s ago"),
            (1_756_499_940, "1m ago"),
            (1_756_496_401, "59m ago"),
            (1_756_496_400, "1h ago"),
            (1_756_400_000, "27h ago"),
        ] {
            assert_eq!(
                section(
                    Some(&format!("{recorded} a/one x=1\n")),
                    Some(1_756_500_000)
                )[1],
                format!("  {expected}: a/one x=1"),
                "recorded at {recorded}"
            );
        }
    }

    #[test]
    fn a_section_quotes_an_entry_it_cannot_read_and_still_renders_its_readable_neighbours() {
        // DROPPING IT SILENTLY is how a log loses the one entry that mattered,
        // and it is also how a truncated write disappears without a trace.
        let mixed = "1756499000 a/one surface=Desk\n\
                     no-space-anywhere\n\
                     1756499900 b/two surface=Desk\n\
                     notanepoch c/three surface=Desk\n";
        assert_eq!(
            section(Some(mixed), Some(1_756_500_000)),
            vec![
                format!("pns doctor: the last 4 decisions,{HEADING_TAIL}"),
                "  unreadable entry: \"notanepoch c/three surface=Desk\"".to_string(),
                "  1m ago: b/two surface=Desk".to_string(),
                "  unreadable entry: \"no-space-anywhere\"".to_string(),
                "  16m ago: a/one surface=Desk".to_string(),
            ]
        );
    }

    #[test]
    fn an_unreadable_entry_is_quoted_short_and_with_its_control_bytes_escaped() {
        // The report goes to a terminal, and a file this never wrote can hold
        // anything: a hand edit, a truncated write, another program's output.
        let rendered = section(Some("\u{1b}[31mred\tand\u{7}long\n"), Some(1_756_500_000));
        assert_eq!(
            rendered[1], "  unreadable entry: \"\\u{1b}[31mred\\tand\\u{7}long\"",
            "escaped rather than executed by the terminal"
        );
        // AND BOUNDED, so a file of garbage cannot fill the report.
        let long = format!("{}\n", "z".repeat(500));
        assert_eq!(
            section(Some(&long), Some(1_756_500_000))[1],
            format!("  unreadable entry: {:?}", "z".repeat(60))
        );
    }

    #[test]
    fn a_parsed_entrys_body_is_escaped_by_the_same_rule_an_unreadable_one_is() {
        // ONE ESCAPE RULE FOR BOTH ARMS. The body of a PARSED entry used to be
        // printed verbatim, so a hand-edited ring holding an escape sequence
        // reached the terminal raw from `pns doctor` as long as its epoch
        // parsed. The bytes are the point: what comes out is the characters
        // that spell the escape, not the escape.
        let rendered = section(
            Some("1756500000 a/one \u{1b}[31mred\u{7}\tand\u{8}back\n"),
            Some(1_756_500_000),
        );
        assert_eq!(
            rendered[1],
            "  0s ago: a/one \\u{1b}[31mred\\u{7}\\tand\\u{8}back"
        );
        for raw in ['\u{1b}', '\u{7}', '\u{8}', '\t'] {
            assert!(
                !rendered[1].contains(raw),
                "{raw:?} reached the terminal: {:?}",
                rendered[1]
            );
        }
    }

    #[test]
    fn a_section_invents_no_age_for_an_entry_or_a_reader_that_had_no_clock() {
        // TWO DIFFERENT MISSING CLOCKS, and neither may become a number. The
        // dash is a RECOGNIZED value, so an entry written without a clock is
        // rendered as itself rather than quoted back as unreadable.
        assert_eq!(
            section(Some("- a/one surface=Away\n"), Some(1_756_500_000)),
            vec![
                format!("pns doctor: the last decision,{HEADING_TAIL}"),
                "  age unknown: a/one surface=Away".to_string(),
            ]
        );
        // THE READER'S OWN CLOCK, absent: every entry is unaged, and none is
        // dropped or complained about.
        assert_eq!(
            section(Some(SEVEN), None),
            vec![
                format!("pns doctor: the last {KEPT} decisions,{HEADING_TAIL}"),
                "  age unknown: g/seven surface=Desk".to_string(),
                "  age unknown: f/six surface=Desk".to_string(),
                "  age unknown: e/five surface=Desk".to_string(),
                "  age unknown: d/four surface=Desk".to_string(),
                "  age unknown: c/three surface=Desk".to_string(),
            ]
        );
    }
}
