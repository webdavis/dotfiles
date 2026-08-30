//! The pns binary: the composition root, and the only place with a main.
//!
//! Everything here is WIRING. The roster is one constant and one constructor
//! in `registry`, so there is no second construction of it to diverge; the
//! environment and the config are read once at this edge, and every decision
//! is delegated to the library. It exits 0 on every path, because a
//! notification must never fail the work it reports on.

use std::collections::BTreeMap;
use std::io::{Read, Seek, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pns::args::parse_args;
use pns::channels::banner::BannerChannel;
use pns::channels::hermes::{
    DEFAULT_HERMES_URL, HermesChannel, UreqSignedPost, channel_url, hermes_secret, remote_deadline,
};
use pns::channels::hue::{Bridge, HuePulse, hue_settings, quiet_now, quiet_window};
use pns::channels::moshi::{DEFAULT_MOSHI_URL, MoshiChannel, UreqPost, moshi_secret};
use pns::channels::{Delivery, native_first};
use pns::config::{LoadOutcome, config_path, load_config};
use pns::engine::{Overrides, decide};
use pns::hooks::{
    HookPayload, condenser_prompt, condenser_verdict, moshi_subcommand, parse_payload,
    transcript_reply,
};
use pns::registry::{roster, select_plugins};
use pns::render;
use pns::system::{
    PROBE_READ_MAX, SystemCommandRunner, SystemProbes, local_minutes_since_midnight, run_bounded,
};

fn main() {
    // The pulse is a MODE, not a leg: it fires on a long command's exit code
    // rather than on an event, so it leaves before any of the event wiring.
    let first = std::env::args_os().nth(1).unwrap_or_default();
    if first == *"pulse" {
        pulse_mode();
        return;
    }
    // The home diagnostic: one reading of the router, said out loud. The
    // doctor mode (P3) will absorb it; until then this is how the probe is
    // drilled and how a wrong config is diagnosed.
    if first == *"home" {
        home_mode();
        return;
    }
    // The operator's mute, typed and timed. Also a MODE: it writes the state
    // the event path reads, and delivers nothing itself.
    if first == *"quiet" {
        std::process::exit(quiet_mode());
    }
    // One test send through every configured channel, and one line per
    // registered plugin about it. A MODE for the same reason the others are:
    // it takes no decision, so nothing about an event's plan reaches it.
    if first == *"doctor" {
        std::process::exit(doctor_mode());
    }
    // The return recap, rendered from the activity ring and posted to Discord.
    // A MODE for the reason the others are: it takes no decision, so no event's
    // plan reaches it. The event path starts it detached; an operator can also
    // run it by hand, which is how it is drilled.
    if first == *"recap" {
        std::process::exit(recap_mode());
    }
    // The gate moshi's OWN extension calls. pi and omp spawn
    // `helperBinary pi-hook`, and that field holds one PATHNAME with no room
    // for a subcommand, so the binary answers the bare harness word itself.
    let first = first.to_string_lossy().into_owned();
    if pns::hooks::is_harness_subcommand(&first) {
        std::process::exit(gate_mode(&first));
    }
    // The same gate, spelled the way an operator reads it. Both forms end in
    // gate_mode, which REFUSES a word it will not vouch for: falling through
    // to the event path instead is how the documented spelling used to fire a
    // notification about an empty event.
    if first == *"gate" {
        std::process::exit(gate_mode(&second_argument()));
    }
    if first == *"hook" {
        std::process::exit(hook_mode(&second_argument()));
    }
    event_mode();
}

/// The word after the subcommand, or empty when there is none.
fn second_argument() -> String {
    std::env::args_os()
        .nth(2)
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// A presence-gated pass-through to moshi-hook, for the harnesses that reach
/// it directly rather than through a pns hook.
///
/// EXIT 0 MEANS "NOT FORWARDED" on every path that declines (no moshi, the
/// operator at the desk, a subcommand this will not vouch for), which is the
/// harness's "no opinion, prompt as usual". The forwarded path is the one
/// place a non-zero exit is correct: there it is MOSHI'S OWN CODE, passed
/// through for whatever reads it, and in production it is 0 whichever way the
/// operator answered. See `moshi_decision` for why, and `answer_within` for
/// why the wait on it is bounded.
fn gate_mode(subcommand: &str) -> i32 {
    if !pns::hooks::is_harness_subcommand(subcommand) || !forward_to_moshi(&system_probes()) {
        return 0;
    }
    let Some(payload) = read_payload().filter(|payload| payload_is_whole(payload)) else {
        return 0;
    };
    // BOUNDED AT THE SHARED SEAM, not here: pi and omp reach this entry point
    // with no pns hook in front of it, and a guard at the other caller alone
    // would leave this one hanging.
    spawn_moshi_hook(subcommand, &payload)
        .map_or(0, |child| answer_within(child, submit_deadline()))
}

/// A harness event, from the payload on stdin.
///
/// THE EXIT CONTRACT AND ITS ONE EXCEPTION. Every path here is a notification,
/// and a notification that cannot be delivered must never fail the turn it
/// reports on, so every path returns 0. The forwarded blocking path is the
/// exception: there the exit code is MOSHI'S OWN, passed through untouched for
/// whatever reads it. It is NOT the operator's decision, which arrives by the
/// road `moshi_decision` describes, and it is not how Claude Code answers a
/// `PermissionRequest` either (measured: that harness reads the exit code on
/// this event nowhere, and decides off the hook's stdout). What the code IS is
/// a pns-side contract the gate's direct callers read, and whose reading by
/// Codex is unverified, so inventing one here would put pns's own word into a
/// channel that is moshi's.
fn hook_mode(event: &str) -> i32 {
    let Some(payload_json) = read_payload() else {
        // A harness that opened the pipe and never wrote must not hold a hook
        // open forever; no payload is no notification, and still exit 0.
        return 0;
    };
    let payload = parse_payload(&payload_json);
    let agent = std::env::var("PNS_AGENT").unwrap_or_else(|_| "claude".to_string());

    match event {
        "prompt" => start_of_turn(&payload),
        "stop" => end_of_turn(&payload, &agent),
        "stop-failure" => failed_turn(&payload, &agent),
        "blocked" => return blocking_event(&payload, &agent, &payload_json),
        // THE MID-TURN NOTIFICATIONS, which is what makes one arm right for
        // all three. Each reports something that happened INSIDE a turn that
        // is still running, so none of them touches the turn marker: the clock
        // belongs to the Stop or the StopFailure that ends the turn, and
        // restarting it here would make a long turn report itself short and
        // lose the tier it earned. None of them forwards to moshi either:
        // `asked` and `plan-ready` are answered at the pane the harness is
        // already holding open, and a denial is a decision the harness has
        // ALREADY taken, so a card offering Allow and Deny would be answering
        // a closed question no prompt is listening to. `denied` states no
        // message of its own, so its detail resolves through `parse_payload`'s
        // existing chain to the tool request.
        "asked" | "plan-ready" | "denied" => run_event(
            &pns::args::EventArgs {
                agent,
                state: event.to_string(),
                project: project_of(&payload.cwd),
                detail: payload.message.clone(),
                pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
                ..Default::default()
            },
            &system_probes(),
        ),
        // An event this binary does not serve is not an error the harness
        // should hear about on a notification path.
        _ => eprintln!("pns: unknown hook event `{event}`"),
    }
    0
}

/// The turn's start marker, so the Stop hook can measure the turn that just
/// finished rather than the whole session.
fn start_of_turn(payload: &HookPayload) {
    let Some(marker) = turn_marker(&payload.session_id) else {
        return;
    };
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Only when none is there: a second prompt inside one turn must not
    // restart the clock.
    if !marker.exists() {
        let _ = std::fs::write(&marker, now_secs().unwrap_or_default().to_string());
    }
}

/// The turn's marker path, or None for a session id that cannot become a
/// filename. The id arrives in the harness payload, and `..` in it would
/// escape the state directory.
fn turn_marker(session_id: &str) -> Option<std::path::PathBuf> {
    if !pns::safety::session_id_is_safe(session_id) {
        return None;
    }
    Some(state_dir().join(format!("session-{session_id}.start")))
}

/// Where this binary keeps what it has to remember between runs.
fn state_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    resolve_path(
        std::env::var("PNS_STATE_DIR").ok().as_deref(),
        &format!("{home}/.local/state/pns"),
    )
}

/// The staleness episode this machine was last told about, if any.
fn remembered_staleness() -> Option<String> {
    let episode = std::fs::read_to_string(state_dir().join(STALENESS_MEMORY)).ok()?;
    let episode = episode.trim().to_string();
    (!episode.is_empty()).then_some(episode)
}

/// Remember one staleness episode, or forget one a HOME reading showed
/// resolved. ONLY A HOME READING CALLS THIS: away and unreadable are not
/// resolutions, so they never reach here to erase a live episode.
///
/// FAIL-QUIET in the `start_of_turn` style: an unwritable state directory
/// must never change a verdict, fail the diagnostic, or crash. The cost of a
/// failed write is one repeated warning.
fn remember_staleness(episode: Option<&str>) {
    let memory = state_dir().join(STALENESS_MEMORY);
    let Some(episode) = episode else {
        let _ = std::fs::remove_file(&memory);
        return;
    };
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = publish_state_line(&memory, episode);
}

/// Publish one line to a state file, atomically. The error is returned rather
/// than swallowed, so each caller states its own fail direction: a background
/// warning drops it, and a human waiting on a typed command hears about it.
///
/// PUBLISHED BY RENAME, the way the turn marker's claim is claimed further
/// down. A plain write truncates first, so a reader landing between the
/// truncate and the bytes sees an empty file, which every reader of these
/// files reads as no state at all. The pending path sits in the SAME
/// directory, because a rename across filesystems is not one, and it carries
/// this process's id so two runs publishing at once cannot share one.
fn publish_state_line(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let pending = path.with_extension(format!("new.{}", std::process::id()));
    // THE PENDING FILE CARRIES THE MODE, because the rename is what publishes
    // it: a prune that wrote its replacement at the umask's mode would undo
    // the one the append created the file with.
    let mut pending_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(STATE_FILE_MODE)
        .open(&pending)?;
    // AND AGAIN AFTER THE OPEN, because `mode` above applies only when the
    // open CREATES the file. The pending path carries this process's own id,
    // so a run interrupted between the open and the rename leaves one for the
    // next run of that pid to REUSE, and a reused inode keeps whatever mode it
    // was made with until this narrows it. Set on the open HANDLE rather than
    // on the path, so nothing can be swapped in underneath between the two.
    pending_file.set_permissions(std::fs::Permissions::from_mode(STATE_FILE_MODE))?;
    pending_file.write_all(format!("{line}\n").as_bytes())?;
    if let Err(error) = std::fs::rename(&pending, path) {
        // Nothing half-written is left in the state directory for the next
        // run to trip over.
        let _ = std::fs::remove_file(&pending);
        return Err(error);
    }
    Ok(())
}

/// Append one decision to the ring, and prune it back to the cap.
///
/// FAIL-QUIET, in `remember_staleness`'s style and deliberately the opposite
/// of `quiet_mode`'s loud write. A mute that did not land is a promise broken
/// to a human standing at the terminal; a decision that did not record is a
/// diagnostic missing later, on a path whose stdout is read by a harness hook
/// and whose only reader already says honestly that it has nothing. Printing a
/// complaint here would put a line about the state directory into every hook's
/// output for the rest of this machine's life.
fn record_decision(record: &pns::decision_log::Record) {
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = append_ring_line(
        &state_dir().join(DECISIONS),
        &pns::decision_log::line(record),
        pns::decision_log::KEPT,
        RING_READ_MAX,
    );
}

/// Journal one event the operator could not have perceived, so a replayer can
/// find it later. A delivered event writes nothing at all.
///
/// ITS OWN FUNCTION rather than a second job inside `record_decision`: the two
/// records have different reasons to change, and this write is conditional
/// where the decision's is not.
///
/// FAIL-QUIET, in `record_decision`'s exact style and for its exact reason. An
/// event path whose stdout a harness hook reads must not gain a line about the
/// state directory, and a journal entry that did not land costs a replay,
/// never a card.
///
/// THE EPOCH IS THE DECISION'S OWN CLOCK READ, taken off the readings it
/// decided from rather than by a second `SystemTime` call here: two readings
/// of one moment can disagree.
fn record_missed(
    event: &pns::args::EventArgs,
    decision: &pns::engine::Decision,
    overrides: &Overrides,
) {
    if !pns::missed_notifications::was_missed(decision, overrides) {
        return;
    }
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = append_ring_line(
        &state_dir().join(MISSED_NOTIFICATIONS),
        &pns::missed_notifications::entry(
            event,
            decision.inputs.now_secs,
            render::PREVIEW_MAX_CHARS,
        ),
        pns::missed_notifications::KEPT,
        RING_READ_MAX,
    );
}

/// Record one event in the activity ring, WHETHER OR NOT anybody perceived it.
///
/// THE THIRD FILE, and it exists because the two already here answer other
/// questions. The decision ring refuses free text by design, since a human
/// reads it through `pns doctor`; the journal is written only for events the
/// operator COULD NOT have perceived, which is the opposite of what a return
/// recap is about. The recap's window is the cards that WERE delivered,
/// glanced at and forgotten, and neither existing file can see one.
///
/// NEVER CLAIMED AND NEVER CONSUMED, unlike the journal. It is a rolling
/// window pruned by depth alone, which is what lets the detached recap child
/// re-read it safely and what makes a recap idempotent by WINDOW rather than
/// by deletion.
///
/// ITS OWN CAP AND ITS OWN READ CEILING, both stated on the constants. A recap
/// line is one of a hundred, so it is capped far shorter than a card, and the
/// depth that covers an overnight window needs a read ceiling of its own.
///
/// FAIL-QUIET, in `record_missed`'s exact style and for its exact reason: an
/// event path whose stdout a harness hook reads must not gain a line about the
/// state directory, and a missing entry costs one line of one recap.
///
/// THE PRIVACY RULE IS THE JOURNAL'S, INHERITED. This file holds the
/// operator's own text for every event, at 0600 like every other state file,
/// and nothing prints an entry to a terminal: `pns doctor` deliberately gains
/// no activity line, and the only reader is the recap that delivers it to the
/// same channels the live event reached.
fn record_activity(event: &pns::args::EventArgs, decision: &pns::engine::Decision) {
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = append_ring_line(
        &state_dir().join(ACTIVITY),
        &pns::missed_notifications::entry(event, decision.inputs.now_secs, ACTIVITY_MAX_CHARS),
        ACTIVITY_KEPT,
        ACTIVITY_READ_MAX,
    );
}

/// Move the recap window's near edge to this event, when this event proves the
/// operator was here.
///
/// THE EVENTS THE RETURN MOMENT NEVER REACHES, and only those in practice:
/// a muted event, an event whose plan decorated nothing because the operator
/// was watching the pane it came from, an event that found the moment held.
/// `claim_moment` moves the edge for every event that does reach it, at the
/// instant it takes the claim, so the read below is already satisfied by the
/// time this runs on those.
///
/// AND THROUGH THE SAME CLAIM, which is not decoration. MEASURED at one run in
/// sixty with eight racers: a run that found the moment held republished the
/// marker here anyway, out from under the holder, and a third run then renamed
/// that fresh marker and became a SECOND owner alongside the first. The two
/// then raced on the journal, and the pair of them put a recap card and a
/// catch-up card on the phone at one moment. Nothing may publish this path
/// while somebody holds it.
///
/// THE READ IN FRONT OF THE CLAIM IS AN OPTIMISATION AND ALSO THE POINT. An
/// edge already at or past this event needs no write, so the ordinary event
/// takes no claim at all and cannot make a racer defer its card; and a marker
/// that is ABSENT reads as None here, which correctly falls through to the
/// claim, where the holder is found and this run stands down.
///
/// AFTER THE CARD SITE, and the ordering is the whole idempotence rule. The
/// window a recap covers ends where this event is, so moving the edge before
/// `replay_missed` counted the window would leave every count at one and no
/// recap could ever fire.
///
/// THE EPOCH IS THE DECISION'S OWN CLOCK READ, taken off the readings it
/// decided from rather than by a second `SystemTime` call, for the reason
/// `record_missed` states: two readings of one moment can disagree.
fn mark_present(decision: &pns::engine::Decision) {
    if !pns::missed_notifications::is_present(decision) {
        return;
    }
    let Some(now) = decision.inputs.now_secs else {
        return;
    };
    if read_epoch(&state_dir().join(LAST_PRESENT)).is_some_and(|held| held >= now) {
        return;
    }
    // NOTHING IS TAKEN AND NOTHING IS DELIVERED: the claim is asked for the
    // edge alone, and its answer is of no use here. What matters is that the
    // write happened inside it.
    let _ = claim_moment(Some(now), false);
}

/// The window's near edge published, and only ever FORWARD.
///
/// READ, COMPARE, PUBLISH. MEASURED as the reason: a slow event that read
/// epoch 100 and a quick one that read 101 both publish at the end of their
/// own run, so the slow one used to land last and put the edge back to 100.
/// Everything the quick event covered then reads as absence activity on the
/// next return, and a long enough tail of it crosses the threshold and posts a
/// recap of a window that never happened.
///
/// CALLED ONLY FROM INSIDE A CLAIM, which is what makes the read and the
/// publish safe as a pair: the caller holds the marker, so nothing else is
/// writing this path between them.
///
/// FAIL-QUIET, in `record_missed`'s exact style. A marker that did not land
/// costs one window's near edge, which the next present event moves anyway.
fn advance_marker(now: u64) {
    let marker = state_dir().join(LAST_PRESENT);
    if read_epoch(&marker).is_some_and(|held| held >= now) {
        return;
    }
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = publish_state_line(&marker, &now.to_string());
}

/// One epoch off a state file, or None for anything this will not vouch for:
/// nothing at the path, a file that cannot be read, or text that is not a
/// plain count.
///
/// AN UNPARSEABLE MARKER IS NO EDGE AT ALL, never an edge at epoch zero. A
/// marker some other hand rewrote is not a near edge this can trust, and
/// reading one as zero would recap the whole ring.
fn read_epoch(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Put the journal in front of an operator who is here to see it, riding the
/// event that proved they are.
///
/// FAIL-QUIET, in `record_missed`'s exact style and for its exact reason: an
/// event path whose stdout a harness hook reads must not gain a line about the
/// state directory, and nothing here is worth a word to the operator anyway.
///
/// A LOSS ON A FAILED DELIVERY IS THE DESIGN, not an oversight. The engine's
/// contract is fire-and-forget for every producer; every journaled event
/// already reached the durable log in full, so nothing is lost that a human
/// cannot recover; re-journaling against a wedged channel is an unbounded
/// retry that grows the file every event; and `dispatch_legs`' outcomes cannot
/// tell delivery from perception in any case, because an executable channel
/// that ran answers `Silent` by design.
///
/// NOTHING IS PRINTED. The event path prints only what a reporting leg said,
/// and this rides an event whose stdout a hook reads.
///
/// `replay_card` IS THE OPERATOR'S SWITCH (`[recap] replay_card = false`) and
/// it gates THE CARD and nothing else. `record_missed` never learns the switch
/// exists, so the journal still records every miss and the doctor still counts
/// them: turning the card back on has something to deliver. `digest` is its
/// own switch over the Discord half, so card-only and recap-only are both
/// valid and neither implies the other.
///
/// THE ONE CARD SITE FOR BOTH FEATURES, which is the whole reason the recap
/// lives here rather than beside this. Two layers were locked, phone and
/// Discord, and a recap that raised its own phone card would put TWO cards on
/// the phone at one return moment. Worse, the case the recap exists for
/// journals NOTHING: a five-hour loop whose cards were all delivered and
/// forgotten leaves the queue empty, so the catch-up alone would raise no card
/// at all and the Discord recap would land with nothing pointing at it. So one
/// site composes at most one card, and which card it is depends on the window.
///
/// AND ONE CLAIM OVER BOTH, taken before anything is counted. `claim_moment`
/// arbitrates the whole return moment rather than the recap alone, so the two
/// halves cannot be won by two different racers; see its own comment for why
/// a claim per file MEASURED as two cards at one moment.
fn replay_missed(
    recap: pns::config::Recap,
    decision: &pns::engine::Decision,
    home: &str,
    moshi_token: Option<String>,
    hermes_key: Option<String>,
    durable_route: bool,
) {
    if !pns::missed_notifications::should_replay(decision) {
        return;
    }
    // NOWHERE THE OPERATOR WOULD SEE IT IS NOT A REPLAY, and that is a
    // stronger test than "nowhere at all". MEASURED: an event narrowed with
    // `--remote-only`, and every event on a machine whose config enables only
    // a durable channel, claimed the queue, posted it into a log that already
    // holds all of it in full, and deleted it, with nothing the operator would
    // ever see. The empty plan (both narrowing flags, a typing mistake) is
    // refused by the same line, because nothing in an empty list is
    // decorative.
    //
    // WHICH LEGS DECORATE IS ROUTING'S ANSWER, carried out on the leg. Asking
    // it here by name, or by re-reading the declarations, would be the second
    // copy of a policy that then drifts, which is the mistake `run_event`
    // states about the mute a few lines above its own decision.
    if !decision.legs.iter().any(|leg| leg.decorative) {
        return;
    }
    // THE MOMENT IS CLAIMED BEFORE ANYTHING IS COUNTED, which is the whole
    // ownership rule and the reverse of what this used to do. Reading the
    // marker first and renaming it afterwards claims a DIFFERENT marker from
    // the one that was counted, because the winner republishes inside that
    // gap; MEASURED at roughly one run in thirty, two racers counted one loud
    // window and both posted it.
    //
    // THE CARD'S OWN SWITCH RIDES INTO THE CLAIM rather than returning in
    // front of it. Claiming the journal renames it out of the way, so a return
    // after that would consume the queue and deliver nothing, which is the one
    // outcome the four-way `Claimed` enum exists to prevent; handing the
    // switch in means the journal is never claimed at all when the card is off.
    let Moment::Owned { since, waiting } =
        claim_moment(decision.inputs.now_secs, recap.replay_card)
    else {
        // A RACER INSIDE SOMEBODY ELSE'S RETURN MOMENT SAYS NOTHING AT ALL.
        // The holder is about to deliver both halves, and this run has claimed
        // neither the window nor the queue, so there is nothing here to lose.
        return;
    };
    // THE WINDOW COMES OFF WHAT WAS CLAIMED, never off a second read: `since`
    // is the value that was renamed out from under every other racer, so a
    // racer holding a republished marker computes the empty window it deserves
    // rather than the one somebody else already posted.
    //
    // A MARKER AHEAD OF NOW IS NO WINDOW EITHER. A clock that moved backwards
    // is not a bracket, and the restore inside the claim kept the newer value,
    // so nothing is lost by refusing it here.
    let window = match (since, decision.inputs.now_secs) {
        (Some(since), Some(until)) if since <= until => Some((since, until)),
        _ => None,
    };
    let counted = window.map_or_else(Vec::new, |(since, until)| activity_in(since, until));
    // FOUR CLAUSES AND NONE OF THEM OPTIONAL. No window means no recap at all,
    // which is what stops a fresh install recapping the whole ring; the
    // threshold is what stops an ordinary afternoon becoming one; `digest` is
    // the operator's own switch over the Discord half; and a machine with no
    // durable route has nowhere for a recap to land, so the card must not
    // point at one.
    let fires =
        recap.digest && durable_route && window.is_some() && counted.len() >= recap.min_events;
    // THE DISCORD HALF GOES FIRST AND IN ITS OWN PROCESS, before the card, so
    // the card can say truthfully whether there is a recap to point at. The
    // spawn is a fork and an exec, so the card is dispatched microseconds
    // later; everything slow happens in the child.
    let posted = match window {
        Some((since, until)) if fires => spawn_recap(since, until),
        _ => false,
    };
    // THE TWO DELIVERIES ARE INDEPENDENT: an operator who wants the recap in
    // Discord and no card on the phone has asked for exactly that, which is
    // why this sits BELOW the spawn.
    if !recap.replay_card {
        return;
    }
    // TWO CARDS, ONE SITE, AND AT MOST ONE OF THEM. Over the threshold the
    // recap card is the delivery, whether or not anything was journaled, because
    // the window itself is the news; under it there is no recap, so an empty
    // queue is nothing to say and the catch-up card is unchanged.
    let detail = if fires {
        pns::missed_notifications::recap_card(
            &pns::missed_notifications::needing_you(&counted),
            counted.len(),
            waiting.len(),
            posted,
        )
    } else if waiting.is_empty() {
        return;
    } else {
        pns::missed_notifications::summary(&waiting)
    };
    // ONE SYNTHETIC EVENT, whatever the count. Empty project and branch,
    // because a batch spans both and `render::message` would otherwise prefix
    // the lot with one branch's name; empty channel, because an entry carries
    // none (the durable route already had the event); empty pane, which is the
    // call `doctor_mode` makes too, because a pane id from an hour ago may
    // name a pane that no longer exists. The title reads `pns · missed`, which
    // is visibly not a live agent card: a replayed card that looked live would
    // be lying about time.
    let replay = pns::args::EventArgs {
        agent: "pns".to_string(),
        state: "missed".to_string(),
        detail,
        ..Default::default()
    };
    // DISPATCHED DIRECTLY AND NEVER THROUGH `run_event`, which is the loop
    // this closes. A synthetic event fed back in would take a SECOND decision
    // (the second reading of one moment `GateInputs` exists to forbid), write
    // a second ring line for something that is not an event, fire a second
    // pulse, and RE-JOURNAL: under a mute the replay would journal itself and
    // the next one would replay the replay, forever, growing by one entry each
    // time. `doctor_mode` is the precedent in this file for the same split;
    // what is left after a decision has been taken is dispatch alone.
    //
    // THE LEGS ARE THIS DECISION'S OWN, verbatim. Deciding again would be a
    // second copy of routing's policy, which `routing` itself warns is how the
    // two come to drift. ACCEPTED CONSEQUENCE: the durable leg is among them,
    // so the summary is posted to a log that already holds every entry in it.
    // That is a duplicate in content and a new fact in kind.
    let _ = dispatch_legs(
        &decision.legs,
        false,
        &replay,
        home,
        moshi_token,
        hermes_key,
    );
}

/// Every activity entry inside a window, oldest first, which is the order the
/// append leaves the ring in.
///
/// THE NEAR EDGE IS EXCLUSIVE and the far edge is not, which is the difference
/// between "since you were last here" and "including the moment you were".
/// MEASURED: with it inclusive, the event that MOVED the marker is counted
/// inside the next window, and every event sharing that same second with it is
/// too. Eight events in one second then read as a loud window opening at the
/// instant it closed, so a burst at the desk earned a recap of an absence that
/// never happened, and a second recap of the window a first one had just
/// posted. Excluding the marker's own second costs nothing real: the event at
/// that instant is the one that proved the operator was present.
///
/// AN ENTRY WITH NO CLOCK IS IN NO WINDOW. Its writer had no readable clock, so
/// nothing can place it, and counting it would put an event of unknown age
/// inside a bracket that is entirely about age.
///
/// A RING THAT CANNOT BE READ IS AN EMPTY WINDOW, which reads as no recap
/// rather than as a recap of nothing: the count would be zero, and zero is
/// under every threshold.
fn activity_in(since: u64, until: u64) -> Vec<pns::missed_notifications::Entry> {
    let Ok(contents) = readable_ring(&state_dir().join(ACTIVITY), ACTIVITY_READ_MAX) else {
        return Vec::new();
    };
    pns::missed_notifications::entries(&contents)
        .into_iter()
        .filter(|entry| entry.at.is_some_and(|at| at > since && at <= until))
        .collect()
}

/// What one event found when it reached for the return moment.
///
/// ONE ARBITRATION OVER BOTH HALVES of what a return delivers, which is the
/// whole reason this is one value rather than a claim per file. The halves are
/// the recap card and the catch-up card, and with a claim each the loser of one
/// could still win the other: MEASURED at roughly one run in three with eight
/// racers, a racer that found the marker held read no window, fell through to
/// the journal, and put its catch-up card on the phone beside the winner's
/// recap card.
enum Moment {
    /// This event OWNS the moment. `since` is the near edge the marker held,
    /// absent when there was no marker to open a window with; `waiting` is the
    /// journal, claimed inside the same critical section.
    Owned {
        since: Option<u64>,
        waiting: Vec<pns::missed_notifications::Entry>,
    },
    /// A run that still exists holds the moment right now, so this event is
    /// inside somebody else's return and has claimed nothing.
    Busy,
}

/// The return moment claimed: the window's near edge and the journal taken
/// together, and the edge handed straight back.
///
/// CLAIMED BY RENAME, which is `claim_by_rename`'s idiom for
/// `claim_by_rename`'s reason. Two events firing at once is ordinary here (a
/// Stop hook and the long-running notifier are a normal pair) and only one
/// rename can win. An unlink cannot stand in: MEASURED on macOS 26.2 (APFS),
/// eight processes unlinking one path were every one of them told they had
/// succeeded.
///
/// THE NEAR EDGE COMES OFF WHAT WAS CLAIMED, and that is the ordering this
/// whole function exists to get right. Reading the marker first and renaming
/// it afterwards claims whatever marker is there BY THEN, which is not the one
/// the window was counted from, because the winner republishes inside that
/// gap. Both racers then post the same window. Claiming first means a racer
/// that takes a republished marker counts the empty window that value opens
/// and correctly earns nothing.
///
/// THE JOURNAL IS TAKEN INSIDE THE SAME CRITICAL SECTION, before the edge goes
/// back. That is what makes a second card of ANY KIND impossible at one return
/// moment: a racer arriving while this run holds the marker is told `Busy` and
/// says nothing, and a racer arriving after the edge is restored finds the
/// queue already gone and has nothing to say either.
///
/// THE EDGE IS RESTORED IMMEDIATELY, before the window is counted and long
/// before anything is dispatched, so the marker's absence is bounded by two
/// renames rather than by a delivery. A kill at any instant then costs the one
/// in-flight recap and never a future window: the next present event finds an
/// edge to open one with.
///
/// AND IT ONLY EVER MOVES FORWARD. `advance_marker` is what publishes it, so
/// the newer of the claimed value and this event's own clock is what stands,
/// and a claim taken with no readable clock puts back exactly what it took.
///
/// NOTHING IS LEFT BEHIND on any path this run completes, and a run killed
/// mid-claim leaves ONE file that the next return adopts by name. The
/// adoption is also the recovery: the edge that run was holding comes back
/// with it rather than being lost.
fn claim_moment(now: Option<u64>, take_journal: bool) -> Moment {
    let state = state_dir();
    let marker = state.join(LAST_PRESENT);
    let claim = marker.with_extension(window_claim_suffix(now));
    let taken = if std::fs::rename(&marker, &claim).is_ok() {
        Some(claim)
    } else {
        match stranded_window_claim(&state, now) {
            // A LIVE HOLDER IS THE ONLY THING THAT SILENCES AN EVENT HERE. No
            // claim at all is a machine that has never published a marker, and
            // that event still owes its catch-up card.
            StrandedWindow::Live => return Moment::Busy,
            // ADOPTED BY A SECOND RENAME, which is `take_claim`'s idiom: two
            // runs that both reach one stranded claim still cannot both take
            // it, because only one rename can win.
            StrandedWindow::Abandoned(left) => std::fs::rename(&left, &claim).ok().map(|()| claim),
            StrandedWindow::None => None,
        }
    };
    let since = taken.as_deref().and_then(read_epoch);
    let waiting = if take_journal {
        claim_journal(&state)
    } else {
        Vec::new()
    };
    if let Some(edge) = since.max(now) {
        advance_marker(edge);
    }
    if let Some(claim) = taken {
        // The failure is dropped: what it leaves is exactly what the adoption
        // above recovers.
        let _ = std::fs::remove_file(claim);
    }
    Moment::Owned { since, waiting }
}

/// Whether another run is inside the return moment right now, and the claim it
/// left behind when it is not.
///
/// MATCHED ON THE MARKER'S OWN CLAIM PREFIX and nothing looser, which is
/// `stranded_claims`' rule: the journal and the turn marker claim themselves
/// in this directory too, and a wider match would hand one of their values
/// back as a window's near edge.
///
/// AT MOST ONE OF THESE CAN EXIST AT A TIME, because a claim is only ever made
/// by renaming the ONE marker or by renaming an existing claim, and a run that
/// finds one live makes none of its own. The loop still answers `Live` for the
/// first live one it meets rather than assuming that, because the directory is
/// a plain directory another hand can reach.
enum StrandedWindow {
    /// A run that still exists holds the marker.
    Live,
    /// A claim nobody is inside any more, and so the near edge it is holding.
    Abandoned(std::path::PathBuf),
    /// Nothing is holding anything: no marker was ever published here.
    None,
}

fn stranded_window_claim(state: &Path, now: Option<u64>) -> StrandedWindow {
    let prefix = format!("{LAST_PRESENT}.claim.");
    let Ok(entries) = std::fs::read_dir(state) else {
        return StrandedWindow::None;
    };
    let mut abandoned = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(owner) = name.strip_prefix(&prefix) else {
            continue;
        };
        if !window_claim_is_free(owner, now) {
            return StrandedWindow::Live;
        }
        abandoned = Some(entry.path());
    }
    abandoned.map_or(StrandedWindow::None, StrandedWindow::Abandoned)
}

/// What a window claim is named after the prefix: the id of the run that took
/// it, and the epoch it was taken at when that run had a clock to read.
///
/// THE EPOCH IS THE CLAIM'S OWN AGE and cannot be taken off the file instead:
/// a rename carries the marker's mtime, which is the time of the last PRESENT
/// event and can be hours before the claim was made. It costs nothing to
/// record, because the caller already holds this event's clock read.
fn window_claim_suffix(now: Option<u64>) -> String {
    match now {
        Some(now) => format!("claim.{}.{now}", std::process::id()),
        None => format!("claim.{}", std::process::id()),
    }
}

/// Whether a window claim may be taken: nobody is inside it.
///
/// THREE WAYS IT IS FREE, and the first two are `claim_by_rename`'s own. It is
/// THIS RUN'S, so nothing else can be inside it; or its owner has EXITED, so
/// nothing is; or it is far OLDER than any run could still be holding it.
///
/// THE AGE TEST IS WHAT A PID CANNOT ANSWER. A claim is held for two renames
/// and a small read, so a claim minutes old is one whose owner died mid-claim
/// and whose id the machine has since handed to something long-lived. Without
/// it that claim reads as live for as long as the new process runs, and every
/// return moment on the machine stands down behind it: no card, no recap and
/// no edge, until that process happens to exit. The bound is deliberately five
/// minutes, four orders of magnitude past what holding one costs, so a real
/// holder can never be stolen from and a stranded one can never wedge for long.
fn window_claim_is_free(owner: &str, now: Option<u64>) -> bool {
    let mut named = owner.split('.');
    let took_it = named.next().unwrap_or_default();
    if took_it == std::process::id().to_string() || owner_is_gone(owner) {
        return true;
    }
    match (named.next().and_then(|at| at.parse::<u64>().ok()), now) {
        (Some(taken), Some(now)) => now.saturating_sub(taken) > STALE_WINDOW_CLAIM_SECS,
        // A CLAIM WITH NO EPOCH, or a run with no clock to compare it against,
        // falls back on the pid alone, which is `abandoned_hold`'s own answer
        // and its own accepted price.
        _ => false,
    }
}

/// How long a window claim may stand before it is taken to be stranded
/// whatever its process id says. See `window_claim_is_free`.
const STALE_WINDOW_CLAIM_SECS: u64 = 300;

/// Start the recap in a process of its own, and say whether it really started.
///
/// THE DIGEST NEVER RUNS IN THIS PROCESS. `run_event` is reached from
/// `pns hook prompt`, which the harness does NOT background, and from the
/// bashrc notifier, where a human is watching their prompt. Rendering and
/// posting a recap sits on neither. NEVER WAITED ON, so this process exits
/// exactly when it would have, and the child is reparented if it goes first.
///
/// AND IN A PROCESS GROUP OF ITS OWN, which is the other half of detachment
/// and used to be claimed rather than done. A hook the harness times out is
/// killed by GROUP, and so is a shell prompt taking `SIGINT`; a child left in
/// the parent's group goes with it, after the marker has already moved on, so
/// the window can never fire again and the card in the operator's hand points
/// at a recap nobody is writing.
///
/// `current_exe` RATHER THAN A PATH, so a test binary re-execs itself and a
/// moved install still works. ONLY THE TWO BOUNDS CROSS: the child re-reads the
/// ring itself, so nothing is serialized between them and nothing is lost if
/// the child never starts.
///
/// TWO INDEPENDENT READS OF ONE RING, STATED. The card's count is this
/// process's own read of the window and the recap's header is the child's, so
/// an event landing in the shared `until` second between them, or a prune, can
/// leave the two counts one apart. Each is honest about what IT read, which is
/// the same rule the header's own comment states about the ring's depth;
/// reconciling them would mean serializing a snapshot the child is deliberately
/// free to re-read.
///
/// THE ANSWER IS WHETHER A CHILD EXISTS, which is what the card says out loud.
/// A spawn that failed must never leave a card pointing at a recap nobody is
/// writing.
///
/// A CHILD THAT DIES COSTS ONE RECAP AND NOTHING ELSE, which is why nothing
/// supervises it: the activity ring is not consumed, the marker has already
/// moved, and the card already carried the counts.
fn spawn_recap(since: u64, until: u64) -> bool {
    let Ok(binary) = std::env::current_exe() else {
        return false;
    };
    let mut child = Command::new(binary);
    child
        .args(["recap", "--since", &since.to_string()])
        .args(["--until", &until.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // A NEW GROUP, WITH ITS OWN ID, which is what `setpgid(0, 0)` in the
        // forked child does and what the doc above promises.
        .process_group(0);
    // AN UNBOUNDED DEADLINE IS A TERMINAL'S CHOICE, NEVER A BACKGROUND
    // CHILD'S. `PNS_REMOTE_TIMEOUT=0` is curl's `-m 0`, no deadline at all,
    // which nobody is behind to interrupt here: a wedged gateway would keep
    // this process alive for good, and every later window would add another.
    if remote_deadline(std::env::var("PNS_REMOTE_TIMEOUT").ok().as_deref()).is_none() {
        child.env("PNS_REMOTE_TIMEOUT", RECAP_DEADLINE_SECS);
    }
    child.spawn().is_ok()
}

/// The deadline a detached recap falls back to when the environment asked for
/// none. Generous, because nobody is waiting on this process; finite, because
/// nobody is watching it either.
const RECAP_DEADLINE_SECS: &str = "30";

/// What became of one claim this run reached for.
///
/// FOUR OUTCOMES RATHER THAN ONE EMPTY VECTOR, because they are four different
/// things to have happened and only one of them may destroy anything. This
/// used to collapse into `Vec::new()`, and that is exactly how a journal whose
/// read failed came to be deleted with nothing delivered: the failure was
/// indistinguishable from an empty queue at the one call site that could still
/// have put it back.
enum Claimed {
    /// Nothing was there to claim, or another run took it first.
    Nothing,
    /// The path holds something this tool never wrote. Put back where it was
    /// found, and not read.
    Refused,
    /// This run OWNS these entries: it read them, and the claim they came from
    /// is gone, so no other run can deliver them too.
    Taken(Vec<pns::missed_notifications::Entry>),
    /// The claim could not be read, or could not be given up. It is STILL ON
    /// DISK, whole: under its claim name when the claim was never taken, or
    /// under a held name, which a return AFTER this process is gone adopts.
    LeftForAdoption,
}

impl Claimed {
    /// The entries this run may deliver, which is none for every outcome but
    /// one. Nothing else may be delivered: an unread claim is still on disk,
    /// and delivering from it as well would show the operator the same batch
    /// twice.
    fn entries(self) -> Vec<pns::missed_notifications::Entry> {
        match self {
            Claimed::Taken(entries) => entries,
            Claimed::Nothing | Claimed::Refused | Claimed::LeftForAdoption => Vec::new(),
        }
    }
}

/// The journal, CLAIMED and consumed: whatever an earlier run stranded is
/// adopted first, then the journal itself is renamed out of the way, read
/// through the one guarded reader, and given up only once that read worked.
///
/// NOTHING UNDELIVERED IS EVER DESTROYED, which is the property the whole
/// order below exists for. What this run cannot read, it leaves; what it
/// cannot give up, it leaves; what it leaves sits under its claim name or a
/// held name, and one of the returns that follow goes looking for both.
///
/// CLAIMED BY RENAME, which is `consume_turn_marker`'s idiom and is atomic:
/// two events racing each other cannot both take one journal, because only one
/// rename can win. A SECOND RENAME IS THE SECOND ARBITER, for a batch an
/// earlier run stranded: `take_claim` moves it on to a name carrying its own
/// process id before it reads a byte, so two runs that both reached one
/// stranded claim still cannot both deliver it. The unlink used to hold that
/// job and MEASURED it cannot: on macOS 26.2 (APFS) eight processes unlinking
/// ONE path were every one of them told they had succeeded.
///
/// ADOPTION IS HOW A LOST BATCH COMES BACK. A run killed between the rename
/// and the delivery, and a run whose read failed, both leave a claim behind;
/// before this nothing ever looked at one again, so the queue sat in the state
/// directory for good, and the doctor's count could not even see it, because
/// that count reads the journal's own name.
///
/// OLDEST FIRST: a stranded claim WAS the journal on an earlier return, so it
/// is older than anything in the file now, and the summary renders newest
/// first from the far end of what this returns.
///
/// AND ALL OF IT BEFORE ANY DELIVERY, which is unchanged. The entries are in
/// memory from the moment this returns, so a channel that hangs to its
/// deadline and takes the process with it leaves no claim behind; and a claim
/// left behind some other way is now recovered rather than lost.
///
/// THE RACE, stated: an append that opened the journal path before the rename
/// writes into the claimed inode, and is replayed or lost depending on which
/// side of the read it lands. That is ONE entry at a rare boundary, the same
/// bound `append_ring_line` already names and accepts.
fn claim_journal(state: &Path) -> Vec<pns::missed_notifications::Entry> {
    let mut waiting = Vec::new();
    for stranded in stranded_claims(state) {
        waiting.extend(take_claim(&stranded).entries());
    }
    waiting.extend(claim_by_rename(&state.join(MISSED_NOTIFICATIONS)).entries());
    waiting
}

/// Every claim an earlier run left in the state directory, oldest first, plus
/// every hold whose owner did not live to give it up.
///
/// MATCHED ON THE JOURNAL'S OWN CLAIM PREFIX and nothing looser: the turn
/// marker claims itself in this directory too, under its own name, and a
/// wider match would hand a turn's start time to the replayer. The one
/// addition is an ABANDONED HOLD, which is a stranded batch in every way that
/// matters here and is admitted only once the run that took it is gone.
///
/// SORTED BY WHEN THEY WERE LAST WRITTEN, which is the journal's own
/// timestamp: a rename does not touch it, so a claim still carries the moment
/// its last entry was appended. A time that cannot be read sorts oldest, which
/// costs an ordering and never a delivery.
fn stranded_claims(state: &Path) -> Vec<std::path::PathBuf> {
    let prefix = format!("{MISSED_NOTIFICATIONS}.claim.");
    let Ok(entries) = std::fs::read_dir(state) else {
        return Vec::new();
    };
    let mut found: Vec<(Option<SystemTime>, std::path::PathBuf)> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with(&prefix) || abandoned_hold(&name)
        })
        // `DirEntry::metadata` does not traverse a symlink, matching the
        // append's and the reader's own refusal to judge one by its target.
        .map(|entry| {
            (
                entry
                    .metadata()
                    .ok()
                    .and_then(|found| found.modified().ok()),
                entry.path(),
            )
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, path)| path).collect()
}

/// Whether a name is a HELD file whose owner is gone.
///
/// A held file is a batch some run had taken and was reading when it died, in
/// a window one rename wide. Nothing else may touch one while its owner lives,
/// which is the whole reason the name sits outside the claim prefix: an owner
/// that is still reading cannot have its batch taken a second time.
fn abandoned_hold(name: &str) -> bool {
    name.strip_prefix(&format!("{MISSED_NOTIFICATIONS}.held."))
        .is_some_and(owner_is_gone)
}

/// Whether the process a claim is named for has exited.
///
/// ONE ANSWER FOR EVERY CLAIM IN THIS DIRECTORY. The journal's holds and the
/// marker's claims both carry the id of the run that took them, and two copies
/// of this test would drift the day one of them learns something.
///
/// A LIVE PROCESS IS THE ONLY THING THAT DEFERS A CLAIM. `kill(pid, 0)`
/// answers `EPERM` for a process this user may not signal, which is still a
/// process that exists, so only `ESRCH` counts as gone. A pid the machine has
/// reused reads as alive, and what that costs is a batch that waits for the
/// first return after the process wearing its number exits, which is the same
/// shape of price `claim_by_rename` names for its own pid guard: a replay
/// deferred, never a replay destroyed and never one delivered twice.
fn owner_is_gone(owner: &str) -> bool {
    // THE PID IS THE SEGMENT BEFORE THE FIRST DOT (held.<pid>.<seq>); a bare
    // held.<pid> from an older build, and the marker's claim.<pid>, both parse
    // the same way.
    let owner = owner.split('.').next().unwrap_or_default();
    let Ok(pid) = owner.parse::<libc::pid_t>() else {
        return false;
    };
    // kill() reads non-positive values as the GROUP and BROADCAST forms, so a
    // hand-planted negative name must never reach it looking like a pid.
    if pid <= 0 {
        return false;
    }
    // SAFETY: `kill` with signal 0 sends nothing and only reports whether the
    // process exists.
    if unsafe { libc::kill(pid, 0) } != -1 {
        return false;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}

/// The journal renamed out of the way, or the reason it was not.
///
/// VERIFIED AFTER THE RENAME AND NOT BEFORE. A check taken first is a check of
/// a path something else is still free to change between the look and the
/// move, and what the remove would then act on is whatever the rename actually
/// carried. So the rename decides, and the claim it produced is what gets
/// judged: anything that is not a regular file goes straight back to the
/// journal's own path, untouched and unread.
///
/// A RENAME BACK THAT FAILS LEAVES IT AT THE CLAIM PATH, which is a state
/// nothing here can improve on: the guarded reader refuses a non-regular file
/// without opening it, so a later adoption leaves it alone as well. It is
/// never read and never removed, which is the same promise the append makes
/// about a path it did not write.
///
/// THE PID GUARD BELOW IS NOT PINNED BY A TEST, and cannot be: no test can
/// plant a claim named for a process id the engine has not been given yet.
/// What it costs if it is ever wrong is one replay deferred to the next
/// return; what it buys is that a rename can never land on an undelivered
/// batch.
fn claim_by_rename(journal: &Path) -> Claimed {
    let claim = journal.with_extension(format!("claim.{}", std::process::id()));
    // NEVER RENAMED OVER A CLAIM THAT IS ALREADY THERE. The name carries this
    // process's id, so the only way one exists at this point is a run of the
    // same id whose batch the adoption above could not take (a pid the machine
    // reused, in practice), and a rename overwrites: the journal would land on
    // top of a batch nobody has seen. Both are left where they are, and the
    // next return tries both again.
    //
    // NOT A RACE, unlike the check this replaced at the journal's own path:
    // only the process holding this id writes this name, and it is this one.
    if std::fs::symlink_metadata(&claim).is_ok() {
        return Claimed::LeftForAdoption;
    }
    if std::fs::rename(journal, &claim).is_err() {
        return Claimed::Nothing;
    }
    if !matches!(std::fs::symlink_metadata(&claim), Ok(found) if found.is_file()) {
        let _ = std::fs::rename(&claim, journal);
        return Claimed::Refused;
    }
    take_claim(&claim)
}

/// One claim HELD BY RENAME, then read and given up, in that order.
///
/// THE RENAME IS THE OWNERSHIP TEST, and the remove is no longer one. It used
/// to be, on the premise that only one of two runs reading a stranded claim
/// could unlink it. MEASURED on macOS 26.2 (APFS), that premise is false:
/// eight processes unlinking ONE path were every one of them told they had
/// succeeded, and two racing runs that both read one claim both delivered it
/// (reproduced twice in 1500 rounds). A rename does arbitrate, measured in the
/// same run: 40 rounds of eight racers, one winner every time.
///
/// THE HELD NAME IS OUTSIDE THE PREFIX THE ADOPTION SCAN MATCHES, so nothing
/// can take this batch a second time while it is being read. It comes back
/// into that scan only once the process named in it is gone.
///
/// THE READ STILL COMES BEFORE THE REMOVE, which is the older half of this and
/// unchanged. Removing first, or removing whatever the read answered, throws
/// away a batch nobody has seen the moment the read fails: MEASURED as a
/// journal with one undecodable byte in it coming back empty, with the file
/// already gone. A read that failed leaves the held file exactly as it is, for
/// the adoption that recovers it.
fn take_claim(claim: &Path) -> Claimed {
    // ONE HELD NAME PER CLAIM, not per process: pid then a per-run sequence.
    // A single per-process name coupled every stranded claim in a run to the
    // first one, and an UNREADABLE first claim then occupied the name, was
    // migrated to a fresh name by every later run's adoption, always sorted
    // oldest, and so STARVED every good batch behind it forever. The sequence
    // dissolves the coupling; the adoption parses the pid segment alone.
    static HELD_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let held = claim.with_file_name(format!(
        "{MISSED_NOTIFICATIONS}.held.{}.{}",
        std::process::id(),
        HELD_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    // The same refusal `claim_by_rename` makes about its own claim, for the
    // same reason: a rename OVERWRITES, and a batch this run has not delivered
    // must never be what it lands on.
    if std::fs::symlink_metadata(&held).is_ok() {
        return Claimed::LeftForAdoption;
    }
    if std::fs::rename(claim, &held).is_err() {
        return Claimed::Nothing;
    }
    let Ok(contents) = readable_ring(&held, RING_READ_MAX) else {
        return Claimed::LeftForAdoption;
    };
    if std::fs::remove_file(&held).is_err() {
        return Claimed::LeftForAdoption;
    }
    Claimed::Taken(pns::missed_notifications::entries(&contents))
}

/// The append and the prune behind it, for ANY of this tool's bounded state
/// rings. The caller names the file and its own depth; everything below is
/// one hardening serving every one of them, because a second hand-written
/// copy of it is how one ring ends up without the FIFO guard.
///
/// WRITTEN BY APPEND, never read-modify-write: an append needs no read, so two
/// events firing at once (a Stop hook and the long-running notifier are a
/// normal pair) cannot lose each other's line. The prune only runs when the
/// file went over the caller's cap, and republishes the last `kept` lines
/// through the same atomic publish every other state file uses.
///
/// NOTHING ABOUT THE FILE IS TRUSTED, because none of it is this tool's word:
/// the ring is a plain file in a directory an operator, a backup tool or
/// another program can reach. Three states were MEASURED to cost more than
/// the record they lost. A FIFO at the path parks the open forever, and with
/// it the hook that called this, on every event. A byte no reader can decode
/// fails the read-back, which is what the prune runs on, so the ring then
/// grows without a bound. A file left without its trailing newline welds this
/// record onto the tail of the last one and costs the reader BOTH. Each is
/// answered here rather than defended against downstream: an irregular file
/// is refused untouched, and a file this cannot read back whole is replaced
/// by the one line it does have.
///
/// ACCEPTED LIMIT: an append landing exactly during a rename, whether the
/// prune's or a heal's, is lost. It costs ONE RECORD at a rare boundary,
/// never a card and never a torn file, because the rename is atomic and the
/// text it publishes is always whole lines.
///
/// `read_max` IS THE CALLER'S TOO, and it travels with `kept` because the two
/// are one decision. The prune runs on the READ-BACK, so a ring deep enough to
/// exceed the reader's ceiling can never be pruned again: the heal fires and
/// the file collapses to the one line just written, silently, exactly when it
/// is fullest. Every caller states both numbers together, and the doc comment
/// on each depth does the arithmetic.
fn append_ring_line(path: &Path, line: &str, kept: usize, read_max: u64) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // BEFORE THE OPEN, and with `symlink_metadata` so the link itself is what
    // is judged rather than whatever it points at. Refused and never
    // repaired: deleting something this tool did not put there, on a path it
    // only ever appends to, is a bigger action than skipping one record.
    let already_there = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => true,
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the ring is not a regular file",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    // The separator rides IN the same write rather than being a write of its
    // own, so the record still lands in one append and two events racing each
    // other still cannot interleave.
    let separator = if already_there && ends_mid_line(path)? {
        "\n"
    } else {
        ""
    };
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(STATE_FILE_MODE)
        .open(path)?
        .write_all(format!("{separator}{line}\n").as_bytes())?;

    let contents = match readable_ring(path, read_max) {
        Ok(contents) => contents,
        // THE HEAL. What could not be read back cannot be pruned either, so
        // leaving it would leave the ring unbounded from here on. The line
        // just written is the part that is known good and known this tool's
        // own, and it is republished alone.
        Err(error) if republish_after(&error) => return publish_state_line(path, line),
        // AND NOT WHEN THE PATH IS SIMPLY GONE. Nothing removes one of these
        // files except a claim, and a claim is a rename: the file this append
        // just wrote into moved to the claim path AND TOOK THIS LINE WITH IT,
        // on its way to being delivered. Republishing it here would put a
        // second copy of an already-claimed record back at the path, and the
        // operator would be shown it twice. There is nothing left to prune, so
        // there is nothing to do.
        Err(_) => return Ok(()),
    };
    let entries: Vec<&str> = contents.lines().collect();
    if entries.len() <= kept {
        return Ok(());
    }
    // Joined with newlines, because the publish writes the one trailing
    // newline back itself.
    publish_state_line(path, &entries[entries.len() - kept..].join("\n"))
}

/// Whether an append whose read-back FAILED has to republish the line it just
/// wrote.
///
/// EVERY REASON BUT ONE. A file that cannot be decoded, is too large to read,
/// or is no longer a regular file is a ring that can never be pruned again, so
/// the one line known to be this tool's own is republished over it. NotFound
/// is the exception and the only one: these files are removed by nothing but a
/// claim, and a claim is a rename, so an absent path means the line just
/// written is already inside the claim and on its way to the operator.
///
/// ITS OWN FUNCTION so the distinction can be stated in a test. The wiring
/// from a real interleaved claim into this arm is a race no test in this tree
/// can stage deterministically; what is pinned here is the decision, and the
/// race itself belongs to the out-of-tree probe.
fn republish_after(error: &std::io::Error) -> bool {
    error.kind() != std::io::ErrorKind::NotFound
}

/// Whether the ring's last byte is anything other than a newline, which is
/// what would FUSE the next record onto the entry already there.
///
/// READ-ONLY AND ON ITS OWN HANDLE, so the handle that writes stays
/// write-only. The end is found by seeking rather than taken from the size
/// the caller already read: another event can append between the two, and an
/// offset from the stale size would sample a byte out of the middle.
fn ends_mid_line(path: &Path) -> std::io::Result<bool> {
    let mut ring = std::fs::File::open(path)?;
    let end = ring.seek(std::io::SeekFrom::End(0))?;
    if end == 0 {
        return Ok(false);
    }
    ring.seek(std::io::SeekFrom::Start(end - 1))?;
    let mut last = [0u8; 1];
    ring.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}

/// One of this tool's state files read back whole, or the reason it was
/// refused: nothing at the path, something there that is not a regular file,
/// too large to pull into memory, or bytes no reader can decode.
///
/// EVERY READER OF THESE FILES GOES THROUGH IT, the prune's read-back and the
/// doctor's two sections alike, because a raw `read_to_string` on a path an
/// operator, a backup tool or another program can reach is the same two bugs
/// wherever it is written. A FIFO parks the open forever, for READING as much
/// as for writing, which wedges the hook that appended or the command a human
/// is waiting on. A file some other hand grew to gigabytes is otherwise
/// learned about by allocating it.
///
/// `symlink_metadata`, so the link itself is judged rather than whatever it
/// points at, matching the append's own refusal a few lines up. The SIZE IS
/// CHECKED FIRST for the reason above; `read_max` is the CALLER'S ceiling, far
/// above anything that caller writes and far below a size worth reading, so
/// only a file some other hand left there can reach it.
///
/// THE REFUSALS ARE `io::Error`s rather than an absence, so a caller that has
/// to tell "there is no file" from "the file could not be read" still can:
/// the doctor says a different sentence for each, and the prune heals on
/// either.
fn readable_ring(path: &Path, read_max: u64) -> std::io::Result<String> {
    let found = std::fs::symlink_metadata(path)?;
    if !found.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the state file is not a regular file",
        ));
    }
    if found.len() > read_max {
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            "the state file is larger than this reads",
        ));
    }
    std::fs::read_to_string(path)
}

/// The most of the decision ring or the journal that is ever read into memory.
/// Their depths (5 and 25) at their field caps sit far under it; see
/// `missed_notifications::KEPT` for that arithmetic.
const RING_READ_MAX: u64 = 256 * 1024;

/// The most of the ACTIVITY ring that is ever read into memory, which is its
/// own number because its depth is its own.
///
/// THE ARITHMETIC, in `KEPT`'s style so the next person to raise either number
/// has the ceiling in front of them. A worst-case entry is five text fields at
/// `ACTIVITY_MAX_CHARS` characters, each character costing six bytes escaped
/// (a control byte is written `\u001b`), plus about eighty bytes of JSON
/// scaffolding: 5 * 120 * 6 + 80 = 3,680 bytes. At `ACTIVITY_KEPT` that
/// MEASURES 552,000 bytes, which is 53% of this ceiling. Raising the depth or
/// the field cap means raising this in the same change, because a ring that
/// cannot be read back cannot be pruned and collapses to one line.
const ACTIVITY_READ_MAX: u64 = 1024 * 1024;

/// The decision ring: one line per event, `KEPT` deep, beside `quiet-until`
/// and `home-staleness`. NOT a log stream and not rotate-logs' business: it is
/// bounded state that prunes itself.
const DECISIONS: &str = "decisions";

/// The missed-notification journal: one JSON object per line, oldest first,
/// `missed_notifications::KEPT` deep, beside `decisions` and `quiet-until`.
/// Bounded state that prunes itself, not a log stream and not rotate-logs'
/// business.
const MISSED_NOTIFICATIONS: &str = "missed-notifications";

/// The activity ring: EVERY event, one JSON object per line in the journal's
/// own shape, oldest first, `ACTIVITY_KEPT` deep. Bounded state that prunes
/// itself, never claimed and never consumed.
const ACTIVITY: &str = "activity";

/// One line holding the epoch of the last event that PROVED the operator was
/// here, which is the near edge of the window a recap covers. Absent means no
/// window at all, so a fresh install cannot recap the whole ring.
const LAST_PRESENT: &str = "last-present";

/// How many events the activity ring keeps.
///
/// A HUNDRED AND FIFTY covers an overnight window at the observed working rate
/// (ten pull requests merged in a ten-hour stretch on 2026-08-29, each spanning
/// many turns and so many events). Past that the ring under-reports its oldest
/// end exactly as the journal's prune does, which is why the recap's header
/// counts the entries it READ rather than claiming a total it cannot back.
/// Raising it means raising `ACTIVITY_READ_MAX` in the same change.
const ACTIVITY_KEPT: usize = 150;

/// How much of each text field one activity entry holds.
///
/// A TIMELINE LINE, NOT A CARD, which is why it is far under the card's own
/// 260: the recap renders one line per event among a hundred, and the full text
/// of every event already reached the durable log the recap's tail points at.
const ACTIVITY_MAX_CHARS: usize = 120;

/// The mode every file this tool creates in its state directory is born with.
///
/// ONE RULE FOR THE DIRECTORY'S CONTENTS rather than a knob for one caller:
/// none of them has a reason to be world-readable, and the journal holds the
/// operator's own text. ACCEPTED LIMIT: an APPEND applies it at create, so a
/// ring an earlier build already left on disk keeps its umask mode until it is
/// next created, and nothing chmods a file it found there, in keeping with the
/// ring's refuse-rather-than-repair stance. THE PUBLISH IS THE ONE PLACE THAT
/// CHMODS, and it is not that case: the pending file it narrows is its own,
/// named for this process, and the rename is about to publish that file's mode
/// over the state file.
const STATE_FILE_MODE: u32 = 0o600;

/// One line, holding the episode the operator has already been warned about,
/// absent when a HOME reading showed no staleness. NO SESSION ID: one config
/// names one device, so there is one staleness state at a time and every
/// reader of it means the same one.
const STALENESS_MEMORY: &str = "home-staleness";

/// How long the finished turn ran, CLAIMING the marker first.
///
/// The claim is a rename, which is atomic: two Stops racing the same turn
/// cannot both read it and both pulse, because only one rename can succeed.
/// Reading first and unlinking after left that window open, and an unlink
/// that failed left the marker wedged for every later turn.
///
/// It runs BEFORE the reply and the condenser for the same reason. Stop is
/// asynchronous, so the next prompt can arrive while this one is still
/// condensing: with the marker still on disk that prompt writes nothing, and
/// this Stop then deletes the marker its successor was relying on. Claiming
/// up front also keeps the condenser's own latency out of the elapsed time it
/// is measuring.
///
/// The value is VALIDATED before it reaches arithmetic: a truncated write or
/// a hand edit must be a decision, not a crash.
fn consume_turn_marker(session_id: &str) -> Option<u64> {
    let marker = turn_marker(session_id)?;
    let claim = marker.with_extension(format!("claim.{}", std::process::id()));
    std::fs::rename(&marker, &claim).ok()?;
    let started = std::fs::read_to_string(&claim);
    let _ = std::fs::remove_file(&claim);
    let started: u64 = started.ok()?.trim().parse().ok()?;
    Some(now_secs()?.saturating_sub(started))
}

/// The Stop hook: what the turn said, and whether it ran long enough to earn
/// the lights.
fn end_of_turn(payload: &HookPayload, agent: &str) {
    // FIRST, before anything slow: see consume_turn_marker.
    let elapsed = consume_turn_marker(&payload.session_id);
    let reply = turn_reply(payload);
    let (state, detail) = match reply.is_empty() {
        true => ("done".to_string(), String::new()),
        false => condense(&reply),
    };
    run_event(
        &pns::args::EventArgs {
            agent: agent.to_string(),
            state,
            project: project_of(&payload.cwd),
            branch: git_branch(&payload.cwd),
            detail,
            pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
            long_running: pns::pulse::session_was_long(elapsed, Some(pulse_threshold_secs())),
            ..Default::default()
        },
        &system_probes(),
    );
}

/// The StopFailure hook: a turn that died on an API error reports itself,
/// where it used to report nothing at all.
///
/// THE MARKER IS CLAIMED HERE for the same reason `end_of_turn` claims it, and
/// this is the arm that used to leak it: StopFailure fires INSTEAD of Stop, so
/// a dead turn left its marker on disk, the next prompt found one and declined
/// to rewrite the clock, and the turn after that was measured from the dead
/// turn's start. `long_running` is what raises the mobile watch card and the
/// pulse, so one API error promoted later short turns to the long-running tier
/// for the rest of the session.
///
/// NO CONDENSER AND NO TRANSCRIPT. The condenser is a model call on the one
/// path where a model call has just failed, the reply's fallback re-reads the
/// transcript in a bounded loop of sleeps, and neither recovers the news: the
/// harness states it as a plain string that is never empty. The payload's
/// partial `last_assistant_message` is dropped for the same reason, since the
/// question at a dead pane is why it stopped rather than what it had said.
fn failed_turn(payload: &HookPayload, agent: &str) {
    let elapsed = consume_turn_marker(&payload.session_id);
    run_event(
        &pns::args::EventArgs {
            agent: agent.to_string(),
            state: "failed".to_string(),
            project: project_of(&payload.cwd),
            branch: git_branch(&payload.cwd),
            detail: payload.message.clone(),
            pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
            long_running: pns::pulse::session_was_long(elapsed, Some(pulse_threshold_secs())),
            ..Default::default()
        },
        &system_probes(),
    );
}

/// The turn's final text: the harness's own copy first, the transcript tail
/// as the fallback.
///
/// THE FALLBACK IS RE-READ inside a bounded window. The harness has not always
/// flushed the assistant's final text when the Stop hook runs (live capture
/// 2026-08-12: one read came back empty and the notification shipped with no
/// detail at all). An expired window proves only that nothing readable arrived
/// in time; a turn that said nothing, an unreadable transcript and an
/// unparseable one all leave the same empty string and are reported the same.
///
/// Emptiness is judged on the FLATTENED reply, because a block carrying only
/// whitespace is non-empty raw and empty once flattened, which is the same
/// missing-summary symptom through another door.
fn turn_reply(payload: &HookPayload) -> String {
    let flatten = |text: &str| pns::render::flatten_reply(text, REPLY_MAX_CHARS);
    let from_payload = flatten(&payload.last_assistant_message);
    if !from_payload.is_empty() || payload.transcript_path.is_empty() {
        return from_payload;
    }
    for attempt in 0..=reread_attempts() {
        if attempt > 0 {
            std::thread::sleep(reread_interval());
        }
        let reply = flatten(&transcript_reply(&transcript_tail(
            &payload.transcript_path,
        )));
        if !reply.is_empty() {
            return reply;
        }
    }
    String::new()
}

/// The tail of a transcript, never the whole file: a long session grows it
/// past 200MB, and the extraction only ever needs the last turn. Measured
/// 2026-08-05: slurping the whole file held ~33MB resident and minutes of CPU.
fn transcript_tail(path: &str) -> String {
    use std::io::{Read, Seek, SeekFrom};
    // CHECKED BEFORE OPENING, and on the link itself. Opening a FIFO blocks
    // until a writer appears and /dev/zero never ends; both hang a hook whose
    // whole contract is answering promptly. A transcript is a regular file.
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return String::new();
    };
    if !metadata.is_file() {
        return String::new();
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let _ = file.seek(SeekFrom::Start(
        metadata.len().saturating_sub(TRANSCRIPT_TAIL_BYTES),
    ));
    let mut tail = Vec::new();
    // Capped as well as sought: the file can grow between the two calls, and
    // a seek that failed would otherwise read all of it.
    let _ = file.take(TRANSCRIPT_TAIL_BYTES).read_to_end(&mut tail);
    String::from_utf8_lossy(&tail).into_owned()
}

/// The turn condensed to a state and a sentence, by a cheap model when one
/// answers and by trimming the reply when it does not.
fn condense(reply: &str) -> (String, String) {
    let fallback = || ("done".to_string(), pns::render::preview(reply));
    // The re-entry guard: the condenser is itself an agent run, and its own
    // Stop hook would call this again. The stripped home below installs no
    // hooks at all, which is the hard guarantee; this is the cheap one.
    if std::env::var("PNS_SUMMARIZING").is_ok() {
        return fallback();
    }
    let Some(home) = condenser_home() else {
        return fallback();
    };
    let codex = std::env::var("CODEX_BIN").unwrap_or_else(|_| "codex".to_string());
    let mut command = Command::new(&codex);
    command
        .args(["exec", "--ephemeral", "--skip-git-repo-check", "-C"])
        .arg(&home)
        .args(["-s", "read-only", "-"])
        .env("PNS_SUMMARIZING", "1")
        .env("CODEX_HOME", &home);
    let deadline = env_deadline("PNS_CONDENSER_DEADLINE_MS").unwrap_or(CONDENSER_DEADLINE);
    match run_bounded(
        command,
        Some(&condenser_prompt(reply)),
        deadline,
        PROBE_READ_MAX,
    )
    .as_deref()
    .and_then(condenser_verdict)
    {
        Some((state, summary)) => (state, summary.trim().to_string()),
        None => fallback(),
    }
}

/// A private, stripped Codex home: a minimal config (fast model, low
/// reasoning) and the live auth symlinked, with NO hooks or plugins. That cuts
/// the load (~9s to ~3s) and means the condenser run has no Stop hook of its
/// own, which is the hard guarantee against a pns-to-codex-to-pns loop.
/// It is created owner-only, because it points at the live Codex credentials.
fn condenser_home() -> Option<std::path::PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let user_home = std::env::var("HOME").unwrap_or_default();
    let home = resolve_path(
        std::env::var("PNS_CODEX_HOME").ok().as_deref(),
        &format!("{user_home}/.config/pns/codex-home"),
    );
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&home)
        .ok()?;
    let config = home.join("config.toml");
    if !config.exists() {
        let written = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&config)
            .map(|mut file| {
                std::io::Write::write_all(
                    &mut file,
                    b"model = \"gpt-5.5\"\nmodel_reasoning_effort = \"low\"\n",
                )
            });
        let _ = written;
    }
    let auth = home.join("auth.json");
    let _ = std::fs::remove_file(&auth);
    let _ = std::os::unix::fs::symlink(format!("{user_home}/.codex/auth.json"), &auth);
    Some(home)
}

/// The branch the work happened on, or none. Bounded like every other spawn:
/// a wedged git must not hold a notification.
fn git_branch(cwd: &str) -> String {
    if cwd.is_empty() || !std::path::Path::new(cwd).is_dir() {
        return String::new();
    }
    let mut command = Command::new("git");
    command.args(["-C", cwd, "branch", "--show-current"]);
    run_bounded(command, None, GIT_DEADLINE, PROBE_READ_MAX)
        .map(|branch| branch.trim().to_string())
        .unwrap_or_default()
}

/// A blocking event: the round trip started, then the notification, then the
/// operator's decision.
///
/// THE FORWARD STARTS BEFORE THE NOTIFICATION, and that order is the whole
/// point. The phone leg is suppressed because moshi is about to raise the
/// actionable card itself and pns's own push would be the same event twice,
/// so the suppression is only correct once that card is really coming. It
/// used to be applied to the INTENT to forward: an away operator whose
/// moshi-hook could not spawn lost the one notification still able to reach
/// them, in exchange for a round trip that never happened.
///
/// The payload goes back BYTE FOR BYTE, because this hook consumed stdin and
/// a consumed-but-not-forwarded stream leaves moshi with an empty parse,
/// after which it silently does nothing. A payload too large to have arrived
/// whole is the one thing not forwarded: see `payload_is_whole`.
fn blocking_event(payload: &HookPayload, agent: &str, payload_json: &str) -> i32 {
    let event = pns::args::EventArgs {
        agent: agent.to_string(),
        state: "blocked".to_string(),
        project: project_of(&payload.cwd),
        detail: payload.message.clone(),
        pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
        ..Default::default()
    };
    // Each test guards the reading below it: the surface probe never runs for
    // a payload that was never going to be forwarded.
    // ONE probe set for the whole event: the forward decision below and the
    // delivery plan inside run_event are two questions about one moment.
    let probes = system_probes();
    let forwarded = moshi_subcommand(agent)
        .filter(|_| payload_is_whole(payload_json))
        .filter(|_| forward_to_moshi(&probes))
        .and_then(|subcommand| spawn_moshi_hook(&subcommand, payload_json));
    if forwarded.is_some() {
        // Suppressed here rather than by the plan: the card moshi is raising
        // is something the surface model cannot know about.
        unsafe { std::env::set_var("PNS_SKIP_PHONE", "1") };
    }
    run_event(&event, &probes);
    // THE CONFIG IS READ A SECOND TIME HERE, after the notification and
    // immediately before the wait. Threading it out of `run_event` would
    // change that function's signature for one duration, and a view torn
    // between the two reads costs at most this one event's bound.
    forwarded.map_or(0, |child| answer_within(child, submit_deadline()))
}

/// Whether the operator can answer from the phone at all. THE SURFACE decides:
/// on mobile or away the card is the only way to reach them, and at the desk
/// the harness prompt in front of them already is one.
///
/// It is handed the caller's probe set rather than building its own, which is
/// what makes this reading and the delivery plan's reading the SAME one: they
/// are two questions about one moment, and a boundary crossed between two
/// measurements cards a phone with no round trip behind it.
fn forward_to_moshi(probes: &SystemProbes<SystemCommandRunner>) -> bool {
    pns::engine::operator_surface(probes, &overrides_from_env(), now_secs())
        != pns::surface::Surface::Desk
}

/// The probe set for ONE invocation. Built here and shared, never per
/// consumer: see `SystemProbes`.
fn system_probes() -> SystemProbes<SystemCommandRunner> {
    let home = std::env::var("HOME").unwrap_or_default();
    SystemProbes::new(
        SystemCommandRunner,
        resolve_path(
            std::env::var("PNS_PHONE_MARKER_FILE").ok().as_deref(),
            &format!("{home}/.local/state/pns/phone-attention.marker"),
        )
        .to_string_lossy()
        .into_owned(),
    )
}

/// Start moshi on the stream. `None` is "not installed", which is the
/// harness's "no opinion": it prompts as usual.
///
/// THE WRITE HAPPENS OFF THIS THREAD. A child that does not read its stdin
/// blocks the writer as soon as the pipe buffer fills, and a payload larger
/// than that buffer is ordinary. Writing here would put that block in front
/// of the notification and in front of the wait below, which is supposed to
/// be the only place this waits on anybody. The thread outlives a caller that
/// stops waiting, which is fine: it holds a pipe and a copy of the payload,
/// and the process is on its way out.
fn spawn_moshi_hook(subcommand: &str, payload_json: &str) -> Option<std::process::Child> {
    let moshi = moshi_hook_bin();
    let mut child = Command::new(&moshi)
        .arg(subcommand)
        .stdin(Stdio::piped())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        let payload = payload_json.to_string();
        // Dropping the pipe when the write finishes is what gives the child
        // its EOF; a child waiting on one would otherwise never start.
        std::thread::spawn(move || {
            let _ = stdin.write_all(payload.as_bytes());
        });
    }
    Some(child)
}

/// Where the moshi-hook binary is, asked ONE WAY for every caller.
///
/// Two spellings of "where is moshi-hook" is exactly the duplicated rule this
/// crate keeps being bitten by: the day one of them learns a second lookup the
/// other keeps answering the old address, and the two disagree silently. It is
/// also the seam every test drives the binary through, which is what makes a
/// caller stubbable at all.
fn moshi_hook_bin() -> String {
    std::env::var("MOSHI_HOOK_BIN").unwrap_or_else(|_| DEFAULT_MOSHI_HOOK_BIN.to_string())
}

/// Homebrew's own prefix, which is where the cask puts it. `MOSHI_HOOK_BIN`
/// overrides it, and that override is how every test points a caller at a stub
/// instead of at the operator's own moshi.
const DEFAULT_MOSHI_HOOK_BIN: &str = "/opt/homebrew/bin/moshi-hook";

/// Become moshi's answer: the code the submission exited with, and 0 when it
/// yielded none at all.
///
/// THIS IS NOT THE OPERATOR'S DECISION, and the comment that said it was is
/// what sent one whole slice of this program off designing against a wait that
/// does not exist. MEASURED 2026-08-29 against `moshi-hook 0.3.3`: every reply
/// shape the daemon can send ends the wait with exit 0 and empty stdout, so
/// approve and deny are indistinguishable here. The operator's real answer
/// travels the daemon's own tui bridge, which finds the pane, screen-reads the
/// numbered menu and SENDS KEYS into it. The code is still passed through
/// untouched, because the harnesses that read a gate's exit code are entitled
/// to whatever moshi said.
fn moshi_decision(mut child: std::process::Child) -> i32 {
    child
        .wait()
        .ok()
        .and_then(|status| status.code())
        .unwrap_or(0)
}

/// Moshi's answer if it comes inside the deadline, and NO OPINION if it does
/// not.
///
/// THIS IS A REGISTRATION, NOT A HUMAN WAIT. `moshi-hook` writes one line to
/// its daemon's socket and returns as soon as the daemon answers it; the
/// operator's own decision arrives later and by the road `moshi_decision`
/// describes, when the daemon types into the prompt that this hook's return is
/// what allows to be drawn. So a wait measured in minutes is never the
/// operator taking their time, it is a daemon that stopped answering, and
/// holding for it keeps the prompt off their screen for as long as the harness
/// allows: MEASURED at 90 seconds and still climbing against a listener that
/// accepted the connection and never replied.
///
/// EXPIRY RETURNS 0, WHICH IS NO OPINION AND NEVER A DECISION. The harness
/// draws the prompt and the operator answers at the pane.
///
/// AND EXPIRY KILLS THE SUBMISSION, WHICH IS WHAT MAKES THE BOUND REAL.
/// Returning is not enough on its own: the harness decides a
/// `PermissionRequest` by READING THIS HOOK'S STDOUT TO EOF, only stdin is
/// piped to the submission, so a survivor holds that write end open and the
/// prompt stays hidden for the survivor's whole life. MEASURED against a
/// ten-second silent submission: a reader waiting on the process alone 0.18s,
/// a reader waiting on stdout EOF with the child left running 10.03s, and with
/// the kill 0.19s. THE COST is the pending action dying with the child, which
/// is a card a daemon wedged enough to earn this expiry had almost certainly
/// not delivered anyway.
///
/// THE KILL REACHES THE DIRECT CHILD ONLY. `moshi-hook` is a single binary
/// that writes to its daemon's socket itself, so the direct child IS the
/// process holding the pipe. A submission that forked could leave a grandchild
/// holding it open, and that day the kill has to widen to the process group.
///
/// THE ANSWERED PATH IS UNTOUCHED. A submission that finishes inside the
/// deadline reaches `moshi_decision` exactly as it did before: no pipe, no
/// cap, stdout still inherited, which is the contract
/// `what_moshi_says_on_stdout_reaches_the_harness_unchanged` pins.
///
/// NOT `run_bounded`. That helper pipes the child's stdout on its way to
/// attaching a deadline, and this path's whole stdout contract is that moshi's
/// stream IS the hook's stream.
fn answer_within(mut child: std::process::Child, deadline: Duration) -> i32 {
    let expires_at = std::time::Instant::now() + deadline;
    loop {
        match child.try_wait() {
            // Still `moshi_decision`'s job to turn a finished child into a
            // code; this only decides WHEN it is asked.
            Ok(Some(_)) => return moshi_decision(child),
            // A wait that cannot be performed yielded no code, which is
            // `moshi_decision`'s own no-opinion case arriving by another route.
            Err(_) => return 0,
            Ok(None) => {}
        }
        if std::time::Instant::now() >= expires_at {
            let _ = child.kill();
            // REAPED, not merely signalled: an unreaped child is a zombie
            // holding its slot until pns exits, and the wait is instant on a
            // process already killed.
            let _ = child.wait();
            return 0;
        }
        std::thread::sleep(SUBMISSION_POLL_INTERVAL);
    }
}

/// How often that wait looks. Ten milliseconds is `run_bounded`'s own tick:
/// short enough to add no latency an operator could notice on a submission
/// answered in roughly 150, long enough not to spin a core.
const SUBMISSION_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// How long that wait may last: the test hatch, then the operator's own
/// `[plugins.moshi] submit_deadline_secs`, then the default.
fn submit_deadline() -> Duration {
    // A LITERAL ZERO IS NOT A BOUND, it is this wait switched off by accident,
    // and the config layer already refuses one by name for that reason. The
    // refusal sits here rather than in `env_deadline`, which keeps the
    // accepted semantics the payload hatch shares with it: a zero here falls
    // through to the config, exactly as an unset variable would.
    env_deadline("PNS_MOSHI_SUBMIT_DEADLINE_MS")
        .filter(|deadline| !deadline.is_zero())
        .unwrap_or_else(configured_submit_deadline)
}

/// The configured bound, and the DEFAULT for every way of not stating one.
///
/// A config that is absent or unreadable asked for nothing, which is the
/// default; a config that states a value this layer refuses says so OUT LOUD
/// and then takes the default too, because an operator who asked for something,
/// did not get it and was told nothing is the defect one level down.
fn configured_submit_deadline() -> Duration {
    let home = std::env::var("HOME").unwrap_or_default();
    let config = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config,
        _ => pns::config::Config::default(),
    };
    pns::config::submit_deadline(&config).unwrap_or_else(|error| {
        eprintln!(
            "pns: config error ({}); the moshi submission keeps its {}-second bound",
            error.detail(),
            pns::config::DEFAULT_SUBMIT_DEADLINE_SECS
        );
        Duration::from_secs(pns::config::DEFAULT_SUBMIT_DEADLINE_SECS)
    })
}

/// The harness payload from stdin, bounded in SIZE and in TIME.
///
/// Neither bound is theoretical: a pipe nobody closes hangs the hook before
/// the exit contract can run, and a payload nobody caps can exhaust memory
/// long before the reply's own character cap applies.
fn read_payload() -> Option<String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut payload = String::new();
        // ONE BYTE PAST the cap, so a payload that hit it is distinguishable
        // from one that merely reached it: see `payload_is_whole`.
        let read = std::io::Read::read_to_string(
            &mut std::io::Read::take(std::io::stdin(), MAX_PAYLOAD_BYTES + 1),
            &mut payload,
        );
        let _ = sender.send(read.ok().map(|_| payload));
    });
    // The reader thread outlives a refusal, which is fine: the process is
    // about to exit, and it holds nothing but its own buffer.
    receiver.recv_timeout(payload_deadline()).ok().flatten()
}

/// The project an event belongs to: the last segment of the working directory.
fn project_of(cwd: &str) -> String {
    cwd.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn now_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs())
}

/// How many extra times the transcript is re-read while the harness flushes.
/// VALIDATED before it is believed, and falling back to the default rather
/// than to no retries.
fn reread_attempts() -> u32 {
    reread_attempts_from(std::env::var("PNS_REPLY_REREAD_ATTEMPTS").ok().as_deref())
}

fn reread_interval() -> Duration {
    reread_interval_from(std::env::var("PNS_REPLY_REREAD_INTERVAL").ok().as_deref())
}

/// The count, clamped. See `MAX_REREAD_ATTEMPTS`.
fn reread_attempts_from(raw: Option<&str>) -> u32 {
    raw.and_then(|raw| raw.parse().ok())
        .unwrap_or(DEFAULT_REREAD_ATTEMPTS)
        .min(MAX_REREAD_ATTEMPTS)
}

/// The interval, clamped.
///
/// `try_from_secs_f64` IS the validation, and it replaced a hand-written one
/// that looked complete: NaN, infinity and negatives were refused, but a
/// finite oversized value like `1e300` passed and panicked the constructor
/// anyway, exiting 101 on a path whose whole contract is exiting 0.
fn reread_interval_from(raw: Option<&str>) -> Duration {
    raw.and_then(|raw| raw.parse::<f64>().ok())
        .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
        .unwrap_or(DEFAULT_REREAD_INTERVAL)
        .min(MAX_REREAD_INTERVAL)
}

/// How long a turn must run to earn the lights.
fn pulse_threshold_secs() -> u64 {
    std::env::var("PNS_PULSE_THRESHOLD_SECS")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(pns::pulse::DEFAULT_LONG_SESSION_SECS)
}

/// A harness payload is a small JSON object; anything larger is not one.
const MAX_PAYLOAD_BYTES: u64 = 1_000_000;

/// Whether the payload is the bytes the harness actually sent.
///
/// A payload that reached the cap was CUT MID-OBJECT, so it is no longer
/// JSON and no longer what anybody wrote. Forwarding it hands moshi an empty
/// parse, which is the exact failure the byte-for-byte rule exists to
/// prevent; measured 2026-08-19, a 1.2MB payload forwarded as exactly
/// 1,000,000 bytes. The notification still goes out, carrying whatever an
/// unparseable payload yields, because something IS blocked either way.
fn payload_is_whole(payload_json: &str) -> bool {
    payload_json.len() <= MAX_PAYLOAD_BYTES as usize
}

/// How long the payload may take to arrive. Generous, because a harness
/// writing a large transcript path is normal and a hang is not.
fn payload_deadline() -> Duration {
    env_deadline("PNS_PAYLOAD_DEADLINE_MS").unwrap_or(Duration::from_secs(5))
}

/// A deadline override in milliseconds, for tests that must prove expiry
/// without waiting out the production window.
fn env_deadline(variable: &str) -> Option<Duration> {
    std::env::var(variable)
        .ok()?
        .parse()
        .ok()
        .map(Duration::from_millis)
}

/// At most this much of a turn reaches the condenser or the notification.
const REPLY_MAX_CHARS: usize = 8000;

/// The last few megabytes of a transcript parse in well under a second, and
/// carry far more than one turn.
const TRANSCRIPT_TAIL_BYTES: u64 = 4_000_000;

/// Four extra reads at 150ms: enough for the harness to finish flushing,
/// short enough that a turn which really said nothing is reported promptly.
const DEFAULT_REREAD_ATTEMPTS: u32 = 4;
const DEFAULT_REREAD_INTERVAL: Duration = Duration::from_millis(150);

/// The ceilings on those two knobs. Their PRODUCT is how long a Stop hook can
/// sit re-reading a transcript that is never going to fill, so each is capped
/// rather than believed: a stray zero in either costs seconds, never hours.
const MAX_REREAD_ATTEMPTS: u32 = 10;
const MAX_REREAD_INTERVAL: Duration = Duration::from_secs(5);

/// The condenser is a model call on a notification path: worth a few seconds,
/// never worth holding a turn's report.
const CONDENSER_DEADLINE: Duration = Duration::from_secs(30);

/// A branch lookup is a local read; anything slower than this is a wedged
/// repository, not an answer worth waiting for.
const GIT_DEADLINE: Duration = Duration::from_secs(5);

/// One notification from argv.
fn event_mode() {
    // Lossy rather than validating: a stray byte in argv degrades into an
    // unknown token, which the lenient contract already skips, instead of
    // aborting an always-exit-0 notification.
    let (event, warnings) = parse_args(
        std::env::args_os()
            .skip(1)
            .map(|argument| argument.to_string_lossy().into_owned()),
    );
    for warning in &warnings {
        eprintln!("pns: {warning}");
    }
    run_event(&event, &system_probes());
}

/// One notification, end to end: decide, render, dispatch. THE one event path,
/// whether the event came from argv or from a harness hook.
fn run_event(event: &pns::args::EventArgs, probes: &SystemProbes<SystemCommandRunner>) {
    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    // Read off the config before selection consumes it: the pulse needs hue's
    // settings, the plan needs moshi's card toggle, the catch-up needs the
    // whole `[recap]` table, and the two network channels need their secrets.
    //
    // THE RECAP TRAVELS AS ONE NAMED VALUE, never as a row of loose booleans.
    // Three of its four fields are bools; spread into this tuple they would sit
    // adjacent here and in the call below, which is a swap nothing would catch,
    // and a struct with named fields cannot be transposed.
    let (hue_table, watch_card, moshi_token, hermes_key, recap, focus_silence) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (
            enabled_hue_table(config),
            mobile_watch_card(config),
            plugin_settings(config, "moshi").and_then(moshi_secret),
            plugin_settings(config, "hermes").and_then(hermes_secret),
            config.recap.clone(),
            config.focus_silence.clone(),
        ),
        // A config that could not be read falls back to the DEFAULTS of all
        // five, and deliberately disagrees with the plugin selection below,
        // which falls back to the whole roster. Selection keeps notifications
        // working through a broken config; these say what an operator asked
        // for, and an unreadable file asked for nothing: with no secrets, the
        // network channels are simply not set up.
        //
        // THE CATCH-UP IS THE ONE THAT FALLS BACK ON, which is `[recap]`'s
        // own rule (absent is every switch on) reaching the case where the
        // file is unreadable rather than absent. A config nobody can parse
        // must not silently stop delivering misses the doctor is already
        // telling the operator are waiting.
        //
        // THE FOCUS LIST FALLS BACK TO EMPTY, which is the feature off. It is
        // the same reading as the secrets rather than the recap's: an
        // unreadable file asked for nothing, and a Focus policy nobody could
        // read must not silence a notification.
        _ => (
            None,
            false,
            None,
            None,
            pns::config::Recap::default(),
            Vec::new(),
        ),
    };
    let (selection, warning) = select_plugins(&roster(), loaded);
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }
    // WHETHER A RECAP HAS ANYWHERE TO LAND, read off the SELECTION rather than
    // off the config directly, so a missing or unreadable config falls back to
    // the whole roster exactly as dispatch does. A machine that turned the
    // durable channel off has said there is nowhere for a recap to go, and a
    // card reading "recap in #pns" against an empty channel is the one thing
    // the card's own spawn check exists to prevent.
    let durable_route = selection.iter().any(|plugin| plugin.name == "hermes");

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs());
    // THE MUTE IS AN INPUT TO THE DECISION, stated here and nowhere else. It
    // is never a filter over `decision.legs` afterwards: which legs are
    // decorative is routing's policy, and re-deriving it here would be the
    // second copy of a rule that then drifts. `overrides_from_env` cannot
    // reach the field, which is what keeps a variable from ever muting the
    // operator or ending a mute they are still inside.
    //
    // THE OPERATING SYSTEM'S MUTE IS STATED THE SAME WAY, off the Do Not
    // Disturb store rather than a state file pns writes. An unreadable store
    // reads as not silenced: see `focus_now`.
    let overrides = Overrides {
        muted: muted_now(now_secs),
        focus_active: focus_now(&home, &focus_silence).is_ok_and(|reading| reading.silenced),
        ..overrides_from_env()
    };

    let decision = decide(
        probes,
        &selection,
        &overrides,
        event.local_only,
        event.remote_only,
        &event.pane,
        now_secs,
        event.long_running,
        watch_card,
    );

    let outcomes = if decision.legs.is_empty() {
        // A verdict that must be SAID, but only for the contradiction the
        // caller asked for: a silent exit is indistinguishable from delivery.
        if event.local_only && event.remote_only {
            println!(
                "pns: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent"
            );
        }
        Vec::new()
    } else {
        // CLONED rather than moved: the catch-up below dispatches on the
        // same two secrets, and reading the config a second time would be a
        // second answer to a question already asked.
        let outcomes = dispatch_legs(
            &decision.legs,
            decision.pane_dropped,
            event,
            &home,
            moshi_token.clone(),
            hermes_key.clone(),
        );
        for (leg, delivered) in &outcomes {
            // THE ONE PLACE a delivery reaches the operator, and the one place
            // the `pns: ` prefix is written. A channel says WHAT happened; the
            // leg's mode says whether anyone hears it, and this says how it is
            // labelled, so a second caller that labels its lines by plugin
            // name does not have to unpick a prefix out of the middle of one.
            if let Some(line) = delivered.clone().line_for(leg.mode) {
                println!("pns: {line}");
            }
        }
        outcomes
    };

    // THE RECORD GOES HERE, after every channel and before the pulse. After,
    // because the leg verdicts are part of it and because a crash in recording
    // must not cost a channel; before, because the pulse talks to a bridge
    // under a ten-second deadline and would take the record with it. THE
    // ACCEPTED PRICE, stated: a decision is lost if a channel hangs to its
    // deadline and the process is killed before this runs.
    //
    // BOTH BRANCHES RECORD. "Nothing fired" is exactly what an operator opens
    // the report to ask about.
    record_decision(&pns::decision_log::Record {
        event,
        decision: &decision,
        overrides: &overrides,
        legs: &outcomes,
    });
    // THE JOURNAL GOES WITH IT, inheriting the ordering contract stated above
    // rather than restating it: same site, same accepted price, and both
    // branches reach it, including the empty-plan branch, which is where most
    // misses live.
    record_missed(event, &decision, &overrides);
    // AND THE ACTIVITY RING WITH IT, at the same site and under the same
    // ordering contract and the same fail-quiet rule. It records
    // UNCONDITIONALLY, which is the whole difference between it and the
    // journal above: the recap's window is every event, delivered or not.
    record_activity(event, &decision);

    // THE CATCH-UP GOES AFTER BOTH RECORDS AND BEFORE THE PULSE, inheriting
    // the ordering contract stated above rather than restating it: a slow
    // replay must not cost either record, and a card the operator may be
    // waiting on outranks decoration.
    replay_missed(
        recap,
        &decision,
        &home,
        moshi_token,
        hermes_key,
        durable_route,
    );
    // AND THE MARKER MOVES AFTER IT, never before: the catch-up above is what
    // READS the window this closes, and moving the edge first would hand it a
    // window one event wide on every return.
    mark_present(&decision);

    // THE PULSE GOES LAST, after every channel the operator might be waiting
    // on. It is part of the PLAN rather than a second invocation (the shell
    // used to call `pns pulse` alongside the notification, so the tier was
    // decided twice and could disagree with itself), but it talks to a bridge
    // over the network under a ten-second deadline, and nothing an operator
    // reads should queue behind decoration. It still fires for a plan that
    // reached no channel at all: the lights are not a leg.
    if decision.plan.pulse {
        // The state IS the exit code here: the shell notifier derives
        // --state from `$?`, and an agent turn that did not fail succeeded.
        fire_pulse_unless_quiet(hue_table, if event.state == "failed" { "1" } else { "0" });
    }
}

/// Every leg to its destination, in the registry's delivery order, each
/// paired with what its channel had to say for itself.
///
/// IT RETURNS ITS OUTCOMES RATHER THAN PRINTING THEM. An event prints only what
/// a reporting leg said; a hand-run check labels every outcome with its
/// plugin's name and prints the lot. Two callers spelling one report two ways
/// is exactly what a returned value is for.
///
/// THE LEGS AND THE SCRUB ARRIVE AS VALUES, not as a `Decision`: a caller that
/// took no decision has none to hand over.
fn dispatch_legs(
    legs: &[pns::routing::Leg],
    pane_dropped: bool,
    event: &pns::args::EventArgs,
    home: &str,
    moshi_token: Option<String>,
    hermes_key: Option<String>,
) -> Vec<(pns::routing::Leg, Delivery)> {
    // Sanitized ONCE here rather than per channel: a channel may be written in
    // any language and cannot be expected to share the guard. Warned about
    // only now, because a scrub nobody was going to receive is not news.
    let pane = if pane_dropped {
        eprintln!("pns: dropped a pane id with shell metacharacters; no channel will focus a pane");
        ""
    } else {
        event.pane.as_str()
    };
    let rendered = rendered_event(event, pane);

    let channels_dir_override = std::env::var("PNS_CHANNELS_DIR")
        .ok()
        .filter(|dir| !dir.is_empty());
    let channels_dir = resolve_path(
        channels_dir_override.as_deref(),
        &format!("{home}/.local/libexec/pns/channels"),
    );
    let banner = banner_channel();
    let moshi = moshi_channel(moshi_token);
    let hermes = hermes_channel(hermes_key, hermes_url_for(&event.channel));

    // NO `?` AND NO EARLY RETURN: one channel's failure costs the others
    // nothing, and every channel above was constructed before the first
    // delivery, so a leg cannot be lost to a sibling's refusal.
    legs.iter()
        .map(|leg| {
            // A PANIC IS ONE LEG'S FAILURE, never the run's. Without this an
            // unwinding channel takes the remaining legs and, in a hand-run
            // check, the rest of the census with it, and a census that ended
            // early is read as a report that finished. The default hook still
            // prints its own trace to stderr, which is left alone: silencing
            // it process-wide would hide every other panic in the binary.
            let delivered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                deliver_leg(
                    leg,
                    &rendered,
                    &banner,
                    &moshi,
                    &hermes,
                    native_first(channels_dir_override.is_some()),
                    &channels_dir,
                )
            }))
            .unwrap_or_else(|_| {
                // NO PAYLOAD TEXT: a panic message is written for a developer
                // and may quote anything the channel was holding.
                Delivery::Failed(format!(
                    "the {} channel PANICKED; nothing was sent",
                    leg.name
                ))
            });
            (*leg, delivered)
        })
        .collect()
}

/// Every override the engine reads, out of the process environment.
fn overrides_from_env() -> Overrides {
    Overrides::from_env(
        &std::env::vars_os()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>(),
    )
}

/// The parsed arguments plus the sanitized pane, rendered into the one event
/// every channel is handed.
fn rendered_event(event: &pns::args::EventArgs, pane: &str) -> pns::channels::Event {
    let message = render::message(&event.branch, &event.detail, &event.state);
    pns::channels::Event {
        agent: event.agent.clone(),
        state: event.state.clone(),
        project: event.project.clone(),
        branch: event.branch.clone(),
        detail: event.detail.clone(),
        title: render::title(&event.agent, &event.state, &event.project),
        preview: render::preview(&message),
        message,
        pane: pane.to_string(),
    }
}

/// Hue's settings, only when the operator enabled it explicitly.
fn enabled_hue_table(config: &pns::config::Config) -> Option<toml::Table> {
    config
        .plugins
        .get("hue")
        .filter(|hue| hue.enabled)
        .map(|hue| hue.settings.clone())
}

/// Whether a card fires while the operator is watching the pane on mobile.
///
/// DEFAULT OFF (operator ruling 2026-08-12): a card about the pane already on
/// screen is noise, and the pulse alone marks the long command finishing.
///
/// A value of the WRONG TYPE is refused out loud, the way the config layer
/// refuses a non-boolean `enabled` by name. Reading `"true"` as false is the
/// same defect one level down: the operator asked for something, did not get
/// it, and was told nothing.
fn mobile_watch_card(config: &pns::config::Config) -> bool {
    let Some(stated) = config
        .plugins
        .get("moshi")
        .and_then(|moshi| moshi.settings.get("mobile_watch_card"))
    else {
        return false;
    };
    stated.as_bool().unwrap_or_else(|| {
        eprintln!(
            "pns: config error (moshi.mobile_watch_card is {}, not a boolean); the mobile watching card stays off",
            stated.type_str()
        );
        false
    })
}

/// One plugin's settings table, when the config carries the plugin at all.
fn plugin_settings<'config>(
    config: &'config pns::config::Config,
    name: &str,
) -> Option<&'config toml::Table> {
    config.plugins.get(name).map(|plugin| &plugin.settings)
}

/// The event path's pulse, which the lights' own quiet window may mute.
///
/// THE GATE LIVES HERE, at the call site, and not in `fire_pulse` below:
/// `pns pulse` shares that function and is deliberately exempt, because the
/// hand-run pulse is how a bridge and key are checked and gating it would make
/// the quiet window untestable exactly while it is on. Inside the `if` that
/// already earned a pulse, so a refusal is printed only where a room would
/// otherwise have lit.
fn fire_pulse_unless_quiet(hue_table: Option<toml::Table>, exit_code: &str) {
    // No table is nothing to quiet: an operator who never enabled the lights
    // gets the same silence `fire_pulse` would have given them.
    let Some(settings) = hue_table else {
        return;
    };
    let window = match quiet_window(&settings) {
        Ok(window) => window,
        // FAIL CLOSED, the direction the pulse takes on every unreadable
        // reading: a window nobody can parse is an operator who asked for
        // quiet hours and cannot be told which ones, so the room stays dark
        // and the refusal says why.
        Err(refusal) => {
            eprintln!("{refusal}");
            return;
        }
    };
    // FRESH, not the run's start: the legs above dial the network under their
    // own deadlines, so a run can cross into the window between starting and
    // reaching the moment a room would actually light, and the older reading
    // would flash it just inside quiet hours. HONEST LIMIT: no suite pins the
    // freshness, because a test's clock does not advance mid-run.
    if !quiet_now(
        window.as_ref(),
        now_secs().and_then(local_minutes_since_midnight),
    ) {
        fire_pulse(Some(settings), exit_code);
    }
}

/// The lights signal, from whichever mode asked for it, and how many rooms it
/// reached. Both notification callers discard the count; the hand-run check is
/// what it exists for, since the bridge acknowledges no write and a room that
/// was addressed is the last observable fact on this path.
fn fire_pulse(hue_table: Option<toml::Table>, exit_code: &str) -> usize {
    let Some(hue) = hue_table.and_then(|settings| {
        hue_settings(&settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())
    }) else {
        return 0;
    };
    HuePulse {
        bridge: UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
        },
        rooms: hue.rooms,
    }
    .run(exit_code)
}

/// Whether the config's hue table resolves to a bridge that could be dialled:
/// the same reading `fire_pulse` takes, taken BEFORE it, so a check can tell a
/// bridge that listed no room from a config that names no bridge at all.
fn hue_resolves(hue_table: Option<&toml::Table>) -> bool {
    hue_table.is_some_and(|settings| {
        hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()).is_some()
    })
}

/// The pulse behind the same boundary every leg gets, so a panicking bridge
/// call costs the census the rest of its lines rather than ending the report
/// where the operator reads it as complete.
fn pulse_outcome(hue_table: Option<toml::Table>) -> pns::doctor::Outcome {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fire_pulse(hue_table, "0"))) {
        Ok(rooms) => pns::doctor::Outcome::Signalled(rooms),
        // NO ROOM IS CLAIMED, and no panic text is quoted: the message is
        // written for a developer and may hold anything the pulse was carrying.
        Err(_) => {
            pns::doctor::Outcome::Failed("the pulse PANICKED; no room was signalled".to_string())
        }
    }
}

/// The banner, which now only needs to know where to send the click.
fn banner_channel() -> BannerChannel<SystemCommandRunner> {
    BannerChannel {
        runner: SystemCommandRunner,
        // An EMPTY override falls through, so an exported-but-blank variable
        // cannot shadow the inherited bundle id.
        terminal_id: std::env::var("PNS_TERMINAL_BUNDLE_ID")
            .ok()
            .filter(|id| !id.is_empty())
            .or_else(|| {
                std::env::var("__CFBundleIdentifier")
                    .ok()
                    .filter(|id| !id.is_empty())
            })
            .unwrap_or_default(),
        herdr_path: executable_in_path("herdr"),
    }
}

/// The moshi push, with the token the config already provided.
fn moshi_channel(token: Option<String>) -> MoshiChannel<UreqPost> {
    MoshiChannel {
        http: UreqPost::default(),
        token,
        url: url_from_env("PNS_MOSHI_URL", DEFAULT_MOSHI_URL),
    }
}

/// The hermes post, with the key the config already provided.
fn hermes_channel(key: Option<String>, url: String) -> HermesChannel<UreqSignedPost> {
    HermesChannel {
        post: UreqSignedPost,
        key,
        url,
        sync_deadline: remote_deadline(std::env::var("PNS_REMOTE_TIMEOUT").ok().as_deref()),
    }
}

/// The hermes endpoint one event posts to. The env override wins (an explicit
/// URL, the tests' escape hatch), then a `--channel` route name derived from
/// the default gateway, then the default route (`/webhooks/pns`) itself. The
/// gateway has no route named "alert"; the default is where an event with no
/// route named goes. An unusable name is said out loud and falls back
/// LOUD-WARD: a misrouted notification on the default route beats a silently
/// dropped one.
fn hermes_url_for(channel: &str) -> String {
    let env_override = std::env::var("PNS_HERMES_URL")
        .ok()
        .filter(|url| !url.is_empty());
    if let Some(url) = env_override {
        return url;
    }
    if channel.is_empty() {
        return DEFAULT_HERMES_URL.to_string();
    }
    channel_url(DEFAULT_HERMES_URL, channel).unwrap_or_else(|| {
        eprintln!(
            "pns: --channel {channel:?} is not a usable route name; posting to the default route"
        );
        DEFAULT_HERMES_URL.to_string()
    })
}

/// An endpoint override, where EMPTY means the default like every other path
/// and URL this binary reads.
fn url_from_env(variable: &str, default: &str) -> String {
    std::env::var(variable)
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// One leg to its destination: the native plugin when it wins, else the
/// executable channel of that name.
fn deliver_leg(
    leg: &pns::routing::Leg,
    rendered: &pns::channels::Event,
    banner: &BannerChannel<SystemCommandRunner>,
    moshi: &MoshiChannel<UreqPost>,
    hermes: &HermesChannel<UreqSignedPost>,
    native_wins: bool,
    channels_dir: &Path,
) -> Delivery {
    if native_wins {
        match leg.name {
            "macos-banner" => return banner.deliver(rendered, leg.mode),
            "moshi" => return moshi.deliver(rendered, leg.mode),
            "hermes" => return hermes.deliver(rendered, leg.mode),
            _ => {}
        }
    }
    deliver(
        &channels_dir.join(format!("{}.sh", leg.name)),
        &rendered.to_json(leg.mode),
    )
}

/// A path from the environment, defaulting like bash's `${VAR:-default}`:
/// EMPTY means the default as much as unset does, because joining a filename
/// to an empty path resolves into the current directory and quietly delivers
/// nothing.
fn resolve_path(candidate: Option<&str>, default: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(
        candidate
            .filter(|value| !value.is_empty())
            .unwrap_or(default),
    )
}

/// The first executable of that name on PATH, absolute, or None. The click
/// string bakes it in because the click runs in a bare launchd context whose
/// PATH cannot find `~/.local/bin`.
fn executable_in_path(name: &str) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            std::fs::metadata(candidate)
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .map(|path| path.to_string_lossy().into_owned())
}

/// Hand one channel its event on stdin. A channel that is missing, is not
/// executable, or fails is not an error: it is simply not installed, or it
/// declined, and neither may take down the siblings or the caller.
///
/// SILENT ON THE NOTIFICATION PATH whichever verdict it answers with: the
/// common failure here is a channel nobody installed, and reporting that on
/// every event would be noise. THE TWO ARE STILL DIFFERENT VERDICTS. A channel
/// that ran and said nothing is `Silent`; one that never started is
/// `Unlaunched`, which prints nowhere an event can see and is what lets a
/// hand-run check tell a delivery from a spawn that never happened. The exit
/// status of a channel that DID run is still dropped, because a channel
/// declining is its own business.
fn deliver(channel: &Path, event: &str) -> Delivery {
    let mut child = match Command::new(channel).stdin(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(error) => {
            return Delivery::Unlaunched(format!(
                "could not launch the channel at {} ({error}); nothing was sent",
                channel.display()
            ));
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        // Newline-terminated, as the bash's `jq -cn` emitted it: a channel
        // reading one line with `read -r` gets nothing without it.
        let _ = stdin.write_all(event.as_bytes());
        let _ = stdin.write_all(b"\n");
    }
    let _ = child.wait();
    Delivery::Silent
}

/// The `pulse` mode: read the hue table and signal the bridge with the exit
/// code it was handed. Every absence is a silent exit 0.
///
/// NOTHING IN THIS REPO CALLS IT. The tiers that used to are part of the event
/// plan now, which is what stopped the tier being decided twice; this stays as
/// the operator's own command for signalling the lights by hand, and for
/// checking that a bridge and key in the config actually work. It ignores
/// `hue.quiet_hours` on purpose: the gate lives at the event path's call site
/// in `fire_pulse_unless_quiet`, so a hand-run pulse still lights the room
/// inside the window, which is what keeps the window checkable while it is on.
fn pulse_mode() {
    let home = std::env::var("HOME").unwrap_or_default();
    // FAIL CLOSED, unlike an event. The roster fallback that keeps every
    // notification working through a broken config is an EVENT-mode rule:
    // applying it here would let an unrelated typo switch a deliberately
    // disabled pulse back on. The pulse runs only when its own table says
    // enabled, explicitly.
    let config = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config,
        // Absent is not a mistake; never opting in earns no warning.
        Ok(LoadOutcome::Missing) => return,
        Err(error) => {
            // The sanitized detail event mode prints, with the outcome THIS
            // mode had: there is no recoverable setting to fall back to, so
            // nothing pulses.
            eprintln!("pns: config error ({}); no pulse", error.detail());
            return;
        }
    };
    fire_pulse(
        enabled_hue_table(&config),
        &std::env::args_os()
            .nth(2)
            .map(|code| code.to_string_lossy().into_owned())
            .unwrap_or_else(|| "0".to_string()),
    );
}

/// The `home` mode: one reading of the home probe, reported in one line, and
/// the one stale-identifier alert that reading may earn.
///
/// A DIAGNOSTIC FIRST: it always exits 0 and says what it found, including
/// every way it can be unconfigured, because its job is to answer "why did the
/// probe not read" as much as "is the device home". The key itself is never
/// printed, on any path.
///
/// AND THE TRIGGER for the stale-identifier alert, on exactly the condition
/// that prints the warning. This is the only code that reads the sensor and it
/// already holds the derive/decide/remember trio, so one call site keeps ONE
/// memory and ONE decision; a second entrypoint would be a second place for
/// the episode decision to fall out of step. The consequence is deliberate: a
/// hand-run `pns home` no longer consumes an episode silently, it delivers it.
fn home_mode() {
    use pns::home::{HomePresence, SetupFailure, report, setup_report};
    let home_dir = std::env::var("HOME").unwrap_or_default();
    let config = match load_config(&config_path(&home_dir)) {
        Ok(LoadOutcome::Loaded(config)) => config,
        Ok(LoadOutcome::Missing) => {
            println!("{}", setup_report(&SetupFailure::NoConfigFile));
            return;
        }
        Err(error) => {
            println!(
                "{}",
                setup_report(&SetupFailure::ConfigError(error.detail().to_string()))
            );
            return;
        }
    };
    // EVERY CAUSE IS DECIDED IN THE LIBRARY, so each line is pinned by a
    // value-in, value-out test and this stays wiring: a missing table, a
    // disabled one, a brand nothing answers and a mistyped value each send
    // the operator to a different edit, and one message covering two of them
    // sends half of them to the wrong one.
    let router_table = match pns::home::enabled_router_table(&config) {
        Ok(table) => table,
        Err(failure) => {
            println!("{}", setup_report(&failure));
            return;
        }
    };
    // WHERE THE ALERT GOES, settled at the config read rather than at the
    // post. `hermes_url_for`'s own refusal names `--channel`, a flag nobody
    // typed on this path; this one names the key in the file, and it is said
    // on every run of the diagnostic instead of only on the run that happens
    // to have something to deliver.
    let (alert_route, complaint) = pns::home::stale_alert_channel(router_table);
    if let Some(complaint) = complaint {
        eprintln!("{complaint}");
    }
    let settings = match pns::home::router_settings(router_table) {
        Ok(settings) => settings,
        Err(failure) => {
            println!("{}", setup_report(&failure));
            return;
        }
    };
    // The key stays its own read, so it never joins the settings in a type
    // that could be dumped whole.
    let Some(key) = pns::home::router_api_key(router_table) else {
        println!("{}", setup_report(&SetupFailure::NoApiKey));
        return;
    };
    let router = pns::home::UniFiRouter::new(settings.router_url, key);
    // STILL WIRING: the library decides what is stale, what its episode is
    // called and whether that is news; this reads the memory, prints, and
    // writes the memory back.
    let reading = pns::home::read_home(&router, &settings.device);
    // ONE DERIVATION, ONE DECISION. The episode is spelled once and the news
    // decided once, then the SAME value is what gets printed and what gets
    // remembered: two derivations of one fact, one in the print and one in
    // the write, can only stay in step for as long as neither grows a
    // condition of its own.
    let staleness = pns::home::stale_identifiers(&reading);
    let episode = staleness.as_ref().map(pns::home::episode_id);
    let news = pns::home::is_new_staleness(remembered_staleness().as_deref(), episode.as_deref());
    // ONE VALUE FEEDS BOTH SURFACES. The sentence the terminal prints and the
    // sentence the alert carries come out of this same Option, so there is no
    // second condition that could deliver what was not printed, or print what
    // was not delivered. It is Some only for a HOME reading with a
    // disagreement that is news, which is what keeps away, unreadable and
    // already-told runs silent without a guard of their own.
    let alert = staleness.as_ref().filter(|_| news);
    println!("{}", report(&reading, alert));
    // THE WARNING, DELIVERED. An ordinary event ABOUT the reading, handed to
    // the one event path: presence, surface and the leg plan decide where it
    // lands exactly as they do for a finished agent turn. Nothing narrows it
    // and it is not long-running, so it raises no pulse.
    //
    // DISPATCH BEFORE REMEMBER, AND THE ORDER IS LOAD-BEARING. Tidied into
    // remember-then-dispatch it would silently LOSE an alert: a crash, a
    // wedged channel or a kill between the two leaves the episode recorded
    // and never delivered, and the next run reads it as already told. This
    // way round the same interruption re-alerts instead, and two overlapping
    // hand runs that both read the memory before either writes both alert.
    // Duplicates are the direction to fail in.
    //
    // THE COST, ACCEPTED: the delivery OUTCOME is not consulted before the
    // write either, so a post the gateway rejected consumes the episode just
    // as a delivered one does. Fire-and-forget is this engine's contract for
    // every producer, and the printed line above has already told the one
    // human who typed the command.
    if let Some(staleness) = alert {
        run_event(
            &pns::args::EventArgs {
                agent: "pns".to_string(),
                state: "stale".to_string(),
                detail: pns::home::stale_warning(staleness),
                channel: alert_route,
                ..Default::default()
            },
            &system_probes(),
        );
    }
    // ONLY A HOME READING HAS AN OPINION ABOUT THE IDENTIFIERS. NotHome and
    // Unknown both hand `stale_identifiers` a None, and writing that back
    // would read "the disagreement resolved" out of a trip to the shops or a
    // five-second router timeout: the same invention as reading a failed
    // fetch as NotHome, one layer up. Away and unreadable leave the memory
    // untouched, so the warning stays once per STATE rather than once per
    // homecoming.
    if matches!(reading.presence, HomePresence::Home { .. }) {
        remember_staleness(episode.as_deref());
    }
}

/// The `doctor` mode: one test send through every enabled channel, and one
/// line per REGISTERED plugin about what happened.
///
/// EVERY SUPPRESSION GATE IS BYPASSED, and structurally rather than by a flag.
/// `decide()` is never called, so the presence verdict, the viewed-pane rule
/// and the two phone overrides have nothing to say here; the mute is read in
/// `run_event`, which this is not on; and the pulse goes through `fire_pulse`,
/// the hand-run path `pns pulse` uses, so the lights' quiet window never sees
/// it either. A check that can be suppressed proves nothing about the channel
/// it was checking, and every one of those gates exists to stop a destination
/// receiving.
///
/// THE CENSUS IS THE WHOLE ROSTER, never the selection: a plugin the config
/// left off has to be VISIBLY absent by choice, or the report answers "what is
/// on" when the operator asked "what will reach me".
///
/// EVERY SEND GOES THROUGH THE ENGINE'S OWN WIRING, down to the constructors
/// and `dispatch_legs`, so a doctor cannot report green through a path an
/// event would not use.
fn doctor_mode() -> i32 {
    // ANY EXTRA WORD IS A REFUSAL, before anything is sent or printed. A
    // doctor that quietly ignored an argument is a check the operator believes
    // was narrower or wider than it was.
    if std::env::args_os().nth(2).is_some() {
        eprintln!("{DOCTOR_USAGE}");
        return 2;
    }
    println!("{DOCTOR_OPENING}");

    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    // The same readings `run_event` takes off the same config, before
    // selection consumes it.
    let (hue_table, moshi_token, hermes_key, replay_card, focus_silence) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (
            enabled_hue_table(config),
            plugin_settings(config, "moshi").and_then(moshi_secret),
            plugin_settings(config, "hermes").and_then(hermes_secret),
            config.recap.replay_card,
            config.focus_silence.clone(),
        ),
        // THE SWITCH FALLS BACK ON, which is the fallback `run_event` takes
        // for the same reading. The two must agree or the doctor describes a
        // delivery the event would not make, and the Focus list falls back
        // EMPTY here for the same reason it does there.
        _ => (None, None, None, true, Vec::new()),
    };
    let registry = roster();
    // THE BROKEN-CONFIG FALLBACK IS INHERITED ON PURPOSE. `select_plugins`
    // runs every built-in and warns, and the doctor's job is to say what an
    // event would do, not what a tidier engine would do.
    let (selection, warning) = select_plugins(&registry, loaded);
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }
    let checks = pns::doctor::checks(&registry.all(), &selection);

    let event = pns::args::EventArgs {
        agent: "pns".to_string(),
        state: "doctor".to_string(),
        detail: DOCTOR_DETAIL.to_string(),
        ..Default::default()
    };
    let legs: Vec<pns::routing::Leg> = checks
        .iter()
        .filter(|check| check.kind == pns::doctor::CheckKind::Send)
        .map(|check| pns::routing::Leg {
            name: check.plugin,
            // The operator is standing here waiting for the answer, which is
            // what the reporting mode means and which deadline hermes posts
            // under. It decides nothing about who hears the report: the doctor
            // prints every outcome itself.
            mode: pns::routing::ReportMode::ReportOutcome,
            // NOT A DECORATION, because no plan chose these: the doctor
            // bypasses every gate and sends to whatever is enabled. The flag
            // says a leg is there BECAUSE the operator was to be shown
            // something, and the honest answer here is no.
            decorative: false,
        })
        .collect();
    // NO PANE: its only consumer is the banner's click target, and whether a
    // click focuses the right pane cannot be verified without a human clicking
    // it, so carrying one would add the scrub rule to a second call site to
    // test nothing this can observe.
    let delivered = dispatch_legs(&legs, false, &event, &home, moshi_token, hermes_key);

    let outcomes: Vec<pns::doctor::Outcome> = checks
        .iter()
        .map(|check| match check.kind {
            pns::doctor::CheckKind::Skipped(reason) => pns::doctor::Outcome::Skipped(reason),
            // NOTHING IS DIALLED FOR SETTINGS THAT RESOLVE TO NO BRIDGE.
            // `fire_pulse` answers zero rooms for that config exactly as it
            // does for a bridge that listed none, and the zero-rooms line
            // blames the listing or the room names: both wrong here, and both
            // send the operator hunting through a bridge nothing contacted.
            pns::doctor::CheckKind::Pulse if !hue_resolves(hue_table.as_ref()) => {
                pns::doctor::Outcome::Failed(NO_HUE_BRIDGE_LINE.to_string())
            }
            pns::doctor::CheckKind::Pulse => pulse_outcome(hue_table.clone()),
            // BY NAME, never by position. The legs above are these checks in
            // this order and `dispatch_legs` answers one outcome per leg, so
            // the two agree today; a positional pairing that ever stopped
            // agreeing would print one channel's verdict under another's
            // label, which is a silent misreport rather than a visible one.
            // The absent case cannot happen and still reports a problem rather
            // than claiming a send, which is the direction to be wrong in.
            pns::doctor::CheckKind::Send => {
                match delivered.iter().find(|(leg, _)| leg.name == check.plugin) {
                    Some((_, Delivery::Delivered(said))) => {
                        pns::doctor::Outcome::Sent(said.clone())
                    }
                    Some((_, Delivery::Failed(said) | Delivery::Unlaunched(said))) => {
                        pns::doctor::Outcome::Failed(said.clone())
                    }
                    // Silent BY DESIGN, which is an executable channel that
                    // RAN: it was handed the event and has no second surface
                    // to answer on.
                    Some((_, Delivery::Silent)) => pns::doctor::Outcome::SentUnreported,
                    None => {
                        pns::doctor::Outcome::Failed("the leg was never dispatched".to_string())
                    }
                }
            }
        })
        .collect();

    for (check, outcome) in checks.iter().zip(&outcomes) {
        println!("{}", pns::doctor::line(check, outcome));
    }
    println!("{}", pns::doctor::summary(&outcomes));
    // BETWEEN THE SUMMARY AND THE DECISION SECTION, which is health beside
    // health and history last: this check can move the exit code and the
    // decision log explicitly cannot, so the other order would put a gradeable
    // line below an ungradeable one.
    let pairing = read_pairing();
    for line in pns::doctor::pairing_lines(&pairing) {
        println!("{line}");
    }
    // GATE STATE ABOVE THE HISTORY THE GATE EXPLAINS, and below the pairing
    // check, which is health. It must NOT move the exit code, for the reason
    // the decision section does not: a Focus being on is not a fault.
    println!("{}", focus_line(&home, &focus_silence));
    // APPENDED AFTER THE SUMMARY, which is what lets it be added at all: the
    // census plus its summary is one complete thought whose line order the
    // suite already pins, and nothing below can disturb it.
    for line in decision_section() {
        println!("{line}");
    }
    // HISTORY BELOW HISTORY, and last for the reason the decision section is
    // second to last: an unreplayed journal is not a failure, so it sits under
    // the one section that already cannot move the exit code.
    println!("{}", missed_line(replay_card));
    // THE DECISION SECTION DOES NOT MOVE THE EXIT CODE. It reports HISTORY,
    // not health: an empty log on a fresh machine is not a failure, and
    // neither is one nothing could read. The pairing IS health and does move
    // it, which is why it is an argument rather than a second code combined
    // here: one decision point, decided in one place.
    pns::doctor::exit_code(&outcomes, &pairing)
}

/// What moshi-hook says about this host's pairing, in TWO BOUNDED SPAWNS of
/// one subcommand.
///
/// The split is a correctness argument rather than a style one. `status
/// --json` is local-only, measured at 77ms with the base URL pointed at an
/// unroutable host, and it carries the pairing fact pns grades. Plain `status`
/// is the only shape carrying a server verdict and is the only thing the
/// doctor puts on the network for its own sake. One plain-only call would put
/// the local fact behind the network, so an outage would read as "pairing
/// could not be checked" on a machine that could have answered.
///
/// `probe` IS NEVER CALLED. Measured on 0.3.3, it answers `running: true` and
/// `gateway: true` against a HOME holding no pairing at all while its hostId
/// disappears, so its daemon-side provenance cannot be stated honestly.
///
/// A FORWARD RISK, named rather than coded around: every pairing state exits 0
/// today, so a future moshi that exited non-zero when unpaired would come back
/// as no answer and be reported as "could not check" while the approval path
/// is really dead. A future moshi that renamed or dropped the `server:` line
/// degrades the other way, silently and safely.
///
/// THE WORST CASE IS THE TWO DEADLINES ADDED, not the larger of them: the legs
/// run one after the other, so a moshi-hook wedged on both puts 5s + 8s on a
/// hand-typed command, measured at 13.07 seconds. Ten seconds is not the
/// bound and nobody should treat it as one.
fn read_pairing() -> pns::doctor::PairingReport {
    let binary = moshi_hook_bin();
    let mut json = Command::new(&binary);
    json.args(["status", "--json"]);
    // ONE BYTE PAST THE CHECK'S OWN CAP on both legs, so an answer over that cap
    // still ARRIVES over it: read to the cap exactly and a truncated answer
    // would pass the refusal that exists to catch it.
    let json = run_bounded(json, None, moshi_json_deadline(), OVER_PAIRING_CAP);
    let mut plain = Command::new(&binary);
    plain.arg("status");
    let plain = run_bounded(plain, None, moshi_status_deadline(), OVER_PAIRING_CAP);
    pns::doctor::pairing_report(json.as_deref(), plain.as_deref())
}

/// One byte past what `doctor::pairing_report` will read, which is what keeps
/// "over the cap" and "exactly at the cap" two different answers.
const OVER_PAIRING_CAP: u64 = pns::doctor::ANSWER_MAX as u64 + 1;

/// How long `moshi-hook status --json` may take.
///
/// GENEROUS AGAINST A MEASURED 77ms, and pinned here rather than inherited
/// from the probe runner's shared window: this leg reaches no network today,
/// and "today" is exactly why the bound has to be this function's own to state
/// and a test's own to move.
fn moshi_json_deadline() -> Duration {
    env_deadline("PNS_MOSHI_JSON_DEADLINE_MS").unwrap_or(MOSHI_JSON_DEADLINE)
}

const MOSHI_JSON_DEADLINE: Duration = Duration::from_secs(5);

/// How long plain `moshi-hook status` may take.
///
/// IT MUST EXCEED MOSHI'S OWN internal timeout, measured at about 5.1 seconds
/// against an unroutable base URL. Killing it mid-wait would throw away the
/// very `unavailable (...)` sentence that explains the delay, which is the one
/// thing this call is for.
fn moshi_status_deadline() -> Duration {
    env_deadline("PNS_MOSHI_STATUS_DEADLINE_MS").unwrap_or(MOSHI_STATUS_DEADLINE)
}

const MOSHI_STATUS_DEADLINE: Duration = Duration::from_secs(8);

/// The decision ring, read back and rendered.
///
/// READ AND NEVER APPENDED. A doctor that recorded would push the decision the
/// operator came to read out of the ring by the act of going to look at it.
fn decision_section() -> Vec<String> {
    let now = now_secs();
    match readable_ring(&state_dir().join(DECISIONS), RING_READ_MAX) {
        Ok(contents) => pns::decision_log::section(Some(&contents), now),
        // ABSENT IS ITS OWN STATE, and the one the section has an honest line
        // for. Anything else is a directory or a permission problem, which is
        // a different thing to say.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            pns::decision_log::section(None, now)
        }
        Err(error) => vec![format!("{DECISIONS_UNREADABLE} ({}).", error.kind())],
    }
}

/// A ring that is there and cannot be read. Said HERE rather than in the log
/// module, for the reason `NO_HUE_BRIDGE_LINE` is: the sentence needs
/// something only the reader of the file knows.
const DECISIONS_UNREADABLE: &str = "pns doctor: the decision log could not be read";

/// The missed-notification journal, COUNTED and never rendered.
///
/// READ AND NEVER APPENDED, for the reason the decision section is: a doctor
/// that journaled would file a miss for the act of going to look for one, and
/// its own test send is the last event anything should ever replay.
///
/// NOTHING HERE PARSES AN ENTRY. The contents go straight to `waiting_line`,
/// which counts lines and has no parse at all, so the operator's own text has
/// no path from this file to a terminal.
///
/// `replay_card` REACHES THE SENTENCE because the sentence makes a promise.
/// With the card switched off nothing will ever deliver what is counted here,
/// and a doctor that still named "the next event" would be telling the
/// operator a lie their own setting makes permanent.
fn missed_line(replay_card: bool) -> String {
    match readable_ring(&state_dir().join(MISSED_NOTIFICATIONS), RING_READ_MAX) {
        Ok(contents) => pns::missed_notifications::waiting_line(Some(&contents), replay_card),
        // ABSENT IS ITS OWN STATE, and the one the line has an honest sentence
        // for. Anything else is a directory or a permission problem, which is
        // a different thing to say.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            pns::missed_notifications::waiting_line(None, replay_card)
        }
        Err(error) => format!("{MISSED_UNREADABLE} ({}).", error.kind()),
    }
}

/// A journal that is there and cannot be read. Said HERE rather than in the
/// module, for the reason `DECISIONS_UNREADABLE` is.
const MISSED_UNREADABLE: &str = "pns doctor: the missed-notification journal could not be read";

/// What Focus is doing to this machine right now, in one sentence.
///
/// THE UNREADABLE STATE IS WHAT EARNS THE LINE. If the store is ever gated
/// behind Full Disk Access, moves, or changes schema, this feature dies OPEN
/// and SILENT: pns simply stops respecting Focus, and nothing else anywhere
/// would ever say so.
///
/// THE ACCEPTED LIMIT, stated rather than designed around: the parser is
/// TOTAL, so bytes that are not JSON at all, and a schema change that leaves
/// the file valid JSON, both read as "no Focus" rather than as an error. Only
/// a failed READ of the file itself reaches the last two sentences, and a
/// store that had stopped being readable in any useful sense would still be
/// reported as quiet. Telling those apart needs a positive assertion about a
/// shape Apple promises nothing about.
///
/// FIVE SENTENCES, because ABSENT AND UNREADABLE ARE DIFFERENT THINGS TO SAY,
/// which is the rule `decision_section` and `missed_line` already follow one
/// screen up. A machine that has never asserted a Focus has no store, and
/// telling that operator their database could not be read sends them after a
/// Full Disk Access grant that was never the problem.
fn focus_line(home: &str, silence: &[String]) -> String {
    if silence.is_empty() {
        return "pns doctor: focus awareness is off (no [focus] table names a mode to silence)"
            .to_string();
    }
    match focus_now(home, silence) {
        Ok(reading) => {
            let state = if reading.silenced {
                "pns doctor: a macOS Focus you named is ON, so banners, cards and pulses \
                 are suppressed"
            } else {
                "pns doctor: no macOS Focus you named is active"
            };
            // A CATALOG NOBODY CAN READ RESOLVES NO NAMES, so a config written
            // the way the template shows it silences nothing while this line
            // otherwise reports perfect health. WHICH entries are names is not
            // decidable without the very file that failed, so the clause is
            // said whenever the catalog failed and the feature is on.
            match reading.catalog {
                None => state.to_string(),
                Some(kind) => format!(
                    "{state}; the mode catalog could not be read ({kind}), so no Focus NAME \
                     can match and only a raw modeIdentifier still would"
                ),
            }
        }
        // ABSENT IS ITS OWN STATE, and the one this machine is in until macOS
        // first writes the store. Anything else is a permission problem, a
        // path holding something that is not a file, or a store past the read
        // ceiling, which is a different thing to say.
        //
        // IT REPORTS WHAT WAS OBSERVED, "no database was found", rather than
        // asserting there is none. Whether a Full Disk Access refusal can
        // arrive as not-found rather than as a permission error is not
        // provable on a machine that holds the grant, so the sentence is
        // written to stay true either way.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "pns doctor: no Focus database was found on this machine, so no Focus is being \
             respected"
                .to_string()
        }
        Err(error) => format!("{FOCUS_UNREADABLE} ({}).", error.kind()),
    }
}

/// A Focus store that is there and cannot be read. Said HERE rather than in
/// the module, for the reason `DECISIONS_UNREADABLE` is, and carrying the KIND
/// for the reason its two neighbours do: gated, oversized and not-a-file are
/// three different investigations.
const FOCUS_UNREADABLE: &str =
    "pns doctor: the Focus database could not be read, so Focus is being ignored";

/// What a doctor typed wrong is told. ONE WORD AND NO FLAGS: a namespace built
/// for callers that do not exist makes the common case longer to type, and the
/// report absorbs a new section without a new spelling.
const DOCTOR_USAGE: &str = "pns: usage: pns doctor";

/// The contract, STATED rather than measured. Whether a gate is currently in
/// effect is the decision log's question, and reporting live gate state here
/// would be that feature built twice, in two places, from two readings.
const DOCTOR_OPENING: &str = "pns doctor: sending one test to every enabled channel. \
     Every suppression gate is bypassed (the operator mute, a macOS Focus you named, \
     the presence gate, the viewed-pane rule, the lights' quiet hours), because a check \
     that can be suppressed proves nothing.";

/// The line for lights that were selected and never set up. It names the
/// settings to write, the way moshi's and hermes's do, because "no rooms"
/// without an address sends the operator to a bridge nothing dialled.
const NO_HUE_BRIDGE_LINE: &str = "pulse SKIPPED -- no hue bridge and key in the config \
     ([plugins.hue] bridge, key); nothing was signalled";

/// The payload's detail, so whoever the card wakes knows at once that nothing
/// is wrong and nothing needs doing.
const DOCTOR_DETAIL: &str = "test send from pns doctor; nothing is wrong and nothing needs doing";

/// The `recap` mode: one window of activity, rendered and posted, in a process
/// nobody is waiting on.
///
/// IT TAKES NO DECISION, which is what makes it a mode. The decision was taken
/// by the event that spawned it, and re-deciding here would be the second
/// reading of one moment `GateInputs` exists to forbid.
///
/// IT REACHES ONE DESTINATION, the durable route, and never the phone or the
/// banner. The phone layer was already delivered by the card that pointed here.
///
/// EXIT 2 FOR A MISTYPED INVOCATION, in `quiet_mode`'s style rather than the
/// hook path's always-zero: this is hand-runnable, and a subcommand that
/// swallows a typo is a recap the operator believes was posted. The spawner
/// never reads the code.
fn recap_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let Some((since, until)) = recap_bounds(&arguments) else {
        eprintln!("{RECAP_USAGE}");
        return 2;
    };
    let home = std::env::var("HOME").unwrap_or_default();
    // FAIL CLOSED ON THE ROUTE AND ON THE SUMMARIZER, AND OPEN ON THE POST,
    // which is `pulse_mode`'s split: a config nobody can read named no route
    // and no command, so the recap goes to the default route, plainly, rather
    // than to a route the operator never asked for or through a program they
    // never named.
    let (hermes_key, recap) = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => (
            plugin_settings(&config, "hermes").and_then(hermes_secret),
            config.recap,
        ),
        _ => (
            None,
            pns::config::Recap {
                digest_as_thread: false,
                ..Default::default()
            },
        ),
    };
    let entries = activity_in(since, until);
    // THE TWO EXTERNAL SOURCES ARE READ ONLY WHEN A KEY NAMES THEM, and both
    // are read HERE, in the process nobody is waiting on. A repository listing
    // is a network call somebody else's machine answers and a glob is a
    // directory read; neither belongs anywhere near the card, and neither is
    // allowed to cost the rest of the recap anything when it does not come
    // back.
    let fetched_merges =
        (!recap.repos.is_empty()).then(|| merged_pull_requests(&recap.repos, since, until));
    let fetched_notes = recap
        .review_notes
        .as_deref()
        .map(|pattern| notes_matching(pattern, &home, since, until));
    // ONE EPISODE, ONE BUDGET. The locked "the LLM runs once at the return
    // moment" is a moment rather than a call: this recap asks up to three
    // questions (the night, the merges, the notes) and `summarizer_deadline_secs`
    // is what the WHOLE episode may spend, so each call is bounded by what is
    // left of it. Per-call deadlines meant a 240-second key could hold two
    // processes for twelve minutes while the card had already said the recap
    // was in #pns, and a laptop that sleeps inside that window loses the recap
    // entirely. Adjudicated 2026-08-29.
    let episode = std::time::Instant::now() + Duration::from_secs(recap.summarizer_deadline_secs);
    // THE ANSWER IS TAKEN BEFORE THE BODY IS COMPOSED and nothing else waits on
    // it: this process was started so that a model could be slow somewhere
    // nobody is standing.
    // AND NOT OVER AN EMPTY WINDOW. A night with nothing in it has nothing to
    // select from, and the model would be handed "nothing was recorded in this
    // window" under an instruction to rewrite it as a timeline. That is a
    // process spawned to summarize nothing and an invitation to invent, on the
    // one path an operator reaches by hand.
    let answered = recap
        .summarizer
        .as_deref()
        .filter(|_| !entries.is_empty())
        .map(|argv| {
            summarize(
                argv,
                left_of(episode),
                &pns::recap::prompt(&entries, &|at| wall_clock(at)),
            )
        });
    let timeline = match &answered {
        None => pns::recap::Timeline::Mechanical,
        Some(None) => pns::recap::Timeline::Unanswered,
        Some(Some(lines)) => pns::recap::Timeline::Summarized(lines),
    };
    // ONE SUMMARIZER CALL PER SECTION, and each falls back on its own. They are
    // three different questions over three different sets of text, so one call
    // answering all three would need the backend to keep them apart in its
    // answer, and a section would then be lost to a separator a model got wrong
    // rather than to anything pns could see. THEY SHARE ONE DEADLINE, above: a
    // call reached with the episode's budget already spent is never started.
    let merge_lines = read_sources(&fetched_merges)
        .and_then(|sources| summarized(&recap, episode, sources, pns::recap::merge_prompt));
    let note_lines = read_sources(&fetched_notes)
        .and_then(|sources| summarized(&recap, episode, sources, pns::recap::note_prompt));
    let externals = pns::recap::Externals {
        merges: pns::recap::External {
            found: found(&fetched_merges),
            answered: merge_lines.as_deref(),
            truncated: truncated(&fetched_merges),
        },
        notes: pns::recap::External {
            found: found(&fetched_notes),
            answered: note_lines.as_deref(),
            truncated: truncated(&fetched_notes),
        },
    };
    let body = pns::recap::body(
        &entries,
        &wall_clock(Some(since)),
        &wall_clock(Some(until)),
        &|at| wall_clock(at),
        timeline,
        &externals,
    );
    post_recap(&body, recap.digest_as_thread, &home, hermes_key)
}

/// What one external source held, and whether a cap stopped the read short of
/// everything there was.
///
/// TRUNCATION TRAVELS WITH THE SOURCES rather than being recomputed from their
/// length, because the two caps are different facts: a listing that came back at
/// exactly `GH_LIMIT` may have more behind it, and a glob matching more files
/// than `MAX_NOTES` certainly does. Only the fetch knows which, and the message
/// says "at least" on either.
struct Fetched {
    sources: Vec<pns::recap::Sourced>,
    truncated: bool,
}

/// One external source's three states, said in the type the body reads.
///
/// THE OUTER `Option` IS THE KEY AND THE INNER ONE IS THE READ, which is what
/// keeps "nobody configured this" and "this would not answer" apart all the way
/// from the config to the message. An empty `Vec` is neither: it is a source
/// that answered with nothing.
fn found(fetched: &Option<Option<Fetched>>) -> pns::recap::Found<'_> {
    match fetched {
        None => pns::recap::Found::Unconfigured,
        Some(None) => pns::recap::Found::Unavailable,
        Some(Some(fetched)) => pns::recap::Found::Read(&fetched.sources),
    }
}

/// Whether what `found` holds is a floor. A source nobody configured and one
/// that would not answer are neither: there is no count to qualify.
fn truncated(fetched: &Option<Option<Fetched>>) -> bool {
    matches!(fetched, Some(Some(fetched)) if fetched.truncated)
}

/// What a source actually held, for the two callers that only have something to
/// do when it held anything.
fn read_sources(fetched: &Option<Option<Fetched>>) -> Option<&[pns::recap::Sourced]> {
    Some(fetched.as_ref()?.as_ref()?.sources.as_slice()).filter(|sources| !sources.is_empty())
}

/// What the summarizer said about one external section, or None for every way
/// of not having an answer.
///
/// NOT OVER AN EMPTY SOURCE, which is `recap_mode`'s own rule about an empty
/// window applied a second time: a model handed nothing to select from is a
/// process spawned to summarize nothing and an invitation to invent.
fn summarized(
    recap: &pns::config::Recap,
    episode: std::time::Instant,
    sources: &[pns::recap::Sourced],
    prompt: fn(&[pns::recap::Sourced]) -> String,
) -> Option<Vec<String>> {
    summarize(
        recap.summarizer.as_deref()?,
        left_of(episode),
        &prompt(sources),
    )
}

/// What is left of the episode's one budget. Zero once it is spent, which
/// `summarize` reads as a call not worth starting.
fn left_of(episode: std::time::Instant) -> Duration {
    episode.saturating_duration_since(std::time::Instant::now())
}

/// The pull requests merged into the named repositories inside the window, or
/// None when the listing could not be had.
///
/// `gh` CARRIES ITS OWN AUTH AND THIS NEVER TOUCHES IT. No token is read, no
/// credential is passed and no network call is made by pns itself: the one
/// spawn is a LIST, and the whole feature is read-only by construction, which
/// is what bounds a pull request body being somebody else's text.
///
/// RESOLVED THROUGH PATH, like `herdr` and unlike the system binaries: it is
/// installed wherever this machine's package manager put it, and a context
/// whose PATH does not carry it reads as unavailable, which costs this section
/// and nothing else.
///
/// BOUNDED THREE WAYS, because every one of them is a way for a remote answer
/// to become this machine's problem: the window is stated in the search so the
/// service does the selecting, `--limit` caps how many come back, and the read
/// is capped in time and in bytes by the seam. A truncated listing is not JSON,
/// so the cap fails CLOSED into "unavailable" rather than into a half-read
/// section.
///
/// ANY REPOSITORY FAILING FAILS THE SECTION, deliberately. A partial list under
/// a count is a count that lies, and this section's remainder line is counted
/// against what was read.
///
/// THE WINDOW IS THE RECAP'S OWN, SHIFTED ONE SECOND. GitHub's range syntax is
/// inclusive at both ends and `activity_in`'s window is `(since, until]`, so a
/// pull request merged in the marker's own second would be fetched while every
/// event in that second is excluded. Starting the search a second later is the
/// same bracket the rest of the recap uses. ACCEPTED LIMIT: the search's
/// granularity is one second, so this is exact rather than approximate only
/// because both bounds are whole seconds to begin with.
///
/// ACCEPTED LIMIT: THE SEARCH INDEX TRAILS THE MERGE, by seconds to minutes. A
/// pull request merged shortly before the return moment can be absent from this
/// listing with no signal, and the next window opens after it, so it is never
/// reported at all. Stating the window server-side is still right (the
/// alternative is fetching everything and selecting here), and the tail pointer
/// to the repository is what closes it.
///
/// ACCEPTED LIMIT: the receipt is the pull request NUMBER, so two repositories
/// merging the same number inside one window produce two lines that cite it.
/// Both are real merges the operator can follow; the alternative is a receipt
/// carrying a repository name, which costs every line its width for a case one
/// configured repository never reaches.
fn merged_pull_requests(repos: &[String], since: u64, until: u64) -> Option<Fetched> {
    let window = format!(
        "merged:{}..{}",
        pns::system::utc_timestamp(since.checked_add(1)?)?,
        pns::system::utc_timestamp(until)?
    );
    let mut merged = Vec::new();
    let mut truncated = false;
    for repo in repos {
        let mut command = Command::new(GH);
        command.args([
            "pr",
            "list",
            "--repo",
            repo,
            "--state",
            "merged",
            "--search",
            &window,
            "--json",
            "number,title,body",
            "--limit",
            &GH_LIMIT.to_string(),
        ]);
        let listing = run_bounded(command, None, GH_DEADLINE, GH_READ_MAX)?;
        let entries = serde_json::from_str::<Vec<serde_json::Value>>(&listing).ok()?;
        // A LISTING THAT CAME BACK AT ITS OWN LIMIT MAY HAVE MORE BEHIND IT,
        // and nothing here can tell a repository with exactly fifty merges from
        // one with five hundred. The count the section prints then says "at
        // least", which is the honest reading of a cap.
        truncated |= entries.len() >= GH_LIMIT;
        for entry in entries {
            // THE NUMBER IS REQUIRED AND ITS ABSENCE FAILS THE WHOLE READ: it
            // is the receipt, so an entry without one is a line nobody could
            // follow, and an answer shaped like that is not the listing that
            // was asked for.
            let number = entry.get("number").and_then(serde_json::Value::as_u64)?;
            merged.push(pns::recap::merged(
                number,
                field(&entry, "title"),
                field(&entry, "body"),
            ));
        }
    }
    Some(Fetched {
        sources: merged,
        truncated,
    })
}

/// One string off a listing entry, or empty. A short entry degrades to a
/// thinner line, which is `missed_notifications::entries`'s own rule.
fn field<'entry>(entry: &'entry serde_json::Value, key: &str) -> &'entry str {
    entry
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
}

/// The review notes the glob names whose own clock falls inside the window, or
/// None when the directory the operator named could not be read at all.
///
/// THE GLOB IS THE WHOLE PERMISSION and this is where that is spent: one
/// directory, named in full by the operator, listed once. Nothing recurses,
/// nothing follows a name the pattern did not match, and the config layer has
/// already refused a pattern whose DIRECTORY carries a `*`, so the set of
/// directories pns opens is a statement rather than a search.
///
/// THE WINDOW IS `activity_in`'S OWN PREDICATE, so a note is in this recap for
/// the same reason an event is: it happened after the operator was last here.
/// A note they had already read when they left is not news.
///
/// EVERY READ IS BOUNDED and every file is a file: a directory or a device
/// entry matching the pattern is skipped rather than opened, and what is read
/// stops at a ceiling, because this is an ordinary directory other tools also
/// write into.
///
/// NEWEST FIRST, WHICH IS WHAT THE CAP THEN CUTS. Sorting by name and taking
/// the first `MAX_NOTES` kept whatever sorted earliest, so `checklist-a*.md`
/// outranked the note written an hour ago, which is the opposite of what a
/// section about the night wants. The name breaks a tie, so one window still
/// renders the same way twice.
///
/// ACCEPTED LIMIT: past `MAX_NOTES` the count is a FLOOR rather than a total,
/// which is the honesty `header` states about a pruned ring. The MESSAGE says
/// so now: `Fetched::truncated` is what turns the section's remainder into "at
/// least", so a glob matching forty notes cannot print a count that reads as a
/// total.
fn notes_matching(pattern: &str, home: &str, since: u64, until: u64) -> Option<Fetched> {
    let expanded = match pattern.strip_prefix("~/") {
        Some(rest) => Path::new(home).join(rest),
        None => std::path::PathBuf::from(pattern),
    };
    let name = expanded.file_name()?.to_str()?.to_string();
    let mut matched: Vec<(Duration, std::path::PathBuf)> = std::fs::read_dir(expanded.parent()?)
        .ok()?
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|found| matches_glob(found, &name))
        })
        .filter_map(|path| Some((modified_at(&std::fs::metadata(&path).ok()?)?, path)))
        .filter(|(at, _)| within(*at, since, until))
        .collect();
    matched.sort_by(|(left, left_path), (right, right_path)| {
        right.cmp(left).then_with(|| left_path.cmp(right_path))
    });
    Some(Fetched {
        truncated: matched.len() > MAX_NOTES,
        sources: matched
            .iter()
            .take(MAX_NOTES)
            .filter_map(|(_, path)| {
                let named = path.file_name()?.to_str()?;
                // A NOTE THAT WOULD NOT OPEN IS STILL A NOTE. It matched the
                // operator's own pattern and its clock puts it in the window,
                // so dropping it renders a night in which that finding never
                // existed; the mode, the race or the swap that stopped the read
                // is exactly what they would want to see.
                Some(match read_note(path, since, until) {
                    Some(contents) => pns::recap::noted(named, &contents),
                    None => pns::recap::unreadable(named),
                })
            })
            .collect(),
    })
}

/// Whether one name matches a pattern holding at most one `*`, which is the
/// only glob the config layer admits. Everything else is a literal, so a
/// pattern names one file.
fn matches_glob(name: &str, pattern: &str) -> bool {
    match pattern.split_once('*') {
        None => name == pattern,
        Some((head, tail)) => {
            name.len() >= head.len() + tail.len() && name.starts_with(head) && name.ends_with(tail)
        }
    }
}

/// One file's own clock, or None when it has none this can read.
fn modified_at(metadata: &std::fs::Metadata) -> Option<Duration> {
    metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()
}

/// Whether a clock puts a file inside the window, on `activity_in`'s half-open
/// rule and AT FULL PRECISION. Truncating to whole seconds excluded a file
/// written half a second after the marker and admitted one written half a
/// second after the window closed, which is the one edge each rule exists to
/// place.
fn within(at: Duration, since: u64, until: u64) -> bool {
    at > Duration::from_secs(since) && at <= Duration::from_secs(until)
}

/// One matched note read up to a ceiling, through a handle that is CHECKED
/// AFTER IT IS OPEN.
///
/// OPEN THEN VERIFY, because the scan and the read are two moments and a
/// directory other tools write into can change between them. The open refuses
/// to follow a link at all (`O_NOFOLLOW`), so a symlink dropped at a name the
/// glob matched cannot widen the read past the one directory the pattern names;
/// and the file type and the clock are re-read off the HANDLE, so a file
/// rewritten after the scan cannot feed this window contents from outside it.
/// Checking the path a second time instead would be the same race with more
/// steps: the answer would still describe whatever the name pointed at then.
///
/// LOSSY, for `run_bounded`'s reason: this is a plain file other tools write,
/// and one invalid byte must cost its own character rather than the note.
fn read_note(path: &Path, since: u64, until: u64) -> Option<String> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || !within(modified_at(&metadata)?, since, until) {
        return None;
    }
    let mut text = Vec::new();
    Read::take(&mut file, NOTE_READ_MAX)
        .read_to_end(&mut text)
        .ok()?;
    Some(String::from_utf8_lossy(&text).into_owned())
}

/// The listing tool, resolved through PATH. See `merged_pull_requests`.
const GH: &str = "gh";

/// How many merged pull requests one repository may contribute.
///
/// FIFTY, which is far past what an absence produces (ten in a ten-hour stretch
/// on this machine, MEASURED) and still a bound on somebody else's answer.
const GH_LIMIT: usize = 50;

/// How long the listing may take. THIRTY SECONDS, which is thirty times the
/// second the same call MEASURED today and short of anything a person would
/// call working. Nobody is waiting on it, so this exists to stop a wedged
/// network call holding the whole recap rather than to hurry a slow one.
const GH_DEADLINE: Duration = Duration::from_secs(30);

/// How much of the listing is read.
///
/// MEASURED AGAINST THE REAL WORKLOAD rather than against a service limit: the
/// last fifty merged pull requests of this repository come back as 187,965
/// bytes of JSON with a longest body of 9,672 characters, so the ordinary case
/// spends 37% of this. A pull request body may be far larger than that, and
/// enough of them at once take the section out for one window: past the cap the
/// JSON is truncated and fails to parse, which is the fail-closed direction
/// (half a listing is not a listing) but reads as "unavailable" with no hint
/// that size was the reason.
const GH_READ_MAX: u64 = 512 * 1024;

/// How many review notes one recap considers, and how much of each it reads.
const MAX_NOTES: usize = 25;

const NOTE_READ_MAX: u64 = 64 * 1024;

/// The configured command, handed the window on stdin, and what it said back
/// as timeline lines. None for every way of not answering.
///
/// ARGV STRAIGHT TO `Command`, NEVER THROUGH A SHELL, which is what makes the
/// key safe to hold anything: the words are the words, so there is no quoting
/// rule to get wrong and nothing in the window can be read as syntax.
///
/// THE SEAM IS THE ONE THE PROBES ALREADY USE. `run_bounded` writes the prompt
/// inside the deadline window, reads stdout lossily, kills the child when the
/// window closes and answers None on a non-zero exit, which is every rung of
/// this ladder but the last; `recap::answer` owns that one.
///
/// A BACKEND THAT IS NOT INSTALLED IS NOT A SPECIAL CASE. The spawn fails, the
/// seam answers None, and the recap posts the plain list saying so, which is
/// the same thing the operator sees when the model is simply slow.
///
/// AND NEITHER IS A SPENT BUDGET. An episode whose deadline is gone starts no
/// process at all: spawning one only to kill it on a zero-length window is a
/// model load nobody reads, and the plain list is already the answer.
fn summarize(argv: &[String], deadline: Duration, prompt: &str) -> Option<Vec<String>> {
    if deadline.is_zero() {
        return None;
    }
    let (program, arguments) = argv.split_first()?;
    let mut command = Command::new(program);
    command.args(arguments);
    pns::recap::answer(&run_bounded(
        command,
        Some(prompt),
        deadline,
        pns::recap::MAX_ANSWER_BYTES as u64 + 1,
    )?)
}

/// The window bounds off argv, or None for anything this will not vouch for.
///
/// EVERY UNKNOWN WORD IS A REFUSAL, never a silent default: a recap over a
/// window nobody asked for is worse than none. Both bounds are required, both
/// are plain counts through the crate's one numeric gate, and a window that
/// runs backwards is refused rather than read as empty.
fn recap_bounds(arguments: &[String]) -> Option<(u64, u64)> {
    let mut since = None;
    let mut until = None;
    let mut tokens = arguments.iter();
    while let Some(token) = tokens.next() {
        let bound = match token.as_str() {
            "--since" => &mut since,
            "--until" => &mut until,
            _ => return None,
        };
        // A REPEATED FLAG IS A REFUSAL TOO: two windows were asked for and only
        // one can be answered.
        if bound.is_some() {
            return None;
        }
        *bound = Some(pns::parse_count(tokens.next()?)?);
    }
    match (since, until) {
        (Some(since), Some(until)) if since <= until => Some((since, until)),
        _ => None,
    }
}

/// One epoch as the operator's own wall clock reads it, or a placeholder of the
/// same width when there is no readable time. ONE FUNCTION for the header's two
/// bounds and every timeline line, so the recap cannot render two clocks.
fn wall_clock(epoch: Option<u64>) -> String {
    epoch
        .and_then(local_minutes_since_midnight)
        .map(|minutes| format!("{:02}:{:02}", minutes / 60, minutes % 60))
        .unwrap_or_else(|| NO_WALL_CLOCK.to_string())
}

/// The recap posted, with the one fallback the locked spec names.
///
/// SYNCHRONOUS INSIDE THIS PROCESS, and REPORTING, which is the mode whose
/// whole purpose is that a failure is visible. Nobody is behind this, and a
/// silently dropped recap is the exact failure the feature exists to prevent.
///
/// THE FALLBACK IS A REAL MECHANISM. hermes answers 404 for a route it does not
/// know and 502 when the target rejects the delivery, and only a 2xx is
/// `delivered`, so a thread route the operator has not prepared refuses loudly.
/// The same body then goes to the default route with ONE line saying why it
/// landed there, which is the locked "falls back to a plain #pns message".
///
/// A VERDICT, NEVER A SENTENCE. The retry fires on `Failed` and `Unlaunched`
/// alone; `Silent` is an executable channel that RAN and has no second surface
/// to answer on, and reading it as a failure would post every recap twice on
/// every machine with a shell channel installed.
///
/// ACCEPTED LIMIT, AND IT IS THE SAME RULE'S OTHER SIDE: on a machine running
/// EXECUTABLE channels (`PNS_CHANNELS_DIR` set), `deliver` always answers
/// `Silent` for a channel that ran, whatever the gateway then said. So a 404
/// from an unprepared `pns-recap` route is invisible there and this fallback
/// never fires; the recap goes to the thread route and stays there. Closing it
/// would mean an executable channel reporting a per-destination outcome, which
/// is a change to the channel contract itself and not to a recap.
///
/// ONE FALLBACK AND NO LOOP. A default route that refuses too is a gateway
/// problem, and a recap is not worth a retry storm against one.
///
/// ACCEPTED LIMIT ON THE CHARACTER CEILING: the fallback line is appended to a
/// body `recap::fit` has already fitted, so the second post may exceed
/// `recap::MAX_CHARS` by that one line. Fitting it in would mean composing the
/// body twice, once per route, on a path taken only when the first route
/// refused. The ceiling has 100 characters of headroom under the gateway's own
/// split threshold and this line is 82 characters plus its newline, so the post
/// still lands as one message.
fn post_recap(body: &str, thread: bool, home: &str, hermes_key: Option<String>) -> i32 {
    if !thread {
        deliver_recap(body, "", home, hermes_key);
        return 0;
    }
    if !refused(&deliver_recap(body, RECAP_ROUTE, home, hermes_key.clone())) {
        return 0;
    }
    deliver_recap(
        &format!("{body}\n{THREAD_UNAVAILABLE}"),
        "",
        home,
        hermes_key,
    );
    0
}

/// One recap posted to one route, and what the route had to say about it.
///
/// IT SAYS WHAT HAPPENED, which is what `ReportMode::ReportOutcome` was for
/// and what it never actually did: `dispatch_legs` RETURNS its outcomes and
/// prints nothing, so the mode only ever moved the deadline. MEASURED against
/// a dead endpoint, `pns recap --since ... --until ...` printed nothing and
/// exited 0, which is exactly the drill an operator runs by hand to check a
/// `pns-recap` route they have just prepared, against exactly the failure it
/// is most likely to meet.
///
/// THE SAME LINE `run_event` PRINTS, prefix and all, because a second spelling
/// of one report is a second thing to keep in step. The detached child's
/// stdout is `/dev/null`, so this costs the event path nothing.
fn deliver_recap(
    body: &str,
    channel: &str,
    home: &str,
    hermes_key: Option<String>,
) -> Vec<(pns::routing::Leg, Delivery)> {
    // ONE LEG AND ONE DESTINATION, built by hand the way `doctor_mode` builds
    // its own: no decision was taken here, so there is no plan to derive legs
    // from. NOT DECORATIVE, because nothing about this was chosen to put
    // something in front of the operator; the card already did that.
    let leg = pns::routing::Leg {
        name: "hermes",
        mode: pns::routing::ReportMode::ReportOutcome,
        decorative: false,
    };
    let event = pns::args::EventArgs {
        agent: "pns".to_string(),
        state: "recap".to_string(),
        detail: body.to_string(),
        channel: channel.to_string(),
        ..Default::default()
    };
    let outcomes = dispatch_legs(&[leg], false, &event, home, None, hermes_key);
    for (leg, delivered) in &outcomes {
        if let Some(line) = delivered.clone().line_for(leg.mode) {
            println!("pns: {line}");
        }
    }
    outcomes
}

/// Whether a dispatch refused the recap, which is the only thing that earns
/// the fallback. See `post_recap`.
fn refused(outcomes: &[(pns::routing::Leg, Delivery)]) -> bool {
    outcomes
        .iter()
        .any(|(_, delivered)| matches!(delivered, Delivery::Failed(_) | Delivery::Unlaunched(_)))
}

/// The hermes route a threaded recap posts to. ONE CONST rather than a key: a
/// second machine wanting another name can have the key the day it exists, and
/// the operator prepares this route in hermes either way.
const RECAP_ROUTE: &str = "pns-recap";

/// The line the fallback adds, so a recap in the wrong place says why it is
/// there rather than looking like the design.
const THREAD_UNAVAILABLE: &str =
    "(the pns-recap route did not take this, so it landed on the default route instead)";

/// What a recap typed wrong is told.
const RECAP_USAGE: &str = "pns: usage: pns recap --since <epoch> --until <epoch>";

/// What a line shows for a moment whose clock could not be read: the same width
/// as a time, so the timeline still lines up.
const NO_WALL_CLOCK: &str = "--:--";

/// The `quiet` mode: the operator's own mute, typed and timed.
///
/// THE ONLY NON-ZERO EXITS HERE THAT ARE NOT AN OPERATOR'S APPROVAL DECISION,
/// and they are correct. The always-exit-0 contract covers the hook and
/// notification paths, where a non-zero exit would fail the turn being
/// reported on; this is hand typed, is never a hook, and a subcommand that
/// silently swallows a typo is a mute the operator believes is on.
///
/// THE REPORT IS READ BACK OFF THE FILE after whatever was asked for, rather
/// than rendered from what this run intended, so the line cannot claim a mute
/// that never landed. A FAILED SET REPORTS TOO, for the mirror of the same
/// reason: it knows only that its own write did not happen, and a previous
/// mute may still be standing behind it.
fn quiet_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let quiet_until = state_dir().join(QUIET_UNTIL);
    // A SET THAT DID NOT HAPPEN, carried to the exit code rather than
    // returned on the spot, so the report below runs on this path too.
    let mut set_failed = false;
    match arguments.as_slice() {
        // NO ARGUMENT REPORTS and mutes nothing. There is no untimed toggle:
        // an indefinite mute the operator forgets is a notification system
        // that has silently stopped working, and making this form the report
        // also means no invocation can mute by accident.
        [] => {}
        // Unlinking is also how a file nothing can parse is cleared, which is
        // the remedy the corrupt-state complaint names.
        [word] if word == "off" => {
            let _ = std::fs::remove_file(&quiet_until);
        }
        [duration] => match pns::quiet::parse_duration(duration) {
            Ok(seconds) => {
                // NEITHER ARM CLAIMS "nothing is muted". A run that could not
                // read a clock or could not write cannot see the state it is
                // making a claim about, and a mute set an hour ago can be
                // standing behind both: measured, the write arm said nothing
                // was muted while `pns quiet` a second later reported sixty
                // minutes left. They say what did not happen, and the report
                // below says what stands.
                match now_secs().map(|now| now.saturating_add(seconds)) {
                    None => {
                        eprintln!(
                            "pns: state error (the clock cannot be read); the mute was not set"
                        );
                        set_failed = true;
                    }
                    // LOUD, unlike `remember_staleness`: that one is a
                    // background warning that must never crash a diagnostic,
                    // and this is a human waiting on an answer. Reporting
                    // success for a mute that is not in effect is the worst
                    // outcome available.
                    Some(expiry) => {
                        if let Err(error) = publish_state_line(&quiet_until, &expiry.to_string()) {
                            eprintln!(
                                "pns: state error (quiet-until could not be written: {error}); \
                                 the mute was not set"
                            );
                            set_failed = true;
                        }
                    }
                }
            }
            Err(refusal) => {
                eprintln!("{refusal}");
                eprintln!("{QUIET_USAGE}");
                return 2;
            }
        },
        // ANY EXTRA WORD IS A REFUSAL, never a silent fallthrough to the
        // report: a typo an operator does not see is a mute they believe is
        // on.
        _ => {
            eprintln!("{QUIET_USAGE}");
            return 2;
        }
    }
    println!(
        "{}",
        pns::quiet::status_line(read_quiet_expiry(), now_secs())
    );
    if set_failed { 1 } else { 0 }
}

/// What a mute typed wrong is told, once, on stderr. The refusal above it
/// quotes what was typed; this says what the command takes.
const QUIET_USAGE: &str =
    "pns: usage: pns quiet [<duration>|off]; duration is <count><s|m|h>, from 1s to 24h";

/// One line, holding the epoch second the operator's mute ends. ABSENT is the
/// ordinary state and the file is never created to say "not muted": every
/// reader compares the expiry with its own clock, so a file left behind after
/// the window is already inert.
const QUIET_UNTIL: &str = "quiet-until";

/// The mute's expiry, if the operator set one.
///
/// A FILE NOTHING CAN READ OR PARSE COMPLAINS AND READS AS NOT MUTED, which
/// is the OPPOSITE of the lights window's fail-closed reading and deliberately
/// so: a window failing closed costs one flash of a lamp, and a mute failing
/// closed costs every notification, including the card for a tool call the
/// operator is blocked on, with no expiry and no way for them to see it. The
/// complaint repeats for as long as the file stays broken, which is
/// proportional: it IS broken until someone fixes it.
///
/// ONLY AN ABSENT FILE IS SILENT, and it is the ordinary state. A single
/// `.ok()?` used to cover both, so a file that could not be read at all was
/// as quiet as one that was never there: unreadable permissions, a directory
/// standing in its place and bytes that are not UTF-8 each muted nothing and
/// announced nothing, which is the state nobody can discover.
fn read_quiet_expiry() -> Option<u64> {
    let raw = match std::fs::read_to_string(state_dir().join(QUIET_UNTIL)) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            eprintln!(
                "pns: state error (quiet-until could not be read: {error}); \
                 nothing is muted, clear it with pns quiet off"
            );
            return None;
        }
    };
    pns::quiet::expiry_from_state(&raw)
        .inspect_err(|complaint| eprintln!("{complaint}"))
        .ok()
}

/// Whether the operator's mute is on, judged on THE RUN'S OWN clock reading:
/// the same one the rest of the decision is taken against. An expiry crossed
/// mid-run costs one event either way, and one decision on one reading is the
/// engine's stated contract.
fn muted_now(now_secs: Option<u64>) -> bool {
    pns::quiet::is_muted(read_quiet_expiry(), now_secs)
}

/// Where macOS keeps the Focus state, under the operator's own home.
const FOCUS_DB: &str = "Library/DoNotDisturb/DB";

/// One reading of the Focus store: the verdict the event path acts on, and
/// what the mode catalog beside it did.
///
/// THE CATALOG'S FAILURE RIDES OUT ON THE ANSWER rather than being read a
/// second time by the doctor. A second read is a second moment, and the doctor
/// would then be reporting on a file the decision never saw.
struct FocusReading {
    /// Whether a mode `[focus] silence` named is asserted right now.
    silenced: bool,
    /// Why the mode catalog could not be read, when it could not. `Some` means
    /// NO display name resolved, so only a raw `modeIdentifier` in the config
    /// could have matched anything.
    catalog: Option<std::io::ErrorKind>,
}

/// Whether a macOS Focus the config NAMED is asserted right now, or the error
/// the assertion store's own read failed with.
///
/// HOME-RELATIVE AND WITH NO ENV HATCH, deliberately. A variable naming this
/// path would let any producer force the answer in either direction, which is
/// the objection `Overrides::muted` already states about the mute. The test
/// seam is the sandbox's own `HOME`, which every binary test already sets.
///
/// NOTHING NAMED MEANS NOTHING READ. With no `[focus] silence` list there is
/// no mode an assertion could match, so the two files are never opened and the
/// default machine pays no IO for a feature it did not ask for.
///
/// `Err` IS "the store could not be read", and it exists for the doctor alone:
/// the event path reads it as not silenced, because this is a private,
/// undocumented Apple store that can change schema on any macOS update and a
/// reader that failed closed would silence every banner, card and pulse on the
/// morning after an upgrade. The doctor is the one place that says so out
/// loud, and the ERROR ITSELF is carried out rather than flattened, because a
/// store that is absent and a store that is gated send the operator to two
/// different places.
///
/// THE CATALOG'S OWN FAILURE IS NOT ONE OF THOSE. An unreadable
/// `ModeConfigurations.json` resolves no names, so only a raw `modeIdentifier`
/// in the config can still match: silencing less rather than more, which is
/// the same direction. It is reported rather than errored for exactly that
/// reason, and the doctor says it in a clause of its own.
///
/// READ THROUGH `readable_ring` for the reasons that function states about
/// this tool's own files, which hold for a foreign one just as well: a FIFO at
/// the path would park the event forever, and a file some other hand grew is
/// otherwise learned about by allocating it. The live store is 6 KiB against
/// the existing 256 KiB ceiling.
fn focus_now(home: &str, silence: &[String]) -> std::io::Result<FocusReading> {
    if silence.is_empty() {
        return Ok(FocusReading {
            silenced: false,
            catalog: None,
        });
    }
    let store = Path::new(home).join(FOCUS_DB);
    let assertions = readable_ring(&store.join("Assertions.json"), RING_READ_MAX)?;
    let catalog = readable_ring(&store.join("ModeConfigurations.json"), RING_READ_MAX);
    Ok(FocusReading {
        silenced: pns::focus::silenced(
            &pns::focus::active_modes(&assertions),
            &pns::focus::mode_names(catalog.as_deref().unwrap_or_default()),
            silence,
        ),
        catalog: catalog.as_ref().err().map(std::io::Error::kind),
    })
}

/// The CLIP v2 bridge over ureq.
struct UreqBridge {
    base: String,
    key: String,
}

impl UreqBridge {
    fn agent(&self) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(BRIDGE_DEADLINE))
            .max_redirects(0)
            // The bridge serves a self-signed certificate for its own LAN
            // address, so verification is disabled here exactly as openhue
            // does it; there is no CA that could vouch for a Hue bridge.
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .disable_verification(true)
                    .build(),
            )
            .build()
            .new_agent()
    }
}

/// How long one bridge call may take. The pulse is decoration on a
/// notification, so it must never be what makes one slow.
const BRIDGE_DEADLINE: Duration = Duration::from_secs(10);

impl Bridge for UreqBridge {
    fn get(&self, path: &str) -> Option<String> {
        self.agent()
            .get(format!("{}/{path}", self.base))
            .header("hue-application-key", &self.key)
            .call()
            .ok()?
            .body_mut()
            .read_to_string()
            .ok()
    }

    fn put(&self, path: &str, body: &str) {
        // Nothing reads the outcome: a pulse that did not land is not worth
        // failing, reporting or retrying on a notification path.
        let _ = self
            .agent()
            .put(format!("{}/{path}", self.base))
            .header("hue-application-key", &self.key)
            .content_type("application/json")
            .send(body);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_REREAD_ATTEMPTS, DEFAULT_REREAD_INTERVAL, MAX_REREAD_ATTEMPTS, MAX_REREAD_INTERVAL,
        STATE_FILE_MODE, matches_glob, publish_state_line, read_note, recap_bounds,
        republish_after, reread_attempts_from, reread_interval_from, resolve_path,
    };
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    #[test]
    fn every_reread_interval_that_is_not_a_duration_falls_back_to_the_default() {
        // The first four panicked `Duration::from_secs_f64` outright. The last
        // two are FINITE and non-negative, so they passed the guard written
        // for the others and panicked in the constructor anyway (exit 101 on
        // a hook whose whole contract is exiting 0).
        for raw in [
            "NaN",
            "inf",
            "-inf",
            "-1",
            "not-a-number",
            "",
            "1e30",
            "1e300",
        ] {
            assert_eq!(
                reread_interval_from(Some(raw)),
                DEFAULT_REREAD_INTERVAL,
                "interval {raw:?}"
            );
        }
        assert_eq!(reread_interval_from(None), DEFAULT_REREAD_INTERVAL);
    }

    #[test]
    fn an_oversized_reread_knob_is_clamped_rather_than_believed() {
        // Both knobs multiply into how long a Stop hook can hold a turn's
        // report open, so each has a ceiling: a stray zero must cost seconds,
        // never hours.
        assert_eq!(reread_interval_from(Some("1000000")), MAX_REREAD_INTERVAL);
        assert_eq!(
            reread_attempts_from(Some("4294967295")),
            MAX_REREAD_ATTEMPTS
        );
        assert_eq!(reread_attempts_from(Some("11")), MAX_REREAD_ATTEMPTS);
    }

    #[test]
    fn a_reread_knob_inside_its_ceiling_is_taken_as_written() {
        assert_eq!(
            reread_interval_from(Some("0.25")),
            Duration::from_millis(250)
        );
        assert_eq!(reread_interval_from(Some("0")), Duration::ZERO);
        assert_eq!(reread_attempts_from(Some("2")), 2);
        assert_eq!(reread_attempts_from(Some("0")), 0);
        assert_eq!(reread_attempts_from(None), DEFAULT_REREAD_ATTEMPTS);
    }

    /// A published state file's mode, which is the only thing the test below
    /// grades.
    fn published_mode(path: &std::path::Path) -> u32 {
        std::fs::metadata(path)
            .expect("the published file")
            .permissions()
            .mode()
            & 0o777
    }

    #[test]
    fn a_pending_file_left_behind_wide_open_is_narrowed_before_the_rename_publishes_it() {
        // MEASURED: `OpenOptions::mode` applies only when the open CREATES the
        // file, so a pending inode an earlier run left at the umask's mode
        // keeps it, and the rename is what publishes that mode OVER the state
        // file. The pending path carries this process's own id, which is
        // exactly what makes a run interrupted between the open and the rename
        // leave one for the next run of the same pid to reuse.
        let directory =
            std::env::temp_dir().join(format!("pns-publish-mode-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("the scratch directory");
        let published = directory.join("missed-notifications");
        let pending = published.with_extension(format!("new.{}", std::process::id()));
        std::fs::write(&pending, "an interrupted run\n").expect("the pending file");
        // STATED RATHER THAN INHERITED from the umask, so the fixture is the
        // same wide mode on every machine and on a rerun that found its own
        // leftovers.
        std::fs::set_permissions(&pending, std::fs::Permissions::from_mode(0o644))
            .expect("the wide mode");

        publish_state_line(&published, "one line").expect("the publish");

        // THE PUBLISH REALLY RAN, asserted before the mode: a file left from an
        // earlier run already at 0600 would pass the mode assertion alone.
        assert_eq!(
            std::fs::read_to_string(&published).expect("the published file"),
            "one line\n"
        );
        assert_eq!(
            published_mode(&published),
            STATE_FILE_MODE,
            "the reused pending inode published its own wide mode"
        );
    }

    #[test]
    fn a_ring_that_vanished_under_the_append_is_never_republished_over() {
        // THE ONE ERROR THAT IS NOT A DAMAGED RING. Nothing removes one of
        // these files except a claim, and a claim is a RENAME, so a read-back
        // that finds nothing means the line just written travelled inside the
        // claim and is already on its way to the operator. Republishing it
        // would put an already-claimed record back at the path and deliver it
        // a second time.
        //
        // THE LIMIT, stated: this pins the DECISION, not the wiring. Staging a
        // real claim between the append's write and its read-back is a race no
        // test in this tree can make deterministic, and it belongs to the
        // out-of-tree probe.
        assert!(!republish_after(&std::io::Error::from(
            std::io::ErrorKind::NotFound
        )));
        // AND EVERY OTHER REASON STILL HEALS: a ring that cannot be read is a
        // ring that can never be pruned again, which is what the republish is
        // for. These three are exactly what the guarded reader answers with.
        for kind in [
            std::io::ErrorKind::InvalidData,
            std::io::ErrorKind::InvalidInput,
            std::io::ErrorKind::FileTooLarge,
        ] {
            assert!(
                republish_after(&std::io::Error::from(kind)),
                "a ring that answered {kind:?} was left unhealed"
            );
        }
    }

    #[test]
    fn a_recap_window_is_two_plain_counts_in_either_order_and_nothing_else() {
        let bounds = |words: &[&str]| {
            recap_bounds(
                &words
                    .iter()
                    .map(|word| word.to_string())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            bounds(&["--since", "1756499000", "--until", "1756500000"]),
            Some((1_756_499_000, 1_756_500_000))
        );
        // Either order, because the spawner writes one and a hand run writes
        // whichever it likes.
        assert_eq!(
            bounds(&["--until", "1756500000", "--since", "1756499000"]),
            Some((1_756_499_000, 1_756_500_000))
        );
        // A window of one instant is a window: nothing happened in it, and the
        // body says so rather than the parser refusing to describe it.
        assert_eq!(bounds(&["--since", "5", "--until", "5"]), Some((5, 5)));
    }

    #[test]
    fn every_recap_window_this_will_not_vouch_for_is_refused_rather_than_defaulted() {
        // A RECAP OVER A WINDOW NOBODY ASKED FOR IS WORSE THAN NONE, so there
        // is no default half and no silent fallthrough: a missing bound, a
        // bound that is not a plain count, a window that runs backwards, a
        // repeated flag and any word this does not serve are each a refusal.
        let bounds = |words: &[&str]| {
            recap_bounds(
                &words
                    .iter()
                    .map(|word| word.to_string())
                    .collect::<Vec<_>>(),
            )
        };
        for refused in [
            vec![],
            vec!["--since", "1756499000"],
            vec!["--until", "1756500000"],
            vec!["--since", "1756500000", "--until", "1756499000"],
            vec!["--since", "yesterday", "--until", "1756500000"],
            vec!["--since", "-5", "--until", "1756500000"],
            vec!["--since", "1756499000", "--since", "1756499500"],
            vec!["--since", "1756499000", "--until", "1756500000", "--now"],
            vec!["--since", "1756499000", "--until"],
            vec!["1756499000", "1756500000"],
        ] {
            assert_eq!(bounds(&refused), None, "case: {refused:?}");
        }
    }

    #[test]
    fn an_empty_channels_dir_variable_means_the_default_not_the_current_dir() {
        // Bash's ${VAR:-default} defaults on EMPTY as well as unset; joining
        // a filename to an empty path would quietly deliver nothing.
        assert_eq!(
            resolve_path(Some(""), "/fallback/channels"),
            std::path::PathBuf::from("/fallback/channels")
        );
        assert_eq!(
            resolve_path(None, "/fallback/channels"),
            std::path::PathBuf::from("/fallback/channels")
        );
        assert_eq!(
            resolve_path(Some("/set/dir"), "/fallback/channels"),
            std::path::PathBuf::from("/set/dir")
        );
    }

    #[test]
    fn a_glob_matches_only_what_its_own_two_ends_bracket() {
        // A STAR STANDS FOR ANYTHING INCLUDING NOTHING, and the ends are ends
        // rather than anywhere in the name.
        for (name, matches) in [
            ("checklist-s17.md", true),
            ("checklist-.md", true),
            ("checklist.md", false),
            ("checklist-s17.txt", false),
            ("other-s17.md", false),
        ] {
            assert_eq!(
                matches_glob(name, "checklist-*.md"),
                matches,
                "{name} against checklist-*.md"
            );
        }
        // AND THE TWO ENDS MAY NOT CLAIM THE SAME CHARACTERS. `notes-notes.md`
        // both starts with `notes-` and ends with `-notes.md`, sharing the one
        // hyphen between them, so a matcher asking only those two questions
        // would match a name too short to hold both ends at once.
        assert!(!matches_glob("notes-notes.md", "notes-*-notes.md"));
        assert!(matches_glob("notes--notes.md", "notes-*-notes.md"));
        // AND A PATTERN WITH NO `*` NAMES ONE FILE, which is the ordinary case
        // of an operator pointing at a single note.
        assert!(matches_glob("notes.md", "notes.md"));
        assert!(!matches_glob("notes.md.bak", "notes.md"));
    }

    #[test]
    fn a_note_is_judged_by_the_handle_it_was_opened_on_rather_than_by_its_name() {
        // THE SCAN AND THE READ ARE TWO MOMENTS, and this is the second one.
        // The directory belongs to the operator's other tools, so a name that
        // was a regular file inside the window when it was listed can be a
        // symlink out of that directory, or a file rewritten since, by the time
        // it is opened. CONSTRUCTED BY CALLING THE READ ITSELF, which is that
        // ordering: whatever the scan believed, this is what the read is handed.
        let directory = std::env::temp_dir().join(format!(
            "pns-note-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos())
        ));
        std::fs::create_dir_all(&directory).expect("the scratch directory");
        let planted = |name: &str, at: Duration| {
            let path = directory.join(name);
            std::fs::write(&path, "# a finding\n").expect("the note");
            std::fs::File::options()
                .write(true)
                .open(&path)
                .expect("the note")
                .set_modified(std::time::UNIX_EPOCH + at)
                .expect("the note's clock");
            path
        };

        let inside = planted("checklist-inside.md", Duration::from_secs(1500));
        assert_eq!(
            read_note(&inside, 1000, 2000).as_deref(),
            Some("# a finding\n")
        );
        // THE WINDOW IS HALF-OPEN AT FULL PRECISION, on `activity_in`'s own
        // rule. Truncating to whole seconds put a note written half a second
        // after the marker outside the window and one written half a second
        // after it closed inside.
        let edge = planted("checklist-edge.md", Duration::from_millis(1_000_500));
        assert!(
            read_note(&edge, 1000, 2000).is_some(),
            "half a second past the near edge is inside the window"
        );
        let past = planted("checklist-past.md", Duration::from_millis(2_000_500));
        assert!(
            read_note(&past, 1000, 2000).is_none(),
            "half a second past the far edge is outside the window"
        );
        // A SYMLINK IS REFUSED RATHER THAN FOLLOWED, so a name swapped after
        // the scan cannot read a file the glob never named. The scan skips
        // links itself; this is what stops the one planted between the two.
        let swapped = directory.join("checklist-swapped.md");
        let _ = std::os::unix::fs::symlink(&inside, &swapped);
        assert!(
            read_note(&swapped, 1000, 2000).is_none(),
            "a symlink planted at a matched name was followed"
        );
        // AND A FILE REWRITTEN SINCE THE SCAN IS REFUSED for the same reason:
        // the clock on the handle is what decides, not the one the scan saw.
        std::fs::write(&inside, "# rewritten after the scan\n").expect("the rewrite");
        std::fs::File::options()
            .write(true)
            .open(&inside)
            .expect("the note")
            .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(9000))
            .expect("the note's clock");
        assert!(
            read_note(&inside, 1000, 2000).is_none(),
            "a note rewritten after the scan was read into the window anyway"
        );
    }
}
