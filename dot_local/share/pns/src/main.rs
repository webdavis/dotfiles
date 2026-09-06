//! The pns binary: the composition root, and the only place with a main.
//!
//! Everything here is WIRING. The roster is one constant and one constructor
//! in `registry`, so there is no second construction of it to diverge; the
//! environment and the config are read once at this edge, and every decision
//! is delegated to the library. The producer path exits 0 on every path,
//! because a notification must never fail the work it reports on, and so
//! does `pns hook <event>` for every event but `blocked`, which, like
//! `pns gate`, passes through moshi's own exit code (see `moshi_decision`).
//! The hand-typed verbs refuse a bad invocation with exit 2, with two gaps
//! still open: `home` is a diagnostic that always exits 0, and a word
//! trailing `lights tick` is dropped rather than refused.

use std::collections::BTreeMap;
use std::io::{IsTerminal, Read, Seek, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pns::args::parse_args;
use pns::channels::banner::BannerChannel;
use pns::channels::hermes::{
    DEFAULT_HERMES_URL, HermesChannel, UreqSignedPost, channel_url, hermes_secret, remote_deadline,
};
use pns::channels::hue::{
    BRIDGE_DEADLINE, HuePulse, TYPED_COMMAND_DEADLINE, UreqBridge, hue_settings, quiet_now,
    quiet_window,
};
use pns::channels::moshi::{
    DEFAULT_MOSHI_URL, MOSHI_TYPE, MoshiChannel, UreqPost, mobile_backend, moshi_secret,
    refused_backend_line,
};
use pns::channels::{Delivery, native_first};
use pns::config::{LoadOutcome, config_path, load_config};
use pns::engine::{Overrides, decide};
use pns::hooks::{
    HookPayload, condenser_prompt, condenser_verdict, flattened, moshi_subcommand, parse_payload,
    transcript_reply,
};
use pns::registry::{roster, select_plugins};
use pns::render;
use pns::system::{
    PROBE_READ_MAX, SystemCommandRunner, SystemProbes, local_minutes_since_midnight, run_bounded,
};

mod blocked_wait_markers;
mod channel_dispatch;
mod channel_settings;
mod command_presence;
mod event_flow;
mod event_records;
mod hook_dispatch;
mod hook_observations;
mod hook_payload;
mod invocation;
mod journal_claims;
mod lamp_event_lease;
mod moshi_submission;
mod presence_runtime;
mod return_replay;
mod return_window;
mod runtime_environment;
mod state_lock_runtime;
mod state_rings;
mod turn_condenser;
mod turn_lifecycle;

#[cfg(test)]
mod runtime_test_support;

pub(crate) use blocked_wait_markers::{end_blocked_wait, update_blocked_marker};
pub(crate) use channel_dispatch::dispatch_legs;
pub(crate) use channel_settings::{
    Mobile, disabled_backend_warnings, enabled_hue_table, plugin_settings, read_mobile,
};
pub(crate) use command_presence::{ensure_presence_poll, presence_mode, presence_settings};
pub(crate) use event_flow::{Attempt, run_event};
pub(crate) use event_records::{
    DECISIONS, MISSED_NOTIFICATIONS, activity_in, record_activity, record_decision, record_missed,
};
pub(crate) use hook_dispatch::hook_mode;
pub(crate) use hook_observations::{
    arm_quota_stale_wait, config_change_detail, model_switch_detail, quota_observation_detail,
    record_policy_settings_change,
};
pub(crate) use hook_payload::{payload_is_whole, read_payload};
pub(crate) use invocation::{USAGE, event_mode, is_producer_argv, second_argument};
pub(crate) use journal_claims::{claim_journal, owner_is_gone};
pub(crate) use lamp_event_lease::{
    LIGHTS_JOB, ORDINARY_LEASE_SECS, clear_held_lamps, register_lights_tick, schedule_lights_tick,
};
pub(crate) use moshi_submission::{blocking_event, gate_mode, moshi_hook_bin};
pub(crate) use presence_runtime::{
    home_presence, last_narrowing, narrow_to_presence, presence_snapshot, presence_status,
    system_probes,
};
pub(crate) use return_replay::replay_missed;
pub(crate) use return_window::{Moment, claim_moment, mark_present, read_epoch};
pub(crate) use runtime_environment::{
    env_deadline, executable_in_path, now_secs, overrides_from_env, resolve_path, state_dir,
};
pub(crate) use state_lock_runtime::{HeldLock, claim_lock};
pub(crate) use state_rings::{
    RING_READ_MAX, STATE_FILE_MODE, append_ring_line, publish_state_line,
};
pub(crate) use turn_condenser::condense;
pub(crate) use turn_lifecycle::{end_of_turn, failed_turn, project_of, start_of_turn};

#[cfg(test)]
pub(crate) use command_presence::{Polled, write_presence_reading};

fn main() {
    // ONE READ OF ARGV, lossy rather than validating: `std::env::args()`
    // panics on non-UTF-8, and a stray byte degrading into an unknown token
    // (which the lenient parser already skips) is the honest failure mode
    // for an always-exit-0 notification path. `first`, the producer check
    // and the event parse each used to read `std::env::args_os()` on their
    // own; this is the one collection they share now.
    let argv: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let first = argv.first().cloned().unwrap_or_default();
    // The pulse is a MODE, not a leg: it fires on a long command's exit code
    // rather than on an event, so it leaves before any of the event wiring.
    if first == "pulse" {
        std::process::exit(pulse_mode());
    }
    // The home diagnostic: one reading of the router, said out loud. The
    // doctor mode (P3) will absorb it; until then this is how the probe is
    // drilled and how a wrong config is diagnosed.
    if first == "home" {
        home_mode();
        return;
    }
    // The operator's mute, typed and timed. Also a MODE: it writes the state
    // the event path reads, and delivers nothing itself.
    if first == "quiet" {
        std::process::exit(quiet_mode());
    }
    // One test send through every configured channel, and one line per
    // registered plugin about it. A MODE for the same reason the others are:
    // it takes no decision, so nothing about an event's plan reaches it.
    if first == "doctor" {
        std::process::exit(doctor_mode());
    }
    // The return recap, rendered from the activity ring and posted to Discord.
    // A MODE for the reason the others are: it takes no decision, so no event's
    // plan reaches it. The event path starts it detached; an operator can also
    // run it by hand, which is how it is drilled.
    if first == "recap" {
        std::process::exit(recap_mode());
    }
    // The clock. A MODE for the reason the others are: `run` takes no event
    // and delivers nothing itself, and the two typed verbs beside it only move
    // a file. Nothing on the event path below reaches it, and nothing here
    // reaches the event path except by re-executing this binary.
    if first == "daemon" {
        std::process::exit(daemon_mode(&second_argument()));
    }
    // The lamps' upkeep. A MODE beside the daemon's for the same reason: it
    // takes no decision and delivers nothing, and the daemon is what runs it.
    // It reaches the event path through nothing at all.
    if first == "lights" {
        std::process::exit(lights_mode(&second_argument()));
    }
    // The room sensor's own upkeep. A MODE beside the lamps' for the same
    // reason: it reads the bridge, publishes one state line and delivers
    // nothing, and the daemon is what runs it.
    if first == "presence" {
        std::process::exit(presence_mode(&second_argument()));
    }
    // The loop lease, taken and given back by hand. A MODE beside the lamps'
    // for the same reason: it moves one file and delivers nothing.
    if first == "loop" {
        std::process::exit(loop_mode(&second_argument()));
    }
    // The nudge about an approval nobody answered. A MODE for the reason the
    // others are: it takes no decision from an event and reads no stdin. It
    // takes NO SESSION ARGUMENT either, because coalescing means it looks at
    // every outstanding record rather than at the one whose timer woke it, so
    // an argument would be a value it had to ignore.
    if first == "nag" {
        std::process::exit(nag_mode());
    }
    // The first-run walk. A MODE that has to be reachable with NO CONFIG AT
    // ALL, which is the state it exists to end, and that is why it sits above
    // everything that loads one. Nothing on the event path reaches it and it
    // reaches nothing there: it asks questions, composes text and publishes a
    // file, and delivers nothing.
    if first == "setup" {
        std::process::exit(setup_mode());
    }
    // The gate moshi's OWN extension calls. pi and omp spawn
    // `helperBinary pi-hook`, and that field holds one PATHNAME with no room
    // for a subcommand, so the binary answers the bare harness word itself.
    if pns::hooks::is_harness_subcommand(&first) {
        std::process::exit(gate_mode(&first));
    }
    // The same gate, spelled the way an operator reads it. Both forms end in
    // gate_mode, which REFUSES a word it will not vouch for: falling through
    // to the event path instead is how the documented spelling used to fire a
    // notification about an empty event.
    if first == "gate" {
        std::process::exit(gate_mode(&second_argument()));
    }
    if first == "hook" {
        std::process::exit(hook_mode(&second_argument()));
    }
    // A WORD THAT NAMES NO COMMAND IS A TYPO, never an event. It is the house
    // rule `pns nag` and `pns lights` already keep, moved up to where argv[1]
    // is decided: the producer parser is deliberately lenient about a token it
    // does not know, so `pns stpo` used to skip the word, render an empty event
    // and deliver it. The always-exit-0 contract governs EVENT deliveries, and
    // a word naming no command never becomes one, so refusing it here
    // contradicts nothing. `--help`/`-h` still reaches `event_mode` from here
    // (see `is_producer_argv`): that parser holds the one help arm now, so
    // there is no second copy of it up here to answer help before anything
    // else runs.
    if !is_producer_argv(&argv) {
        eprint!("{USAGE}");
        std::process::exit(2);
    }
    event_mode(&argv);
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

/// One line, holding the episode the operator has already been warned about,
/// absent when a HOME reading showed no staleness. NO SESSION ID: one config
/// names one device, so there is one staleness state at a time and every
/// reader of it means the same one.
const STALENESS_MEMORY: &str = "home-staleness";

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

/// The event path's pulse, which the lights' own quiet window may mute.
///
/// THE GATE LIVES HERE, at the call site, and not in `fire_pulse` below:
/// `pns pulse` shares that function and is deliberately exempt, because the
/// hand-run pulse is how a bridge and key are checked and gating it would make
/// the quiet window untestable exactly while it is on. Inside the `if` that
/// already earned a pulse, so a refusal is printed only where a room would
/// otherwise have lit.
fn fire_pulse_unless_quiet(
    hue_table: Option<toml::Table>,
    lights: Option<&pns::config::Lights>,
    behaviour: pns::config::Behaviour,
    presence: Option<&pns::presence_policy::Snapshot>,
) {
    // No table is nothing to quiet: an operator who never enabled the lights
    // gets the same silence `fire_pulse` would have given them.
    let Some(settings) = hue_table else {
        return;
    };
    // FRESH, not the run's start: the legs above dial the network under their
    // own deadlines, so a run can cross into a dim window between starting and
    // reaching the moment a lamp would actually light, and the older reading
    // would flash it just inside quiet hours. HONEST LIMIT: no suite pins the
    // freshness, because a test's clock does not advance mid-run.
    let now = now_secs();
    let minutes_now = now.and_then(local_minutes_since_midnight);
    let Some(lights) = lights else {
        // TODAY'S PATH, UNCHANGED, and it is the compatibility claim of this
        // whole change: one house window for the whole pulse, one write per room
        // in `[plugins.hue] rooms`, and one refusal that costs the pulse when
        // nobody can read the window. A machine that never wrote a `[lights]`
        // table reaches nothing new.
        match quiet_window(&settings) {
            Ok(window) => {
                if !quiet_now(window.as_ref(), minutes_now) {
                    fire_pulse(Some(settings), behaviour);
                }
            }
            // FAIL CLOSED, the direction the pulse takes on every unreadable
            // reading: a window nobody can parse is an operator who asked for
            // quiet hours and cannot be told which ones, so the room stays
            // dark and the refusal says why.
            Err(refusal) => eprintln!("{refusal}"),
        }
        return;
    };
    // THE OPERATOR'S OWN AD-HOC QUIET, read here rather than inside the walk
    // for the reason every reading on this path is: the modules take no files
    // and no clock, and the composition root decides where a complaint goes.
    // A machine that has never typed the command reads no file and pays one
    // failed open.
    let state = state_dir();
    let (muted, mut complaints) = ad_hoc_quiet(&state, now);
    complaints.extend(fire_lights(
        &settings,
        lights,
        behaviour,
        &pns::channels::hue::Reading {
            minutes_now,
            muted: &muted,
        },
        held_lamps(&state).as_deref(),
        presence,
    ));
    // SAY-ONCE, NOT ONCE PER EVENT. A state file something else corrupted stays
    // corrupt until a human fixes it, and this path fires many times a session,
    // so a bare print here is one stderr line per hook invocation forever.
    //
    // AND IT CARRIES THE RESOLUTION'S OWN FINDINGS TOO, which used to be
    // discarded here. A machine whose map routes only `done` and `failed` holds
    // no state, so its tick never resolves anything and never complains: a
    // mistyped lamp name on such a config was dark forever with the whole
    // system silent about it, and this is the path that meets it.
    say_lights_once(&state, &complaints, LIGHTS_QUIET_SAID);
}

/// The ROOM-BASED lights signal, from whichever mode asked for it, and how many
/// rooms it reached. Both notification callers discard the count; the hand-run
/// check is what it exists for, since the bridge acknowledges no write and a
/// room that was addressed is the last observable fact on this path.
///
/// `[plugins.hue] rooms` IS THE PATH WITHOUT A `[lights]` TABLE, and it is also
/// `pns pulse`'s path with one. That is deliberate: the hand-run pulse is the
/// bridge-and-key check, not a feature, and keeping it room-based means it
/// stays one write to one obvious place while the routing map grows.
fn fire_pulse(hue_table: Option<toml::Table>, behaviour: pns::config::Behaviour) -> usize {
    let Some(hue) = hue_table.and_then(|settings| {
        hue_settings(&settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())
    }) else {
        return 0;
    };
    HuePulse {
        bridge: UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
            deadline: BRIDGE_DEADLINE,
        },
        rooms: hue.rooms,
    }
    .run(behaviour)
}

/// The ROUTED lights signal: resolve the map on the bridge, then flash every
/// lamp routed for this pulse that nothing is currently holding.
///
/// THE HELD RECORD IS THE GATE, and it is the TICK'S record read here rather
/// than a held state re-derived on this path. One writer and one reader, at the
/// cost of up to one refresh interval of staleness: a lamp that took a held
/// state a second ago may still flash once, and a lamp whose state ended a
/// second ago may skip one flash. Re-deriving it here would mean two processes
/// each deciding what the house is holding, from readings taken at different
/// moments, which is the divergence this crate keeps paying for.
fn fire_lights(
    settings: &toml::Table,
    lights: &pns::config::Lights,
    behaviour: pns::config::Behaviour,
    reading: &pns::channels::hue::Reading<'_>,
    held: Option<&[String]>,
    presence: Option<&pns::presence_policy::Snapshot>,
) -> Vec<String> {
    let Some(hue) = hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()) else {
        return Vec::new();
    };
    let bridge = UreqBridge {
        base: format!("https://{}/clip/v2/resource", hue.bridge),
        key: hue.key,
        deadline: BRIDGE_DEADLINE,
    };
    run_pulse_writes(
        &bridge,
        &state_dir(),
        lights,
        behaviour,
        reading,
        held,
        presence,
    )
}

/// The event path's routed writes: one pulse body per lamp the behaviour is
/// routed for, with the mute and the TICK'S held record each answered at the
/// per-lamp decision, once.
///
/// IT ANSWERS WITH THE RESOLUTION'S COMPLAINTS rather than printing or dropping
/// them. This path resolves the map on every pulse, so it is where a mistyped
/// name on a pulse-only config is met; the caller owns the say-once memory.
///
/// A HELD RECORD OF `None` IS EVERY LAMP HELD, which is the fail-dark direction
/// on the one gate that decides whether a blink writes over a breath. Read as
/// nothing held, an unreadable record let the pulse flash straight over a lamp
/// that was breathing about a question.
fn run_pulse_writes<B: pns::channels::hue::Bridge>(
    bridge: &B,
    state: &Path,
    lights: &pns::config::Lights,
    behaviour: pns::config::Behaviour,
    reading: &pns::channels::hue::Reading<'_>,
    held: Option<&[String]>,
    presence: Option<&pns::presence_policy::Snapshot>,
) -> Vec<String> {
    // A BRIDGE THAT ANSWERED NOTHING RESOLVES NOTHING, and says nothing here.
    // The doctor is where an unreachable bridge is reported; a warning on every
    // notification for the rest of a machine's life is noise.
    let Some(routing) = pns::channels::hue::resolve_on_bridge(bridge, lights) else {
        return Vec::new();
    };
    // THE COMPLAINTS COME OFF THE WHOLE RESOLUTION, before the narrowing: a
    // lamp name the bridge could not answer is a typo in the config whether or
    // not the operator is standing in that room.
    let mut routing = routing;
    let complaints = routing_complaints(&routing);
    // WHAT THIS EVENT WOULD ACTUALLY WRITE, decided ONCE and used twice: as
    // the set presence narrows over, and as the write itself. It answers the
    // whole per-lamp question, the mute, the routing, the held record and the
    // dim window together, because "eligible" has to mean "would light" or the
    // narrowing's own fallback is judging the wrong set.
    let write_for = |routed: &pns::channels::hue::Routed| -> Option<(String, String)> {
        let path = pns::channels::hue::Fixture::Light(routed.lamp.id.clone()).path();
        let lamp_is_held = held.is_none_or(|held| held.contains(&path));
        if pns::channels::hue::muted_now(&routed.lamp, reading.muted)
            || !pns::lights::pulse_fires(&routed.shows, behaviour, lamp_is_held)
        {
            return None;
        }
        let showing =
            pns::channels::hue::dim_showing(routed.dim.as_ref(), behaviour, reading.minutes_now);
        let (color, pulse, brightness) =
            pns::channels::hue::pulse_render(behaviour, lights, showing)?;
        Some((
            path,
            pns::channels::hue::pulse_body(&pulse, color, brightness),
        ))
    };
    // NARROWED OVER THE ELIGIBLE SET AND NOT THE WHOLE ONE. A room holding a
    // lamp that carries some OTHER behaviour is a room this event lights
    // nothing in, so narrowing to it and filtering afterwards produced exactly
    // the silence the fallback exists to prevent.
    routing.lamps.retain(|routed| write_for(routed).is_some());
    let routing = narrow_to_presence(state, routing, presence);
    for routed in &routing.lamps {
        if let Some((path, body)) = write_for(routed) {
            bridge.put(&path, &body);
        }
    }
    complaints
}

/// What one resolution has to say for itself: every declared name the bridge
/// could not answer, and every declaration it refused.
///
/// ONE WORDING FOR BOTH READERS, the tick's and the event path's, because a
/// typo reported in two spellings is two entries in two say-once memories and
/// an operator reading the same problem twice.
///
/// `pns ` AND NOT `pns lights: `, because every sentence already begins
/// `lights: ` (the doctor prefixes the same sentences `pns doctor: `).
fn routing_complaints(routing: &pns::channels::hue::Routing) -> Vec<String> {
    routing
        .unresolved
        .iter()
        .map(|missing| format!("pns {}", pns::channels::hue::missing_sentence(missing)))
        .chain(
            routing
                .refusals
                .iter()
                .map(|refusal| format!("pns {refusal}")),
        )
        .collect()
}

/// Whether the config's hue table resolves to a bridge that could be dialled:
/// the same reading `fire_pulse` takes, taken BEFORE it, so a check can tell a
/// bridge that listed no room from a config that names no bridge at all.
fn hue_resolves(hue_table: Option<&toml::Table>) -> bool {
    hue_table.is_some_and(|settings| {
        hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()).is_some()
    })
}

/// What the doctor can say about the lamps, and the ONE place that decides
/// which of its five states this machine is in.
///
/// THE BRIDGE IS DIALLED HERE, and only here, and only for a config that has
/// asked for the lamps AND enabled hue AND named a bridge. It costs the three
/// listings the routing resolves from, whatever the map says: arbitration and
/// the dim window are per lamp, so the joins are needed by every config that
/// routes anything at all.
///
/// BEHIND THE PANIC BOUNDARY every other bridge call gets, for `pulse_outcome`'s
/// reason: a panicking call must cost this section its lines rather than end
/// the report where the operator reads it as complete. A call that panicked
/// resolved no lamp, which is what the unreachable line says.
///
/// THE COST, NAMED: each GET is bounded by `BRIDGE_DEADLINE`, so a bridge that
/// accepts and never answers adds up to thirty seconds to `pns doctor`. That is
/// the same order as the pairing check's own two deadlines and it is paid only
/// by a machine that wrote the table.
fn lights_report(
    lights: Option<&pns::config::Lights>,
    hue_table: Option<&toml::Table>,
    hue_declared: bool,
) -> pns::doctor::LightsReport {
    let Some(lights) = lights else {
        return pns::doctor::LightsReport::Off;
    };
    let Some(settings) = hue_table else {
        // NEVER WRITTEN AND SWITCHED OFF ARE DIFFERENT CONFIGS, and the
        // enabled table is one `None` for both, so the declaration is read
        // separately rather than inferred from its absence.
        return if hue_declared {
            pns::doctor::LightsReport::HueDisabled
        } else {
            pns::doctor::LightsReport::HueMissing
        };
    };
    let Some(hue) = hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()) else {
        return pns::doctor::LightsReport::NoBridge;
    };
    let resolved = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pns::channels::hue::resolve_on_bridge(
            &UreqBridge {
                base: format!("https://{}/clip/v2/resource", hue.bridge),
                key: hue.key,
                deadline: BRIDGE_DEADLINE,
            },
            lights,
        )
    }));
    match resolved {
        Ok(Some(map)) => pns::doctor::LightsReport::Resolved(map),
        Ok(None) | Err(_) => pns::doctor::LightsReport::Unreachable,
    }
}

/// The pulse behind the same boundary every leg gets, so a panicking bridge
/// call costs the census the rest of its lines rather than ending the report
/// where the operator reads it as complete.
fn pulse_outcome(hue_table: Option<toml::Table>) -> pns::doctor::Outcome {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fire_pulse(hue_table, pns::config::Behaviour::Done)
    })) {
        Ok(rooms) => pns::doctor::Outcome::Signalled(rooms),
        // NO ROOM IS CLAIMED, and no panic text is quoted: the message is
        // written for a developer and may hold anything the pulse was carrying.
        Err(_) => {
            pns::doctor::Outcome::Failed("the pulse PANICKED; no room was signalled".to_string())
        }
    }
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
///
/// THE WORD IS READ BEFORE THE CONFIG LOADS. `pulse --help` used to load the
/// config first: with none it silently exited 0 having printed nothing, and
/// with one it pulsed the room red, because a non-numeric word was read as a
/// failing exit code. Reading the word first means `--help` and a bad code
/// both answer with no machine read at all.
fn pulse_mode() -> i32 {
    // THE WHOLE TAIL IS READ, not just the word right after `pulse`: H-B
    // requires help to win in flag position anywhere, and an unknown extra
    // word to be refused rather than silently dropped.
    let tail: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    if tail.iter().any(|token| pns::args::is_help_flag(token)) {
        println!("{PULSE_USAGE}");
        return 0;
    }
    if tail.len() > 1 {
        eprintln!("{PULSE_USAGE}");
        return 2;
    }
    let word = tail.first().cloned().unwrap_or_default();
    let Some(behaviour) = pns::pulse::exit_behaviour(&word) else {
        eprintln!("{PULSE_USAGE}");
        return 2;
    };
    let home = std::env::var("HOME").unwrap_or_default();
    // FAIL CLOSED, unlike an event. The roster fallback that keeps every
    // notification working through a broken config is an EVENT-mode rule:
    // applying it here would let an unrelated typo switch a deliberately
    // disabled pulse back on. The pulse runs only when its own table says
    // enabled, explicitly.
    let config = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config,
        // Absent is not a mistake; never opting in earns no warning.
        Ok(LoadOutcome::Missing) => return 0,
        Err(error) => {
            // The sanitized detail event mode prints, with the outcome THIS
            // mode had: there is no recoverable setting to fall back to, so
            // nothing pulses.
            eprintln!("pns: config error ({}); no pulse", error.detail());
            return 0;
        }
    };
    fire_pulse(enabled_hue_table(&config), behaviour);
    0
}

const PULSE_USAGE: &str = "pns: usage: pns pulse [<exit-code>] | \
pns pulse --help, -h (a bare `pulse` is a success pulse)";

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
    // disabled one, a `type` nothing answers and a mistyped value each send
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
            &HookPayload::default(),
            Attempt::First,
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
    let (
        hue_table,
        mobile,
        hermes_key,
        replay_card,
        focus_silence,
        daemon_enabled,
        nag_after_secs,
        lights,
        hue_declared,
    ) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (
            enabled_hue_table(config),
            read_mobile(config),
            plugin_settings(config, "hermes").and_then(hermes_secret),
            config.recap.replay_card,
            config.focus_silence.clone(),
            config.daemon_enabled,
            config.nag_after_secs,
            config.lights.clone(),
            // WHETHER THE TABLE WAS WRITTEN AT ALL, which
            // `enabled_hue_table` cannot say: it answers `None` both for a
            // table nobody wrote and for one whose switch is off, and the
            // lamps' report tells those two apart.
            config.plugins.contains_key("hue"),
        ),
        // THE SWITCH FALLS BACK ON, which is the fallback `run_event` takes
        // for the same reading. The two must agree or the doctor describes a
        // delivery the event would not make, and the Focus list falls back
        // EMPTY here for the same reason it does there.
        // AND THE NAG FALLS BACK OFF, which is the fallback `nag_after_secs`
        // takes for the same reading: the two must agree or the doctor
        // describes a schedule the fire would not keep.
        _ => (
            None,
            Mobile::default(),
            None,
            true,
            Vec::new(),
            true,
            NAG_OFF,
            None,
            false,
        ),
    };
    // THE SWITCHED-OFF TABLES THE EVENT PATH SAYS NOTHING ABOUT, said here
    // and only here: see `disabled_backend_warning`.
    if let Ok(LoadOutcome::Loaded(config)) = &loaded {
        for warning in disabled_backend_warnings(config) {
            eprintln!("{warning}");
        }
    }
    // THE ROOM SENSOR'S OWN SETTINGS, read here because the census below has
    // one line to print about them. A refusal is LOUD and leaves the reading
    // absent rather than half-honoured.
    let presence_settings = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => match pns::config::parse_presence(config) {
            Ok(settings) => settings,
            Err(error) => {
                eprintln!(
                    "pns: config error ({}); the room sensor is unread",
                    error.detail()
                );
                None
            }
        },
        _ => None,
    };
    let registry = roster();
    // WHAT LOADING FOUND, taken BEFORE `select_plugins` consumes it: the
    // census reports a plugin the selection left out, and which sentence is
    // true of that depends entirely on whether there was a config to read.
    let config_state = match &loaded {
        Ok(LoadOutcome::Loaded(_)) => pns::doctor::ConfigState::Read,
        Ok(LoadOutcome::Missing) => pns::doctor::ConfigState::Absent,
        Err(_) => pns::doctor::ConfigState::Unreadable,
    };
    // THE CONFIG FALLBACK IS INHERITED ON PURPOSE. `select_plugins` is what an
    // event would run and warn about, and the doctor's job is to say what an
    // event would do, not what a tidier engine would do.
    let (selection, warning) = select_plugins(&registry, loaded);
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }
    let checks = pns::doctor::checks(&registry.all(), &selection, config_state);

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
    let delivered = dispatch_legs(&legs, false, &event, &home, &mobile, hermes_key);

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
            // A READING, NEVER A SEND: nothing is dispatched to a sensor, and
            // what an operator cannot see any other way is what it says now.
            pns::doctor::CheckKind::Presence => pns::doctor::Outcome::Presence(
                presence_status(presence_settings.as_ref()),
                last_narrowing(&state_dir()),
            ),
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
    // BESIDE THE FOCUS LINE, which is the other line that reports state without
    // grading it. It must NOT move the exit code in any state, including the
    // dead one: a daemon that is down costs ambient features, and this exit
    // code is what an operator's automation reads as "notifications are
    // broken".
    println!("{}", daemon_line(daemon_enabled));
    // IMMEDIATELY BELOW THE DAEMON'S OWN LINE, and that placement is the whole
    // mitigation for the one thing this line does not say: a nag with a dead
    // daemon never fires, and the line above already reports the daemon from its
    // heartbeat. Two lines deriving one fact is how they drift apart, so these
    // two read as one paragraph instead.
    println!("{}", pns::doctor::nag_line(nag_after_secs));
    // AND THE LAMPS BELOW THE GATE, for the same reason: a dark lamp is not a
    // broken notifier, so this section reports and never grades. It is the last
    // thing that touches the network, so a bridge that hangs cannot delay a
    // line above it.
    for line in pns::doctor::lights_lines(&lights_report(
        lights.as_deref(),
        hue_table.as_ref(),
        hue_declared,
    )) {
        println!("{line}");
    }
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

// --- the nag ----------------------------------------------------------------

/// `pns nag`: one card about every approval nobody has answered, or silence.
///
/// RUN BY THE DAEMON AND TYPEABLE BY THE OPERATOR, which is what makes the
/// drill forceable without waiting out a timer. It PRINTS what it did, one
/// line, in `recap`'s shape.
///
/// OWNERSHIP IS TAKEN AT TWO LEVELS, and they answer two different questions.
/// The WINDOW is claimed once, before anything is enumerated (`claim_fire`), so
/// two processes woken by two jobs in one tick produce one card between them
/// rather than one card each. Each RECORD is then claimed by rename before it
/// is read for anything, which is what stops a single approval being counted
/// twice by a fire that broke in after a stale window claim aged out. Both are
/// renames because a plain unlink does not arbitrate on this filesystem; the
/// measurement is in
/// `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`.
///
/// THE ORDER IS THE SAFE ONE AT EVERY STEP. The markers are written BEFORE the
/// card and the claims removed AFTER it: a crash before the card leaves
/// approvals marked and silent, a crash after it leaves claims nothing
/// re-enumerates, and neither ordering can produce a SECOND card, which is the
/// property that matters.
fn nag_mode() -> i32 {
    // ANY EXTRA WORD IS A REFUSAL, per the house rule that an unknown argument
    // never falls through to help with exit 0. `pns nag <session>` is a command
    // an operator would believe narrowed the fire, and coalescing means nothing
    // here can honour it.
    if std::env::args_os().nth(2).is_some() {
        eprintln!("{NAG_USAGE}");
        return 2;
    }
    let state = state_dir();
    let directory = pns::nag::nag_dir(&state);
    // A CONFIG THAT TURNED THE FEATURE OFF BETWEEN ARMING AND FIRING MEANS NO
    // NUDGE, and the records go with it: the operator cancelled the timer, and
    // a card from it would be the feature ignoring them.
    let after_secs = nag_after_secs();
    if after_secs == NAG_OFF {
        let dropped = record_entries(&directory)
            .iter()
            .filter(|record| std::fs::remove_file(record).is_ok())
            .count();
        println!("pns nag: the nag is off; {dropped} waiting approval(s) dropped");
        return 0;
    }
    // NO CLOCK IS NO NUDGE. Every input this cannot read resolves to silence,
    // and a wait nothing can measure is one of them.
    let Some(now) = now_secs() else {
        eprintln!("pns nag: this machine has no clock to measure a wait against");
        return 0;
    };
    // THE DIRECTORY BEFORE THE LOCK THAT LIVES IN IT. The arm makes this
    // directory, but an operator running the fire by hand before anything has
    // ever armed (drill step 10) has no directory to take a lock in, and a
    // fire that could not say "nothing is waiting" would read as broken.
    let _ = std::fs::create_dir_all(&directory);
    // AND THE WHOLE FIRE CLAIMED ONCE, BEFORE ANYTHING IS ENUMERATED. See
    // `claim_fire`: the per-record claim is per-approval crash safety and does
    // not arbitrate a WINDOW, so without this two woken processes split the
    // outstanding records between them and card twice.
    let Some(fire) = claim_fire(&directory, now) else {
        // A LOSER SAYS NOTHING AT ALL, on either stream, and exits 0. The
        // window belongs to another process whose one card names every approval
        // this one would have, so a line here would be noise about work that is
        // being done.
        return 0;
    };

    let mut held: Vec<(std::path::PathBuf, pns::nag::Record, String)> = Vec::new();
    for record in record_entries(&directory) {
        // SOMEBODY ELSE OWNS IT, or it is not a regular file: either way this
        // process never opened it and never counts it.
        let Some(claim) = claim_record(&record) else {
            continue;
        };
        // A NAME THAT IS NOT A SESSION IS DROPPED, LOUDLY, AND ONLY ONCE. This
        // is the unreadable-CONTENT case one branch down wearing a different
        // coat, and it gets the same answer for the same stated reason: a file
        // skipped in silence sits at a record's name being re-read on every
        // fire forever. Nothing can be resolved from it (no marker, no job and
        // no card has a name to be written under), so there is nothing to
        // degrade to.
        let Some(session) = record
            .file_name()
            .and_then(|name| pns::nag::session_of(&name.to_string_lossy()))
        else {
            eprintln!(
                "pns nag: {} is not named for a session this can act on; it is dropped",
                record.display()
            );
            let _ = std::fs::remove_file(&claim);
            continue;
        };
        let parsed = std::fs::read_to_string(&claim)
            .ok()
            .as_deref()
            .and_then(pns::nag::parse);
        let answered = pns::nag::marker_name(&session)
            .is_some_and(|marker| marker_path(&state, &marker).exists());
        match (
            pns::nag::fate(parsed.as_ref(), answered, now, after_secs),
            parsed,
        ) {
            (pns::nag::Fate::Count, Some(record)) => held.push((claim, record, session)),
            // AN ACTION THAT SUPPRESSED ITS OWN ERROR HAS ONLY BEEN ATTEMPTED:
            // a file at a record's path that this could not read is somebody
            // else's write, and dropping it in silence is how one would sit
            // there being re-claimed on every fire forever.
            (pns::nag::Fate::Drop(pns::nag::Dropped::Unreadable), _) => {
                eprintln!(
                    "pns nag: {} is not a record this can read; it is dropped",
                    record.display()
                );
                let _ = std::fs::remove_file(&claim);
            }
            (_, _) => {
                let _ = std::fs::remove_file(&claim);
            }
        }
    }

    // OLDEST FIRST, so the card is built from the approval that has waited
    // longest: it is the one whose wait the multi-case names, and the one whose
    // pane is likeliest to still be the one worth focusing.
    held.sort_by_key(|(_, record, _)| record.armed);
    let Some((_, oldest, _)) = held.first() else {
        release_fire(&fire);
        println!("pns nag: nothing is waiting");
        return 0;
    };
    // THE MARKERS FIRST, FOR EVERY COUNTED RECORD. Those approvals have now
    // spent their one nudge, and the marker is what makes each of their OWN
    // daemon jobs drop silently when its turn comes; without it the siblings
    // would each wake a process that found nothing and said so.
    for (_, _, session) in &held {
        let Some(marker) = pns::nag::marker_name(session) else {
            continue;
        };
        if let Err(error) = write_marker(&state, &marker) {
            eprintln!("pns nag: an answered marker could not be written ({error})");
        }
    }
    // ONE CARD, WHATEVER THE COUNT, which is the operator's coalescing ruling
    // and the structural rate limit it buys: at most one nudge card per
    // `after_secs`, however many approvals are waiting.
    //
    // `PNS_SKIP_PHONE` IS NOT IN PLAY HERE. It is set by `blocking_event` in
    // that process only, and this is a different process minutes later that
    // never inherits it, so the nudge reaches the phone the first card was
    // suppressed from. That is deliberate and must not be "tidied" into the
    // record by a later refactor.
    run_event(
        &pns::args::EventArgs {
            agent: oldest.agent.clone(),
            // THE STATE WORD STAYS `blocked`. A new word would fall out of
            // `missed_notifications::NEEDS_YOU`, and an unanswered approval is
            // exactly what that section is for.
            state: BLOCKED_STATE.to_string(),
            project: oldest.project.clone(),
            branch: oldest.branch.clone(),
            detail: pns::nag::nudge(held.len(), now.saturating_sub(oldest.armed), &oldest.detail),
            pane: oldest.pane.clone(),
            ..Default::default()
        },
        &system_probes(),
        // NO PAYLOAD, and coalescing is why: one card stands for every record
        // in `held`, so naming one of their sessions would be inventing an
        // identity the card does not have. A nudge returns before the lamps'
        // needs marker is touched at all, so this is the honest default rather
        // than a value chosen to be ignored.
        &HookPayload::default(),
        Attempt::Nudge,
    );
    for (claim, _, _) in &held {
        if let Err(error) = std::fs::remove_file(claim) {
            eprintln!(
                "pns nag: the working file {} could not be removed ({error}); it is left behind",
                claim.display()
            );
        }
    }
    release_fire(&fire);
    // ATTEMPTED, NEVER SENT. `run_event` answers nothing about delivery and
    // this mode cannot know whether a single leg fired: a mute, a named Focus
    // or a plan that selected nothing all mean the nudge did not happen. The
    // drill reads this line, and an action reported as done when it was
    // suppressed is bug class 19 spoken out loud.
    println!("pns nag: {} waiting; one card attempted", held.len());
    0
}

/// The ONE clearing rule, and both signals go through it.
///
/// THE MARKER FIRST, THEN THE RECORD. A crash between the two leaves an
/// approval that is never nudged rather than one nudged after being answered,
/// which is the safe direction; and a marker whose write FAILED still removes
/// the record, because the record's absence already carries the same fact and
/// the marker is only what saves the daemon a no-op spawn.
///
/// THE MARKER IS WRITTEN WHETHER OR NOT A RECORD IS THERE, and that is a
/// correctness requirement rather than a simplification. The fire owns a record
/// by RENAMING it out of its own name, so between that rename and the fire's
/// marker check there is no `.pending` file for the session at all; a clear
/// gated on the record's presence does nothing in that window and the fire
/// cards an approval that has just been dealt with. The marker is the only
/// signal that reaches a record somebody else is holding.
///
/// WHAT THAT COSTS, NAMED: one marker file per session that ever resolves a
/// tool batch or ends a turn, rather than one per session that armed a nag.
/// They are empty, they are 0600, and one session writes one (the name is
/// constant per session, so a second batch rewrites the same file). That is the
/// accumulation the turn-start markers have carried since the turn clock
/// shipped, and it is accepted on the same terms (Risks 6, and the
/// no-removal-mechanisms ruling).
///
/// IT DOES NOT SILENCE A LATER APPROVAL. The arm clears this session's marker
/// BEFORE it publishes the new record, so a marker left by a batch that
/// resolved long ago cannot make the next approval's job drop.
///
/// NO COMMENT HERE MAY SAY THE MARKER RECORDS THE OPERATOR'S ANSWER. It records
/// the BATCH'S RESOLUTION, which is the only per-batch fact the harness's hook
/// vocabulary carries: an approval answered at ten seconds whose tool then runs
/// past the schedule is nudged about anyway. That cost is named in the template
/// rather than papered over here.
fn clear_nag(session_id: &str) {
    let state = state_dir();
    let (Some(record), Some(marker)) = (
        pns::nag::record_path(&state, session_id),
        pns::nag::marker_name(session_id),
    ) else {
        return;
    };
    if let Err(error) = write_marker(&state, &marker) {
        // ON STDERR AND NEVER ON STDOUT: this runs on a harness hook whose
        // output the harness reads.
        eprintln!("pns: an answered marker could not be written ({error})");
    }
    // BEST EFFORT, PRESENT OR NOT. Nothing here has to exist: the ordinary case
    // is a session that never armed, and the racing case is a record another
    // process is holding under a name this one does not know.
    let _ = std::fs::remove_file(&record);
}

/// One nudge armed for a blocked approval: the record, the marker clear, the
/// job.
///
/// EACH STEP'S FAILURE LEAVES A STATE THE NEXT FIRE RESOLVES, which is why any
/// order is safe and this one is stated: a crash after the record leaves a
/// record with no job, which the next fire enumerates and drops as stale, and a
/// failed registration leaves a record nothing will read.
///
/// EVERY FAILURE IS A LINE ON STDERR, NEVER ON STDOUT, and none of them changes
/// the exit code. Claude Code parses this hook's stdout as `let t = e.trim();
/// if (!t.startsWith("{")) return { plainText: e }`, so one stray line in front
/// of moshi's object turns an Allow into no decision at all. Bug class 19 is why
/// they are SAID rather than swallowed: the read-back here is deliberately weak,
/// so the honest move is a line naming what did not get armed.
///
/// WHAT IT COSTS THE BLOCKED PATH, BOUNDED AND MEASURED. Every step is local
/// filesystem work: one config open and TOML parse, one marker unlink, one
/// record published by write-then-rename, and one spool entry published the
/// same way. NO NETWORK, NO SUBPROCESS, NO SPAWN AND NO WAIT ON ANY OF THEM,
/// which is what makes it safe to sit in front of a notification the operator
/// is waiting on: nothing here can block on something that is not this
/// machine's own disk.
///
/// MEASURED ON DRESDEN, 500 runs of the blocked hook each way, one HOME with
/// `[nag] after_secs = 300` and one with no `[nag]` table and everything else
/// identical: 134.7ms +/- 14.1ms armed against 134.8ms +/- 13.3ms unarmed. The
/// arm is not separable from the hook's own run-to-run variation, which is the
/// bound worth stating: it is smaller than the noise of the thing it sits in.
fn arm_nag(session_id: &str, event: &pns::args::EventArgs) {
    // NO NAG ON CODEX, and the gate is POSITIVE rather than a `!= "codex"`, so
    // an empty or unknown `PNS_AGENT` arms nothing either (bug class 16:
    // set-but-empty is not unset). Codex wires exactly Stop and
    // PermissionRequest, so it has a turn-end clear and no batch-level one, and
    // agent turns in this repo routinely run tens of minutes: a Codex nag would
    // be wrong in the COMMON case rather than at an edge.
    if event.agent != CLAUDE_AGENT {
        return;
    }
    let after_secs = nag_after_secs();
    if after_secs == NAG_OFF {
        return;
    }
    let state = state_dir();
    let (Some(record), Some(marker), Some(id)) = (
        pns::nag::record_path(&state, session_id),
        pns::nag::marker_name(session_id),
        pns::nag::job_id(session_id),
    ) else {
        return;
    };
    // NO CLOCK IS NO ARM. A record whose `armed` nothing could read would be
    // judged stale on the first fire anyway; not writing it is the same answer
    // one step earlier.
    let Some(now) = now_secs() else {
        return;
    };
    // THE MARKER GOES FIRST, AND THE ORDER IS LOAD BEARING TWICE OVER.
    //
    // CLEARING IT AT ALL is required for correctness rather than hygiene: the
    // marker name is constant PER SESSION, so one left by the PREVIOUS approval
    // in this session would make the new job drop silently and this approval
    // would never be nudged. That is bug class 14 wearing this feature's
    // clothes, since the marker's identity is not the approval's presence.
    //
    // CLEARING IT BEFORE THE RECORD closes a window a concurrent fire can walk
    // into. Published first, the new record can be claimed by a fire that then
    // finds the PREVIOUS approval's marker still on disk and drops it as
    // answered, which costs this approval its nudge. Cleared first, the worst a
    // fire in the window can find is the previous approval's own record with no
    // marker, which is an outstanding approval being nudged about correctly.
    if let Err(error) = std::fs::remove_file(marker_path(&state, &marker))
        && error.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!(
            "pns: a previous approval's answered marker could not be cleared ({error}); \
             this approval will not be nudged"
        );
    }
    let written = publish_state_line(
        &record,
        &pns::nag::render(&pns::nag::Record {
            agent: event.agent.clone(),
            project: event.project.clone(),
            branch: event.branch.clone(),
            detail: event.detail.clone(),
            pane: event.pane.clone(),
            armed: now,
        }),
    );
    if let Err(error) = written {
        eprintln!(
            "pns: the nag record could not be written ({error}); this approval will not be nudged"
        );
        return;
    }
    let due = now.saturating_add(after_secs);
    let job = pns::daemon::Job {
        id,
        due,
        // THE LEASE IS ONE MORE SCHEDULE PAST THE DUE SECOND, which resolves to
        // the same instant as the fire-time staleness cap. The two are not
        // redundant: this drops the JOB, so a machine that slept through the
        // window never spawns at all, while the cap judges RECORDS, which is a
        // different set because a fire enumerates siblings whose own jobs have
        // not fired yet.
        until: due.saturating_add(after_secs),
        every: None,
        unless_marker: Some(marker),
        // NO FREE TEXT REACHES THE SPOOL. `args` are visible in the spool file
        // and in whatever the daemon logs, and the detail is the operator's own
        // question, so it lives in the record and `pns nag` takes no argument.
        args: vec![NAG_MODE_WORD.to_string()],
    };
    if let Err(refusal) = pns::daemon::schedule(&state, &job, now) {
        // AND THE RECORD GOES WITH IT, which is what makes the sentence true. A
        // record with no job wakes no fire of its own, but it stays ENUMERABLE:
        // a sibling approval's fire, or the operator running `pns nag` by hand,
        // counts it and cards about it. Leaving it would be this line saying
        // one thing while the state on disk said another.
        let dropped = match std::fs::remove_file(&record) {
            Ok(()) => "its record is dropped",
            Err(_) => "and its record could not be dropped either",
        };
        eprintln!(
            "pns: the nag could not be scheduled ({refusal}); this approval will not be nudged, {dropped}"
        );
    }
}

/// The one agent a nag is armed for. See `arm_nag`.
const CLAUDE_AGENT: &str = "claude";

/// The word the daemon re-executes this binary with.
const NAG_MODE_WORD: &str = "nag";

const NAG_USAGE: &str = "pns: usage: pns nag (it takes no arguments: one fire cards every \
outstanding approval at once)";

/// The state word a blocked approval and its nudge both carry.
const BLOCKED_STATE: &str = "blocked";

/// The schedule that means the nag is off, in the composition root's own
/// spelling of `config`'s default.
const NAG_OFF: u64 = 0;

/// Every file in the nag directory that could be a record, sorted so a fire is
/// deterministic.
///
/// THE SUFFIX IS THE WHOLE FILTER, which is what keeps a claim out of this: a
/// held claim is `<name>.claim.<pid>` and can never end in the record suffix,
/// so a record another process is mid-fire on is never re-enumerated here.
fn record_entries(directory: &Path) -> Vec<std::path::PathBuf> {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|entry| {
            entry
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(pns::nag::RECORD_SUFFIX))
        })
        .collect();
    entries.sort();
    entries
}

/// One record taken by rename, or None when somebody else has it.
///
/// THE RENAME IS THE OWNERSHIP TEST, in `consume_turn_marker`'s exact shape and
/// for `take_claim`'s measured reason: a plain unlink reports success to EVERY
/// racer on APFS, so a remove could tell two processes they each own this
/// record.
///
/// NOT THE SAME GUARANTEE AS THE FIRE CLAIM, and not made redundant by it. The
/// fire claim is what stops two processes carding in one window; this is what
/// stops ONE approval being counted twice when a second process is legitimately
/// running, which is what happens after a crashed fire's window claim ages out
/// while its records are still on disk. NO TEST IN THIS SUITE KILLS THIS
/// RENAME: reading each record in place and removing it afterwards passes
/// everything, because every fire in the suite bar one is single-process, and
/// that one is arbitrated a level up. It is kept on the measurement, not on a
/// test.
///
/// AN IRREGULAR FILE GOES BACK WHERE IT WAS AND IS NEVER OPENED, following
/// `append_ring_line`'s own refusal at a state path: a FIFO here would park the
/// read forever. The rename is still what tests it, because only the winner is
/// entitled to look at all.
fn claim_record(record: &Path) -> Option<std::path::PathBuf> {
    let claim = pns::nag::claim_path(record, std::process::id());
    // NEVER RENAMED OVER A CLAIM ALREADY THERE, for `claim_by_rename`'s reason:
    // the name carries this process's id, so anything sitting at it is a record
    // this pid claimed and could not finish, and a rename would land the new one
    // on top of it.
    if std::fs::symlink_metadata(&claim).is_ok() {
        return None;
    }
    std::fs::rename(record, &claim).ok()?;
    if !matches!(std::fs::symlink_metadata(&claim), Ok(found) if found.is_file()) {
        let _ = std::fs::rename(&claim, record);
        return None;
    }
    Some(claim)
}

/// The whole fire owned ONCE, or None when this process is not the one holding
/// this window.
///
/// NOT A DUPLICATE OF THE PER-RECORD CLAIM, which answers a different question.
/// That one is per-approval crash safety: it is what stops one record being
/// counted by two processes, and it stays. But ownership taken per record lets
/// two woken processes each win a DISJOINT, NON-EMPTY subset and each card its
/// own true count, which is one card per FIRE rather than one card per fire
/// WINDOW, and that is precisely what the coalescing ruling forbids. Measured
/// on the build before this: sixteen concurrent fires over one directory
/// produced sixteen cards. The window is what has to be owned, so it is.
///
/// AN EXCLUSIVE CREATE IS THE ARBITRATION, NOT A RENAME, and the difference is
/// measured rather than stylistic. A rename claim moves the contended name OUT
/// of the way: the winner renames `fire.lock` to its own claim, so a racer that
/// looked for a holder a moment earlier finds no lock at that name, creates one
/// and takes it too. That form delivered TWO cards from four concurrent fires,
/// reproducibly, under load. An exclusive create leaves the lock sitting at its
/// name for the whole fire, so every later racer is refused by the same atomic
/// operation, whenever it arrives. The rename survives below, in the one place
/// a remove would be unsafe.
///
/// AND AGED OUT AT A MINUTE, so a crash mid-fire cannot wedge the feature for
/// good. A minute is a wide margin over the work the lock has to cover: the
/// holder claims every record by rename before it delivers anything, so a fire
/// that broke in later finds an empty directory in any case. What the wait
/// costs when the holder really did die is one nudge window, which is the safe
/// direction.
fn claim_fire(directory: &Path, now: u64) -> Option<std::path::PathBuf> {
    let lock = directory.join(pns::nag::FIRE_LOCK);
    claim_lock(&lock, now, pns::nag::FIRE_STALE_SECS).then_some(lock)
}

/// The fire given up, so the next window can be claimed without waiting out
/// `FIRE_STALE_SECS`.
///
/// SAID WHEN IT FAILS, and the consequence is named rather than implied: the
/// feature is not broken by a claim left behind, it is DELAYED, because the age
/// test is what recovers it.
fn release_fire(fire: &Path) {
    if let Err(error) = std::fs::remove_file(fire) {
        eprintln!(
            "pns nag: the fire claim {} could not be given up ({error}); the next fire waits it out",
            fire.display()
        );
    }
}

/// Where one answered marker lives. The daemon owns the directory and resolves
/// the NAME inside it; this is the same resolution for the two writers that are
/// not the daemon.
fn marker_path(state: &Path, marker: &str) -> std::path::PathBuf {
    pns::daemon::marker_dir(state).join(marker)
}

/// One answered marker written: empty, 0600, and present is the whole message.
fn write_marker(state: &Path, marker: &str) -> std::io::Result<()> {
    let path = marker_path(state, marker);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(STATE_FILE_MODE)
        .open(&path)?;
    // AND AGAIN AFTER THE OPEN, for `publish_state_line`'s reason: `mode`
    // applies only when the open CREATES the file, and a marker left by an
    // earlier arm in this session is reused rather than made.
    file.set_permissions(std::fs::Permissions::from_mode(STATE_FILE_MODE))
}

/// How long an unanswered approval waits before it is carded again, or
/// `NAG_OFF`.
///
/// AN UNREADABLE CONFIG MEANS OFF, which is `focus_silence`'s reading and for
/// the same reason: a file nobody can parse asked for nothing, and a feature
/// that INTERRUPTS must not be switched on by a parse failure. This
/// deliberately differs from `[recap]`, whose fallback is on because it
/// delivers something the operator is owed.
fn nag_after_secs() -> u64 {
    let home = std::env::var("HOME").unwrap_or_default();
    match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config.nag_after_secs,
        _ => NAG_OFF,
    }
}

// --- the daemon -------------------------------------------------------------

/// `pns daemon <verb>`: the clock, and the two typed commands that feed it.
///
/// A BARE `pns daemon` IS A REFUSAL, per the house rule that an unknown
/// argument never falls through to help with exit 0: a verb this does not serve
/// is a command the operator believes ran.
fn daemon_mode(verb: &str) -> i32 {
    match verb {
        "run" => daemon_run(),
        "schedule" => daemon_schedule(),
        "cancel" => daemon_cancel(),
        _ => {
            eprintln!("{DAEMON_USAGE}");
            2
        }
    }
}

const DAEMON_USAGE: &str = "pns: usage: pns daemon run | \
pns daemon schedule --id <id> [--in <secs>] [--every <secs>] [--until +<secs>|<epoch>] \
[--unless-marker <name>] -- <event args> | \
pns daemon cancel --id <id>";

fn lights_mode(verb: &str) -> i32 {
    match verb {
        "tick" => lights_tick(),
        "quiet" => lights_quiet(),
        // UNKNOWN IS AN ERROR, never a silent fallthrough. Argv parsing on the
        // event path is deliberately lenient, so a bare `pns lights` reaching
        // it would skip the word it did not know and fire a notification about
        // an empty event.
        _ => {
            eprintln!("{LIGHTS_USAGE}");
            2
        }
    }
}

const LIGHTS_USAGE: &str = "pns: usage: pns lights tick | \
pns lights quiet [<place> [<duration>|off]]";

/// `pns loop begin|end`: take the loop lamp by hand, and give it back.
///
/// THE LEASE IS THE SECOND TRIGGER, beside the automatic one, and it exists for
/// work whose length nothing can measure in advance: an overnight run is a loop
/// from the moment it starts, not once it has been going five minutes.
///
/// IT WRITES A FILE AND REGISTERS THE TICK. The tick is what reads the lease,
/// and its own lease is refreshed by EVENT traffic: a lease taken by hand in a
/// pane that then goes quiet for an hour would be read by nobody, because the
/// tick would have expired minutes into the run it was taken for. A daemon that
/// is down still means the lamp simply does not light, and `pns loop end` on a
/// machine that never began is a removal of a file that is not there.
fn loop_mode(verb: &str) -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(3)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let command = match pns::lights::loop_command(
        verb,
        &arguments,
        std::env::var("HERDR_PANE_ID").ok().as_deref(),
    ) {
        Ok(command) => command,
        Err(refusal) => {
            eprintln!("{refusal}");
            return 2;
        }
    };
    let state = state_dir();
    match command {
        pns::lights::LoopCommand::Begin(pane) => {
            // NO CLOCK IS NO LEASE, never a lease at epoch zero: the timeout is
            // measured against this number, and a zero would be expired the
            // moment it was written.
            let (Some(marker), Some(now)) = (pns::lights::lease_marker(&state, &pane), now_secs())
            else {
                eprintln!("pns: loop: the clock cannot be read; the lease was not taken");
                return 1;
            };
            if let Err(error) = publish_state_line(&marker, &now.to_string()) {
                // LOUD, because a human is waiting on the answer: a lease that
                // was not taken is a lamp that never lights, and reporting
                // success for one is the worst outcome available.
                eprintln!("pns: loop: the lease could not be written: {error}");
                return 1;
            }
            // AND THE TICK IS REGISTERED FOR THE WHOLE LEASE, because nothing
            // else will register it in time. The tick's own lease is refreshed
            // by EVENT traffic, so a lease taken by hand in a pane that then
            // goes quiet, which is exactly the overnight run this verb exists
            // for, would be read by a tick that expired minutes into it.
            let home = std::env::var("HOME").unwrap_or_default();
            if let Ok(LoadOutcome::Loaded(config)) = load_config(&config_path(&home))
                && let Some(lights) = config.lights.as_deref()
            {
                schedule_lights_tick(&state, lights, now, lights.looping.lease_timeout_secs);
            }
        }
        pns::lights::LoopCommand::End(pane) => {
            if let Err(refusal) = end_lease(&state, &pane) {
                eprintln!("{refusal}");
                return 1;
            }
        }
    }
    0
}

/// Give a lease back, or say why it could not be given back.
///
/// LOUD, because a human is waiting on the answer and the lamp is a liveness
/// signal: reporting that a loop has ended while its lease is still on disk
/// leaves the loop lamp breathing for the whole timeout with nothing behind it,
/// and the operator has been told the opposite.
///
/// A LEASE THAT IS NOT THERE IS NOT A FAILURE. `pns loop end` on a machine that
/// never began, or a second one after the first, is a removal of a file that is
/// already gone, which is exactly the state the command is for.
fn end_lease(state: &Path, pane: &str) -> Result<(), String> {
    let Some(marker) = pns::lights::lease_marker(state, pane) else {
        return Ok(());
    };
    match std::fs::remove_file(&marker) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "pns: loop: the lease could not be given back ({error}); the loop lamp \
             keeps breathing until it times out"
        )),
    }
}

/// Renew the lease this pane holds, if it holds one.
///
/// THE PANE'S ORDINARY HOOK TRAFFIC IS THE RENEWAL, which is what makes the
/// lease a liveness signal rather than a timer: an agent that is still working
/// is still firing events from its own pane, and one that stopped stops
/// renewing. Nothing else in this crate renews it.
///
/// IT CREATES NOTHING, and that is a property of the WRITE rather than of a
/// check in front of one. The open states no `create`, so the file has to be
/// there already, and the bytes go through the HANDLE rather than through a
/// fresh file renamed over the path: a `pns loop end` that lands after the open
/// sends these bytes to an inode nobody can reach any more, where a look-then-
/// publish would have written the lease back into existence and left the lamp
/// breathing for a whole timeout over work that had finished.
///
/// IT WRITES IN PLACE RATHER THAN TRUNCATING FIRST, so a tick reading the file
/// mid-renewal cannot see an empty one and sweep the lease. Both epochs are ten
/// digits and will be for the next two centuries, so a read caught between the
/// two sees a mix of two same-length numbers, which is a second or two out
/// rather than a lease nobody can parse. The `set_len` after the write is for
/// the day that stops being true.
fn renew_loop_lease(state: &Path, pane: &str, now: Option<u64>) {
    let (Some(marker), Some(now)) = (pns::lights::lease_marker(state, pane), now) else {
        return;
    };
    // The failures are DROPPED here: a lease that did not renew costs the lamp
    // one timeout, and this process has no reader for a complaint.
    let line = format!("{now}\n");
    if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(&marker)
        && file.write_all(line.as_bytes()).is_ok()
    {
        let _ = file.set_len(line.len() as u64);
    }
}

/// Every live lease's epoch, with the ones past the timeout REMOVED on the way
/// through.
///
/// THE SWEEP LIVES WITH THE READ, for `sweep_blocked`'s reason: the tick is the
/// only process that ever looks in this directory, and a pane that ends without
/// `pns loop end` leaves a file nothing else would remove.
fn sweep_leases(state: &Path, now: u64, timeout_secs: u64) -> Vec<u64> {
    sweep_markers(&pns::lights::lease_dir(state), now, timeout_secs)
}

/// Every live epoch one marker directory holds, with everything past the bound
/// REMOVED on the way through.
///
/// ONE SWEEP FOR THE WAITS AND THE LEASES, because they are one mechanism twice:
/// a directory of one-epoch files, a bound, and a tick that is the only process
/// that ever looks. Written twice, the second copy is where the race fix, the
/// working-file rule and the collection of what a dead run left behind would
/// each have to be remembered a second time.
///
/// A REMOVAL IS OWNED BY RENAME AND NEVER READ-THEN-UNLINK. Concurrent unlink
/// does not arbitrate on this filesystem (see
/// `docs/decisions/0001-ownership-by-rename-not-by-unlink.md`), so a sweep that
/// read an expired epoch and then unlinked could delete a FRESH marker a racing
/// event had published in between. Taking the file by rename first means what
/// this removes is what this took, and the epoch is READ AGAIN off the claim: a
/// marker that turned out to be live in the meantime is put back rather than
/// destroyed.
///
/// THE LIVE PATH TOUCHES NOTHING, which is what keeps that safety free. A
/// marker still inside its bound is read and left exactly where it is, so the
/// ordinary tick renames nothing at all.
///
/// A PUT-BACK CAN OVERWRITE A NEWER PUBLISH, and that is the residue rather than
/// a rule: the epoch restored is live and at most one racing publish old, which
/// is seconds against bounds measured in hours.
///
/// A MARKER ALREADY NAMED FOR THE WORKING GRAMMAR IS A RESIDUAL, not a case
/// this handles: `pane_file_is_safe` and `session_id_is_safe` refuse a NEW id
/// `working_owner` would read as a working file, but a marker written under one
/// before that guard existed is read here as that pid's own working file
/// (`owner_is_gone` judges it, never `marker_is_live`), so it neither lights a
/// lamp nor ages out. No id this crate's own callers produce can spell the
/// shape (a UUID session id and a `wW:p21` pane cannot).
///
/// THE SHAPE IS `working_owner`'S, NOT `.new.<digits>` ALONE, which is what the
/// operator check has to match: the RIGHTMOST of `.new.` and `.sweep.` decides,
/// so `s.sweep.7` and a mixed `a.new.b.sweep.1` are residuals exactly as
/// `s.new.4321` is, and `a.new.b` (no pid after the last marker) is an ordinary
/// marker that sweeps normally. The check is therefore
/// `ls ~/.local/state/pns/lights-blocked ~/.local/state/pns/lights-loop` for any
/// name whose last `.new.` or `.sweep.` is followed by digits alone, removed by
/// hand.
///
/// AND THE SWEEP IS NOT WEAKENED TO REACH IT, which is a statement about this
/// function rather than a claim that the residual gets collected: while the pid
/// in the name belongs to a LIVE process it is never swept at all, and pid 1 is
/// launchd, so that name in particular is permanent until the operator removes
/// it. A code fix was weighed and refused. Sweeping a working file whose owner
/// is alive is the one thing this must never do, because it unlinks a publish
/// caught between its open and its rename and loses a wait with the agent still
/// waiting; and moving working files to a directory of their own is a state
/// layout migration that leaves the same legacy names behind at the other end.
/// The residual costs one stale file per legacy name and never grows, which is
/// less than either fix.
fn sweep_markers(directory: &Path, now: u64, max_age_secs: u64) -> Vec<u64> {
    let mut live = Vec::new();
    for entry in std::fs::read_dir(directory).into_iter().flatten().flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // A WORKING FILE IS NOT A MARKER, and one whose run is GONE is litter
        // nothing else collects. A publish caught between its open and its
        // rename has no epoch in it yet, and unlinking it there wins the race
        // against the rename, which then publishes nothing: the wait is lost
        // with the agent still waiting on the operator.
        if let Some(owner) = pns::lights::working_owner(&name) {
            if owner_is_gone(owner) {
                let _ = std::fs::remove_file(&path);
            }
            continue;
        }
        if let Some(at) = read_epoch(&path)
            && pns::lights::marker_is_live(at, now, max_age_secs)
        {
            live.push(at);
            continue;
        }
        // EXPIRED, OR AN EPOCH NOBODY CAN READ, which is swept for the same
        // reason: nothing can ever age out a file whose epoch is unreadable, so
        // leaving it is the same unbounded growth through a different door.
        let claim = pns::lights::sweep_claim(directory, &name, std::process::id());
        if std::fs::rename(&path, &claim).is_err() {
            continue;
        }
        match read_epoch(&claim) {
            // IT CAME BACK LIVE, so a fresh publish landed between the read and
            // the claim and this run is holding it. Put it back.
            Some(at) if pns::lights::marker_is_live(at, now, max_age_secs) => {
                live.push(at);
                if std::fs::rename(&claim, &path).is_err() {
                    let _ = std::fs::remove_file(&claim);
                }
            }
            _ => {
                let _ = std::fs::remove_file(&claim);
            }
        }
    }
    live
}

/// The lamps' own mute: one place, quiet for a bounded while, by hand.
///
/// LIGHTS ONLY, and that is the operator's own scope: cards, banners, the
/// durable log and `pns quiet` are untouched, so an agent that needs an answer
/// still reaches the phone while the bedroom lamp stays out of it. The two
/// mutes share a duration parser and nothing else, and neither reads the
/// other's file.
///
/// FAIL OPEN AT EVERY TURN, which is `quiet.rs`'s direction rather than the
/// window's: a state file nobody can parse mutes NOTHING and says so, because a
/// lights mute the operator cannot see is worse than a lamp that flashed.
///
/// THE READ-MODIFY-WRITE RACE IS REAL AND ACCEPTED. This is hand-typed, so two
/// runs racing means an operator typing two commands in the same second, and
/// the loser is one mute they can see is missing and retype. A lock between two
/// interactive commands would be a mechanism with no reader.
fn lights_quiet() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(3)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    let known = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => config
            .lights
            .as_deref()
            .map(|lights| mutable_names(lights, config, &arguments))
            .unwrap_or_default(),
        // A CONFIG THIS CANNOT READ NAMES NO PLACE, so every mute is refused by
        // name rather than stored against a map nobody could load. The report
        // still runs, which is what an operator with a broken config needs from
        // this command first.
        _ => Vec::new(),
    };
    let state = state_dir();
    let now = now_secs();
    // HOW LONG A BARE MUTE LASTS, off the operator's OWN schedule rather than
    // any one room's dim window: a mute typed at bedtime is about their night.
    // A window nobody can parse states no schedule, which the refusal covers.
    let until_quiet_ends = pns::lights::bare_mute_secs(
        match &loaded {
            Ok(LoadOutcome::Loaded(config)) => enabled_hue_table(config)
                .and_then(|settings| quiet_window(&settings).ok().flatten())
                .map(|window| window.ends_at()),
            _ => None,
        },
        now.and_then(local_minutes_since_midnight),
    );
    let command = match pns::lights::quiet_command(&arguments, &known, until_quiet_ends) {
        Ok(command) => command,
        Err(refusal) => {
            eprintln!("{refusal}");
            eprintln!("{LIGHTS_USAGE}");
            return 2;
        }
    };
    let (entries, complaints) = muted_state(&state);
    // SAID BEFORE ANYTHING IS WRITTEN, because the write below republishes the
    // whole file: an operator whose file was unreadable is losing whatever it
    // held, and that is a line they get to see rather than a silent repair.
    for complaint in &complaints {
        eprintln!("{complaint}");
    }
    let rebuilt = match &command {
        pns::lights::QuietCommand::Report => Ok(entries.clone()),
        pns::lights::QuietCommand::Unmute { place } => {
            pns::lights::muted_after(&entries, place, None, now)
        }
        pns::lights::QuietCommand::Mute { place, seconds } => {
            match now.map(|now| now.saturating_add(*seconds)) {
                Some(expiry) => pns::lights::muted_after(&entries, place, Some(expiry), now),
                // THE CLOCK IS WHAT A MUTE IS MADE OF, so a run that cannot
                // read one says the mute was not set rather than writing an
                // expiry it guessed. `pns quiet`'s own wording, one file over.
                None => Err(
                    "pns: state error (the clock cannot be read); the mute was not set".to_string(),
                ),
            }
        }
    };
    // A REFUSED REBUILD IS A MUTE THAT WAS NOT SET, and nothing is written or
    // reported after one: the file on disk is exactly what it was, and a report
    // built from a list this run refused to publish would describe a house that
    // does not exist.
    let kept = match rebuilt {
        Ok(kept) => kept,
        Err(refusal) => {
            eprintln!("{refusal}");
            return 1;
        }
    };
    if !matches!(command, pns::lights::QuietCommand::Report)
        && let Err(error) = publish_muted(&state.join(LIGHTS_QUIET), &kept)
    {
        // LOUD, because a human is waiting on the answer: reporting a mute that
        // is not in effect is the worst outcome available.
        eprintln!(
            "pns: state error (lights-quiet could not be written: {error}); \
             the mute was not set"
        );
        // AND NO REPORT AFTER IT. `kept` is what the file WOULD have held: for
        // a failed mute it would say the place is quiet when it is not, and for
        // a failed `off` it would say nothing is quiet while the old mute is
        // still on disk and still taking the lamp. The disk is the answer and
        // this run did not change it.
        return 1;
    }
    for line in pns::lights::muted_report(&kept, now) {
        println!("{line}");
    }
    0
}

/// Every name `pns lights quiet` will take, for the command as it was typed.
///
/// THE GRAMMAR IS LAMP, ROOM AND ZONE, which are the BRIDGE'S nouns as much as
/// the config's: a lamp that inherits its room's declaration has a real name no
/// declaration writes, and refusing it sends the operator away from the room
/// they are standing in. So the bridge's own listing widens the vocabulary.
///
/// AND THE DIAL IS ON THE MISS PATH ALONE. A place a declaration already holds
/// is a name a mute can enforce whatever the bridge says, so the ordinary
/// command, muting a room the config routes, costs no network at all. Only a
/// word neither this run's declarations nor `off` can account for is worth
/// asking a bridge about, and `off` is allowed over any name because it can
/// only remove.
fn mutable_names(
    lights: &pns::config::Lights,
    config: &pns::config::Config,
    arguments: &[String],
) -> Vec<String> {
    let declared = pns::channels::hue::mutable_names(lights, None);
    if !asks_the_bridge(&declared, arguments) {
        return declared;
    }
    pns::channels::hue::mutable_names(lights, bridge_inventory(config).as_ref())
}

/// Whether the typed command holds a word only a bridge listing could account
/// for.
///
/// THE FIRST ARGUMENT IS THE PLACE in every form that names one (`<place>`,
/// `<place> <duration>`, `<place> off`), and the bare report names none. A
/// second word of `off` needs no listing either: `off` is allowed over any
/// name, because it can only remove a mute the operator can see.
fn asks_the_bridge(declared: &[String], arguments: &[String]) -> bool {
    arguments.first().is_some_and(|place| {
        !declared.contains(place) && arguments.get(1).is_none_or(|word| word != "off")
    })
}

/// What the bridge says it holds, or nothing at all.
///
/// A BRIDGE THAT ANSWERS NOTHING IS NOT A REFUSAL. The declarations are still
/// names a mute can enforce once the transport is back, so the command works
/// with the bridge down at the cost of a narrower vocabulary.
fn bridge_inventory(config: &pns::config::Config) -> Option<pns::channels::hue::Inventory> {
    let settings = enabled_hue_table(config)?;
    let hue = hue_settings(&settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())?;
    // THE HUMAN'S OWN DEADLINE, not the transport's. Nothing else here dials a
    // bridge with somebody standing at a terminal waiting on the answer, and
    // three calls at the transport's ten seconds is half a minute before a mute
    // typed at bedtime says anything at all. A bridge on the same LAN answers
    // these in milliseconds, so a second apiece is generous; past it the
    // vocabulary narrows to the declarations, which is what a bridge that
    // answered nothing leaves anyway.
    let bridge = UreqBridge {
        base: format!("https://{}/clip/v2/resource", hue.bridge),
        key: hue.key,
        deadline: TYPED_COMMAND_DEADLINE,
    };
    Some(pns::channels::hue::inventory(
        &pns::channels::hue::Bridge::get(&bridge, "room")?,
        &pns::channels::hue::Bridge::get(&bridge, "light")?,
        &pns::channels::hue::Bridge::get(&bridge, "zone")?,
    ))
}

/// Publish the file, or remove it when nothing is muted.
///
/// AN EMPTY FILE IS NO FILE, which is `remember_held`'s own rule and is
/// what keeps the reader's refusal of an empty one honest: this never writes
/// one, so a file with no lines in it was written by something else.
fn publish_muted(state: &Path, kept: &[pns::lights::Muted]) -> std::io::Result<()> {
    if kept.is_empty() {
        return match std::fs::remove_file(state) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    publish_state_line(state, &pns::lights::render_muted(kept))
}

/// Everything the ad-hoc quiet file holds, and the complaint from a file this
/// cannot vouch for.
///
/// ONE READER FOR BOTH READERS, which is why the command and the event path
/// share it: they want different things out of the file (the entries to rebuild
/// and the names that are live), and two readers is two chances for one of them
/// to swallow a failure the other reports.
///
/// A MISSING FILE IS THE ORDINARY CASE and says nothing: the command has
/// never been run, or its last mute expired and took the file with it. EVERY
/// OTHER READ FAILURE IS A COMPLAINT, and the distinction is the point: a file
/// that is unreadable, not UTF-8, or a directory standing where it should be
/// says NOTHING about which places are quiet, exactly as a corrupt one does,
/// and the two readers of that complaint take opposite directions with it.
/// `ad_hoc_quiet` mutes EVERYTHING (a lamp path fails dark), and the command
/// prints it and rebuilds from an empty list. Either way the operator is told,
/// which is what a complaint is for: a mute nobody can see, in either
/// direction, is the state worth a sentence.
fn muted_state(state: &Path) -> (Vec<pns::lights::Muted>, Vec<String>) {
    let contents = match std::fs::read_to_string(state.join(LIGHTS_QUIET)) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Vec::new(), Vec::new());
        }
        Err(error) => {
            return (
                Vec::new(),
                vec![format!(
                    "pns: state error (lights-quiet could not be read: {error}); \
                     nothing is quiet"
                )],
            );
        }
    };
    match pns::lights::muted_entries(&contents) {
        Ok(entries) => (entries, Vec::new()),
        Err(complaint) => (Vec::new(), vec![complaint]),
    }
}

/// What an ad-hoc quiet is muting right now, and that same complaint.
///
/// A READING THIS CANNOT TAKE MUTES EVERYTHING, which is the fail direction
/// every lamp-path input takes and the OPPOSITE of what both halves used to do.
/// A record nobody can parse and a clock nobody can read each answered with an
/// empty list, which is a house with every lamp loud: exactly the 3am the mute
/// was armed to prevent, on the one night the machine could not tell anybody
/// why.
///
/// THE COMPLAINT IS STILL THE OTHER HALF. Going dark silently would be a lamp
/// that stopped working for a reason nobody can see, so the caller says it
/// once through `say_lights_once` and the state is repaired by the next
/// `pns lights quiet` write, which republishes the whole file.
fn ad_hoc_quiet(state: &Path, now: Option<u64>) -> (pns::channels::hue::Muting, Vec<String>) {
    let (entries, complaints) = muted_state(state);
    if !complaints.is_empty() {
        return (pns::channels::hue::Muting::Everything, complaints);
    }
    let Some(now) = now else {
        return (
            pns::channels::hue::Muting::Everything,
            vec![pns::lights::NO_CLOCK_FOR_THE_MUTE.to_string()],
        );
    };
    (
        pns::channels::hue::Muting::Places(pns::lights::muted_places(&entries, Some(now))),
        complaints,
    )
}

/// One upkeep pass: read the machine, derive the one state the house is in,
/// and write it to every lamp that should show it.
///
/// EXIT 0 ON EVERY PATH, and SILENT on every happy one. This runs three times
/// a minute forever under a daemon nobody is watching, so a line per tick is a
/// log the rotation job then rotates a real log out of.
///
/// EVERY STATE IS RE-DERIVED FROM SCRATCH. Nothing is carried between runs
/// except what is on disk, which is the daemon's own rule: this process exists
/// for a fraction of a second and the next one is a different process
/// entirely.
///
/// THE JOURNAL IS READ AND NEVER CLAIMED. `claim_journal` is how the replay
/// CONSUMES a queue; a tick that claimed it would delete the misses the
/// operator has not seen yet, which is the opposite of what the glow is for.
fn lights_tick() -> i32 {
    let home = std::env::var("HOME").unwrap_or_default();
    // AN UNREADABLE CONFIG ASKED FOR NOTHING, which is the same reading the
    // event path takes of the lamps one function over: a file nobody could
    // parse routed no lamp, and a map this could not read must not be replaced
    // with a guess about which lamps carry what.
    let Ok(LoadOutcome::Loaded(config)) = load_config(&config_path(&home)) else {
        return 0;
    };
    // NO BRIDGE NAMED IS NO CLEAR EITHER, so held lamps KEEP their record here.
    // Hue switched off, or absent, is a machine this process cannot reach a
    // lamp on at all; forgetting the record would leave the lamp lit with
    // nothing in the system that knows about it, and the operator with the wall
    // switch. Keeping it means the tick that follows the switch going back on
    // still has a name to write the clear to.
    let Some(settings) = enabled_hue_table(&config) else {
        return 0;
    };
    // THE FEATURE BEING OFF STILL PUTS A HELD LAMP OUT. `[lights]` removed, or a
    // clock this machine cannot read, is a tick that can arm nothing; the
    // bridge above is still named, so the one thing it can still do is put out
    // what the last tick was holding and forget it.
    // ONE PROBE SET FOR THE WHOLE TICK, and its clock is the tick's `now`.
    // Every reading the narrowing takes below comes off this set's memoized
    // cells, so the reading, the age it is judged by and the moment the
    // decision is stamped with cannot straddle a boundary.
    let probes = system_probes();
    let (Some(lights), Some(now)) = (config.lights.as_deref(), probes.now_secs()) else {
        clear_held_lamps(Some(&settings));
        return 0;
    };
    // AND CREDENTIALS THAT ARE GONE KEEP THE RECORD for the reason the hue
    // switch does: nothing here can address a lamp.
    let Some(hue) = hue_settings(&settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())
    else {
        return 0;
    };
    let state = state_dir();
    sweep_legacy_state(&state);
    let standing = lights_house(&state, lights, now);
    let (muted, mut complaints) = ad_hoc_quiet(&state, Some(now));
    // A RECORD THIS CANNOT READ NAMES NOTHING TO CLEAR, and the tick is its
    // only writer, so it goes on: the pass below publishes the record it
    // derived, which is what repairs the file. The residue is stated: a lamp
    // held under a name this run could not read stays lit until the repaired
    // record names it again or the operator's next return clears it.
    // ONE READ FOR BOTH THE BARE GATE AND THE PHASE A RESUMED BREATH NEEDS,
    // rather than two: `held_lamps` is `read_held` with the phase dropped, and
    // reading the record twice here would be two disk reads of one fact this
    // tick only ever reads once.
    let held_before_entries = read_held(&state);
    let held_before: Option<Vec<String>> = held_before_entries
        .as_deref()
        .map(|entries| entries.iter().map(|entry| entry.path.clone()).collect());
    if held_before.is_none() {
        complaints.push(HELD_RECORD_UNREADABLE.to_string());
    }
    let active = pns::lights::active_held(&standing.house);
    // NOTHING TO LIGHT AND NOTHING TO PUT OUT IS NO BRIDGE CALL AT ALL, which
    // is what keeps an idle machine off the network several times a minute.
    //
    // THE GATE IS THE HOUSE STATE ALONE, and that is a deliberate narrowing from
    // the shipped one. The old gate also asked whether any place could be awake,
    // which took the quiet-hours chain out of the config with no bridge listing
    // to judge it against and paid for it with two stated limits; the dim window
    // is now a per-lamp answer that needs the listing anyway, so the cheap half
    // of that question no longer exists. A house holding nothing still costs
    // nothing, which is the case that matters.
    if !active.is_empty() || held_before.as_deref().is_none_or(|held| !held.is_empty()) {
        // THE ONE MONOTONIC CLOCK THE WHOLE TICK IS MEASURED ON, started here
        // and read by nothing else: the resolve's cost, every fade's due
        // millisecond and the moment each write actually happened are all
        // offsets from this instant, so they can never disagree about when the
        // tick began. It is a parameter for the reason the sleeper is one: the
        // driver fills its whole interval by design, so a test that read the
        // real clock would live the interval too.
        let started = std::time::Instant::now();
        complaints.extend(run_tick_writes(
            &UreqBridge {
                base: format!("https://{}/clip/v2/resource", hue.bridge),
                key: hue.key,
                // THE CHILD IS BOUNDED BY ITS OWN INTERVAL, and the resolve is
                // the part of it that is not this process's to shorten: three
                // calls at the transport's ten seconds outlive every interval
                // the config permits, so a wedged bridge would have tick after
                // tick piling up, each one still dialling. A quarter of the
                // interval apiece leaves the fades the rest of it, and a bridge
                // on the same LAN answers these in milliseconds.
                deadline: tick_bridge_deadline(lights.refresh_secs),
            },
            &state,
            lights,
            &active,
            &pns::channels::hue::Reading {
                minutes_now: local_minutes_since_midnight(now),
                muted: &muted,
            },
            held_before_entries.as_deref(),
            now.saturating_mul(1000),
            // THE TICK TAKES ITS OWN READINGS, because it decides no event and
            // so inherits nobody's, and it takes them off the ONE probe set
            // this tick built, whose clock read is the `now` above.
            presence_snapshot(
                pns::config::parse_presence(&config).ok().flatten().as_ref(),
                &probes,
                pns::probes::IdleProbe::idle_secs(&probes),
                pns::probes::ScreenLockProbe::screen_locked(&probes),
                home_presence(),
            )
            .as_ref(),
            || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            std::thread::sleep,
        ));
    }
    // AND THE SAYING IS OUTSIDE THAT GATE, deliberately. `say` FORGETS a
    // complaint that has cleared, and a complaint clears exactly when the house
    // goes dark; leaving the bookkeeping inside the gate meant a remembered
    // complaint was never forgotten on the tick that ended it, so the same
    // complaint returning later would not read as news.
    say_lights_once(&state, &complaints, LIGHTS_SAID);
    // AND THE TICK KEEPS ITSELF ALIVE while anything could still light a lamp.
    // Its lease was refreshed by EVENTS alone, which reaches only the states an
    // event ARRIVES with: a shell command produces no events at all, and the
    // automatic loop trigger is five minutes by default and six on the
    // operator's own machine, both PAST the five-minute lease an event leaves.
    // So the one lamp whose whole job is a long run could never arm itself, and
    // a lease taken by hand in a pane that then went quiet expired unread.
    //
    // IT IS STILL BOUNDED BY THE CONDITION, not a self-perpetuating job: a
    // house holding nothing with no run and no lease renews nothing, so an idle
    // machine's tick lapses exactly as it did.
    if !active.is_empty() || standing.in_flight {
        schedule_lights_tick(&state, lights, now, ORDINARY_LEASE_SECS);
    }
    0
}

/// One lamp's breath for this tick: what to send, and where in its own
/// schedule it resumes.
///
/// A NAMED STRUCT, NOT A TUPLE, once a fourth field (`resume`) joined the
/// three the routing loop already carried: a positional fourth slot is a
/// silent transposition waiting to happen, and every field here already has
/// a name at its own call site.
struct Breathing {
    path: String,
    /// THE STATE THIS BREATH IS SHOWING, carried alongside the shape and the
    /// colour it selected rather than derived back out of them: it is what the
    /// phase is recorded under, and what the next tick compares its own state
    /// against before it resumes anything.
    held: pns::lights::Held,
    /// The legs this lamp fades between, in order. WHICH SHAPE THEY CAME FROM
    /// IS ALREADY SETTLED (`held_render`), so the driver schedules a two-leg
    /// breath and the loop's three-leg motion through one path.
    cycle: Vec<pns::lights::Leg>,
    color: pns::pulse::PulseColor,
    resume: pns::lights::Resume,
}

/// The tick's writes, in the ONE ORDER that cannot leave a lamp lit and
/// unaccounted for: arm every lamp, clear only what the arm did not write to,
/// record what is held (bare), breathe, and only then record the phase each
/// lamp landed on.
///
/// THE ORDER IS THE BEHAVIOUR, which is why these are one function rather than
/// five lines at the bottom of the tick. Every held body is a plain state write
/// that does NOT expire, so a clear computed before the arm, or a record written
/// before the clear, is a lamp left lit with nothing that knows its name.
///
/// THE PRE-ARM WRITE IS BARE, deliberately dropping any phase this tick read:
/// it is what a killed child leaves behind, and a killed child cannot finish a
/// fade, so a bare token is a lamp this run cannot promise landed anywhere in
/// particular. The PHASE is a SECOND write, after the breath returns, guarded
/// by a re-read of the SAME bare list this tick's own pre-arm write left: a
/// return that cleared the record mid-breath already emptied it, and writing
/// the phase over that would resurrect a hold the operator just ended.
///
/// THE BREATHING RUNS LAST AND HOLDS THIS PROCESS OPEN until the last fade has
/// been ISSUED, one seamless turn-around's lead before the budget ends. That is
/// what makes the lamp a liveness signal: the fades are issued by this process
/// on a cadence, so a daemon that dies, a machine that sleeps and a pns that
/// crashes all stop the motion within one interval. The record and the clear are
/// already on disk before the first sleep, so a driver killed mid-breath costs a
/// lamp frozen at its last brightness and never a lamp nothing can put out.
///
/// AND THE CHILD IS GONE BEFORE THE NEXT TICK'S CHILD RUNS, which the daemon's
/// own `running` check enforces rather than the schedule alone: the last fade
/// is routinely still running on the bridge when this budget ends (that is the
/// seamless join), so a write that overran its lead can no longer be met by a
/// second child. The tick's own lock is the half of that the daemon cannot
/// see, and it covers a tick run by hand and an orphan a daemon replacement
/// left behind.
///
/// A BRIDGE THAT ANSWERED NO LISTING CHANGES NOTHING AT ALL. It is direct
/// evidence the transport is down, and both halves of acting on it are wrong: a
/// clear it refused is invisible, and forgetting the paths after it leaves the
/// lamp lit with nothing in the system that knows about it.
///
/// IT PRINTS NOTHING. The complaints are answered for the caller to say once.
#[allow(clippy::too_many_arguments)]
fn run_tick_writes<B: pns::channels::hue::Bridge>(
    bridge: &B,
    state: &Path,
    lights: &pns::config::Lights,
    active: &[pns::lights::Held],
    reading: &pns::channels::hue::Reading<'_>,
    held_before: Option<&[pns::lights::HeldEntry]>,
    now_ms: u64,
    presence: Option<&pns::presence_policy::Snapshot>,
    mut elapsed_ms: impl FnMut() -> u64,
    sleep: impl FnMut(Duration),
) -> Vec<String> {
    let mut complaints = Vec::new();
    // ONE TICK DRIVES THE HOUSE AT A TIME. Taken before the resolve rather than
    // around the record alone, because two ticks that both got past a record
    // comparison would still spend a whole interval issuing fades at each
    // other. The second returns having done nothing at all, which is what a
    // tick with nothing to say has always returned.
    //
    // `now_ms / 1000` IS THE SECOND THE CALLER IS ON: production hands this
    // function the wall clock in milliseconds, and the age rule compares that
    // against the lock file's own mtime.
    let lock = state.join(LIGHTS_TICK_LOCK);
    if !claim_lock(&lock, now_ms / 1000, lights_tick_stale_secs()) {
        return complaints;
    }
    let _lock = HeldLock(lock);
    let mut breathing: Vec<Breathing> = Vec::new();
    if !active.is_empty() {
        // The doctor is where an unreachable bridge is REPORTED; this process
        // runs unattended and has no reader for that sentence.
        let Some(mut routing) = pns::channels::hue::resolve_on_bridge(bridge, lights) else {
            return complaints;
        };
        // OFF THE WHOLE RESOLUTION, before the narrowing, for
        // `run_pulse_writes`'s reason: a name the bridge could not answer is a
        // typo whether or not the operator is standing in that room.
        complaints.extend(routing_complaints(&routing));
        // WHAT THIS TICK WOULD ACTUALLY ARM, in `run_pulse_writes`'s shape and
        // for its reason: presence has to narrow over the lamps this house
        // state reaches, or a room whose only lamp carries some other state
        // reads as narrowable and then breathes on nothing.
        let breath_for = |routed: &pns::channels::hue::Routed| -> Option<(
            pns::lights::Held,
            pns::channels::hue::Showing,
        )> {
            if pns::channels::hue::muted_now(&routed.lamp, reading.muted) {
                return None;
            }
            let held = pns::lights::shown(active, &routed.shows)?;
            let showing = pns::channels::hue::dim_showing(
                routed.dim.as_ref(),
                held.behaviour(),
                reading.minutes_now,
            );
            (showing != pns::channels::hue::Showing::Dark).then_some((held, showing))
        };
        routing.lamps.retain(|routed| breath_for(routed).is_some());
        let routing = narrow_to_presence(state, routing, presence);
        for routed in &routing.lamps {
            let Some((held, showing)) = breath_for(routed) else {
                continue;
            };
            let (color, cycle) = pns::channels::hue::held_render(held, lights, showing);
            let path = pns::channels::hue::Fixture::Light(routed.lamp.id.clone()).path();
            // A LAMP NOT NAMED IN LAST TICK'S RECORD, OR NAMED THERE WITH NO
            // PHASE, RESUMES AT THE DEFAULT: a fresh arm, an external switch,
            // a killed child's bare token and a dim-window shape change all
            // read the same way, and all cost at most one fade of motion.
            let previous =
                held_before.and_then(|entries| entries.iter().find(|entry| entry.path == path));
            let resume = pns::lights::resume_from(previous, now_ms, held, &cycle);
            breathing.push(Breathing {
                path,
                held,
                cycle,
                color,
                resume,
            });
        }
    }
    let held_before_bare: Option<Vec<String>> =
        held_before.map(|entries| entries.iter().map(|entry| entry.path.clone()).collect());
    // THE RECORD IS READ AGAIN BEFORE ANYTHING IS WRITTEN, and this run stands
    // down if it moved. The states above were derived BEFORE the bridge work,
    // which is seconds of network, and the event path clears every held lamp and
    // empties this record the moment the operator comes back: a tick still
    // resolving when that happened would arm the lamps again off a snapshot
    // taken before the clear, and the operator would watch a lamp they had just
    // put out come back on. The other writer has already done the clearing, so
    // there is nothing left here to do.
    if held_lamps(state).as_deref() != held_before_bare.as_deref() {
        return complaints;
    }
    let held_now: Vec<String> = breathing.iter().map(|entry| entry.path.clone()).collect();
    // WHATEVER WAS HELD AND IS NOT HELD NOW GETS PUT OUT BY NAME. Written as a
    // difference rather than as a special case, so a lamp dropped by a dim
    // window, a mute, a config edit or the condition simply ending is covered by
    // one line rather than four.
    let stale: Vec<String> = held_before_bare
        .unwrap_or_default()
        .iter()
        .filter(|path| !held_now.contains(path))
        .cloned()
        .collect();
    pns::channels::hue::clear_held(bridge, &stale);
    // A RECORD THAT DID NOT LAND STOPS THE ARM, and that is the whole reason
    // this answer is read. Every held body is a plain state write that does not
    // expire, so arming a lamp the record does not name is a lamp nothing in
    // the system can ever put out: not the next tick, which computes its clear
    // by name off this file, not the return from an absence, and not the
    // operator's own mute. Nothing armed is one interval of a dark lamp, which
    // the next tick fixes by itself.
    let pre_arm: Vec<pns::lights::HeldEntry> = held_now
        .iter()
        .cloned()
        .map(pns::lights::HeldEntry::bare)
        .collect();
    if let Err(error) = remember_held(state, &pre_arm) {
        complaints.push(format!(
            "pns lights: the held record could not be written ({error}); no lamp \
             was armed, because nothing would have been able to put one out"
        ));
        return complaints;
    }
    // WHAT IS LEFT OF THE INTERVAL, and not the interval: the resolve above is
    // three bridge calls, and the fades have to be issued and finished inside
    // the time this child still has.
    let spent_ms = elapsed_ms();
    let budget_ms = lights
        .refresh_secs
        .saturating_mul(1000)
        .saturating_sub(spent_ms);
    let landings = drive_breaths(
        bridge,
        budget_ms,
        &breathing,
        || elapsed_ms().saturating_sub(spent_ms),
        sleep,
    );
    // THE PHASE, WRITTEN ONLY IF THE PRE-ARM LIST IS STILL THIS TICK'S OWN. A
    // return that cleared every held lamp during the breath already emptied
    // the record; resurrecting it here with a phase would hold a lamp the
    // operator just put out. A lamp whose schedule came back empty (a budget
    // too short to fit even one fade) keeps its bare, phaseless entry.
    if held_lamps(state).as_deref() == Some(held_now.as_slice()) {
        // WALKED OVER `breathing` AND NOT OVER THE BARE PATHS, because a phase
        // carries the STATE it belongs to and that is the one place still
        // holding it. The two lists are the same paths in the same order:
        // `held_now` is this one, mapped.
        let phased: Vec<pns::lights::HeldEntry> = breathing
            .iter()
            .map(|entry| {
                landings
                    .iter()
                    .find(|(landed_path, _, _)| *landed_path == entry.path)
                    // THE RESOLVE IS PART OF THE OFFSET. A landing is reported
                    // from the DRIVER's own start, which is `spent_ms` after
                    // this tick's, so a record written without that term would
                    // put every end a whole resolve early and the next tick
                    // would take the breath over before this one finished it.
                    .map(
                        |(path, landed_on, end_relative_ms)| pns::lights::HeldEntry {
                            path: path.clone(),
                            resume: Some(pns::lights::Phase {
                                end_unix_ms: now_ms + spent_ms + end_relative_ms,
                                landed_on: *landed_on,
                                held: entry.held,
                            }),
                        },
                    )
                    .unwrap_or_else(|| pns::lights::HeldEntry::bare(entry.path.clone()))
            })
            .collect();
        let _ = remember_held(state, &phased);
    }
    complaints
}

/// Issue every lamp's breath on cadence for the rest of this interval, and
/// report which end each one landed on and when.
///
/// ONE SLEEP SCHEDULE FOR EVERY LAMP, against one clock. Each fade carries the
/// millisecond it is due at, measured from this function's own start, so a lamp
/// whose write took a moment does not push every later fade of every lamp out by
/// that moment: the overshoot is absorbed rather than accumulated.
///
/// NOTHING IS ISSUED AT OR PAST THE BUDGET, and the check is made immediately
/// before each write rather than once from the schedule. Writes are synchronous
/// and sequential, so the schedule is only ever NOMINAL: four slow lamps due
/// together at 11,850ms with the first taking 150ms puts the rest of that round
/// at or past a 12,000ms budget, and issuing them anyway would hand the bridge
/// fades belonging to an interval this child no longer owns. A dropped fade
/// costs the lamp one turn-around, which the next tick resumes from; an issued
/// one costs two children writing to one lamp.
///
/// AND EVERY LANDING IS DERIVED FROM A WRITE THAT ACTUALLY HAPPENED, at the
/// moment it actually started. The phase this returns is what the next tick
/// resumes off, so a landing taken from the nominal schedule would tell that
/// tick the lamp finished moving earlier than it did, and it would take the
/// breath over early on every interval the bridge ran slow in.
///
/// IT EXITS INSIDE THE BUDGET IT IS HANDED, WITH ITS LAST FADE STILL RUNNING.
/// `breath_fades` issues that fade strictly before the budget ends and lets it
/// finish after, which is the seamless join: the fade keeps moving on the
/// bridge with no child left to interrupt it, and the caller's second held-
/// record write is what lets the next tick pick the join up where this one
/// left it. The budget is what the caller has LEFT of its interval, not the
/// interval, because the map is resolved before the first fade is issued.
///
/// A LAMP WHOSE FADES ARE ALREADY DONE SIMPLY STOPS, which is how lamps with
/// different shapes share one schedule: the blocked lamp's two-second cycles run
/// more often than the unread lamp's four-second one, and the landing each is
/// reported at is exactly the brightness its own last ISSUED fade targeted.
///
/// THE CLOCK AND THE SLEEPER ARE PARAMETERS for one reason: the driver fills its
/// whole interval BY DESIGN, so a test that read the real clock and slept for
/// real would live the interval too. The cadence a fake pair is handed is the
/// same schedule the real one runs.
fn drive_breaths<B: pns::channels::hue::Bridge>(
    bridge: &B,
    budget_ms: u64,
    breathing: &[Breathing],
    mut elapsed_ms: impl FnMut() -> u64,
    mut sleep: impl FnMut(Duration),
) -> Vec<(String, u8, u64)> {
    // (the fade, the lamp it belongs to, its body), in the order they are due.
    let mut schedule: Vec<(pns::lights::Fade, &Breathing, String)> = Vec::new();
    for entry in breathing {
        let fades = pns::lights::breath_fades(budget_ms, &entry.cycle, entry.resume);
        for (index, fade) in fades.iter().enumerate() {
            // THE FIRST FADE CARRIES THE COLOUR AND THE `on`, which is what arms
            // the lamp; every one after it states brightness and duration alone,
            // so the bridge has nothing else to reconcile mid-transition. THIS
            // HOLDS ON A RESUMED TICK TOO: an externally switched-off lamp comes
            // back on with its first fade whichever leg the record names.
            let body = if index == 0 {
                pns::channels::hue::breath_arm_body(entry.color, fade)
            } else {
                pns::channels::hue::fade_body(fade)
            };
            schedule.push((*fade, entry, body));
        }
    }
    schedule.sort_by(|left, right| {
        (left.0.start_ms, &left.1.path).cmp(&(right.0.start_ms, &right.1.path))
    });
    let mut landings: Vec<(String, u8, u64)> = Vec::new();
    for (fade, entry, body) in schedule {
        // SATURATING, so a write that ran long simply issues the next fade at
        // once rather than sleeping a wrapped duration.
        let now_ms = elapsed_ms();
        if fade.start_ms > now_ms {
            sleep(Duration::from_millis(fade.start_ms - now_ms));
        }
        // READ AGAIN AFTER THE SLEEP, because the sleep is the one thing here
        // that is allowed to overshoot, and this is the moment the write starts.
        let at_ms = elapsed_ms();
        if at_ms >= budget_ms {
            break;
        }
        bridge.put(&entry.path, &body);
        // THE FADE'S OWN DURATION, so the accent at the peak of the loop's
        // motion is recorded landing two hundred milliseconds out rather than
        // four seconds out. A landing taken from the shape instead would tell
        // the next tick the lamp finishes moving long after it has, and that
        // tick would hold the lamp still waiting for it.
        let landing = (
            entry.path.clone(),
            fade.brightness,
            at_ms + fade.duration_ms,
        );
        match landings.iter_mut().find(|(path, _, _)| *path == entry.path) {
            Some(previous) => *previous = landing,
            None => landings.push(landing),
        }
    }
    landings
}

/// What one tick found: the states the house is holding, and whether anything
/// is still in flight that could become one before the next tick.
///
/// TWO ANSWERS OFF ONE READING, because the tick's own lease is a function of
/// both. A lamp that is ON has to be re-armed; a run of work that has NOT yet
/// reached its threshold has to still be watched when it does, and taking that
/// as a second reading would be a second sweep of the same directories.
struct Standing {
    house: pns::lights::House,
    /// A run of work or a lease that is live and has not lit a lamp YET.
    in_flight: bool,
}

/// The states the house is in, taken off the machine.
///
/// THE STREAK IS ADVANCED HERE, which is the one reading that WRITES: a run of
/// work is a duration, and a duration needs somewhere to have started.
fn lights_house(state: &Path, lights: &pns::config::Lights, now: u64) -> Standing {
    // THE SAME CALL THE VISIBILITY MODEL MAKES, bounded the same way, and read
    // for a different field. A herdr that is missing, wedged or answering
    // something this cannot parse yields no working workspace, which is the
    // fail-toward-dark direction.
    let statuses =
        pns::system::CommandRunner::run(&SystemCommandRunner, "herdr", &["workspace", "list"])
            .map(|answer| pns::lights::workspace_agent_statuses(&answer))
            .unwrap_or_default();
    // THE SHELL'S OWN MARKERS, which each interactive shell writes while a
    // plain command runs in it. Nothing in this crate writes them.
    let shell_since = sweep_shell_markers(state);
    // BOTH SOURCES ARE WORK IN FLIGHT (operator ruling), which is the question
    // the UNREAD lamp asks: news that arrives while anything is still running is
    // not news anybody has missed yet.
    let working = pns::lights::any_working(&statuses, shell_since);
    // AND THE STREAK IS THE AGENTS' ALONE, because it exists to supply a start
    // that herdr does not give: a status word carries no clock. The shell
    // publishes the second its command began, so pooling the two had a fresh
    // command inherit an agent's finished run and a long build restart its own.
    let agents_working = pns::lights::any_working(&statuses, None);
    let streak = advance_streak(state, agents_working, now);
    let leases = sweep_leases(state, now, lights.looping.lease_timeout_secs);
    Standing {
        // WORK THAT HAS NOT REACHED ITS THRESHOLD IS STILL IN FLIGHT, and this
        // is the reading that keeps the tick alive long enough to see it get
        // there: the automatic trigger's default is five minutes and the
        // operator's is six, both of them PAST the ordinary lease an event
        // leaves behind.
        in_flight: streak.is_some() || shell_since.is_some() || !leases.is_empty(),
        house: pns::lights::House {
            blocked: blocked_lamp(state, lights, now),
            looping: pns::lights::loop_running(&pns::lights::Loop {
                streak: streak.as_ref(),
                agents_working,
                shell_since,
                leases: &leases,
                now,
                threshold_secs: lights.looping.threshold_secs,
                lease_timeout_secs: lights.looping.lease_timeout_secs,
            }),
            unread: pns::lights::unread_arming(
                &read_news(state),
                last_interaction(),
                working,
                now,
                lights.unread.after_secs,
            ),
        },
    }
}

/// When the operator last touched this machine, by ANY road: the desk, the
/// phone's input, or the deliberate phone marker. The rule is
/// `lights::last_interaction`'s; this reads the three probes and hands them in.
///
/// THE CLOCK IS READ LAST, BY DESIGN, after the three samples rather than
/// before them. The two phone edges are file times and need no clock; the
/// desk edge is the one `lights::last_interaction` computes, as
/// `t_now - idle(t_sample)`. Reading `t_now` first would put it BEFORE the
/// sample, so the edge would land earlier than the true touch and news the
/// operator had already seen could arm the lamp. Reading it last puts the
/// residual the other way: `t_now` is later than the sample by at most the
/// four bounded spawns above this line (one `ioreg` for idle, then the phone
/// probe's `pgrep`, `pgrep -P` and `ps`), each capped at `PROBE_DEADLINE`
/// (5 seconds in `system.rs`), so the bound is four five-second receive
/// budgets, plus spawn and cleanup overhead on top, sub-second in the common
/// case. The desk touch reads that much YOUNGER
/// than it was, never older. The direction is DARK: news that landed inside
/// that residual reads as seen and the lamp stays off, and no edge can arm
/// it early.
///
/// HOISTING `let now = now_secs()?;` ABOVE THE SAMPLES WOULD BREAK THIS
/// SILENTLY: no test can catch a clock read moving a few hundred milliseconds
/// earlier, so the order below is load-bearing and not provable by a diff
/// alone. Do not reorder it.
///
/// THE OVERRIDES ARE NOT CONSULTED HERE. `PNS_IDLE_SECS` and
/// `PNS_PHONE_INPUT_AGE` steer the delivery decision in `engine::decide`, not
/// this reading: the unread lamp always sees the machine's own probes.
fn last_interaction() -> Option<u64> {
    let probes = system_probes();
    pns::lights::last_interaction(
        pns::probes::IdleProbe::idle_secs(&probes),
        pns::probes::PhoneInputProbe::phone_input_atime_secs(&probes),
        pns::probes::PhoneMarkerProbe::marker_mtime_secs(&probes),
        now_secs()?,
    )
}

/// The news record, or nothing at all for a file this cannot vouch for.
///
/// FAIL TO DARK, which is `parse_news`' own direction reached through the one
/// place that knows where the file lives: an unreadable record arms no lamp
/// rather than arming one about news nobody can name.
fn read_news(state: &Path) -> pns::lights::News {
    news_at(&state.join(LIGHTS_NEWS))
}

/// The same reading, taken at whichever path holds the record: the published
/// one, or the claim a merge is holding it under.
fn news_at(path: &Path) -> pns::lights::News {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|line| pns::lights::parse_news(&line))
        .unwrap_or_default()
}

/// Record that a turn just finished or just died.
///
/// WRITTEN ON THE EVENT PATH WHATEVER THE DELIVERY DID, which is the whole point
/// of a record separate from the journal: a card that was suppressed, muted or
/// dropped is exactly the news the unread lamp exists to carry.
///
/// OWNED BY RENAME FOR THE MERGE, which is this crate's rule for a record two
/// runs can write at once. Two events landing together (an agent that finished
/// beside one that died) each read, change their own field and publish the whole
/// line, so a plain read-modify-write loses the other's field: the record then
/// says a turn finished when one had also died, which is the red the lamp never
/// shows. Taking the file first means the winner merges a record nobody else can
/// still be reading.
///
/// A MISS IS RETRIED ONCE, because absent and held look the same from here: a
/// rename answers `NotFound` for a machine that has never recorded any news and
/// for one whose holder is mid-merge, and a holder publishes within three
/// syscalls. So the wait is paid once in a machine's life for the first record,
/// and it is what closes the window for every record after it.
///
/// THE RESIDUAL, STATED: a run whose second attempt also misses merges into
/// whatever it can read at the path instead, which is the winner's record once
/// the winner has published and nothing while it is still merging. Its cost is
/// one lamp colour, which is what fail-quiet buys everywhere on this path.
///
/// FAIL-QUIET, in `record_missed`'s style: a record that did not land costs one
/// lamp its colour, and this process has no reader for a complaint.
fn record_news(state: &Path, behaviour: pns::config::Behaviour, now: Option<u64>) {
    let Some(now) = now else {
        return;
    };
    // A WAIT IS NOT NEWS AND TOUCHES NOTHING, decided BEFORE the record is
    // claimed rather than after it is read. `news_after` is the one place that
    // knows which behaviours count, so this asks it rather than keeping a second
    // list; claiming for one that does not count would rename the record away
    // and have to put it back, which is a window over a file this is trying to
    // make safe.
    if pns::lights::news_after(pns::lights::News::default(), behaviour, now).is_none() {
        return;
    }
    let path = state.join(LIGHTS_NEWS);
    let claim = path.with_extension(format!("claim.{}", std::process::id()));
    let claimed = claim_news(&path, &claim);
    let held = news_at(if claimed { &claim } else { &path });
    if let Some(next) = pns::lights::news_after(held, behaviour, now) {
        // The failure is DROPPED here and nowhere else: see the doc comment.
        let _ = publish_state_line(&path, &pns::lights::render_news(&next));
    }
    if claimed {
        // THE CLAIM GOES WHETHER OR NOT THE PUBLISH LANDED, because the publish
        // above writes the whole record: a claim left behind would be a second
        // file holding a stale copy that nothing ever reads and nothing removes.
        let _ = std::fs::remove_file(&claim);
    }
}

/// Take the record for a merge, or answer that this run is merging blind.
fn claim_news(path: &Path, claim: &Path) -> bool {
    for attempt in 0..NEWS_CLAIM_ATTEMPTS {
        if std::fs::rename(path, claim).is_ok() {
            return true;
        }
        if attempt + 1 < NEWS_CLAIM_ATTEMPTS {
            std::thread::sleep(NEWS_CLAIM_WAIT);
        }
    }
    false
}

/// How many times a merge looks for the record before going ahead without it,
/// and how long it waits between two looks.
///
/// TWO LOOKS AND TWO MILLISECONDS, which is the whole recovery: a holder is
/// three syscalls from publishing, and the only other reason the file is not
/// there is a machine that has never recorded any news, which pays this wait
/// exactly once.
const NEWS_CLAIM_ATTEMPTS: u32 = 2;
const NEWS_CLAIM_WAIT: Duration = Duration::from_millis(2);

/// The oldest epoch a LIVE shell is holding, with the markers whose shells are
/// gone REMOVED on the way through.
///
/// THE SWEEP LIVES WITH THE READ, for `sweep_blocked`' reason: the tick is the
/// only process that ever looks in this directory, and a shell killed
/// mid-command leaves a file its own precmd will never run to remove.
///
/// THE OLDEST AND NOT THE FRESHEST. Several panes hold markers at once, and
/// the reader's one question is how long work has been going: the freshest
/// would restart the breathe clock every time any pane ran anything, so a
/// build running for an hour beside a prompt somebody keeps typing at would
/// never reach a threshold measured in minutes.
///
/// AN EPOCH THAT CANNOT BE READ IS NOT SWEPT WHILE ITS SHELL IS ALIVE, which
/// is the one place this differs from `sweep_blocked`. The shell publishes with a
/// truncating redirect, so a tick landing between that open and the write sees
/// an empty file for a command that is genuinely starting; unlinking it there
/// wins the race and the build then runs to completion with no marker at all.
/// Nothing accumulates by leaving it: the pid in the name collects the file
/// when that shell ends.
fn sweep_shell_markers(state: &Path) -> Option<u64> {
    let mut oldest: Option<u64> = None;
    for entry in std::fs::read_dir(state.join(LIGHTS_SHELL_DIR))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // THE SAME LIVENESS ANSWER THE CLAIMS USE, so this binary has one
        // reading of "that process is gone" rather than two that can drift.
        // The positive-pid test comes first because `kill()` reads 0 as this
        // process's own group and -1 as every process the user owns, and
        // because a name that is not a pid at all is litter nothing else here
        // would ever age out.
        if !name.parse::<libc::pid_t>().is_ok_and(|pid| pid > 0) || owner_is_gone(&name) {
            let _ = std::fs::remove_file(entry.path());
            continue;
        }
        if let Some(at) = read_epoch(&entry.path()) {
            oldest = Some(at.min(oldest.unwrap_or(at)));
        }
    }
    oldest
}

/// Every live wait's epoch, with the ones past the bound REMOVED on the way
/// through.
///
/// THE SWEEP LIVES WITH THE READ because the tick is the only process that
/// ever looks in this directory: a session that ends without another event
/// leaves a marker nothing else would ever remove, and one file per abandoned
/// session for the life of a machine is unbounded growth.
fn sweep_blocked(state: &Path, now: u64, give_up_after_secs: u64) -> Vec<u64> {
    sweep_markers(&pns::lights::blocked_dir(state), now, give_up_after_secs)
}

/// The blocked lamp's reading for this tick: the sweep that removes an aged
/// marker and the aggregate that lights the lamp, both handed the one
/// configured backstop.
///
/// ITS OWN FUNCTION SO ITS TEST SPAWNS NOTHING: the rest of the house asks
/// herdr and the idle probes, and this half never depends on either.
fn blocked_lamp(state: &Path, lights: &pns::config::Lights, now: u64) -> bool {
    let give_up_after_secs = lights.blocked.give_up_after_secs;
    pns::lights::any_blocked(
        &sweep_blocked(state, now, give_up_after_secs),
        now,
        give_up_after_secs,
    )
}

/// The working streak after this tick's reading, published or removed.
fn advance_streak(state: &Path, working: bool, now: u64) -> Option<pns::lights::Streak> {
    let marker = state.join(LIGHTS_STREAK);
    let held = std::fs::read_to_string(&marker)
        .ok()
        .and_then(|line| pns::lights::parse_streak(&line));
    let next = pns::lights::next_streak(held, working, now, WORKING_GRACE_SECS);
    // FAIL-QUIET, in `record_missed`'s style: a streak that did not land costs
    // one lamp its breathing, and this process has no reader for a complaint.
    match &next {
        Some(streak) => {
            let _ = publish_state_line(&marker, &pns::lights::render_streak(streak));
        }
        None => {
            let _ = std::fs::remove_file(&marker);
        }
    }
    next
}

/// The held record's entries, path and phase both, or None for a record this
/// cannot read.
///
/// ABSENT AND UNREADABLE ARE DIFFERENT ANSWERS, and collapsing them into an
/// empty list is what made a corrupt record read as a house holding nothing.
/// The event path's pulse gate then flashed straight over a lamp that was
/// breathing, and no reader was told. The ordinary case, a machine holding
/// nothing at all, is an ABSENT file and still answers with an empty list.
///
/// THE ONE PARSE, shared by every reader: `held_lamps` is this with the phase
/// dropped, so the three path-only consumers (the event path's pulse gate, the
/// operator's return, and the mute) read bare paths off the very same tokens
/// the breath's resume reads a phase from, and neither can drift from the
/// other's idea of what a token means.
fn read_held(state: &Path) -> Option<Vec<pns::lights::HeldEntry>> {
    match std::fs::read_to_string(state.join(LIGHTS_HELD)) {
        Ok(line) => Some(
            line.split_whitespace()
                .map(pns::lights::parse_held_token)
                .collect(),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(Vec::new()),
        Err(_) => None,
    }
}

/// The fixture paths a held write is currently holding, bare, or None for a
/// record this cannot read. See `read_held` for the phase this drops.
fn held_lamps(state: &Path) -> Option<Vec<String>> {
    read_held(state).map(|entries| entries.into_iter().map(|entry| entry.path).collect())
}

/// Record what is held now, or forget the file when nothing is.
///
/// ONE LINE, SPACE SEPARATED, because a fixture path is `light/<id>` or
/// `grouped_light/<id>` and neither can carry a space, and neither can carry
/// `@` or `:` either, which is what lets a phased token
/// (`light/<id>@<end-unix-ms>:<h|l>:<state>`) share the line with a bare one.
/// That keeps this a `publish_state_line` write like every other state file
/// rather than a second file format.
///
/// A TICK CAN REPUBLISH A GLOW THE RETURN JUST CLEARED, and that is a stated
/// limit rather than a rule. The tick reads its condition before it reaches the
/// bridge, so a present event that advances the return edge and clears the held
/// paths while an older tick is still resolving fixtures loses the race here:
/// that tick writes the glow and records it again. Nothing arbitrates, because
/// there is no lock between two processes that are deliberately independent.
/// The next present event clears it with no daemon at all, and the next tick
/// after it reads the advanced edge and finds no condition, so the exposure is
/// one refresh interval. It is unbounded only for a tick that was its lease's
/// LAST run, and there the lamp waits for the operator's return, which is the
/// event that clears it.
/// THE FAILURE IS RETURNED, not dropped, because the caller has to stop: a
/// lamp armed after a record that did not land is a lamp nothing in the system
/// knows the name of, and the return from an absence, the next tick and the
/// operator's own mute all put lamps out BY NAME off this file.
fn remember_held(state: &Path, held: &[pns::lights::HeldEntry]) -> std::io::Result<()> {
    let marker = state.join(LIGHTS_HELD);
    if held.is_empty() {
        return match std::fs::remove_file(&marker) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let line = held
        .iter()
        .map(pns::lights::render_held_token)
        .collect::<Vec<_>>()
        .join(" ");
    publish_state_line(&marker, &line)
}

/// Say a complaint ONCE, and say it again only when it changes.
///
/// THE MARKER IS A PARAMETER because two paths say things at different rates
/// about different sets: the tick folds every refusal of a pass into one line,
/// and the event path says only what it read off the ad-hoc quiet file. Sharing
/// one memory would have each of them forgetting the other's line and repeating
/// it, which is the chatter this whole mechanism exists to stop.
fn say_lights_once(state: &Path, complaints: &[String], marker: &str) {
    let marker = state.join(marker);
    let remembered = std::fs::read_to_string(&marker).unwrap_or_default();
    match pns::lights::say(complaints, remembered.trim_end_matches('\n')) {
        pns::lights::Say::Nothing => {}
        pns::lights::Say::Aloud(said) => {
            for complaint in complaints {
                eprintln!("{complaint}");
            }
            let _ = publish_state_line(&marker, &said);
        }
        pns::lights::Say::Forget => {
            let _ = std::fs::remove_file(&marker);
        }
    }
}

/// Delete the state the lamps kept under their OLD names, and never read it.
///
/// THE DEPLOY TRANSITION, and it is a deletion rather than a migration. Every
/// one of these files is derived from the machine on the next tick anyway (a
/// wait re-arrives with its session's next event, a streak restarts the moment
/// work is seen), so carrying the contents forward would buy nothing and would
/// mean two readers of one fact for as long as the code lived.
///
/// THE DARK DIRECTION, which is what makes the held record safe to drop: the
/// old record named lamps a steady write was holding, and the binary that wrote
/// them is gone. Deleting it leaves at most one lamp lit until the operator's
/// next event, and keeping it would have the NEW tick clear lamps it never
/// wrote by names it never chose.
///
/// ONCE, WITHOUT A MARKER TO SAY SO. A removal of a name that is not there is
/// one failed syscall, so the deletion happens exactly once and every tick after
/// it pays three of those rather than a fourth state file.
fn sweep_legacy_state(state: &Path) {
    for legacy in ["lights-glow", "lights-working-since"] {
        let _ = std::fs::remove_file(state.join(legacy));
    }
    let _ = std::fs::remove_dir_all(state.join("lights-needs"));
}

/// How long a run of work survives readings that say nothing is working.
///
/// THE GAP BETWEEN A LOOP'S TURNS IS WHAT THIS COVERS, and it is why the
/// streak is not simply "is something working right now": an agent reads idle
/// for the seconds between one turn and the next, and a streak that reset
/// there could never reach a threshold measured in minutes.
const WORKING_GRACE_SECS: u64 = 120;

/// Where the streak lives.
const LIGHTS_STREAK: &str = "lights-streak";

/// Where the shell says a tracked command is running: ONE FILE PER INTERACTIVE
/// SHELL, named for that shell's pid, holding ONE EPOCH, the second the
/// command started. Written by the interactive shell and removed when the
/// command ends; only read here.
///
/// ONE FILE PER SHELL AND NOT ONE FILE. Every interactive shell on the machine
/// runs the same two bash-preexec functions, so a single shared path is a
/// marker any other pane erases: opening a tab, or running `ls` next door,
/// would delete a running build's evidence and leave this lamp dark for the
/// rest of that build. A directory makes each shell the only writer and the
/// only ordinary remover of its own file.
///
/// THE LONG TIER IS DERIVED FROM THAT EPOCH AND IS NOT A SECOND FIELD, because
/// it cannot be one. The marker is written when the command STARTS, and at
/// that instant the command has run for zero seconds, so nothing on the shell
/// side knows the tier yet; a flag would take a background timer rewriting the
/// file mid-command. `now - since` against the notifier's own threshold
/// answers the same question with one source of truth instead of two that can
/// disagree.
///
/// A SHELL KILLED MID-COMMAND LEAVES ITS FILE, and the pid in the NAME is what
/// collects it: the tick sweeps a marker whose process is gone, so a killed
/// terminal costs one tick's reading rather than a lamp breathing forever. The
/// lease stays the backstop for the case the pid cannot answer, a marker whose
/// shell is alive and whose command is not, because nothing renews the tick's
/// lease but a pns event.
const LIGHTS_SHELL_DIR: &str = "lights-shell";

/// Where the fixture paths a steady glow is holding are recorded.
const LIGHTS_HELD: &str = "lights-held";

/// Where a lights tick holds the whole house for as long as it is driving it.
///
/// THE DAEMON'S OWN BOOKKEEPING IS NOT A LOCK. `decide` refuses to fire a
/// second lights child while the first is still listed, and that list is ONE
/// process's memory: a tick the operator ran by hand and an orphan left behind
/// by a daemon replacement are both invisible to it. Two ticks driving one lamp
/// interleave their fades against two schedules, and the phase the LAST of them
/// writes is the one the next tick resumes off, so the breath it picks up is
/// one no lamp was ever running. A file the operating system arbitrates is the
/// only guard every writer can see.
///
/// IT DOES NOT LOCK OUT THE EVENT PATH, deliberately. The operator's return
/// clears the held record from a process that holds no lock and must never wait
/// on one; `run_tick_writes` re-reads the record instead and stands down when
/// it moved, which is the guard that case has always had.
const LIGHTS_TICK_LOCK: &str = "lights-tick.lock";

/// How long a lights tick's lock is believed before it is read as an orphan.
///
/// `child_bound`'S OWN ARITHMETIC FOR THIS JOB, because it bounds the same
/// process: the longest interval the config permits, plus the longest a single
/// write may take at that interval, plus the second the daemon takes to notice
/// the child is gone. A tick still holding the lock past that has already been
/// killed, so the file is leavings. Standing down for a live holder costs one
/// interval of an unchanged lamp; stealing the lock from one that is still
/// driving is the failure the lock exists to stop, so the bound errs long.
fn lights_tick_stale_secs() -> u64 {
    pns::config::MAX_REFRESH_SECS
        + tick_bridge_deadline(pns::config::MAX_REFRESH_SECS).as_secs()
        + 1
}

/// Where the two news epochs live: the second a turn last finished, and the
/// second one last died.
///
/// ONE LINE AND TWO NUMBERS, which is what keeps this a `publish_state_line`
/// write like every other state file rather than a second file format, and what
/// makes it inherently capped: a record that cannot grow cannot collapse at a
/// cap either.
const LIGHTS_NEWS: &str = "lights-news";

/// Where a tick remembers what it last complained about.
const LIGHTS_SAID: &str = "lights-said";

/// What a tick says about a held record it could not read at all.
///
/// THE TICK GOES ON, because it is the file's only writer: it names no lamp to
/// clear, derives the states it wants and publishes a record for them, which is
/// what repairs an unreadable file. Where the path cannot be WRITTEN either, the
/// publish refuses and nothing is armed, which is the second sentence the
/// operator gets.
const HELD_RECORD_UNREADABLE: &str = "pns lights: the held record could not be read, \
so no lamp can be put out by name";

/// How long ONE of a tick's bridge calls may take.
///
/// A FIFTH OF THE INTERVAL, so the three the resolve makes cannot outlive the
/// child that makes them AND still leave a breath: at the transport's own ten
/// seconds they outlive every interval the config permits, and a wedged bridge
/// would then have tick after tick piling up, each still dialling while the next
/// was spawned. A fifth is what keeps a full cycle of the shortest locked shape
/// inside what is left even when all three calls run to their deadline, which is
/// the whole point of the child staying alive.
///
/// A SECOND AT LEAST, which the division cannot reach anyway inside the config's
/// own bounds; a bridge on the same LAN answers these in milliseconds either
/// way.
fn tick_bridge_deadline(refresh_secs: u64) -> Duration {
    Duration::from_secs((refresh_secs / 5).max(1))
}

/// Where the EVENT path remembers the ad-hoc quiet complaint it last made,
/// which is a file of its own for the reason `say_lights_once` states.
const LIGHTS_QUIET_SAID: &str = "lights-quiet-said";

/// Where the operator's own ad-hoc quiet lives: one line per place, each an
/// expiry second and the name they typed.
///
/// ONE FILE RATHER THAN ONE PER PLACE, and that is a path-safety decision as
/// much as a tidiness one: a place is a room name the operator typed, spaces
/// and all, and nothing in this crate turns typed text into a filename unless a
/// predicate already vouches for it.
const LIGHTS_QUIET: &str = "lights-quiet";

/// The loop. It sleeps, drains the spool, and reaps what it started.
///
/// IT HOLDS NO DURABLE STATE. Restarting re-reads the directory, which is the
/// whole recovery path, and reboot works the same way because the state
/// directory survives it and the lease drops whatever went stale. There is no
/// in-memory schedule to diverge from the disk.
///
/// SIGTERM NEEDS NO HANDLER. launchd stops a job with SIGTERM and the default
/// disposition terminates the process; a loop sleeping one second dies inside
/// the tick. A child mid-flight is orphaned rather than killed, and an orphaned
/// nudge is at worst one extra card.
fn daemon_run() -> i32 {
    if std::env::args_os().nth(3).is_some() {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    }
    if !daemon_enabled() {
        // ONE LINE, ONCE, on the path that exits. `SuccessfulExit = false` in
        // the plist is what keeps a clean exit 0 exited, so this is written at
        // most once per bootstrap rather than once per throttle window.
        println!("pns daemon: disabled in the config; exiting");
        return 0;
    }
    let state = state_dir();
    let spool = pns::daemon::spool_dir(&state);
    // EXIT 0 ON A REFUSAL RETRYING CANNOT FIX. Both of them (a spool path that
    // is not a directory, a state directory that will not take one) are
    // permanent, and `KeepAlive { SuccessfulExit = false }` relaunches a
    // non-zero exit every ten seconds forever: ~8,640 relaunches and ~8,640
    // copies of this line a day, which is behavior 15's chatter arriving
    // through the restart door. A clean exit keeps the job DOWN and the
    // doctor's line is what tells the operator.
    if let pns::daemon::Startup::Refused(refusal) = pns::daemon::prepare_spool(&state) {
        eprintln!("pns daemon: {refusal}");
        return 0;
    }
    let tick = daemon_tick();
    let mut children: Vec<Bounded> = Vec::new();
    let mut reported: std::collections::BTreeSet<std::path::PathBuf> =
        std::collections::BTreeSet::new();
    let mut ticks: u64 = 0;
    loop {
        std::thread::sleep(tick);
        ticks = ticks.wrapping_add(1);
        // THE SWITCH IS RE-READ, so `enabled = false` reaches a daemon that is
        // ALREADY RUNNING. Read once at startup it was inert: nothing bounces
        // this job on a config change (the loader's trigger is the plist hash),
        // so the operator's off switch did nothing until a hand-typed bootout.
        // Once every `SWITCH_TICKS` rather than every tick, which is one config
        // read per thirty seconds at the production tick.
        let now = now_secs();
        if ticks.is_multiple_of(SWITCH_TICKS) {
            if !daemon_enabled() {
                println!("pns daemon: disabled in the config; exiting");
                return 0;
            }
            // THE ROOM SENSOR IS THE DAEMON'S OWN JOB, on the same cadence and
            // out of the same config read: no event asks for a room reading,
            // so nothing else would ever register it, and a job registered
            // once at startup would die with its lease on the first daemon
            // that outran it.
            if let Some(now) = now {
                ensure_presence_poll(&state, presence_settings().as_ref(), now);
            }
        }
        daemon_pass(&spool, &state, now, tick, &mut children, &mut reported);
    }
}

/// Everything one turn of the daemon's loop does, in the ONE ORDER that makes
/// `decide`'s running answer true.
///
/// REAPED BEFORE THE SPOOL IS DRAINED, so a child `decide` finds still in
/// `children` really is alive THIS pass. Reaped the other way round, a child
/// that exited moments ago still reads as running and holds its own due
/// occurrence to one more `Wait` than it needed, which on the lights job is a
/// tick of a lamp that has stopped breathing.
///
/// IT IS A FUNCTION AND NOT FOUR LINES IN THE LOOP for exactly that reason:
/// the order is the behaviour, so a test has to be able to run it in the
/// order production runs it rather than in one of its own.
///
/// A SECOND THAT COULD NOT BE READ STOPS THE DRAIN AND NEVER THE REAP. A bound
/// is still a bound with no wall clock to publish against, and a child left
/// running past its own because the clock would not answer is the one failure
/// here that accumulates.
fn daemon_pass(
    spool: &Path,
    state: &Path,
    now: Option<u64>,
    tick: Duration,
    children: &mut Vec<Bounded>,
    reported: &mut std::collections::BTreeSet<std::path::PathBuf>,
) {
    reap(children);
    let Some(now) = now else {
        return;
    };
    // FAIL-QUIET, in `remember_staleness`'s style: a heartbeat that did not
    // land costs one doctor line, and complaining about it every tick is
    // the chatter this daemon must never produce.
    let _ = pns::daemon::publish_heartbeat(
        state,
        &pns::daemon::Heartbeat {
            pid: std::process::id(),
            at: now,
        },
    );
    drain_spool(spool, state, now, tick, children, reported);
}

/// One child the daemon started, and the moment it stops being allowed to run.
struct Bounded {
    /// The job's own id, so `decide` can ask whether THIS job's child is
    /// still running rather than merely whether any child is.
    id: String,
    child: std::process::Child,
    expires_at: std::time::Instant,
}

/// One pass over the spool, under a protocol with THREE INVARIANTS.
///
/// 1. **A CLIENT ALWAYS WINS.** Every write this daemon makes into the spool
///    (a re-arm, a put-back) is create-if-absent, so a registration or a
///    refresh that landed while a record was claimed keeps its name and the
///    daemon's older copy is discarded. An overwriting rename here would put a
///    stale due, lease and argv back over the newest signal, which is the one
///    guarantee the id-is-the-filename refresh rule makes.
/// 2. **THE DAEMON ACTS ONLY ON WHAT IT OWNS.** A read-only peek decides one
///    thing and one only: whether there is nothing to do. Everything else
///    claims the entry by rename FIRST and re-reads the claim, so the record
///    that fires is the record this daemon took, never one a refresh replaced
///    between the look and the act. A `Wait` is never claimed, because a wait
///    performs no action and renaming a waiting job out and back would be the
///    very write invariant 1 forbids.
/// 3. **ONE OCCURRENCE RUNS ONCE.** The rename is still the arbiter and it is
///    now taken before the content is read, so of two daemons exactly one
///    holds the record and the loser reads nothing at all.
///
/// THE RESIDUAL WINDOWS, STATED HONESTLY. A refresh that lands AFTER the claim
/// is taken cannot stop the occurrence already claimed from running, so the
/// operator can see one card from the record that was in flight plus the
/// refreshed job afterwards. Nothing is LOST and nothing runs twice; the old
/// occurrence simply ran. A refresh that lands after the claim also wins the
/// re-arm's link, so the repeat continues on the client's terms rather than the
/// daemon's. And a claim this process took and could not remove holds its own
/// working name; the line naming it is printed either way, because a job that
/// vanished with nothing in the log is the failure that costs the most to find.
fn drain_spool(
    spool: &Path,
    state: &Path,
    now: u64,
    tick: Duration,
    children: &mut Vec<Bounded>,
    reported: &mut std::collections::BTreeSet<std::path::PathBuf>,
) {
    for entry in pns::daemon::spool_entries(spool) {
        let Some(id) = entry
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        match pns::daemon::peek(&entry, &id) {
            // SAID ONCE, never once a tick: the file is left where it is, so
            // the alternative is one line a second about a thing nobody is
            // going to fix while the daemon is watching.
            pns::daemon::Peeked::Irregular => {
                if reported.insert(entry.clone()) {
                    eprintln!(
                        "pns daemon: {} is not a regular file; left alone and never opened",
                        entry.display()
                    );
                }
            }
            // NOTHING TO DO, DECIDED WITHOUT TOUCHING IT. This is the only
            // verdict a peek is allowed to be the last word on.
            pns::daemon::Peeked::Job(job)
                if pns::daemon::decide(
                    &job,
                    now,
                    pns::daemon::marker_exists(state, &job),
                    children.iter().any(|bounded| bounded.id == job.id),
                ) == pns::daemon::Verdict::Wait => {}
            // Anything else is an ACTION, so the record is taken first and read
            // again afterwards. A failed claim means another run got there,
            // which is exactly what the rename is for.
            _ => {
                if let Some(claim) = pns::daemon::claim(&entry) {
                    act(&claim, &id, spool, state, now, tick, children);
                }
            }
        }
    }
}

/// One CLAIMED record, re-read and acted on.
///
/// THE RE-READ IS THE POINT. Between the peek that decided to act and the
/// rename that took the record, a client can have replaced it with a refresh
/// carrying a new due, a new lease and new arguments. Acting on the peek would
/// fire the old argv and then delete the new record on the way out; acting on
/// the claim fires whatever this daemon actually holds.
fn act(
    claim: &Path,
    id: &str,
    spool: &Path,
    state: &Path,
    now: u64,
    tick: Duration,
    children: &mut Vec<Bounded>,
) {
    match pns::daemon::peek(claim, id) {
        // A RENAME MOVES A REGULAR FILE AS A REGULAR FILE, so this is not
        // reachable by the paths above; it is still answered rather than
        // ignored, because the alternative is a claim held forever.
        pns::daemon::Peeked::Irregular => {
            println!("pns daemon: dropped `{id}`: it is not a regular file");
            release(claim);
        }
        pns::daemon::Peeked::Unusable(refusal) => {
            println!("pns daemon: dropped `{id}`: {refusal}");
            release(claim);
        }
        pns::daemon::Peeked::Job(job) => {
            // ASKED AGAIN, AND REDUNDANT WHILE THE PEEK ASKS IT TOO: the peek
            // stands a running job down before anything is claimed, so this is
            // only ever reached with no child of this id alive, and no test can
            // tell this argument from a literal `false`. It stays because the
            // peek is an optimisation over a re-read and this is the decision
            // the claim is actually acted on.
            let running = children.iter().any(|bounded| bounded.id == job.id);
            match pns::daemon::decide(&job, now, pns::daemon::marker_exists(state, &job), running) {
                // The refresh this daemon claimed is not due yet, so it goes
                // back CREATE-IF-ABSENT: a client that registered again in the
                // meantime keeps its own record and this copy is dropped.
                pns::daemon::Verdict::Wait => match pns::daemon::hand_back(spool, &job) {
                    Ok(_) => release(claim),
                    Err(error) => {
                        eprintln!("pns daemon: `{id}` could not be put back ({error})");
                        release(claim);
                    }
                },
                pns::daemon::Verdict::Drop(reason) => {
                    println!("pns daemon: dropped `{id}` because {}", reason.said());
                    release(claim);
                }
                pns::daemon::Verdict::Fire => fire(&job, spool, now, tick, claim, children),
            }
        }
    }
}

/// A working file this daemon is done with, removed and NAMED IF IT SURVIVES.
///
/// A CLAIM THAT COULD NOT BE REMOVED IS A LEAK, not a nothing: it is invisible
/// to the scan (the working prefix is outside the id charset), so it sits there
/// until a hand removes it, and `claim` refuses to reuse a name already taken,
/// which can wedge that one id after a pid is reused. One line naming the file
/// is the whole remedy, and it costs nothing on the path where the remove
/// works.
fn release(claim: &Path) {
    if let Err(error) = std::fs::remove_file(claim) {
        eprintln!(
            "pns daemon: the working file {} could not be removed ({error}); it is left behind",
            claim.display()
        );
    }
}

/// One claimed job re-armed and started, in that order.
///
/// THE RE-ARM IS DURABLE BEFORE THE SPAWN. Written the other way round, a
/// daemon killed between the two loses the repeat with the job already run,
/// which is the lamp going dark on a loop that is still alive.
///
/// AND THE RE-ARM IS CREATE-IF-ABSENT. A client that refreshed this id while
/// the occurrence was claimed published the newer signal, and a rename here
/// would overwrite it with the due and lease this daemon computed from the
/// record it had already taken.
fn fire(
    job: &pns::daemon::Job,
    spool: &Path,
    now: u64,
    tick: Duration,
    claim: &Path,
    children: &mut Vec<Bounded>,
) {
    if let Some(next) = pns::daemon::rearm(job, now) {
        match pns::daemon::hand_back(spool, &next) {
            Ok(true) => {}
            Ok(false) => println!(
                "pns daemon: `{}` was registered again while it ran, so its repeat stands down",
                job.id
            ),
            Err(error) => eprintln!("pns daemon: `{}` will not repeat ({error})", job.id),
        }
    }
    release(claim);
    // AN ACTION THAT SUPPRESSED ITS OWN ERROR HAS NOT BEEN PERFORMED: a spawn
    // that failed is said out loud, because the alternative is a job that
    // reports as run and delivered nothing.
    //
    // AND A SPAWN THAT WORKED SAYS NOTHING, which is the daemon's own
    // no-chatter rule applied to the thing it actually does. The lights tick
    // repeats every twelve seconds for as long as its lease holds, so a line
    // per firing is 300 an hour in the file the log rotation then rotates a
    // real log out of. What a job has to say, the job says itself: its stderr
    // is the daemon's now.
    match spawn_job(job) {
        Ok(child) => {
            children.push(Bounded {
                id: job.id.clone(),
                child,
                expires_at: std::time::Instant::now() + child_bound(tick, &job.id),
            });
        }
        Err(error) => eprintln!("pns daemon: `{}` could not start ({error})", job.id),
    }
}

/// The job's argv handed to THIS binary, detached.
///
/// `current_exe` AND NEVER A STORED PATH, exactly as `spawn_recap` does: the
/// record carries arguments, so nothing in the spool can name another program.
/// Anyone who can write a 0600 file in this directory can already run `pns`, so
/// this is a blast-radius limit rather than a security boundary, and it costs
/// nothing.
///
/// STDIN AND STDOUT NULL, STDERR INHERITED, and IN A GROUP OF ITS OWN, so
/// launchd stopping the daemon orphans a child in flight rather than killing it
/// mid-delivery.
///
/// STDERR IS THE ONE READER A JOB HAS. A job runs unattended with no terminal
/// behind it, so a complaint it writes goes wherever this puts that stream:
/// null sent it to `/dev/null`, and the lights tick's say-once memory then
/// recorded the complaint as SAID, so no later tick repeated it either. A lamp
/// renamed on the bridge was therefore reported exactly once, into nothing. The
/// daemon's plist points both of its own streams at `~/.local/log/`, so
/// inheriting is what puts a child's line in front of the operator.
///
/// STDOUT STAYS NULL, because that is where a job's ORDINARY output goes and
/// the ordinary case here is a tick that ran three times a minute and has
/// nothing to report. Only what could not be said anywhere else crosses.
fn spawn_job(job: &pns::daemon::Job) -> std::io::Result<std::process::Child> {
    let mut child = Command::new(std::env::current_exe()?);
    child
        .args(&job.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .process_group(0);
    child.spawn()
}

/// Every child looked at once, and any that outlived its bound killed.
///
/// `try_wait` AND NEVER `wait`. A blocking wait on a child that hangs holds the
/// whole loop, so one wedged delivery stops every later job: the clock would
/// pass every other test here and stop in production. The `wait` below runs
/// only on a child that has ALREADY been killed, which returns at once and is
/// what stops a zombie.
fn reap(children: &mut Vec<Bounded>) {
    children.retain_mut(|bounded| match bounded.child.try_wait() {
        Ok(Some(_)) | Err(_) => false,
        Ok(None) if std::time::Instant::now() >= bounded.expires_at => {
            kill_group(bounded.child.id());
            // The direct child again, in case the group could not be signalled
            // at all, and then the wait that turns a killed child into a reaped
            // one rather than a zombie held for the daemon's lifetime.
            let _ = bounded.child.kill();
            let _ = bounded.child.wait();
            false
        }
        Ok(None) => true,
    });
}

/// Every process in a bounded child's group, killed.
///
/// THE GROUP AND NOT THE CHILD, which is the difference between a bound and a
/// bound that holds. `spawn_job` puts each job in a group of its own, and the
/// job is a `pns` that spawns a delivery of its own and waits on it: killing
/// the direct child alone leaves that delivery running, MEASURED still alive
/// 750ms past a 300ms bound, and a repeating job that hangs then accumulates
/// them. A negative pid names the group, which is the only reason
/// `process_group(0)` is set in the first place.
fn kill_group(pid: u32) {
    // NEVER 0 AND NEVER 1. `kill(0, ...)` signals THIS process's own group and
    // `kill(-1, ...)` signals every process the user owns, so a pid that is
    // neither a real child nor representable is refused rather than trusted.
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return;
    };
    if pid <= 1 {
        return;
    }
    // SAFE: `kill` takes two integers by value, reads and writes no memory this
    // process owns, and the only outcomes are a signal delivered or an errno
    // nothing here reads.
    unsafe { libc::kill(-pid, libc::SIGKILL) };
}

/// How many ticks a spawned job may run before it is killed, as a FLOOR.
///
/// THIRTY, so the bound moves with the tick and there is ONE knob rather than
/// two. In production that is thirty seconds, which is generous for the event
/// dispatch most of these children are: every channel inside one already
/// carries its own deadline, so a child still alive at this point is wedged
/// rather than slow. The LIGHTS tick is the exception, and `child_bound` is
/// where its own arithmetic lives.
const CHILD_TICKS: u32 = 30;

/// How long a spawned job may actually run before it is killed.
///
/// THE LIGHTS TICK IS THE ONE JOB WHOSE WORK IS AN INTERVAL, and it is named
/// here rather than generalised over every repeat. Every other child is an
/// event delivery whose channels each carry their own deadline, so one still
/// alive at `CHILD_TICKS` is wedged rather than slow and the tick-scaled bound
/// is exactly right for it. Widening the floor to all of them would only make a
/// wedged delivery take longer to kill.
///
/// THE TICK'S OWN ARITHMETIC, STATED: the longest interval it can be given
/// (`MAX_REFRESH_SECS`, thirty seconds), plus the longest a single write may
/// take at that interval (`tick_bridge_deadline`, a fifth of it, so six), plus
/// one reap tick, because a child is only noticed as gone on the pass after it
/// exits. Thirty-seven seconds at the production clock.
///
/// WHY IT IS NOT `CHILD_TICKS` ALONE: that made the tick's child life equal to
/// the longest interval a tick can be given, and a seamless breath issues its
/// last fade strictly INSIDE that interval and lets it finish after. At a
/// thirty-second refresh with 749ms spent resolving, the last write starts at
/// child time 29,999ms and its legal six-second reply was killed before the
/// tick could record where the lamp landed, leaving the next tick to resume
/// from a phase nothing had written. `max` keeps the tick-scaled bound wherever
/// it is the larger of the two, so a deliberately slow clock still gets the
/// generous child it always had.
fn child_bound(tick: Duration, id: &str) -> Duration {
    if id != LIGHTS_JOB {
        return tick * CHILD_TICKS;
    }
    let one_lights_tick = Duration::from_secs(pns::config::MAX_REFRESH_SECS)
        + tick_bridge_deadline(pns::config::MAX_REFRESH_SECS)
        + tick;
    (tick * CHILD_TICKS).max(one_lights_tick)
}

/// How many ticks pass between two reads of the config's own switch.
///
/// THIRTY, so the cost is one config read per thirty seconds at the production
/// tick, and the switch still takes effect within half a minute of being
/// flipped. Counted in TICKS rather than seconds for `CHILD_TICKS`'s reason:
/// one knob moves with the clock instead of two disagreeing about it.
const SWITCH_TICKS: u64 = 30;

/// Whether the clock is switched on.
///
/// THE BROKEN-CONFIG FALLBACK IS ON, inherited from `select_plugins`' own: a
/// file that will not parse must not silently stop a service the operator
/// enabled, and the warning says which it was.
fn daemon_enabled() -> bool {
    match load_config(&config_path(&std::env::var("HOME").unwrap_or_default())) {
        Ok(LoadOutcome::Loaded(config)) => config.daemon_enabled,
        Ok(LoadOutcome::Missing) => true,
        Err(error) => {
            eprintln!(
                "pns daemon: the config could not be read ({}); carrying on enabled",
                error.detail()
            );
            true
        }
    }
}

/// How long the loop sleeps between passes.
///
/// A CONSTANT WITH A TEST HATCH rather than a config key, following
/// `PNS_PAYLOAD_DEADLINE_MS`: the only party who has ever needed a different
/// tick is a test, and a knob nobody turns is a knob that only ever holds a
/// wrong value.
///
/// STRICTLY PARSED, FLOORED AND CAPPED, and anything else falls back to the
/// constant rather than being clamped towards it. A stray `1` in a launchd
/// environment would spin the loop a thousand times a second, and clamping
/// would honour a value nobody meant to write.
fn daemon_tick() -> Duration {
    let milliseconds = std::env::var("PNS_DAEMON_TICK_MS")
        .ok()
        .and_then(|raw| pns::parse_count(&raw))
        .filter(|milliseconds| (MIN_TICK_MS..=MAX_TICK_MS).contains(milliseconds))
        .unwrap_or(DEFAULT_TICK_MS);
    Duration::from_millis(milliseconds)
}

/// One second: fast enough that a nag is on time and a light re-arms before it
/// lapses, slow enough that the idle cost is one `read_dir` of an empty
/// directory per second.
const DEFAULT_TICK_MS: u64 = 1000;

/// The floor, so no environment can spin the loop.
const MIN_TICK_MS: u64 = 10;

/// The ceiling, so no environment can park it.
const MAX_TICK_MS: u64 = 60_000;

/// `pns daemon schedule`: one registration, typed.
///
/// FOR DRILLS AND FOR TESTS. The library function beneath it is what a rider
/// will call, in-process, so nothing ever spawns a process to talk to the
/// daemon.
fn daemon_schedule() -> i32 {
    let argv: Vec<String> = std::env::args_os()
        .skip(3)
        .map(|word| word.to_string_lossy().into_owned())
        .collect();
    let Some(request) = parse_schedule(&argv) else {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    };
    let Some(now) = now_secs() else {
        eprintln!("pns daemon: this machine has no clock to schedule against");
        return 1;
    };
    let due = now.saturating_add(request.in_secs);
    let job = pns::daemon::Job {
        id: request.id,
        due,
        until: match request.until {
            Some(Until::Epoch(epoch)) => epoch,
            Some(Until::FromNow(seconds)) => now.saturating_add(seconds),
            // A LEASE IS NEVER ABSENT, only unstated: a job with no expiry is
            // the parked job the whole design refuses, so an unstated one gets
            // a small slack past its due second.
            None => due.saturating_add(DEFAULT_LEASE_SLACK_SECS),
        },
        every: request.every,
        unless_marker: request.marker,
        args: request.args,
    };
    match pns::daemon::schedule(&state_dir(), &job, now) {
        Ok(()) => 0,
        Err(refusal) => {
            eprintln!("pns daemon: {refusal}");
            1
        }
    }
}

/// How long past its due second an unstated lease runs. A minute: long enough
/// that a busy tick or a slow boot still delivers, short enough that a machine
/// asleep through the moment wakes to a job whose point has passed.
const DEFAULT_LEASE_SLACK_SECS: u64 = 60;

/// `--until` in its two spellings.
enum Until {
    Epoch(u64),
    FromNow(u64),
}

/// Everything `schedule` was asked for, before a clock is read.
struct ScheduleRequest {
    id: String,
    in_secs: u64,
    every: Option<u64>,
    until: Option<Until>,
    marker: Option<String>,
    args: Vec<String>,
}

/// The typed request, or None for anything this will not run.
///
/// UNKNOWN IS AN ERROR, never a silent skip: `pns`'s own event parser is
/// lenient because it sits on a notification path that must not fail, and this
/// one sits in front of an operator who typed a command and will believe it
/// did what they wrote.
fn parse_schedule(argv: &[String]) -> Option<ScheduleRequest> {
    let mut id = None;
    let mut in_secs = 0;
    let mut every = None;
    let mut until = None;
    let mut marker = None;
    let mut args = Vec::new();
    let mut words = argv.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            // Everything past the separator is the event, untouched.
            "--" => {
                args = words.cloned().collect();
                break;
            }
            "--id" => id = Some(words.next()?.clone()),
            "--in" => in_secs = pns::parse_count(words.next()?)?,
            "--every" => every = Some(pns::parse_count(words.next()?)?),
            "--unless-marker" => marker = Some(words.next()?.clone()),
            "--until" => {
                let raw = words.next()?;
                until = Some(match raw.strip_prefix('+') {
                    Some(seconds) => Until::FromNow(pns::parse_count(seconds)?),
                    None => Until::Epoch(pns::parse_count(raw)?),
                });
            }
            _ => return None,
        }
    }
    (!args.is_empty()).then_some(ScheduleRequest {
        id: id?,
        in_secs,
        every,
        until,
        marker,
        args,
    })
}

/// `pns daemon cancel --id <id>`: forget one job.
fn daemon_cancel() -> i32 {
    let argv: Vec<String> = std::env::args_os()
        .skip(3)
        .map(|word| word.to_string_lossy().into_owned())
        .collect();
    let [flag, id] = argv.as_slice() else {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    };
    if flag != "--id" {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    }
    match pns::daemon::cancel(&state_dir(), id) {
        Ok(true) => {
            println!("pns daemon: cancelled `{id}`");
            0
        }
        // NOT AN ERROR. The end state the operator asked for is the one they
        // already have, and a non-zero exit here would make a drill's cleanup
        // step fail the second time it ran.
        Ok(false) => {
            println!("pns daemon: no job named `{id}` was scheduled");
            0
        }
        Err(refusal) => {
            eprintln!("pns daemon: {refusal}");
            1
        }
    }
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
    // WELL PAST THE CHECK'S OWN CAP on both legs, so an answer over that cap
    // still ARRIVES over it: read to the cap exactly and a truncated answer
    // would pass the refusal that exists to catch it.
    let json = run_bounded(json, None, moshi_json_deadline(), PAIRING_READ_MAX);
    let mut plain = Command::new(&binary);
    plain.arg("status");
    let plain = run_bounded(plain, None, moshi_status_deadline(), PAIRING_READ_MAX);
    pns::doctor::pairing_report(json.as_deref(), plain.as_deref())
}

/// How much of moshi's answer is read off the wire, which is NOT the same
/// number as how much of it the check will look at.
///
/// TWICE WHAT `doctor::pairing_report` READS, and the doubling is the whole
/// point of the constant. The reader refuses anything past its own ceiling
/// (`system::run_bounded`), and the check refuses anything past
/// `doctor::ANSWER_MAX`, and those two refusals say DIFFERENT things: over the
/// reader's ceiling nothing usable arrived at all, while over the check's cap
/// moshi-hook ran and said something pns declined to read. Read to the check's
/// cap exactly and the second sentence would be unreachable, so the room
/// between them is what keeps it a state an operator can actually be told
/// about. It is still a bound: a child streaming without end is stopped here.
///
/// ACCEPTED LIMIT, PAST THIS CEILING: a moshi-hook that answers with more than
/// two megabytes is reported as a daemon that DID NOT ANSWER, because that is
/// the only thing the reader can say about an answer it refused to read. A
/// wedged daemon streaming prose is then diagnosed as a dead one, which sends
/// the operator to `brew services restart` rather than to the output. The
/// trade is deliberate: the alternative is reading without a ceiling to be
/// able to describe what came back, and the ceiling is the point.
const PAIRING_READ_MAX: u64 = 2 * pns::doctor::ANSWER_MAX as u64;

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
    match pns::system::readable_state_file(&state_dir().join(DECISIONS), RING_READ_MAX) {
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
    match pns::system::readable_state_file(&state_dir().join(MISSED_NOTIFICATIONS), RING_READ_MAX) {
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

/// Whether the clock is running, said in one line that grades nothing.
///
/// TWO READS THAT COST NOTHING: the heartbeat file, and a count of the spool.
/// IT DOES NOT SIGNAL THE PID, because a pid can be reused and the age of a
/// file the daemon rewrites every second answers the same question honestly.
/// `enabled` COMES FROM THE ONE CONFIG READ the doctor already took, never a
/// second one: a report assembled from two reads of one file can describe a
/// switch the run itself never saw. Its broken-config fallback is ON, the same
/// one `daemon_run` takes, so the report and the service cannot disagree.
fn daemon_line(enabled: bool) -> String {
    let state = state_dir();
    let path = pns::daemon::heartbeat_path(&state);
    // A NON-REGULAR FILE IS NOT A BEAT AND IS NEVER OPENED, the same refusal
    // the spool takes and for a worse reason: `open` on a FIFO blocks until a
    // writer arrives, so a doctor that read whatever it found there would hang
    // instead of printing any of its four states, with the pairing check and
    // the exit code never reached.
    let beat = matches!(std::fs::symlink_metadata(&path), Ok(found) if found.is_file())
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|line| pns::daemon::parse_heartbeat(&line));
    pns::doctor::daemon_line(enabled, beat, now_secs(), pns::daemon::job_count(&state))
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
    // NO MOBILE VERDICT TO CARRY: the one leg is hermes, so the mobile table
    // was never read on this path and the default states exactly that.
    let outcomes = dispatch_legs(&[leg], false, &event, home, &Mobile::default(), hermes_key);
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

/// The first ancestor of `path` that exists in its own right but resolves to
/// nothing, and why it does not resolve.
///
/// WHAT THIS IS FOR: `NotFound` at the config path is not proof the config is
/// absent. A dangling link ANYWHERE ABOVE it (`~/.config/pns` naming a
/// directory that was moved or never created) fails the leaf's own stat with
/// ENOENT, exactly as a genuinely missing config does. Told apart nowhere,
/// that reading walks the whole questionnaire and only fails at publication,
/// with every answer already typed and every secret already handed over.
///
/// IT CLIMBS ONLY AS FAR AS THE FIRST COMPONENT THAT EXISTS. Above that
/// everything resolves by definition, and below it the components really are
/// missing, which is the ordinary first run this must not refuse.
fn unresolvable_ancestor(path: &Path) -> Option<(PathBuf, std::io::Error)> {
    // `skip(1)`: `path` ITSELF has already been stated by the caller, and it
    // is the leaf's own `NotFound` that brought us here.
    for ancestor in path.ancestors().skip(1) {
        match ancestor.symlink_metadata() {
            // NOT THERE AS A NAME AT ALL: keep climbing. The component under
            // it is genuinely missing rather than broken.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            // UNREADABLE, NOT ABSENT: refuse by the same rule the leaf's own
            // non-NotFound arm refuses under.
            Err(error) => return Some((ancestor.to_path_buf(), error)),
            // A NAME IS STANDING HERE. Whether it LEADS anywhere is the whole
            // question: `metadata` follows the link `symlink_metadata` did
            // not, so a dangling one (or a loop, or a file where a directory
            // belongs) answers with its own cause here.
            Ok(_) => {
                return match ancestor.metadata() {
                    Ok(_) => None,
                    Err(error) => Some((ancestor.to_path_buf(), error)),
                };
            }
        }
    }
    None
}

/// The `setup` mode: the first-run walk, and the only writer of the config.
///
/// A THIN EDGE OVER A PURE COMPOSER. Everything about what lands in the file
/// is `pns::setup`; this asks, reads a line, and publishes. It EXITS NON-ZERO
/// on every refusal, which the always-exit-0 contract permits for the same
/// reason `quiet` does: that contract covers the hook and notification paths,
/// where a non-zero exit fails the turn being reported on, and this is hand
/// typed and is never a hook.
///
/// IT REFUSES A WALK NOBODY CAN ANSWER. Without a terminal there is no walk,
/// and guessing every answer would write a config the operator never agreed
/// to, over one they may already have.
fn setup_mode() -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(2)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let force = match arguments.as_slice() {
        [] => false,
        [word] if word == "--force" => true,
        // ANY OTHER WORD IS A REFUSAL, never a silent fallthrough to the walk:
        // a mistyped `--force` that walked anyway would ask ten questions and
        // then refuse at the end, over a config it was told to replace.
        _ => {
            eprintln!("{SETUP_USAGE}");
            return 2;
        }
    };
    // AN EMPTY HOME IS REFUSED BY NAME, before the config is even located: an
    // unset or empty HOME would otherwise compose a config path relative to
    // the current directory, which is not the operator's own machine-wide
    // config no matter where this happened to be run from.
    let Some(home) = std::env::var("HOME").ok().filter(|home| !home.is_empty()) else {
        eprintln!("pns setup: HOME is unset or empty; nothing was written");
        return 2;
    };
    // THE CONFIG IS CHECKED BEFORE THE TERMINAL IS, because it is the more
    // specific answer: an operator who already has one is told that, whether
    // or not they are sitting in front of the questions.
    let path = config_path(&home);
    // `symlink_metadata`, NOT `exists`: `exists` follows a symlink and asks
    // what it resolves to, so a dangling one at the config name reads as
    // nothing at all here and the whole walk runs before the publish refuses
    // it with a claim that it "appeared while the questions were being
    // answered", which would not be true.
    match path.symlink_metadata() {
        Ok(_) if !force => {
            eprintln!(
                "pns setup: {} already exists; pass --force to replace it, \
                 which keeps the old file beside it",
                path.display()
            );
            return 2;
        }
        Ok(_) => {}
        // NOTHING AT THE NAME IS NOT YET NOTHING IN THE WAY: a dangling link
        // above the config reports `NotFound` here too, and it refuses
        // REGARDLESS OF `--force`, because what `--force` agrees to replace
        // is a config, not a path that leads nowhere.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some((ancestor, cause)) = unresolvable_ancestor(&path) {
                eprintln!(
                    "pns setup: {} could not be checked: {} does not resolve ({cause}); \
                     nothing was written",
                    path.display(),
                    ancestor.display()
                );
                return 2;
            }
        }
        // ANY OTHER ERROR REFUSES REGARDLESS OF --force: the comment above
        // only holds for NotFound, and a directory this walk cannot even
        // stat is not one it can safely publish into either.
        Err(error) => {
            eprintln!(
                "pns setup: {} could not be checked: {error}; nothing was written",
                path.display()
            );
            return 2;
        }
    }
    if !std::io::stdin().is_terminal() {
        eprintln!(
            "pns setup: this is a walk through questions and stdin is not a terminal; \
             nothing was written"
        );
        return 2;
    }
    let answers = match walk() {
        Ok(answers) => answers,
        Err(reason) => {
            eprintln!("pns setup: {reason}; nothing was written");
            return 2;
        }
    };
    let composed = pns::setup::compose_config(&answers);
    // THROUGH THE ENGINE'S OWN PARSER BEFORE IT IS PUBLISHED. A wizard that
    // writes a config pns then refuses is worse than no wizard: it leaves a
    // machine falling back to the core with a complaint nobody is standing in
    // front of, and it does it while the operator is being told it worked.
    if let Err(error) = pns::config::parse_config(&composed) {
        eprintln!(
            "pns setup: what it composed does not load ({}); nothing was written",
            error.detail()
        );
        return 2;
    }
    match publish_config(&path, &composed, force) {
        Ok(backup) => {
            if let Some(backup) = backup {
                println!("pns setup: kept the old config at {}", backup.display());
            }
            println!("pns setup: wrote {}", path.display());
            0
        }
        Err(refusal) => {
            eprintln!("pns setup: {refusal}");
            1
        }
    }
}

/// The walk itself: one question at a time, in the order the file is written.
///
/// NONE OF THIS DECIDES ANYTHING. Every answer is carried to the composer as
/// it was typed, and a blank one is what declines a feature there. An `Err`
/// is the walk ending mid-conversation, named by its own reason, which
/// publishes nothing at all rather than composing a file out of half of one.
///
/// THE CREDENTIALS ARE ASKED INSIDE THE WALK, right after the feature they
/// arm, because a feature switched on now and credentialed later is exactly
/// the empty-value config this wizard exists to avoid.
fn walk() -> Result<pns::setup::Answers, String> {
    println!("{SETUP_PREAMBLE}");
    let mut answers = pns::setup::Answers {
        mobile_token: ask_hidden(
            "The phone card is on. Paste moshi's webhook secret to complete it, \
             or press enter to pair later",
        )?,
        ..Default::default()
    };

    if ask_yes("Post every event to hermes, for the durable log and the recap?")? {
        answers.hermes_key = armed_secret("hermes", "the signing key that route verifies")?;
    }
    if ask_yes("Flash hue lights green when work finishes and red when it dies?")? {
        // EACH ANSWER GATES THE NEXT QUESTION: once one comes back empty the
        // feature is already declined, and the rest would be questions whose
        // answers are thrown away.
        answers.hue_bridge = armed("the light pulse", "the hue bridge's address on the network")?;
        if !answers.hue_bridge.is_empty() {
            answers.hue_key = armed_secret("the light pulse", "an API key the bridge issued")?;
        }
        if !answers.hue_key.is_empty() {
            answers.hue_rooms = list(armed(
                "the light pulse",
                "the rooms to flash, comma separated, spelled as the bridge spells them",
            )?);
        }
    }
    if ask_yes("Read whether your phone is on the home wifi, off the router's client list?")? {
        // THE BACKEND HAS A WORKING DEFAULT and every other field here does
        // not, so this is the one question enter answers rather than declines.
        // A NAME NOTHING ANSWERS DECLINES THE PROBE, said here and not only in
        // the file: the composer writes that answer's table commented out, and
        // an operator who typed their router's brand deserves to hear why.
        match router_backend(&ask(&format!(
            "Which router backend? [{}]",
            pns::home::UNIFI_TYPE
        ))?) {
            None => println!(
                "  nothing here reads that router, so the home probe stays off; \
                 the file says how to arm it"
            ),
            Some(backend) => {
                answers.router_type = backend.to_string();
                answers.router_url = armed("the home probe", "the router's URL")?;
                if !answers.router_url.is_empty() {
                    answers.router_api_key =
                        armed_secret("the home probe", "an API key the router issued")?;
                }
                if !answers.router_api_key.is_empty() {
                    answers.router_device_hostname =
                        armed("the home probe", "the phone's hostname on that router")?;
                }
            }
        }
    }
    if ask_yes("Hold notifications back while a macOS Focus is on?")? {
        answers.focus_modes = list(armed(
            "focus silencing",
            "which Focus modes mean it, comma separated",
        )?);
    }
    answers.nag = ask_yes("Card you a second time about an approval left unanswered?")?;
    Ok(answers)
}

/// One credentialed answer, and the line that says what a blank one costs.
///
/// SAID WHEN IT HAPPENS rather than only in the file: an operator who meant to
/// arm a feature and pressed enter has one chance to notice, and the composed
/// file's own commented block is read later if at all.
fn armed(feature: &str, wanted: &str) -> Result<String, String> {
    Ok(nothing_given(feature, ask(wanted)?))
}

/// The same shape as `armed`, for a secret: read with the terminal's echo
/// held off, because this is where the token, the hermes key, the hue key
/// and the router key are all asked.
fn armed_secret(feature: &str, wanted: &str) -> Result<String, String> {
    Ok(nothing_given(feature, ask_hidden(wanted)?))
}

/// What `armed` and `armed_secret` share: the line a blank answer costs.
fn nothing_given(feature: &str, answer: String) -> String {
    if answer.is_empty() {
        println!("  nothing given, so {feature} stays off; the file says how to arm it");
    }
    answer
}

/// One question, and the line typed back. An `Err` names why nothing did: the
/// input ending and a read failing are different reasons, and this walk asks
/// for pasted answers, so a byte that is not valid UTF-8 is not a rare guest.
fn ask(question: &str) -> Result<String, String> {
    print!("{question}: ");
    let _ = std::io::stdout().flush();
    read_answer()
}

/// The same question, answered with the terminal's echo held off so a typed
/// secret never reaches the pane grid, herdr's persisted pane history, or any
/// attached client. THE GUARD ARMS BEFORE THE PROMPT PRINTS: arming after
/// would leave a window in which the prompt is already visible but echo is
/// still on, so an operator who types ahead of it, or this crate's own pty
/// test, could still have a secret echoed before `TCSAFLUSH` takes hold.
///
/// ONE CLIENT IS OUTSIDE THIS GUARD'S REACH: mosh, the transport under a
/// Moshi-connected phone, predicts keystrokes locally and can draw them on
/// that client transiently, ahead of the terminal's own echo state. Nothing
/// here controls that.
///
/// Ctrl-C, Ctrl-\, Ctrl-Z, a TERM or HUP, an alarm, and the two tty-stop
/// signals a backgrounded read raises are all held for the read rather than
/// answered immediately, the same trade `readpassphrase(3)` makes: each is
/// still delivered, just not until the guard drops, so Ctrl-C takes effect at
/// the next Enter rather than instantly.
fn ask_hidden(question: &str) -> Result<String, String> {
    let _hushed = Hushed::arm()?;
    print!("{question}: ");
    let _ = std::io::stdout().flush();
    read_answer()
}

/// What every read shares, hidden or not.
fn read_answer() -> Result<String, String> {
    let mut typed = String::new();
    match std::io::stdin().read_line(&mut typed) {
        Ok(0) => Err("the answers ended before the walk did".to_string()),
        Err(error) => Err(read_failure(&error, reading_from_the_background())),
        Ok(_) => Ok(answered(&typed)),
    }
}

/// Whether stdin's terminal is currently owned by some OTHER process group.
///
/// A FAILED `tcgetpgrp` IS NOT THIS CASE: a terminal that hung up answers -1
/// as well, and a read that failed on a dead terminal really did fail for its
/// own reason. A zero is no foreground group at all, which is not this either.
fn reading_from_the_background() -> bool {
    let foreground = unsafe { libc::tcgetpgrp(libc::STDIN_FILENO) };
    foreground > 0 && foreground != unsafe { libc::getpgrp() }
}

/// Why a read failed, in terms the operator can act on.
///
/// EIO FROM A BACKGROUND JOB IS NOT AN I/O FAULT, it is job control. The
/// hidden read blocks SIGTTIN, which is the set `readpassphrase(3)` holds and
/// what stops a suspension from stranding the terminal echo-off. termios(4)
/// names the trade directly: a background process that blocks or ignores
/// SIGTTIN gets `EIO` from the read "and no signal is sent", where an
/// unblocked one would have been stopped and could be resumed with `fg`.
///
/// Passed straight through, `pns setup &` therefore refuses with "Input/output
/// error", which names the symptom and hides the only thing the operator can
/// do about it. BOTH HALVES ARE REQUIRED: a bare EIO on a hung-up terminal is
/// a real failure, and a non-EIO error from the background (a non-UTF-8 paste,
/// say) still has its own honest reason to give.
fn read_failure(error: &std::io::Error, in_background: bool) -> String {
    if in_background && error.raw_os_error() == Some(libc::EIO) {
        return "this walk cannot read the terminal from the background; \
                bring it to the foreground with fg"
            .to_string();
    }
    format!("the answers could not be read: {error}")
}

/// Turns the terminal's echo off for as long as it lives. `Drop` restores
/// both the termios state and the signal mask it holds, on every exit path
/// including EOF and an unwinding panic: this crate carries no
/// `panic = "abort"`, so Drop always runs. Arming and the restore both apply
/// `TCSAFLUSH`, which also discards whatever was already queued, so a secret
/// typed ahead of its own prompt is lost rather than read, and so is an
/// answer typed ahead of the question after it.
struct Hushed {
    original: libc::termios,
    original_mask: libc::sigset_t,
}

impl Hushed {
    /// Arm the guard. FAILS CLOSED: a termios or signal call this cannot
    /// complete is refused as loudly as a bad answer, rather than silently
    /// leaving echo on and asking for a secret anyway.
    fn arm() -> Result<Hushed, String> {
        let mut original: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut original) } != 0 {
            return Err(format!(
                "the terminal's settings could not be read (tcgetattr: {})",
                std::io::Error::last_os_error()
            ));
        }
        let mut blocked: libc::sigset_t = unsafe { std::mem::zeroed() };
        if unsafe { libc::sigemptyset(&mut blocked) } != 0 {
            return Err(format!(
                "the signal mask could not be built (sigemptyset: {})",
                std::io::Error::last_os_error()
            ));
        }
        // BLOCKED FOR THE READ, not disabled: each one is still delivered,
        // once the guard drops and the mask is restored. THIS IS THE WHOLE
        // SET `readpassphrase(3)` HOLDS, all nine of them, because the doc
        // comment above cites that function as the model and a quietly
        // shorter set is the model's holes without its name. SIGTTIN and
        // SIGTTOU: a read that becomes a background job would otherwise be
        // stopped by SIGTTIN with echo still off, and Drop's own
        // `tcsetattr` from a background group can raise SIGTTOU before it
        // gets the chance to restore. SIGALRM: an alarm armed before the
        // walk began would otherwise end the process mid-prompt, and a
        // process that dies before `Drop` leaves the operator's terminal
        // echo-off with no prompt in front of it. SIGPIPE is inert today
        // (the Rust runtime sets it to `SIG_IGN` before `main`, so it ends
        // nothing to begin with) and is held anyway, so this set does not
        // have to be re-argued against the manual page every time the
        // runtime's own default moves.
        for signal in [
            libc::SIGINT,
            libc::SIGQUIT,
            libc::SIGTSTP,
            libc::SIGTERM,
            libc::SIGHUP,
            libc::SIGTTIN,
            libc::SIGTTOU,
            libc::SIGALRM,
            libc::SIGPIPE,
        ] {
            if unsafe { libc::sigaddset(&mut blocked, signal) } != 0 {
                return Err(format!(
                    "the signal mask could not be built (sigaddset: {})",
                    std::io::Error::last_os_error()
                ));
            }
        }
        let mut original_mask: libc::sigset_t = unsafe { std::mem::zeroed() };
        // `pthread_sigmask` IS POSIX, NOT BSD `errno`-STYLE: it RETURNS its
        // error number directly rather than setting errno, so the result
        // itself, not `last_os_error()`, is the only honest source for one.
        let masked =
            unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut original_mask) };
        if masked != 0 {
            return Err(format!(
                "signals could not be held for the read (pthread_sigmask: {})",
                std::io::Error::from_raw_os_error(masked)
            ));
        }
        let mut hushed = original;
        hushed.c_lflag &= !libc::ECHO;
        hushed.c_lflag |= libc::ECHONL;
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &hushed) } != 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                libc::pthread_sigmask(libc::SIG_SETMASK, &original_mask, std::ptr::null_mut());
            }
            return Err(format!(
                "the terminal's echo could not be turned off (tcsetattr: {error})"
            ));
        }
        Ok(Hushed {
            original,
            original_mask,
        })
    }
}

impl Drop for Hushed {
    fn drop(&mut self) {
        unsafe {
            // TERMIOS FIRST, THEN THE MASK: a signal delivered between the
            // two would otherwise run with the operator's terminal still
            // echo-off. Neither call's failure is checked: a tty that hung
            // up during the read (EOF from a closed pty) makes `tcsetattr`
            // fail, and Drop must never panic over a terminal already gone.
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &self.original);
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.original_mask, std::ptr::null_mut());
        }
    }
}

/// What a typed line means as an answer.
///
/// A LINE OF NOTHING BUT SPACES IS A BLANK ONE, which is the rule the whole
/// walk rests on: `compose_config` declines a feature whose credential is
/// empty, and it asks `is_empty`, so a credential of two spaces would arm a
/// plugin with two spaces and deliver nothing while reading as set up. That is
/// the exact state this wizard exists to keep off a fresh machine, and the
/// trailing newline every line carries is what makes it reachable.
fn answered(line: &str) -> String {
    line.trim().to_string()
}

/// One yes-or-no question. ENTER MEANS NO, and so does anything that is not a
/// yes: this walk arms features that deliver to a phone and to lamps, and the
/// answer nobody typed on purpose must be the one that changes nothing.
fn ask_yes(question: &str) -> Result<bool, String> {
    Ok(means_yes(&ask(&format!("{question} [y/N]"))?))
}

/// Whether an answer to a yes-or-no question was a yes.
///
/// ONLY A YES IS ONE. Enter, a word nobody meant, and a mistyped `yes` all
/// mean no, because every question this answers arms something that delivers
/// to a phone or to a lamp and takes a credential to do it.
fn means_yes(answer: &str) -> bool {
    matches!(answer.to_lowercase().as_str(), "y" | "yes")
}

/// Which compiled-in backend an answer names, or `None` for one no backend
/// answers.
///
/// THE SET IS THE CODE'S, never a list kept here: `home` is what refuses a
/// type at probe time, so a wizard restating its own copy of that set would go
/// on accepting yesterday's answer the day a second backend lands. Enter names
/// the one there is, and a spelling that differs only in case is that one too,
/// written back as the code spells it rather than as it was typed.
fn router_backend(answer: &str) -> Option<&'static str> {
    (answer.is_empty() || answer.eq_ignore_ascii_case(pns::home::UNIFI_TYPE))
        .then_some(pns::home::UNIFI_TYPE)
}

/// A comma-separated answer as the values it names, blanks dropped.
fn list(answer: String) -> Vec<String> {
    answer
        .split(',')
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

/// Publish the composed config, keeping the old one when replacing it.
///
/// CREATE-IF-ABSENT, NEVER A BLANKET RENAME, on both paths: a config that
/// appeared between the check in `setup_mode` and this moment is another
/// writer's, and this run has not read it. The link failing with
/// `AlreadyExists` IS that refusal. NOTHING ASKS WHETHER A CONFIG IS THERE
/// either, because the answer stops being true the instant it is given: what
/// `--force` moves aside is the file it found at the name, and what it
/// publishes into is a name it emptied itself.
///
/// THE OLD CONFIG IS MOVED ASIDE RATHER THAN COPIED ASIDE, so the backup holds
/// what was actually replaced rather than what stood there when a copy ran, and
/// the old config is at one of the two names at every instant.
///
/// THE PENDING FILE CARRIES THE MODE, because it is what gets published:
/// writing at the umask would publish a config whose plugin secrets any
/// process on the machine can read.
fn publish_config(path: &Path, composed: &str, force: bool) -> Result<Option<PathBuf>, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no directory to write in", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("{} could not be created: {error}", parent.display()))?;
    let pending = parent.join(pending_name());
    // CREATED OR NOT AT ALL, and never opened. A pending file is a second name
    // for the live config between the link that publishes it and the unlink
    // that removes it, so an abandoned run leaves one behind and process ids
    // are reused: an open that truncates would empty a config this run has not
    // read, and the backup taken next would hold the replacement.
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(CONFIG_FILE_MODE)
        .open(&pending)
        .map_err(|error| format!("{} could not be written: {error}", pending.display()))?;
    let published = write_then_publish(path, &pending, file, composed, force);
    // WHICHEVER WAY IT WENT, and only ever the file the line above made: a
    // pending file left in the config directory would be read by nobody and
    // found by everybody, and removing one this run did not create is the
    // mirror of the write it refuses to do.
    let _ = std::fs::remove_file(&pending);
    published
}

/// The name the composed config is written under before it is published.
///
/// THE MOMENT AS WELL AS THE PROCESS, because the create above is exclusive: a
/// leftover from an abandoned run of the same id would otherwise refuse a
/// wizard nobody can unblock, and a name nothing else is holding is also a
/// name nothing else can be waiting at.
fn pending_name() -> String {
    format!(
        "config.toml.new.{}.{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since_epoch| since_epoch.subsec_nanos())
    )
}

/// The publish itself, with `publish_config` owning the cleanup around it.
fn write_then_publish(
    path: &Path,
    pending: &Path,
    mut file: std::fs::File,
    composed: &str,
    force: bool,
) -> Result<Option<PathBuf>, String> {
    // AND AGAIN AFTER THE OPEN, for `publish_state_line`'s reason: the mode an
    // open asks for is masked by the umask, and a config published without the
    // operator's own bits is one they cannot read.
    file.set_permissions(std::fs::Permissions::from_mode(CONFIG_FILE_MODE))
        .map_err(|error| format!("{} could not be secured: {error}", pending.display()))?;
    file.write_all(composed.as_bytes())
        .map_err(|error| format!("{} could not be written: {error}", pending.display()))?;

    // THE FORCED PATH EMPTIES THE NAME FIRST, and what it moves out of the way
    // is the backup. Nothing here asks whether a config is there: the move
    // itself is the answer, and it is the same answer a moment later.
    let kept = if force { keep_aside(path)? } else { None };
    // AND BOTH PATHS PUBLISH THE SAME WAY. A link that refuses an occupied
    // name cannot write over a config this run never read: after the dangling
    // symlink pre-check in `setup_mode`, the only way a config can be
    // standing here is a genuine arrival while the questions were being
    // answered, so "appeared" below is exact rather than one of two guesses.
    match std::fs::hard_link(pending, path) {
        Ok(()) => Ok(kept),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Err(format!(
            "{} appeared while the questions were being answered; \
             nothing was written over it{}",
            path.display(),
            also_kept(kept.as_deref())
        )),
        Err(error) => Err(format!(
            "{} could not be written: {error}{}",
            path.display(),
            also_kept(kept.as_deref())
        )),
    }
}

/// The tail a refusal carries when this run had already moved a config aside,
/// so nobody is left hunting for a file the wizard took the name of.
fn also_kept(kept: Option<&Path>) -> String {
    kept.map_or_else(String::new, |backup| {
        format!(
            "; the config that was there is kept at {}",
            backup.display()
        )
    })
}

/// Move the existing config aside, and answer with where it went.
///
/// A MOVE RATHER THAN A COPY, which is what makes the answer true: a copy says
/// only what stood at the name when the copy ran, and the publish that follows
/// replaces whatever stands there THEN. Moving it is the one act that both
/// keeps the old config and frees the name, so the two can never disagree.
///
/// NOTHING TO MOVE IS NOT A FAILURE: `--force` on a machine with no config is
/// an ordinary first run.
fn keep_aside(path: &Path) -> Result<Option<PathBuf>, String> {
    let now = now_secs().ok_or_else(|| {
        "the clock cannot be read, so the config already there cannot be named \
         and kept; nothing was written"
            .to_string()
    })?;
    keep_aside_at(path, now)
}

/// `keep_aside` with the moment NAMED rather than read.
///
/// THE SPLIT EXISTS FOR THE TEST, and the test is what makes it worth having.
/// With the clock read in here, a test that pre-claims a backup name has to
/// read the clock itself and hope neither read lands on the far side of a
/// second boundary. Pre-claiming both candidate names only narrows that
/// window: a thread parked across more than one boundary still picks a third
/// name and the test fails on a working build. Naming the second removes the
/// race instead of shrinking it.
fn keep_aside_at(path: &Path, epoch_secs: u64) -> Result<Option<PathBuf>, String> {
    let backup = pns::setup::backup_path(path, epoch_secs).ok_or_else(|| {
        format!(
            "{} cannot be named for keeping, so the config already there \
             cannot be kept; nothing was written",
            path.display()
        )
    })?;
    // THE NAME IS CLAIMED BEFORE ANYTHING MOVES ONTO IT, so a second forced run
    // inside the same second refuses rather than writing over the copy the
    // first one kept: a rename would replace that copy without a word.
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(CONFIG_FILE_MODE)
        .open(&backup)
        .map_err(|error| match error.kind() {
            // THE NAME BEING TAKEN PROVES NOTHING ABOUT WHAT IT HOLDS: a run
            // killed between this claim and the rename that follows it
            // leaves an empty file at the same name, so the refusal says
            // only that the name is spoken for, not what a prior run "kept"
            // there.
            std::io::ErrorKind::AlreadyExists => format!(
                "{} is already claimed by another run this same second; \
                 nothing was written",
                backup.display()
            ),
            // ANY OTHER FAILURE IS ITS OWN REASON: naming the same-second
            // collision for a permission refusal would blame a run that
            // never happened.
            _ => format!("{} could not be claimed: {error}", backup.display()),
        })?;
    if let Err(error) = std::fs::rename(path, &backup) {
        // THE CLAIM GOES WITH THE RUN THAT MADE IT, whether there was nothing
        // to move or the move could not be made: an empty file named like a
        // backup is worse than no backup at all.
        let _ = std::fs::remove_file(&backup);
        return match error.kind() {
            std::io::ErrorKind::NotFound => Ok(None),
            // THE BACKUP WAS NEVER THE PROBLEM HERE: it is a fresh file this
            // call just created, and what could not be moved onto it is
            // `path` itself, so the refusal names that instead.
            _ => Err(format!(
                "{} could not be moved aside to keep it: {error}",
                path.display()
            )),
        };
    }
    // AS PRIVATE AS THE CONFIG IT HOLDS, when what moved is a file at all: the
    // mode of a symlink is the mode of what it points at, and this one points
    // at a file this run did not replace and has no business changing.
    if backup.symlink_metadata().is_ok_and(|entry| entry.is_file()) {
        std::fs::set_permissions(&backup, std::fs::Permissions::from_mode(CONFIG_FILE_MODE))
            .map_err(|error| format!("{} could not be secured: {error}", backup.display()))?;
    }
    Ok(Some(backup))
}

/// The config carries every plugin's secret, so it is the operator's alone.
const CONFIG_FILE_MODE: u32 = 0o600;

/// What the walk says before it starts asking.
const SETUP_PREAMBLE: &str = "\
pns setup: a few questions, and a config at the end of them.
The macOS banner and the phone card are on and are not asked about. Everything
else is off unless you arm it here, and enter is no. Nothing is written until
the last answer.";

/// What a setup typed wrong is told.
const SETUP_USAGE: &str =
    "pns: usage: pns setup [--force]; --force replaces an existing config, keeping it beside";

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
/// READ THROUGH `readable_state_file` for the reasons that function states about
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
    let assertions =
        pns::system::readable_state_file(&store.join("Assertions.json"), RING_READ_MAX)?;
    let catalog =
        pns::system::readable_state_file(&store.join("ModeConfigurations.json"), RING_READ_MAX);
    Ok(FocusReading {
        silenced: pns::focus::silenced(
            &pns::focus::active_modes(&assertions),
            &pns::focus::mode_names(catalog.as_deref().unwrap_or_default()),
            silence,
        ),
        catalog: catalog.as_ref().err().map(std::io::Error::kind),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_test_support::*;
    use std::cell::RefCell;
    use std::os::unix::fs::MetadataExt;
    #[test]
    fn an_unreadable_lights_quiet_complains_and_an_absent_one_says_nothing() {
        // THE DIFFERENCE BETWEEN "NOBODY EVER RAN THE COMMAND" AND "THIS FILE
        // CANNOT BE READ", which both readers of the ad-hoc quiet depend on:
        // every read failure mutes nothing, so the second one has to be said
        // out loud or the operator believes a mute is on while every lamp goes
        // loud at 3am.
        let state = scratch("muted-state");
        assert_eq!(
            muted_state(&state),
            (Vec::new(), Vec::new()),
            "no file at all is the ordinary case and says nothing"
        );
        let file = state.join("lights-quiet");
        std::fs::create_dir(&file).expect("a directory standing where the file goes");
        let (entries, complaints) = muted_state(&state);
        assert!(
            entries.is_empty() && complaints.len() == 1,
            "a directory mutes nothing and is complained about once: \
             {entries:?} {complaints:?}"
        );
        assert!(
            complaints[0].starts_with("pns: state error (lights-quiet could not be read:"),
            "and the complaint names the file and what went wrong: {}",
            complaints[0]
        );
        std::fs::remove_dir(&file).expect("the directory goes");
        std::fs::write(&file, [0x66, 0xff, 0xfe]).expect("bytes that are not UTF-8");
        let (entries, complaints) = muted_state(&state);
        assert!(
            entries.is_empty() && complaints.len() == 1,
            "and so does a file that is not text: {entries:?} {complaints:?}"
        );
        // AND WHAT AN UNREADABLE ONE MUTES IS EVERYTHING, which is the fail
        // direction on a lamp path and the opposite of what it used to do: a
        // record nobody can parse says nothing about which places are quiet,
        // and read as an empty list it was a house with every lamp loud.
        assert_eq!(
            ad_hoc_quiet(&state, Some(1_000)).0,
            pns::channels::hue::Muting::Everything
        );
        std::fs::write(&file, "9999999999 3F - Studio\n").expect("a file it can read");
        assert_eq!(
            muted_state(&state).1,
            Vec::<String>::new(),
            "the control: a file it can read complains about nothing"
        );
        assert_eq!(
            ad_hoc_quiet(&state, Some(1_000)),
            (
                pns::channels::hue::Muting::Places(vec!["3F - Studio".to_string()]),
                Vec::new()
            ),
            "and it mutes exactly the place the file names"
        );
        // A CLOCK THAT WILL NOT ANSWER GOES THE SAME WAY. Nothing can judge a
        // mute live without one, and the direction is dark rather than loud.
        //
        // THE LITERAL SENTENCE, never the constant: a mutation that renamed
        // or emptied `NO_CLOCK_FOR_THE_MUTE` and every reader of it together
        // would still pass a comparison against itself.
        let (muting, complaints) = ad_hoc_quiet(&state, None);
        assert_eq!(muting, pns::channels::hue::Muting::Everything);
        assert_eq!(
            complaints,
            vec![
                "pns lights: the clock cannot be read, so no mute can be judged \
                 live; every lamp is quiet until it can"
                    .to_string()
            ]
        );
    }

    #[test]
    fn only_a_word_no_declaration_accounts_for_is_worth_a_bridge_listing() {
        // THE MUTE'S VOCABULARY IS BOTH SOURCES, and the bridge half costs a
        // human three round trips while they stand at a terminal. A place the
        // config already declares can be enforced whatever the bridge says, so
        // the ordinary bedtime mute must not pay for a listing that cannot
        // change the answer.
        let declared = vec!["3F - Studio".to_string()];
        let typed = |words: &[&str]| -> Vec<String> {
            words.iter().map(|word| (*word).to_string()).collect()
        };
        assert!(!asks_the_bridge(&declared, &typed(&[])), "the bare report");
        assert!(!asks_the_bridge(&declared, &typed(&["3F - Studio"])));
        assert!(!asks_the_bridge(&declared, &typed(&["3F - Studio", "2h"])));
        assert!(
            !asks_the_bridge(&declared, &typed(&["3F - Nowhere", "off"])),
            "`off` is allowed over any name, so no listing could change it"
        );
        // AND THE ONE CASE A LISTING DECIDES: a name no declaration holds may
        // still be a real lamp, room or zone, which is the whole grammar.
        assert!(asks_the_bridge(&declared, &typed(&["3F - Studio - HCL1"])));
        assert!(asks_the_bridge(
            &declared,
            &typed(&["3F - Studio - HCL1", "2h"])
        ));
    }

    #[test]
    fn a_held_record_that_is_absent_holds_nothing_and_one_that_will_not_read_holds_everything() {
        // TWO DIFFERENT FACTS, and collapsing them into an empty list is what
        // let a blink write straight over a lamp that was breathing. The
        // ORDINARY case is a machine holding nothing at all, which is an absent
        // file; a file that exists and cannot be read says nothing about which
        // lamps are held, and the gate that reads it decides whether a pulse
        // fires over one.
        let state = scratch("held-record-absent-or-unreadable");
        assert_eq!(
            held_lamps(&state),
            Some(Vec::new()),
            "no file at all is a house holding nothing"
        );
        std::fs::create_dir(state.join(LIGHTS_HELD)).expect("a directory where the record goes");
        assert_eq!(
            held_lamps(&state),
            None,
            "and one nobody can read is unknown"
        );
    }

    #[test]
    fn a_held_records_phase_round_trips_through_remember_held_and_read_held() {
        // ONE PARSER, ONE RENDERER, so a phase written by `remember_held`
        // reads back exactly through `read_held`, and `held_lamps` (the three
        // bare-path consumers' own read) sees the same path with the phase
        // silently dropped.
        let state = scratch("held-record-phase-round-trip");
        let phased = pns::lights::HeldEntry {
            path: LAMP_PATH.to_string(),
            resume: Some(pns::lights::Phase {
                end_unix_ms: 1_700_000_000_123,
                landed_on: 100,
                held: pns::lights::Held::Blocked,
            }),
        };
        remember_held(&state, std::slice::from_ref(&phased)).expect("the write lands");
        assert_eq!(
            read_held(&state),
            Some(vec![phased]),
            "the phase round-trips through the same file"
        );
        assert_eq!(
            held_lamps(&state),
            Some(vec![LAMP_PATH.to_string()]),
            "and the bare consumers see only the path"
        );
    }

    #[test]
    fn a_bare_token_on_disk_still_reads_as_a_held_lamp_with_no_phase() {
        // THE FORMAT A HAND-WRITTEN OR OLDER-BUILD RECORD USES, and every test
        // above that writes `LAMP_PATH\n` directly to the file: a bare token
        // is a lamp this record holds with no phase, never an unreadable
        // record.
        let state = scratch("held-record-bare-token");
        std::fs::write(state.join(LIGHTS_HELD), format!("{LAMP_PATH}\n")).expect("the record");
        assert_eq!(
            read_held(&state),
            Some(vec![pns::lights::HeldEntry::bare(LAMP_PATH)])
        );
        assert_eq!(held_lamps(&state), Some(vec![LAMP_PATH.to_string()]));
    }

    #[test]
    fn a_tick_arms_a_held_lamp_records_it_and_a_dark_house_puts_it_out_by_name() {
        // THE ARM, THE RECORD AND THE CLEAR ARE ONE ORDERED TRIO, and this is
        // that trio. Every held body is a plain state write that does NOT
        // expire, so a record written before the clear, or a clear computed
        // before the arm, is a lamp left lit with nothing that knows its name.
        let state = scratch("tick-arms-and-clears");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        let puts = bridge.puts.borrow();
        assert_eq!(
            puts.first().map(|(path, _)| path.as_str()),
            Some(LAMP_PATH),
            "the lamp is addressed individually, never through its room's group: \
             arbitration, the dim window and the mute are each per lamp, and a \
             group write would reach one that answered any of the three differently"
        );
        assert!(
            puts[0].1.contains(r#""x":0.3395"#) && puts[0].1.contains(r#""brightness":30.0"#),
            "the arm states the blocked magenta and the first fade in one write: {}",
            puts[0].1
        );
        assert!(
            puts.len() > 1 && !puts[1].1.contains("color"),
            "and every fade after it states brightness and duration alone"
        );
        assert_eq!(
            held_lamps(&state).as_deref(),
            Some([LAMP_PATH.to_string()].as_slice()),
            "the record carries the lamp, or nothing will ever put it out"
        );
        assert!(
            recorded(&state)
                .expect("a record is on disk")
                .starts_with(&format!("{LAMP_PATH}@")),
            "and the second write, after the breath returns, carries the phase \
             the lamp landed on"
        );

        // THE OTHER DIRECTION, which is what the clear exists for: a house with
        // nothing to show writes to no lamp at all, so the held path really is
        // stale and goes out by name.
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[],
            &noon(&nothing_muted()),
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(LAMP_PATH.to_string(), CLEAR_BODY.to_string())],
            "the lamp is put out by name, off the recorded path, with no listing \
             resolved at all"
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            recorded(&state),
            None,
            "and the tick stops claiming to hold it"
        );
    }

    #[test]
    fn a_phased_record_clears_by_its_bare_path_never_by_the_suffix() {
        // THE SUFFIX A RESUMED BREATH WRITES MUST NEVER LEAK INTO A PUT PATH.
        // A lamp the previous tick recorded with a phase is cleared exactly
        // like a bare one: by the fixture path alone.
        let state = scratch("tick-phased-record-clears-bare");
        std::fs::write(
            state.join(LIGHTS_HELD),
            format!("{LAMP_PATH}@1700000000123:h\n"),
        )
        .expect("a phased record");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[],
            &noon(&nothing_muted()),
            Some(&[pns::lights::HeldEntry {
                path: LAMP_PATH.to_string(),
                resume: Some(pns::lights::Phase {
                    end_unix_ms: 1_700_000_000_123,
                    landed_on: 100,
                    held: pns::lights::Held::Blocked,
                }),
            }]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(LAMP_PATH.to_string(), CLEAR_BODY.to_string())],
            "the clear addresses the bare path, never `{LAMP_PATH}@1700000000123:h`"
        );
        assert!(complaints.is_empty(), "{complaints:?}");
    }

    #[test]
    fn a_lamp_this_arm_wrote_to_stays_held_rather_than_being_put_out_behind_the_arm() {
        // THE CLEAR SUBTRACTS EVERY PATH THIS ARM WROTE TO, and it has to: a
        // held body is a plain state write, so a clear computed as "everything
        // that was held" would PUT the arm and then the off to the same lamp on
        // every single re-arm, in that order, and the lamp would be dark for the
        // whole of every interval after the first.
        let state = scratch("tick-rearm-keeps-the-lamp");
        // THE RECORD ON DISK IS WHAT THE TICK READ, and it has to agree with
        // the reading handed in: the pass stands down when the record moved
        // under it, which is how a return that cleared every lamp mid-tick
        // stops this run re-arming them.
        std::fs::write(state.join(LIGHTS_HELD), format!("{LAMP_PATH}\n")).expect("the record");
        let bridge = scripted(true);
        run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(
            !bridge
                .puts
                .borrow()
                .iter()
                .any(|(_, body)| body == CLEAR_BODY),
            "no off reaches a lamp this arm wrote to: {:?}",
            bridge.puts.borrow()
        );
        assert_eq!(
            held_lamps(&state).as_deref(),
            Some([LAMP_PATH.to_string()].as_slice()),
            "and it is still recorded as held, or nothing will ever put it out"
        );
    }

    #[test]
    fn a_lamp_the_operator_muted_is_not_armed_and_is_put_out_if_it_was_held() {
        // THE MUTE IS A RENDER FILTER AT THE PER-LAMP DECISION, decided once:
        // the lamp simply drops out of the arm, which makes its held path stale
        // and puts it out through the ordinary clear rather than a second path.
        let state = scratch("tick-mute-clears");
        // THE RECORD ON DISK IS WHAT THE TICK READ, and it has to agree with
        // the reading handed in: the pass stands down when the record moved
        // under it, which is how a return that cleared every lamp mid-tick
        // stops this run re-arming them.
        std::fs::write(state.join(LIGHTS_HELD), format!("{LAMP_PATH}\n")).expect("the record");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&quieted("3F - Studio")),
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(LAMP_PATH.to_string(), CLEAR_BODY.to_string())],
            "a muted lamp is armed with nothing and put out if it was lit"
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(recorded(&state), None);
    }

    #[test]
    fn a_mute_reading_nobody_could_take_leaves_every_lamp_quiet_rather_than_loud() {
        // THE FAIL DIRECTION ON A LAMP PATH IS DARK. An unreadable mute record
        // and a clock that would not answer each arrived at the walk as an
        // EMPTY list of quiet places, which is a house with every lamp loud:
        // the one outcome the operator armed the mute to prevent, on the one
        // night the machine could not say why.
        let state = scratch("tick-mute-unreadable");
        // THE RECORD ON DISK IS WHAT THE TICK READ, and it has to agree with
        // the reading handed in: the pass stands down when the record moved
        // under it, which is how a return that cleared every lamp mid-tick
        // stops this run re-arming them.
        std::fs::write(state.join(LIGHTS_HELD), format!("{LAMP_PATH}\n")).expect("the record");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&pns::channels::hue::Muting::Everything),
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            bridge.puts.borrow().as_slice(),
            &[(LAMP_PATH.to_string(), CLEAR_BODY.to_string())],
            "every lamp is quiet, so the lamp is armed with nothing and put out"
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(recorded(&state), None);
    }

    #[test]
    fn a_held_record_that_will_not_publish_stops_the_arm_rather_than_lighting_a_lamp() {
        // A LAMP THE RECORD DOES NOT NAME IS A LAMP NOTHING CAN PUT OUT. Every
        // held body is a plain state write that does not expire, and the next
        // tick, the return from an absence and the operator's own mute all
        // clear BY NAME off this file, so arming after a failed publish is a
        // bulb held by nothing until somebody finds the wall switch.
        let state = scratch("tick-record-unwritable");
        std::fs::create_dir(state.join(LIGHTS_HELD)).expect("a directory where the record goes");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            None,
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(
            bridge.puts.borrow().is_empty(),
            "no lamp is armed once the record refused to land: {:?}",
            bridge.puts.borrow()
        );
        assert!(
            complaints
                .iter()
                .any(|said| said.contains("the held record could not be written")),
            "and the tick says so rather than carrying on quietly: {complaints:?}"
        );
    }

    #[test]
    fn a_child_outlives_the_longest_interval_plus_the_write_and_the_reap_that_follow_it() {
        // THE SEAMLESS BREATH ISSUES ITS LAST FADE INSIDE THE BUDGET AND LETS
        // IT FINISH AFTER, so a tick's child is alive for its whole interval,
        // then for however long that last write takes, and it is only noticed
        // as gone on the reap tick after that. Bounded at the interval alone,
        // the supported thirty-second refresh equalled a thirty-second child,
        // and a legal last write was killed before the tick could record where
        // its breath had landed.
        assert_eq!(
            child_bound(Duration::from_secs(1), LIGHTS_JOB),
            Duration::from_secs(37),
            "at the production clock: thirty seconds of interval, the six-second \
             write deadline that ceiling implies, and one reap tick"
        );
        assert_eq!(
            child_bound(Duration::from_secs(60), LIGHTS_JOB),
            Duration::from_secs(1800),
            "and a slow clock keeps the tick-scaled bound, which is the larger of \
             the two there"
        );
        // AND NO OTHER JOB IS WIDENED BY IT. An event delivery's channels each
        // carry their own deadline, so one still alive at `CHILD_TICKS` is
        // wedged; giving it thirty-seven seconds would only delay the kill.
        assert_eq!(
            child_bound(Duration::from_millis(10), "nag:a-session"),
            Duration::from_millis(300),
            "every job but the lights tick keeps the tick-scaled bound exactly"
        );
    }

    #[test]
    fn three_of_a_ticks_bridge_calls_fit_inside_its_own_interval_with_the_breath_to_spare() {
        // THE PROPERTY, not the arithmetic. The resolve makes three calls before
        // the first fade is issued, and at the transport's own ten seconds they
        // outlive every interval the config permits: a wedged bridge then had
        // tick after tick piling up, each still dialling while the next was
        // spawned. What has to hold is that the three fit with room left for a
        // breath, at both ends of the range the config accepts.
        //
        // EVERY LEGAL INTERVAL AND NOT A SAMPLE OF FOUR. `tick_bridge_deadline`
        // divides by five, so the budget is a STEP FUNCTION of the refresh and
        // a four-point sample walks straight past whichever step is tight.
        let shipped = pns::config::Lights::default();
        let cycles = [
            (
                "the locked blocked shape",
                pns::lights::breath_cycle(&shipped.blocked.breath),
            ),
            (
                "the locked loop motion",
                pns::lights::breathe_then_flare_cycle(&shipped.looping.breathe_then_flare),
            ),
        ];
        for refresh_secs in pns::config::MIN_REFRESH_SECS..=pns::config::MAX_REFRESH_SECS {
            let three = tick_bridge_deadline(refresh_secs).as_millis() * 3;
            let interval = u128::from(refresh_secs) * 1000;
            assert!(
                three < interval,
                "refresh {refresh_secs}s: three calls at {three}ms do not fit"
            );
            let left = u64::try_from(interval - three).expect("a budget in milliseconds");
            for (named, cycle) in &cycles {
                assert!(
                    !pns::lights::breath_fades(left, cycle, pns::lights::Resume::default())
                        .is_empty(),
                    "refresh {refresh_secs}s: the {left}ms left over will not hold one \
                     cycle of {named}"
                );
                // AND RESUMED AT THE WORST A LIVE RECORD CAN LEAVE, which is the
                // case a fresh schedule never reaches: `resume_from` caps a
                // phase at the step of the leg it names, so the latest first
                // fade any tick can inherit is the cycle's longest leg's step.
                // A schedule that comes back EMPTY there is a lamp holding
                // still for a whole interval, which is the one thing a liveness
                // signal must never do.
                let worst = cycle
                    .iter()
                    .map(|leg| pns::lights::step_ms(leg.duration_ms))
                    .max()
                    .expect("a cycle has legs");
                assert!(
                    !pns::lights::breath_fades(
                        left,
                        cycle,
                        pns::lights::Resume {
                            first_due_ms: worst,
                            next_leg: 0,
                        }
                    )
                    .is_empty(),
                    "refresh {refresh_secs}s: {named} resumed a whole {worst}ms step \
                     late has no room in the {left}ms the interval leaves"
                );
            }
        }
    }

    #[test]
    fn a_tick_whose_record_moved_under_it_stands_down_rather_than_re_arming_the_lamps() {
        // THE RACE THE SOURCE USED TO ADMIT TO. The house is derived BEFORE the
        // bridge work, which is seconds of network, and the operator's return
        // clears every held lamp and empties the record in the middle of it: a
        // tick that then published its own snapshot armed the lamps again and
        // the operator watched a lamp they had just put out come back on, with
        // the record naming it once more.
        //
        // THE OTHER WRITER HAS ALREADY DONE THE CLEARING, so standing down is
        // the whole remedy: nothing is armed, nothing is cleared twice, and the
        // next tick reads a house that agrees with the disk.
        let state = scratch("tick-record-moved");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            // WHAT THIS TICK READ before the bridge work, against a record the
            // event path has emptied since.
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(
            bridge.puts.borrow().is_empty(),
            "the lamps were re-armed off a snapshot the disk had already moved past: {:?}",
            bridge.puts.borrow()
        );
        assert_eq!(
            recorded(&state),
            None,
            "and the record the other writer left is not overwritten either"
        );
        assert!(complaints.is_empty(), "{complaints:?}");
    }

    #[test]
    fn a_second_tick_stands_down_while_a_first_still_holds_the_lamps() {
        // THE GUARD THE DAEMON'S OWN BOOKKEEPING CANNOT BE. `decide` refuses to
        // fire a second lights child while the first is listed, and that list is
        // ONE process's memory: a tick the operator ran by hand and an orphan a
        // daemon replacement left behind are both invisible to it. Two ticks
        // driving one lamp interleave their fades, and the phase the last of
        // them writes is the one the next tick resumes off.
        let state = scratch("tick-lock-held");
        std::fs::write(state.join(LIGHTS_TICK_LOCK), "").expect("a lock a live tick holds");
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(
            bridge.puts.borrow().is_empty(),
            "a second tick drove the lamps while the first still held them: {:?}",
            bridge.puts.borrow()
        );
        assert_eq!(
            recorded(&state),
            None,
            "and it wrote no record over the holder's own"
        );
        assert!(complaints.is_empty(), "{complaints:?}");

        // AND A LOCK NO LIVE TICK COULD STILL BE HOLDING IS TAKEN, so an orphan
        // costs one stale window rather than the lamps forever. The moment is
        // handed in rather than waited out: this test never sleeps.
        let long_past_any_holder_ms = 4_000_000_000_000;
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            long_past_any_holder_ms,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert!(
            !bridge.puts.borrow().is_empty(),
            "a lock older than any tick may hold it was never taken, so the lamps \
             stayed dark for as long as the orphan sat there"
        );
        assert!(
            !state.join(LIGHTS_TICK_LOCK).exists(),
            "and the tick that took it never gave it back, which stands every later \
             tick down for a whole stale window"
        );
    }

    #[test]
    fn a_tick_whose_bridge_answered_nothing_keeps_the_record_it_was_holding() {
        // A LISTING THAT FAILED IS DIRECT EVIDENCE THE TRANSPORT IS DOWN, and
        // clearing off it forgets the paths after PUTs nobody can prove landed.
        // The lamp is then lit with nothing left in the system that knows about
        // it: the condition ends, so no later tick has anything held to clear,
        // and the event path reads an empty record and returns without a call.
        let state = scratch("bridge-down-keeps-the-record");
        std::fs::write(state.join(LIGHTS_HELD), format!("{LAMP_PATH}\n")).expect("the held record");

        let bridge = scripted(false);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[pns::lights::HeldEntry::bare(LAMP_PATH)]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert!(
            bridge.puts.borrow().is_empty(),
            "a bridge that answered no listing is written to for nothing: {:?}",
            bridge.puts.borrow()
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            recorded(&state).as_deref(),
            Some(LAMP_PATH),
            "and the record survives the outage, so the next reachable tick still \
             has a name to write the clear to"
        );
    }

    #[test]
    fn two_breathing_lamps_share_one_schedule_rather_than_running_back_to_back() {
        // ONE SLEEP SCHEDULE FOR EVERY LAMP, in due order ACROSS lamps. Issued
        // per lamp instead, every fade of the second lamp would be past due by
        // the time the first lamp's breath ended: all issued at once, late, a
        // jump rather than a breath.
        let bridge = scripted(true);
        // TWO SHAPES THIS TEST OWNS, written out rather than read from
        // `Lights::default()`. They equal the locked blocked and loop shapes as
        // it happens, but the interleave asserted below is the exact due-order
        // these two durations produce, so reading them from the defaults would
        // rewrite the expected order every time a cadence is retuned and this
        // test would start failing for a reason it is not about. Leave these
        // alone when a cadence change sends you grepping for a duration.
        let quick = pns::config::Breath {
            duration_ms: 2000,
            high: 100,
            low: 30,
        };
        let slow = pns::config::Breath {
            duration_ms: 4000,
            high: 60,
            low: 10,
        };
        drive_breaths(
            &bridge,
            12_000,
            &[
                Breathing {
                    path: "light/a".to_string(),
                    held: pns::lights::Held::Blocked,
                    cycle: pns::lights::breath_cycle(&quick),
                    color: pns::pulse::BLOCKED_COLOR,
                    resume: pns::lights::Resume::default(),
                },
                Breathing {
                    path: "light/b".to_string(),
                    held: pns::lights::Held::Looping,
                    cycle: pns::lights::breath_cycle(&slow),
                    color: pns::pulse::LOOP_COLOR,
                    resume: pns::lights::Resume::default(),
                },
            ],
            no_time_passes(),
            |_| {},
        );
        let order: Vec<String> = bridge
            .puts
            .borrow()
            .iter()
            .map(|(path, _)| path.clone())
            .collect();
        assert_eq!(
            order,
            [
                "light/a", "light/b", "light/a", "light/a", "light/b", "light/a", "light/a",
                "light/b", "light/a", "light/a", "light/b",
            ],
            "the fades interleave by their due milliseconds, not by lamp: the quick \
             shape's seven fades and the slow shape's four, seamless past the old \
             stop-at-the-peak count"
        );
    }

    #[test]
    fn a_slow_write_stops_the_schedule_at_the_budget_and_lands_where_it_really_did() {
        // THE SCHEDULE IS NOMINAL AND THE WRITES ARE NOT. Writes are
        // synchronous and sequential, so a lamp answering slowly pushes every
        // later fade past the moment it was due, and the locked blocked shape's
        // seventh fade would be issued three seconds AFTER the budget it
        // belongs to. Two things follow, and both are asserted here: nothing is
        // issued at or past the budget, and the phase left for the next tick is
        // the end of a write that ACTUALLY HAPPENED, timed from when it
        // actually started.
        let clock = FakeClock::default();
        let bridge = SlowBridge {
            clock: &clock,
            get_cost_ms: 0,
            put_cost_ms: 3_000,
            answers: true,
            puts: RefCell::new(Vec::new()),
        };
        let landings = drive_breaths(
            &bridge,
            12_000,
            &[Breathing {
                path: "light/a".to_string(),
                held: pns::lights::Held::Blocked,
                cycle: pns::lights::breath_cycle(&pns::config::Breath {
                    duration_ms: 2_000,
                    high: 100,
                    low: 30,
                }),
                color: pns::pulse::BLOCKED_COLOR,
                resume: pns::lights::Resume::default(),
            }],
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert_eq!(
            bridge.puts.borrow().len(),
            4,
            "four writes at three seconds apiece fill a twelve-second budget, and \
             the fifth would be issued AT the budget, so it is not issued at all"
        );
        assert_eq!(
            landings,
            vec![("light/a".to_string(), 100, 11_000)],
            "the last write really happened at 9,000ms and its fade runs 2,000ms \
             from there, so the next tick resumes off 11,000ms rather than off the \
             13,700ms the nominal schedule would have claimed"
        );
    }

    #[test]
    fn a_landing_is_timed_by_the_fade_that_ran_and_not_by_the_cycles_first_leg() {
        // THE ONE CYCLE WHOSE LEGS DO NOT SHARE A DURATION. Every other shape
        // fades at one cadence throughout, so a landing timed from the shape
        // and one timed from the fade agree and neither can be told from the
        // other. An accent is what separates them: timed from the shape, the
        // flash would claim to finish a whole fade out instead of at its own
        // brief duration, and `resume_from` reads a landing that far past the
        // accent's own step as a clock that moved. The next tick would throw
        // the phase away and restart the breath rather than falling out of the
        // flash.
        //
        // THE MOTION IS WRITTEN OUT rather than read from `Lights::default()`,
        // for the reason the interleave test above states: the milliseconds
        // asserted here are the exact arithmetic these durations produce, and
        // reading them from the defaults would rewrite them on every retune.
        let clock = FakeClock::default();
        let bridge = SlowBridge {
            clock: &clock,
            get_cost_ms: 0,
            put_cost_ms: 0,
            answers: true,
            puts: RefCell::new(Vec::new()),
        };
        // A BUDGET THAT ENDS ON THE ACCENT: the fall after it is due at
        // 4,350ms, so 4,300ms leaves the flash as the last fade issued.
        let landings = drive_breaths(
            &bridge,
            4_300,
            &[Breathing {
                path: "light/a".to_string(),
                held: pns::lights::Held::Looping,
                cycle: pns::lights::breathe_then_flare_cycle(&pns::config::BreatheThenFlare {
                    breath: pns::config::Breath {
                        duration_ms: 2_000,
                        high: 80,
                        low: 30,
                    },
                    flare: 100,
                    flare_ms: 500,
                }),
                color: pns::pulse::LOOP_COLOR,
                resume: pns::lights::Resume::default(),
            }],
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert_eq!(
            landings,
            vec![("light/a".to_string(), 100, 4_400)],
            "the flash is issued at 3,900ms and runs its OWN 500ms, so it lands at \
             4,400ms; timed from the cycle's first leg it would claim 5,900ms, which \
             is further out than the accent's own step and so is read as stale"
        );
    }

    #[test]
    fn the_recorded_end_counts_the_resolve_the_driver_started_after() {
        // THE DRIVER'S TIMELINE STARTS AFTER THE RESOLVE, so a landing it
        // reports is an offset from a moment three bridge calls later than the
        // tick's own. Written into the record without that term, every end
        // would be a whole resolve early and the next tick would take the
        // breath over before this one had finished it: exactly the pause this
        // slice exists to remove, reintroduced through the record.
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 12\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let state = scratch("resolve-counted-in-the-record");
        let clock = FakeClock::default();
        let bridge = SlowBridge {
            clock: &clock,
            get_cost_ms: 250,
            put_cost_ms: 0,
            answers: true,
            puts: RefCell::new(Vec::new()),
        };
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            read_held(&state).expect("a record this tick wrote"),
            vec![pns::lights::HeldEntry {
                path: LAMP_PATH.to_string(),
                resume: Some(pns::lights::Phase {
                    end_unix_ms: 12_500,
                    landed_on: 100,
                    held: pns::lights::Held::Blocked,
                }),
            }],
            "three listings at 250ms leave an 11,250ms budget, whose sixth and last \
             fade is issued 9,750ms into the DRIVER and ends 2,000ms later: 12,500ms \
             from the moment the tick itself began"
        );
    }

    #[test]
    fn a_resumed_breath_composes_across_two_ticks_on_a_fake_clock() {
        // THE HANDOFF, END TO END, on numbers a real clock never has to
        // supply: both ticks are handed their own `now_ms`, so nothing here
        // sleeps or waits for real time. Tick one's breath lands on an end
        // and records it; tick two reads that record and picks the breath
        // back up from exactly where it left off.
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 12\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let state = scratch("resumed-breath-two-ticks");

        // TICK ONE, at N=0, with nothing yet held: the locked blocked shape's
        // seven fades (the seamless schedule at a twelve-second budget) land
        // on low, 13,700ms after this tick's own start.
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        let held_after_tick_one = read_held(&state).expect("a record this tick wrote");
        assert_eq!(
            held_after_tick_one,
            vec![pns::lights::HeldEntry {
                path: LAMP_PATH.to_string(),
                resume: Some(pns::lights::Phase {
                    end_unix_ms: 13_700,
                    landed_on: 30,
                    held: pns::lights::Held::Blocked,
                }),
            }],
            "seven fades of the locked blocked shape land on low at 13,700ms"
        );

        // TICK TWO, at N=12,400: the previous tick's last fade does not
        // finish landing on the bridge until 13,700, less the seamless
        // lead, less now, which is 1,250ms still to wait.
        let sleeps: RefCell<Vec<Duration>> = RefCell::new(Vec::new());
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&held_after_tick_one),
            12_400,
            None,
            || clock.elapsed_ms(),
            |waited| {
                sleeps.borrow_mut().push(waited);
                clock.slept(waited);
            },
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        // EXACTLY 1,250ms, not a tolerance: the clock this tick was handed
        // moves only when the sleeper moves it, so nothing here reads or waits
        // on wall-clock time and the number is the schedule's own.
        assert_eq!(
            sleeps.borrow()[0],
            Duration::from_millis(1_250),
            "tick two's first fade is due 1,250ms in, and it sleeps that out \
             before issuing anything"
        );
        let puts = bridge.puts.borrow();
        assert!(
            puts[0].1.contains(r#""brightness":100.0"#) && puts[0].1.contains("color"),
            "tick one landed on low, so tick two resumes toward high, armed with \
             the colour and `on` again: {}",
            puts[0].1
        );
    }

    #[test]
    fn a_lamp_that_changed_state_starts_its_new_colour_at_once_rather_than_resuming() {
        // THE LOCKED PRECEDENCE IS "RED WINS, BLOCKED OUTRANKS LOOP", and a
        // resume taken on the fixture path alone delays it. The slow loop shape
        // lands its last fade almost four seconds past the interval that issued
        // it; the next tick, now holding BLOCKED, would wait that fade out
        // before its first blocked body reached the lamp, because the first fade of
        // every tick is the one that carries the colour. The same delay hits an
        // unread lamp that has to turn red.
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 12\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\", \"loop\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let state = scratch("state-change-starts-at-once");

        // TICK ONE holds the LOOP state, whose four-second shape issues its
        // last fade at 11,850ms and lands it 15,850ms after this tick began.
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Looping],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        let held_after_the_loop = read_held(&state).expect("a record this tick wrote");

        // TICK TWO holds BLOCKED instead. Resumed off the loop's phase it would
        // sleep 3,400ms before its first blocked body; it starts down at once
        // instead, and only then keeps the blocked cadence.
        let sleeps: RefCell<Vec<Duration>> = RefCell::new(Vec::new());
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&held_after_the_loop),
            12_400,
            None,
            || clock.elapsed_ms(),
            |waited| {
                sleeps.borrow_mut().push(waited);
                clock.slept(waited);
            },
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            sleeps.borrow().first().copied(),
            Some(Duration::from_millis(1_950)),
            "the first blocked fade is issued before anything is slept for, so the \
             first sleep is the blocked shape's own step"
        );
    }

    #[test]
    fn the_phase_reaches_disk_only_after_the_breath_that_earned_it_has_run() {
        // THE PRE-ARM WRITE IS BARE, AND THE PHASE IS A SECOND WRITE. A record
        // written with its phase BEFORE the fades are issued is a promise about
        // a breath that has not happened: a child killed mid-interval would
        // leave the next tick resuming from an end no lamp ever reached, and
        // the whole point of the bare token is that a killed child leaves
        // something this run cannot promise anything about.
        let state = scratch("phase-lands-after-the-breath");
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let seen_mid_breath: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            || clock.elapsed_ms(),
            |waited| {
                seen_mid_breath
                    .borrow_mut()
                    .push(recorded(&state).unwrap_or_default());
                clock.slept(waited);
            },
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            seen_mid_breath.borrow().first().map(String::as_str),
            Some(LAMP_PATH),
            "the record carried a phase while the breath was still being issued"
        );
        assert!(
            recorded(&state).is_some_and(|line| line.starts_with(&format!("{LAMP_PATH}@"))),
            "and the phase never landed once the breath had actually run: {:?}",
            recorded(&state)
        );
    }

    #[test]
    fn a_record_cleared_during_the_breath_is_left_cleared_rather_than_resurrected() {
        // THE OPERATOR'S RETURN, ARRIVING MID-BREATH. It clears every held lamp
        // and empties this record from a process that holds no lock, and the
        // phase write comes seconds later: written unguarded it would put the
        // lamp back into the record with a phase attached, so the pulse gate
        // would go on treating a lamp the operator just put out as held.
        let state = scratch("record-cleared-mid-breath");
        let clock = FakeClock::default();
        let bridge = scripted(true);
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &held_lights(),
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            || clock.elapsed_ms(),
            |waited| {
                let _ = std::fs::remove_file(state.join(LIGHTS_HELD));
                clock.slept(waited);
            },
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        assert_eq!(
            recorded(&state),
            None,
            "the phase write resurrected a hold the return had already ended"
        );
    }

    #[test]
    fn a_job_waits_while_its_own_child_lives_and_fires_once_that_child_has_gone() {
        // THE TWO HALVES OF THE ONE-CHILD RULE, run in the order the daemon
        // runs them. A seamless breath is issued to still be running when its
        // child exits, so the schedule alone can no longer promise the previous
        // child is gone: `decide` is told whether one is, and it is told the
        // truth only because the reap happens first.
        let state = scratch("daemon-pass-one-child");
        let spool = pns::daemon::spool_dir(&state);
        std::fs::create_dir_all(&spool).expect("the spool");
        let job = pns::daemon::Job {
            id: "lights".to_string(),
            due: 100,
            until: 100_000,
            every: Some(12),
            unless_marker: None,
            // THE HARNESS'S OWN LISTING FLAG: a fired job re-executes THIS
            // binary, which under test is the test binary, and listing its
            // tests exits at once with nothing on either stream.
            args: vec!["--list".to_string()],
        };
        pns::daemon::hand_back(&spool, &job).expect("the record lands");
        let record = spool.join("lights");
        let armed = std::fs::read_to_string(&record).expect("the record is readable");
        // THE RECORD'S IDENTITY, not just its bytes. A wait must never CLAIM,
        // because a claim is a rename out and a write back, and a refresh that
        // landed in between would be overwritten by the copy this daemon was
        // already holding. The inode is what says the file was never replaced.
        let armed_inode = std::os::unix::fs::MetadataExt::ino(
            &std::fs::metadata(&record).expect("the record is there"),
        );

        let mut children = vec![Bounded {
            id: "lights".to_string(),
            child: std::process::Command::new("/bin/sleep")
                .arg("30")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("a child that is still running"),
            expires_at: std::time::Instant::now() + Duration::from_secs(300),
        }];
        let mut reported = std::collections::BTreeSet::new();
        daemon_pass(
            &spool,
            &state,
            Some(200),
            Duration::from_secs(1),
            &mut children,
            &mut reported,
        );
        assert_eq!(
            std::fs::read_to_string(&record).ok().as_deref(),
            Some(armed.as_str()),
            "a job due while its own child was still running fired anyway, so two \
             children were driving one house"
        );
        assert_eq!(children.len(), 1, "and the live child was not reaped");
        assert_eq!(
            std::os::unix::fs::MetadataExt::ino(
                &std::fs::metadata(&record).expect("the record is still there")
            ),
            armed_inode,
            "a waiting job's record was claimed and written back, which is the one \
             write that can lose a refresh a client landed in the meantime"
        );

        // THE CHILD IS GONE NOW, and the occurrence that was held fires on the
        // very next pass rather than being lost.
        let _ = children[0].child.kill();
        let _ = children[0].child.wait();
        daemon_pass(
            &spool,
            &state,
            Some(200),
            Duration::from_secs(1),
            &mut children,
            &mut reported,
        );
        assert_ne!(
            std::fs::read_to_string(&record).ok().as_deref(),
            Some(armed.as_str()),
            "the job never fired once its child had exited, which is a reap that \
             ran after the drain rather than before it"
        );
        for bounded in &mut children {
            let _ = bounded.child.kill();
            let _ = bounded.child.wait();
        }
    }

    #[test]
    fn the_tick_says_what_could_not_be_resolved_and_what_was_refused() {
        // THE LOUD HALF of "a dark lamp must never be ambiguous with a typo":
        // the resolution's findings have to leave the tick as complaints, or an
        // unattended machine routes a behaviour to a name nobody can light and
        // no one is ever told.
        let state = scratch("tick-complains");
        let bridge = scripted(true);
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 10\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n\
             dim_window = \"2200-0700\"\n\
             [lights.lamp.\"3F - Nowhere\"]\nshows = [\"blocked\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let complaints = run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            None,
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            complaints,
            vec![
                "pns lights: `3F - Nowhere` (lamp) is not on the bridge".to_string(),
                "pns lights: `3F - Studio - HCL1` has dim_window \"2200-0700\", which is \
                 not a HH:MM-HH:MM window; that lamp stays dark"
                    .to_string(),
            ],
        );
    }

    #[test]
    fn the_first_tick_sweeps_the_state_the_old_names_held() {
        // THE DEPLOY TRANSITION: delete, dark direction, once. Files under the
        // old names would otherwise sit unread forever, and the old held-glow
        // record names lamps only the binary that is gone knew how to put out.
        let state = scratch("legacy-sweep");
        std::fs::write(state.join("lights-glow"), "light/l9\n").expect("the old held record");
        std::fs::write(state.join("lights-working-since"), "1000\n").expect("the old streak");
        std::fs::create_dir_all(state.join("lights-needs")).expect("the old needs directory");
        std::fs::write(state.join("lights-needs").join("s1"), "1000\n").expect("an old wait");
        sweep_legacy_state(&state);
        assert!(
            !state.join("lights-glow").exists()
                && !state.join("lights-working-since").exists()
                && !state.join("lights-needs").exists(),
            "every old name is gone, contents and all"
        );
    }

    #[test]
    fn a_complaint_that_cleared_is_forgotten_so_its_return_is_news_again() {
        // THE FORGET ARM IS THE ONE THAT NEEDS ITS OWN PIN: `say` decides it,
        // but only this wiring removes the memory, and a memory that outlives
        // its complaint keeps the same complaint silent when it comes back.
        let state = scratch("lights-said-forget");
        let marker = state.join(LIGHTS_SAID);
        say_lights_once(
            &state,
            &["lights: `HCL9` (lamp) is not on the bridge".to_string()],
            LIGHTS_SAID,
        );
        assert!(marker.exists(), "the first complaint is remembered");
        say_lights_once(&state, &[], LIGHTS_SAID);
        assert!(
            !marker.exists(),
            "a clear tick forgets, or the same complaint returning would never \
             be said again"
        );
    }

    #[test]
    fn a_pulse_reaches_only_a_routed_lamp_that_is_neither_muted_nor_held() {
        // THE EVENT PATH'S TWO PER-LAMP GATES, at the seam. The TCP spy the
        // integration tests dial can only count connections, and the resolve's
        // GETs happen either way, so a gate dropped here is invisible to every
        // other test in the crate.
        let lights = *pns::config::parse_config(
            "[lights]\n[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let free = scripted(true);
        run_pulse_writes(
            &free,
            &scratch("pulse-writes-free"),
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            Some(&[]),
            None,
        );
        let puts = free.puts.borrow();
        assert_eq!(puts.len(), 1, "{puts:?}");
        assert_eq!(
            puts[0].0, LAMP_PATH,
            "the pulse reaches the routed lamp individually"
        );
        assert!(
            puts[0].1.contains("signaling"),
            "and it is the bridge-run signal body: {}",
            puts[0].1
        );
        // THE MUTE IS A RENDER FILTER AT THE PER-LAMP DECISION, on this path
        // exactly as on the tick's.
        let muted = scripted(true);
        run_pulse_writes(
            &muted,
            &scratch("pulse-writes-muted"),
            &lights,
            pns::config::Behaviour::Done,
            &noon(&quieted("3F - Studio")),
            Some(&[]),
            None,
        );
        assert!(
            muted.puts.borrow().is_empty(),
            "a muted lamp is not flashed: {:?}",
            muted.puts.borrow()
        );
        // AND A MUTE READING NOBODY COULD TAKE MUTES EVERY LAMP, which is the
        // fail direction on a lamp path: an unreadable record or clock arrived
        // here as an empty list, which is a house with every lamp loud.
        let dark = scripted(true);
        run_pulse_writes(
            &dark,
            &scratch("pulse-writes-dark"),
            &lights,
            pns::config::Behaviour::Done,
            &noon(&pns::channels::hue::Muting::Everything),
            Some(&[]),
            None,
        );
        assert!(
            dark.puts.borrow().is_empty(),
            "a mute nobody could read let the lamp flash anyway: {:?}",
            dark.puts.borrow()
        );
        // AND THE TICK'S HELD RECORD PREEMPTS THE PULSE on the lamp it holds,
        // which is the dedicated-but-helps-when-free ruling's event-path half.
        let held = scripted(true);
        run_pulse_writes(
            &held,
            &scratch("pulse-writes-held"),
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            Some(&[LAMP_PATH.to_string()]),
            None,
        );
        assert!(
            held.puts.borrow().is_empty(),
            "a held lamp is not flashed over: {:?}",
            held.puts.borrow()
        );
        // AND A PHASED RECORD ON DISK GATES EXACTLY LIKE A BARE ONE: the
        // suffix a resumed breath now writes must never leak into this gate,
        // which reads bare paths off `held_lamps`, the same parser the breath
        // itself reads a phase from.
        let state = scratch("pulse-gate-phased-record");
        std::fs::write(
            state.join(LIGHTS_HELD),
            format!("{LAMP_PATH}@1700000000123:h\n"),
        )
        .expect("a phased record");
        let phased = scripted(true);
        run_pulse_writes(
            &phased,
            &state,
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            held_lamps(&state).as_deref(),
            None,
        );
        assert!(
            phased.puts.borrow().is_empty(),
            "a phased record still gates the pulse over the lamp it names: {:?}",
            phased.puts.borrow()
        );
        // AND A HELD RECORD NOBODY COULD READ HOLDS EVERY LAMP, for the same
        // reason: read as nothing held, a corrupt record let a blink write
        // straight over a lamp breathing about a question.
        let unreadable = scripted(true);
        run_pulse_writes(
            &unreadable,
            &scratch("pulse-writes-unreadable"),
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            None,
            None,
        );
        assert!(
            unreadable.puts.borrow().is_empty(),
            "a held record nobody could read let the pulse fire anyway: {:?}",
            unreadable.puts.borrow()
        );
    }

    #[test]
    fn a_held_lamp_breathes_only_in_the_room_the_reading_names() {
        // THE TICK'S OWN HALF OF THE WIRING. It is the path the SUSTAINED lamp
        // takes, so a narrowing wired into the pulse alone would leave a
        // blocked breath lit in every room while the operator sits in one.
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 10\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n\
             [lights.room.\"2F - Kitchen\"]\nshows = [\"blocked\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let bridge = TwoRoomBridge {
            puts: RefCell::new(Vec::new()),
        };
        let state = scratch("tick-narrowed-by-presence");
        run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            Some(&at_the_desk(pns::presence::PresenceStatus::Nowhere {
                poll_age_secs: 1,
            })),
            no_time_passes(),
            |_| {},
        );
        let armed: Vec<String> = bridge
            .puts
            .borrow()
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(
            armed,
            vec!["light/l1".to_string()],
            "only the lamp in the desk's own room is armed"
        );
        assert_eq!(
            last_narrowing(&state).and_then(|entry| entry.room),
            Some("3F - Studio".to_string()),
            "and the decision is where the doctor reads it back"
        );
    }

    #[test]
    fn a_pulse_narrows_over_the_lamps_this_behaviour_would_reach_and_not_the_rest() {
        // NARROWING TO A ROOM THAT CARRIES THE BEHAVIOUR IS NOT THE SAME
        // QUESTION as narrowing to a room that holds a lamp. The kitchen holds
        // one, routed for `blocked` alone; narrowed first and filtered second,
        // a `done` event kept that lamp, then dropped it at the per-lamp gate
        // and wrote nothing at all, which is the silence the fallback exists
        // to prevent.
        let lights = *pns::config::parse_config(
            "[lights]\n[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
             [lights.room.\"2F - Kitchen\"]\nshows = [\"blocked\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let bridge = TwoRoomBridge {
            puts: RefCell::new(Vec::new()),
        };
        let state = scratch("pulse-narrow-over-eligible");
        run_pulse_writes(
            &bridge,
            &state,
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            Some(&[]),
            Some(&in_the_kitchen()),
        );
        assert_eq!(
            bridge
                .puts
                .borrow()
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>(),
            vec!["light/l1".to_string()],
            "the kitchen carries nothing for this event, so the whole routing stands"
        );
        assert_eq!(
            last_narrowing(&state).and_then(|entry| entry.room),
            None,
            "and the record says the routing was left whole"
        );
    }

    #[test]
    fn a_tick_narrows_over_the_lamps_this_state_would_reach_and_not_the_rest() {
        // The tick's half of the same rule, through `shown` rather than
        // `pulse_fires`: a kitchen lamp carrying only `unread` is not a lamp a
        // blocked wait can breathe on.
        let lights = *pns::config::parse_config(
            "[lights]\nrefresh_secs = 10\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\"]\n\
             [lights.room.\"2F - Kitchen\"]\nshows = [\"unread\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let bridge = TwoRoomBridge {
            puts: RefCell::new(Vec::new()),
        };
        let state = scratch("tick-narrow-over-eligible");
        run_tick_writes(
            &bridge,
            &state,
            &lights,
            &[pns::lights::Held::Blocked],
            &noon(&nothing_muted()),
            Some(&[]),
            0,
            Some(&in_the_kitchen()),
            no_time_passes(),
            |_| {},
        );
        assert_eq!(
            bridge
                .puts
                .borrow()
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["light/l1".to_string()],
            "the kitchen carries nothing for this state, so the whole routing stands"
        );
    }

    #[test]
    fn a_pulse_reaches_only_the_room_the_reading_names_and_records_the_decision() {
        // THE WIRING, not the rule. `narrow` is pure and total, so every one of
        // its unit tests stays green with this call site gutted: what is pinned
        // here is that the pulse path narrows AT ALL, and that the decision is
        // written where `pns doctor` reads it back.
        let lights = *pns::config::parse_config(
            "[lights]\n[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
             [lights.room.\"2F - Kitchen\"]\nshows = [\"done\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        let bridge = TwoRoomBridge {
            puts: RefCell::new(Vec::new()),
        };
        let state = scratch("pulse-narrowed-by-presence");
        run_pulse_writes(
            &bridge,
            &state,
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            Some(&[]),
            Some(&in_the_kitchen()),
        );
        assert_eq!(
            bridge
                .puts
                .borrow()
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>(),
            vec!["light/l2".to_string()],
            "only the lamp in the room the reading named is flashed"
        );
        assert_eq!(
            last_narrowing(&state).and_then(|entry| entry.room),
            Some("2F - Kitchen".to_string()),
            "and the decision is where the doctor reads it back"
        );
    }

    #[test]
    fn the_pulse_path_says_what_it_could_not_resolve_rather_than_dropping_it() {
        // THE PATH A PULSE-ONLY MAP ACTUALLY TAKES. A config that routes only
        // `done` and `failed` holds no state, so its tick never resolves
        // anything and never complains; every resolution such a machine ever
        // does happens right here, and the findings were discarded on the
        // floor. A mistyped lamp name was therefore dark forever with the whole
        // system silent about it.
        let lights = *pns::config::parse_config(
            "[lights]\n[lights.room.\"3F - Studio\"]\nshows = [\"done\"]\n\
             [lights.lamp.\"3F - Nowhere\"]\nshows = [\"done\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table");
        assert_eq!(
            run_pulse_writes(
                &scripted(true),
                &scratch("pulse-writes-complaints"),
                &lights,
                pns::config::Behaviour::Done,
                &noon(&nothing_muted()),
                Some(&[]),
                None,
            ),
            vec!["pns lights: `3F - Nowhere` (lamp) is not on the bridge".to_string()],
        );
    }

    #[test]
    fn a_lease_is_renewed_only_while_it_exists_and_swept_once_it_times_out() {
        // THE WIRING, not the rule. `loop_running` is pure and total and reads
        // no directory, so a lease list invented at the call site leaves every
        // one of its unit tests green while the lamp never arms by hand. The
        // renewal is the half that matters most: it must never CREATE a lease,
        // or every event from every pane would take one.
        const TIMEOUT: u64 = 3_900;
        let state = scratch("loop-lease");
        let marker =
            pns::lights::lease_marker(&state, "wW:p21").expect("herdr's own id names a lease");
        std::fs::create_dir_all(pns::lights::lease_dir(&state)).expect("the lease directory");

        renew_loop_lease(&state, "wW:p21", Some(1_000));
        assert!(
            !marker.exists(),
            "a pane with no lease is not given one by its own traffic"
        );

        std::fs::write(&marker, "1000\n").expect("a lease taken by hand");
        renew_loop_lease(&state, "wW:p21", Some(2_000));
        assert_eq!(
            sweep_leases(&state, 2_000, TIMEOUT),
            vec![2_000],
            "the pane's own traffic moved the lease forward"
        );
        assert_eq!(
            sweep_leases(&state, 2_000 + TIMEOUT, TIMEOUT),
            vec![2_000],
            "exactly at the timeout it is still live: both edges closed"
        );
        assert_eq!(
            sweep_leases(&state, 2_000 + TIMEOUT + 1, TIMEOUT),
            Vec::<u64>::new(),
            "and one second past it, an abandoned lease is gone"
        );
        assert!(
            !marker.exists(),
            "swept on the way through, because nothing else would ever remove it"
        );
        // AN UNREADABLE LEASE IS SWEPT TOO: nothing can age out a file whose
        // epoch cannot be read.
        std::fs::write(&marker, "not an epoch\n").expect("a garbled lease");
        assert_eq!(sweep_leases(&state, 2_000, TIMEOUT), Vec::<u64>::new());
        assert!(!marker.exists());
    }

    #[test]
    fn a_renewal_writes_through_the_lease_it_found_rather_than_publishing_a_new_one() {
        // A LEASE `pns loop end` REMOVED MUST STAY REMOVED. A look followed by
        // a publish is two moments: an end landing between them is undone by
        // the rename, and the lamp then breathes for a whole timeout over work
        // that finished. Writing through a handle opened on the EXISTING file
        // closes that window, because an unlink after the open sends the bytes
        // to an inode nobody can reach.
        //
        // THE INODE IS WHAT PROVES IT, and it is the only observable difference:
        // a publish-by-rename leaves a different file at the same path.
        let state = scratch("lease-renew-in-place");
        let marker = pns::lights::lease_marker(&state, "wW:p21").expect("herdr's own id");
        std::fs::create_dir_all(pns::lights::lease_dir(&state)).expect("the lease directory");
        std::fs::write(&marker, "1000\n").expect("a lease taken by hand");
        let before = std::fs::metadata(&marker).expect("the lease").ino();

        renew_loop_lease(&state, "wW:p21", Some(1_700_000_002));

        assert_eq!(
            std::fs::metadata(&marker).expect("the lease").ino(),
            before,
            "the renewal published a NEW file over the lease, so an end landing \
             between the look and the rename is undone by it"
        );
        assert_eq!(
            std::fs::read_to_string(&marker).expect("the lease"),
            "1700000002\n",
            "and the epoch really moved: the file is rewritten, not merely kept"
        );
        // AND A SHORTER EPOCH LEAVES NO TAIL of the longer one behind it, which
        // is what the truncation after the write is for.
        renew_loop_lease(&state, "wW:p21", Some(9));
        assert_eq!(std::fs::read_to_string(&marker).expect("the lease"), "9\n");
    }

    #[test]
    fn a_lease_that_could_not_be_given_back_is_reported_rather_than_called_a_success() {
        // THE WORST OUTCOME THIS VERB HAS: telling the operator a loop has
        // ended while its lease is still on disk. The lamp is a liveness signal,
        // so it goes on breathing for the whole timeout with nothing behind it,
        // and they have been told the opposite.
        let state = scratch("lease-end-refused");
        std::fs::create_dir_all(pns::lights::lease_dir(&state)).expect("the lease directory");
        assert_eq!(
            end_lease(&state, "wW:p21"),
            Ok(()),
            "a machine that never began is a removal of a file that is not there"
        );
        let marker = pns::lights::lease_marker(&state, "wW:p21").expect("herdr's own id");
        std::fs::write(&marker, "1000\n").expect("a lease taken by hand");
        assert_eq!(end_lease(&state, "wW:p21"), Ok(()));
        assert!(!marker.exists(), "and the lease is really gone");

        std::fs::create_dir(&marker).expect("a directory standing where the lease goes");
        let refused = end_lease(&state, "wW:p21").expect_err("a lease that will not be removed");
        assert!(
            refused.contains("the lease could not be given back"),
            "{refused}"
        );
    }

    #[test]
    fn the_news_record_is_written_for_a_finished_or_a_dead_turn_and_read_back_as_it_was() {
        // THE WIRING, not the rule. `unread_arming` is pure and total and has no
        // file of its own, so a record invented at the call site leaves every one
        // of its unit tests green while the lamp never arms on a real machine.
        // This is the seam that costs the whole state, pinned against real files.
        let state = scratch("news-record");
        assert_eq!(
            read_news(&state),
            pns::lights::News::default(),
            "a machine that has seen nothing yet has no news"
        );
        record_news(&state, pns::config::Behaviour::Done, Some(1_000));
        assert_eq!(
            read_news(&state),
            pns::lights::News {
                done_at: Some(1_000),
                failed_at: None
            },
        );
        record_news(&state, pns::config::Behaviour::Failed, Some(1_200));
        assert_eq!(
            read_news(&state),
            pns::lights::News {
                done_at: Some(1_000),
                failed_at: Some(1_200)
            },
            "the second kind moves its own epoch and leaves the first where it was"
        );
        record_news(&state, pns::config::Behaviour::Blocked, Some(1_400));
        assert_eq!(
            read_news(&state).done_at,
            Some(1_000),
            "and a wait is not news, so it changes nothing"
        );
        // AND THE RECORD IS TAKEN BY RENAME TO MERGE IT, so two runs recording
        // at once cannot each publish a whole line built from the same stale
        // read. What that leaves behind is nothing: a claim outliving its run
        // would be a second file holding a stale copy nothing reads.
        assert_eq!(
            std::fs::read_dir(&state)
                .expect("the state directory")
                .flatten()
                .filter(|entry| entry.file_name().to_string_lossy().contains(".claim."))
                .count(),
            0,
            "a claim was left behind in {}",
            state.display()
        );
        // FAIL TO DARK. A record some other hand rewrote arms no lamp rather
        // than arming one about news nobody can name.
        std::fs::write(state.join(LIGHTS_NEWS), "not a record\n").expect("a garbled record");
        assert_eq!(read_news(&state), pns::lights::News::default());
        // AND A CLOCK NOBODY CAN READ WRITES NOTHING, never an epoch of zero:
        // zero is 1970, which is older than every interaction there has been.
        std::fs::remove_file(state.join(LIGHTS_NEWS)).expect("the record goes");
        record_news(&state, pns::config::Behaviour::Done, None);
        assert!(!state.join(LIGHTS_NEWS).exists());
    }

    /// One shell's marker planted by hand: the pid it is named for, and the
    /// second its command started.
    fn plant_shell_marker(state: &std::path::Path, pid: &str, body: &str) -> PathBuf {
        let shell = state.join(LIGHTS_SHELL_DIR);
        std::fs::create_dir_all(&shell).expect("the shell marker directory");
        let path = shell.join(pid);
        std::fs::write(&path, body).expect("the shell marker");
        path
    }

    #[test]
    fn the_shell_reading_is_the_oldest_marker_a_live_shell_is_holding() {
        // THE LONGEST-RUNNING COMMAND IS WHAT THE THRESHOLDS MEASURE. One
        // shell per pane means several markers at once, and the freshest of
        // them would restart the breathe clock every time any pane ran
        // anything, so a build running for an hour beside a prompt someone
        // keeps typing at would never reach a threshold measured in minutes.
        //
        // TWO KINDS OF LIVE SHELL, because `kill(pid, 0)` has two ways of
        // saying the process is there: this test's own process answers
        // success, and pid 1 is launchd, which this user may not signal and
        // which answers EPERM. Only ESRCH is gone.
        let state = scratch("lights-shell-oldest");
        plant_shell_marker(&state, &std::process::id().to_string(), "2000\n");
        plant_shell_marker(&state, "1", "1000\n");

        assert_eq!(
            sweep_shell_markers(&state),
            Some(1000),
            "the reading must be the oldest live marker, not the newest and \
             not whichever the directory happened to list first"
        );
    }

    #[test]
    fn a_marker_whose_shell_is_gone_is_swept_and_never_read() {
        // A SHELL KILLED MID-COMMAND is the case the pid in the name exists
        // for. Nothing else would ever remove that file: its own precmd never
        // runs again and its EXIT trap never fired, so without this sweep it
        // is both a lamp breathing forever about a command nobody is running
        // and one file per killed terminal for the life of the machine.
        let state = scratch("lights-shell-dead-pid");
        let dead = a_reaped_pid().to_string();
        let dead_marker = plant_shell_marker(&state, &dead, "1000\n");
        plant_shell_marker(&state, &std::process::id().to_string(), "2000\n");

        assert_eq!(
            sweep_shell_markers(&state),
            Some(2000),
            "a dead shell's epoch was still being read as work in progress"
        );
        assert!(
            !dead_marker.exists(),
            "and the file it left behind is gone: nothing else ever collects it"
        );
    }

    #[test]
    fn a_name_that_is_not_a_shell_pid_is_swept() {
        // Nothing this crate or the bashrc writes lands here under a name that
        // is not a pid, so anything else is litter no liveness test can ever
        // age out. A NON-POSITIVE NUMBER IS LITTER TOO, and it matters more
        // than it looks: `kill()` reads 0 as this process's own group and -1 as
        // every process the user owns, so a hand-planted `0` or `-1` must never
        // reach the liveness test looking like a pid.
        let state = scratch("lights-shell-bad-name");
        let junk = plant_shell_marker(&state, "not-a-pid", "1000\n");
        let zero = plant_shell_marker(&state, "0", "1000\n");
        let live = plant_shell_marker(&state, &std::process::id().to_string(), "2000\n");

        assert_eq!(
            sweep_shell_markers(&state),
            Some(2000),
            "only a marker a live shell is named by may feed the reading"
        );
        assert!(
            !junk.exists(),
            "the unparseable name was left to accumulate"
        );
        assert!(!zero.exists(), "a non-positive pid was left to accumulate");
        assert!(
            live.exists(),
            "and the sweep took the live shell's marker with it, which would \
             darken the lamp under every build"
        );
    }

    #[test]
    fn a_live_shell_whose_marker_holds_no_epoch_yet_is_left_alone() {
        // THE WRITE IS A TRUNCATING REDIRECT. `printf ... >"$marker"` empties
        // the file at open and fills it a moment later, so a tick landing in
        // that window reads an empty file for a command that is genuinely
        // starting. Unlinking it there wins the race against the write, which
        // then fills a file nothing will ever look at, and the build runs to
        // completion with no marker at all: exactly the dark lamp this whole
        // slice exists to fix. The pid is what collects the file when that
        // shell ends, so nothing accumulates by leaving it.
        let state = scratch("lights-shell-mid-write");
        let starting = plant_shell_marker(&state, &std::process::id().to_string(), "");
        plant_shell_marker(&state, "1", "1000\n");

        assert_eq!(
            sweep_shell_markers(&state),
            Some(1000),
            "an epoch that cannot be read is not an epoch: it must not become \
             a reading of its own"
        );
        assert!(
            starting.exists(),
            "a live shell's marker was unlinked out from under its own write"
        );
    }

    #[test]
    fn no_directory_and_an_empty_one_both_read_as_nothing() {
        // A MACHINE WHOSE SHELL NEVER PUBLISHED is the ordinary case on a host
        // that has not applied this bashrc yet, and it must read as no shell
        // work rather than as an error or a zero epoch: a zero would be a
        // command that started in 1970 and would pass every threshold there is.
        let state = scratch("lights-shell-empty");
        assert_eq!(
            sweep_shell_markers(&state),
            None,
            "a state directory with no shell directory in it read as work"
        );

        std::fs::create_dir_all(state.join(LIGHTS_SHELL_DIR)).expect("the shell directory");
        assert_eq!(
            sweep_shell_markers(&state),
            None,
            "an empty shell directory read as work"
        );
    }

    #[test]
    fn the_ticks_blocked_reading_takes_its_backstop_from_the_config_on_both_halves() {
        // THE TICK COMPOSES TWO READERS OF THE SAME BOUND, the sweep that
        // deletes an aged marker and the aggregate that lights the lamp, and
        // each is handed the knob separately. A knob past every number this
        // bound was ever hardcoded to, and a wait older than all of them but
        // inside it: a reader that kept an old constant on EITHER half puts
        // the lamp out here.
        const GIVE_UP_AFTER_SECS: u64 = 100_000;
        let state = scratch("blocked-knob-tick");
        let marker = pns::lights::blocked_marker(&state, "s1").expect("a usable session id");
        std::fs::create_dir_all(marker.parent().expect("the wait directory"))
            .expect("the wait directory");
        std::fs::write(&marker, "1000\n").expect("a wait in progress");
        // THROUGH THE PARSER, not a field poked on a default: the knob the
        // operator writes is the one the tick must read.
        let config = pns::config::parse_config(&format!(
            "[lights.blocked]\ngive_up_after_secs = {GIVE_UP_AFTER_SECS}\n"
        ))
        .expect("a config stating the knob");
        let lights = config.lights.as_deref().expect("the lights table");

        assert!(
            blocked_lamp(&state, lights, 1_000 + 90_000),
            "a day-old question inside the configured backstop still holds the lamp"
        );
        assert!(
            !blocked_lamp(&state, lights, 1_000 + GIVE_UP_AFTER_SECS + 1),
            "and one second past the backstop the lamp is given back"
        );
        assert!(
            !marker.exists(),
            "by the sweep, which read the same knob and removed the marker"
        );
    }

    #[test]
    fn a_wait_nobody_has_answered_still_holds_its_lamp_until_the_configured_backstop() {
        // THE LOCK SAYS "CONTINUOUS UNTIL THE OPERATOR ANSWERS", and half an
        // hour was not that: a question asked while they were at lunch went
        // dark before they came back, with nothing anywhere to say it had. What
        // is left is an ABANDONED-SESSION BACKSTOP and nothing else, so the
        // lamp survives every absence the knob names.
        //
        // A KNOB THAT IS NOT THE SHIPPED DEFAULT, so a `sweep_blocked` that
        // silently kept an old hardcoded number instead of reading the
        // configured one would still be caught here.
        const GIVE_UP_AFTER_SECS: u64 = 3_600;

        let state = scratch("blocked-bound");
        let marker = pns::lights::blocked_marker(&state, "s1").expect("a usable session id");
        std::fs::create_dir_all(marker.parent().expect("the wait directory"))
            .expect("the wait directory");
        std::fs::write(&marker, "1000\n").expect("a wait in progress");

        assert_eq!(
            sweep_blocked(&state, 1_000 + GIVE_UP_AFTER_SECS - 1, GIVE_UP_AFTER_SECS),
            vec![1_000],
            "a question just short of the knob is still a question nobody has answered"
        );
        assert_eq!(
            sweep_blocked(&state, 1_000 + GIVE_UP_AFTER_SECS, GIVE_UP_AFTER_SECS),
            vec![1_000],
            "exactly at the backstop it is still live: the bound is closed"
        );
        assert_eq!(
            sweep_blocked(&state, 1_000 + GIVE_UP_AFTER_SECS + 1, GIVE_UP_AFTER_SECS),
            Vec::<u64>::new(),
            "and one second past it the abandoned session gives the bulb back"
        );
        assert!(!marker.exists(), "swept on the way through");
    }

    #[test]
    fn the_sweep_leaves_a_marker_that_is_mid_publish_alone() {
        // `publish_state_line` writes `<name>.new.<pid>` INTO THIS DIRECTORY
        // and renames it over the marker, so a pending file is an ordinary
        // entry the sweep walks. Between the open and the rename there is no
        // epoch in it to read, and an unreadable-means-delete rule unlinks it
        // there: the racing rename then publishes nothing and the wait is lost
        // with the agent still waiting on the operator.
        let state = scratch("sweep-skips-pending");
        let needs = pns::lights::blocked_dir(&state);
        std::fs::create_dir_all(&needs).expect("the needs directory");
        std::fs::write(needs.join("s1"), "1000\n").expect("a live wait");
        let pending = needs.join(format!("s2.new.{}", std::process::id()));
        std::fs::write(&pending, "").expect("a marker caught mid-publish");
        std::fs::write(needs.join("s3"), "not an epoch\n").expect("an unreadable marker");

        assert_eq!(
            sweep_blocked(&state, 1000, 3_600),
            vec![1000],
            "the live wait is still what the sweep answers with"
        );
        assert!(
            pending.exists(),
            "and the pending file is left for its own rename to publish"
        );
        assert!(
            !needs.join("s3").exists(),
            "while a marker that really is unreadable is still swept: nothing \
             else ages out a file whose epoch cannot be read"
        );
    }

    #[test]
    fn a_pending_file_whose_run_is_gone_is_collected_and_a_marker_that_spells_it_is_swept() {
        // TWO HALVES OF ONE COLLISION. A session id and a pane id are opaque
        // words from another program, and both alphabets admit a dot, so a name
        // matched on the bare `.new.` put a real marker beyond every sweep: it
        // aged out never and its lamp could not be released. The same match let
        // a publish whose run had DIED sit in the directory forever, which is
        // the unbounded growth the sweep exists to prevent, through a door it
        // opened itself.
        let state = scratch("sweep-pending-collection");
        let leases = pns::lights::lease_dir(&state);
        std::fs::create_dir_all(&leases).expect("the lease directory");
        let spelled = leases.join("a.new.b");
        std::fs::write(&spelled, "1000\n").expect("a pane whose own id spells the suffix");
        let abandoned = leases.join(format!("s2.new.{}", a_reaped_pid()));
        std::fs::write(&abandoned, "").expect("a publish whose run died");
        let in_flight = leases.join(format!("s3.new.{}", std::process::id()));
        std::fs::write(&in_flight, "").expect("a publish still in flight");

        assert_eq!(
            sweep_markers(&leases, 100_000, 60),
            Vec::<u64>::new(),
            "the expired marker is not answered with"
        );
        assert!(
            !spelled.exists(),
            "a marker whose name spells the pending suffix was invisible to the sweep"
        );
        assert!(
            !abandoned.exists(),
            "a publish whose own run is gone is litter nothing else collects"
        );
        assert!(
            in_flight.exists(),
            "while a publish still in flight is left for its own rename"
        );
    }

    #[test]
    fn a_sweep_takes_a_marker_before_removing_it_and_leaves_no_working_file_behind() {
        // OWNED BY RENAME, NEVER READ-THEN-UNLINK. Concurrent unlink does not
        // arbitrate on this filesystem: it reports success to every caller, so a
        // sweep that read an expired epoch and then unlinked could remove a
        // FRESH marker a racing event published in between, and both runs would
        // believe they had removed the old one.
        //
        // WHAT A SINGLE-THREADED TEST CAN PIN is the shape either way: the
        // expired marker really goes, the live one is untouched, and no working
        // file is left in the directory. The interleaving itself is a race no
        // test in this tree can stage.
        let state = scratch("sweep-owns-by-rename");
        let leases = pns::lights::lease_dir(&state);
        std::fs::create_dir_all(&leases).expect("the lease directory");
        std::fs::write(leases.join("live"), "1000\n").expect("a live lease");
        std::fs::write(leases.join("expired"), "10\n").expect("an expired lease");
        let live_inode = std::fs::metadata(leases.join("live"))
            .expect("the live lease")
            .ino();

        assert_eq!(sweep_markers(&leases, 1_000, 60), vec![1_000]);

        assert!(!leases.join("expired").exists(), "the expired lease goes");
        assert_eq!(
            std::fs::metadata(leases.join("live"))
                .expect("the live lease")
                .ino(),
            live_inode,
            "and the live one is not even renamed: the ordinary tick moves nothing"
        );
        let left: Vec<String> = std::fs::read_dir(&leases)
            .expect("the lease directory")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["live".to_string()], "a claim was left behind");
    }

    #[test]
    fn a_hue_table_nobody_wrote_and_one_switched_off_are_different_reports() {
        // NO BRIDGE IS DIALLED BY ANY ROW HERE: every case answers before the
        // enabled-and-configured branch that makes the two GETs, which is the
        // only branch that touches a network.
        let lights = pns::config::Lights::default();
        assert!(
            matches!(
                lights_report(None, None, false),
                pns::doctor::LightsReport::Off
            ),
            "no [lights] table is off, whatever hue is doing"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), None, false),
                pns::doctor::LightsReport::HueMissing
            ),
            "a table and NO [plugins.hue] at all is a config that is half written"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), None, true),
                pns::doctor::LightsReport::HueDisabled
            ),
            "and a table beside a hue that IS written is a switch somebody turned \
             off, which is a decision rather than an omission"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), Some(&toml::Table::new()), true),
                pns::doctor::LightsReport::NoBridge
            ),
            "an enabled hue naming no bridge dials nothing and says so"
        );
    }

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

    #[test]
    fn the_only_answer_that_arms_a_feature_is_a_yes_somebody_typed() {
        // ENTER MEANS NO, and this is the assertion that says so. Every
        // question it answers arms a delivery to a phone or a lamp and takes a
        // credential to do it, so the answer nobody typed on purpose has to be
        // the one that changes nothing. A predicate reading "not a no" would
        // arm the whole walk by default and pass every test about the file.
        for yes in ["y", "yes", "Y", "YES", "Yes"] {
            assert!(means_yes(yes), "`{yes}` is a yes");
        }
        for no in ["", "n", "no", "N", "sure", "ok", "yeah", "yep", "y ", "1"] {
            assert!(!means_yes(no), "`{no}` is not the yes this walk requires");
        }
    }

    #[test]
    fn an_answer_of_nothing_but_spaces_is_a_blank_one() {
        // THE RULE THE WHOLE WALK RESTS ON. `compose_config` declines a
        // feature whose credential is empty and it asks `is_empty`, so a
        // credential that survives here as `"  "` arms its plugin with two
        // spaces: a table that reads as set up and delivers nothing, which is
        // the one state this wizard exists to keep off a fresh machine.
        assert_eq!(answered("   \n"), "");
        assert_eq!(answered("\t\n"), "");
        assert_eq!(answered("\n"), "");
        // AND A REAL ANSWER SURVIVES IT: trimming that ate the answer would
        // decline every feature the operator armed.
        assert_eq!(answered("  192.168.1.9  \n"), "192.168.1.9");
        assert_eq!(answered("Studio, Kitchen\n"), "Studio, Kitchen");
    }

    #[test]
    fn a_comma_separated_answer_names_only_the_values_somebody_typed() {
        // A BLANK BETWEEN TWO COMMAS IS NOT A ROOM. It would reach the file as
        // `rooms = [""]`, which the bridge matches to no room at all while the
        // table reads as configured.
        assert_eq!(list("Studio, Kitchen".to_string()), ["Studio", "Kitchen"]);
        assert_eq!(
            list("Studio, , Kitchen,".to_string()),
            ["Studio", "Kitchen"]
        );
        assert_eq!(list("  Studio  ".to_string()), ["Studio"]);
        assert!(list(String::new()).is_empty());
        assert!(list(" , ".to_string()).is_empty());
    }

    #[test]
    fn the_only_backend_the_walk_accepts_is_one_the_home_probe_answers() {
        // THE ONE QUESTION WHOSE ANSWER IS NOT FREE TEXT. Every other answer
        // here is a credential nothing but the operator's own network can
        // judge; this one is judged by `home`, which refuses a type it does
        // not implement at probe time, long after the wizard said it worked.
        assert_eq!(router_backend(""), Some(pns::home::UNIFI_TYPE));
        assert_eq!(router_backend("unifi"), Some(pns::home::UNIFI_TYPE));
        // AND THE ANSWER IS WRITTEN AS THE CODE SPELLS IT, because the probe
        // compares the whole string and would refuse the operator's capitals.
        assert_eq!(router_backend("UniFi"), Some(pns::home::UNIFI_TYPE));
        for unanswerable in ["asus", "unifi-controller", "u", "unifix", "eero"] {
            assert_eq!(
                router_backend(unanswerable),
                None,
                "`{unanswerable}` is a backend nothing here reads"
            );
        }
    }

    /// The mode a file was published with, and nothing else about it.
    fn mode_of(path: &std::path::Path) -> u32 {
        std::fs::metadata(path)
            .expect("the file")
            .permissions()
            .mode()
            & 0o777
    }

    /// Everything beside the published config in its directory: empty when a
    /// publish left no pending file and claimed no unclaimed backup name.
    fn leftovers(path: &std::path::Path) -> Vec<String> {
        std::fs::read_dir(path.parent().expect("the directory"))
            .expect("the directory")
            .filter_map(|entry| Some(entry.ok()?.file_name().to_string_lossy().into_owned()))
            .filter(|name| name != "config.toml")
            .collect()
    }

    #[test]
    fn a_first_config_is_published_for_its_operator_alone_and_leaves_no_pending_file() {
        // THE FILE CARRIES EVERY PLUGIN'S SECRET, so publishing it at the
        // umask hands the moshi token and the hue key to every process on the
        // machine. The pending file carries them too, which is why it is
        // created with the mode rather than chmodded into it afterwards, and
        // why it never outlives the publish.
        let home = scratch("setup-publish-first");
        let path = home.join(".config/pns/config.toml");
        assert_eq!(
            publish_config(&path, "# composed\n", false),
            Ok(None),
            "a first publish keeps nothing aside"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
        assert_eq!(
            mode_of(&path),
            CONFIG_FILE_MODE,
            "the config is the operator's alone"
        );
        let extra = leftovers(&path);
        assert!(
            extra.is_empty(),
            "a pending file was left behind: {extra:?}"
        );
    }

    #[test]
    fn a_config_that_appeared_during_the_walk_is_refused_rather_than_written_over() {
        // CREATE-IF-ABSENT, NEVER A BLANKET RENAME. The questions take
        // minutes, and a config that arrived while they were being answered is
        // another writer's: a rename would replace it with no backup and no
        // word, and the refusal earlier in `setup_mode` cannot see it because
        // it ran before the walk did.
        let home = scratch("setup-publish-raced");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        std::fs::write(&path, "# somebody else got here first\n").expect("the config");

        let refusal = publish_config(&path, "# composed\n", false).expect_err("it must refuse");
        assert!(
            refusal.contains("appeared"),
            "it says what happened: {refusal}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# somebody else got here first\n",
            "the config that was already there was written over"
        );
        let extra = leftovers(&path);
        assert!(extra.is_empty(), "a refusal left a pending file: {extra:?}");
    }

    #[test]
    fn a_forced_replacement_keeps_the_old_config_before_it_writes_the_new_one() {
        // THE BACKUP IS TAKEN FIRST, and the way to say that as an assertion
        // is to read the backup: taken afterwards it would be a copy of the
        // REPLACEMENT, the old file would be gone, and the line printed to the
        // operator would name a path that does not hold what it says it holds.
        let home = scratch("setup-publish-forced");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        std::fs::write(&path, "# the one it replaces\n").expect("the config");

        let backup = publish_config(&path, "# composed\n", true)
            .expect("a forced publish")
            .expect("it kept the old config");
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
        assert_eq!(
            std::fs::read_to_string(&backup).expect("the backup"),
            "# the one it replaces\n",
            "the backup holds the replacement rather than what was replaced"
        );
        // AND IT IS AS PRIVATE AS THE FILE IT COPIES: a backup of a config
        // full of plugin secrets is a config full of plugin secrets.
        assert_eq!(mode_of(&backup), CONFIG_FILE_MODE);
        assert!(
            !backup.to_string_lossy().contains(':'),
            "the stamp carries colons: {}",
            backup.display()
        );
    }

    #[test]
    fn a_forced_replacement_with_nothing_to_replace_keeps_nothing_aside() {
        // THE MIRROR: `--force` on a machine with no config is an ordinary
        // first run, and naming a backup that holds nothing would send the
        // operator to a file that was never written.
        let home = scratch("setup-publish-forced-first");
        let path = home.join(".config/pns/config.toml");
        assert_eq!(publish_config(&path, "# composed\n", true), Ok(None));
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
        assert_eq!(mode_of(&path), CONFIG_FILE_MODE);
        // AND IT LEAVES NO FILE NAMED LIKE ONE EITHER. Claiming the backup's
        // name is how a second forced run in the same second is refused, and a
        // claim left standing over nothing is a backup that holds nothing.
        let extra = leftovers(&path);
        assert!(extra.is_empty(), "it kept something aside: {extra:?}");
    }

    #[test]
    fn a_forced_run_keeps_a_config_the_existence_check_reads_as_absent() {
        // THE CHECK IS NOT THE AUTHORITY, THE PUBLISH IS. The walk's own
        // pre-check reads `symlink_metadata` rather than `exists`, so a
        // dangling symlink at the config name is refused before the first
        // question is even asked; this proves the FORCED publish handles the
        // same dangling symlink correctly on its own, which must not depend
        // on the pre-check having caught it. Either way a blanket rename
        // replaced a config this run never read, with no backup and no word,
        // so the publish moves aside whatever is standing there and asks for
        // the name rather than taking it.
        let home = scratch("setup-publish-unseen");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        let pointed_at = path.with_file_name("config-in-a-checkout.toml");
        std::os::unix::fs::symlink(&pointed_at, &path).expect("the link");

        let backup = publish_config(&path, "# composed\n", true)
            .expect("a forced publish")
            .expect("it kept the config that was standing there");
        assert_eq!(
            std::fs::read_link(&backup).expect("the backup"),
            pointed_at,
            "the config that was there went nowhere this run can name"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
    }

    #[test]
    fn a_forced_run_keeps_the_config_it_replaced_rather_than_what_that_config_named() {
        // WHAT THE BACKUP HOLDS IS WHAT THE PUBLISH REPLACED. A copy taken
        // from the name reads THROUGH it: with a symlinked config it copied
        // the file at the far end, which the publish then did not touch, and
        // the link itself, which the publish did replace, went unrecorded. The
        // same gap a config replaced between the copy and the publish leaves,
        // which no test can reach without a seam.
        let home = scratch("setup-publish-through");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        let pointed_at = path.with_file_name("config-in-a-checkout.toml");
        std::fs::write(&pointed_at, "# the one it points at\n").expect("the config");
        std::os::unix::fs::symlink(&pointed_at, &path).expect("the link");

        let backup = publish_config(&path, "# composed\n", true)
            .expect("a forced publish")
            .expect("it kept the config that was standing there");
        assert_eq!(
            std::fs::read_link(&backup).expect("the backup"),
            pointed_at,
            "the backup holds what the config named rather than the config it replaced"
        );
        // AND WHAT IT NAMED WAS NOT REPLACED, so it is where it always was.
        assert_eq!(
            std::fs::read_to_string(&pointed_at).expect("the config it points at"),
            "# the one it points at\n"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
    }

    #[test]
    fn a_pending_file_left_by_an_abandoned_run_is_never_the_file_this_one_writes_into() {
        // A PENDING FILE IS A SECOND NAME FOR THE LIVE CONFIG between the link
        // that publishes it and the unlink that removes it, so a run killed in
        // that window leaves one behind. PROCESS IDS ARE REUSED, so a later
        // run naming its pending file after its own id can find that leftover,
        // and opening it to truncate would empty the config this run has not
        // read yet: the backup taken next would hold the REPLACEMENT, under a
        // path printed to the operator as the file they had.
        let home = scratch("setup-publish-leftover");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        std::fs::write(&path, "# the one it replaces\n").expect("the config");
        let leftover = path.with_file_name(format!("config.toml.new.{}", std::process::id()));
        std::fs::hard_link(&path, &leftover).expect("the leftover");

        let backup = publish_config(&path, "# composed\n", true)
            .expect("a forced publish")
            .expect("it kept the old config");
        assert_eq!(
            std::fs::read_to_string(&backup).expect("the backup"),
            "# the one it replaces\n",
            "the leftover was truncated, so the backup holds the replacement"
        );
        assert_eq!(
            std::fs::read_to_string(&leftover).expect("the leftover"),
            "# the one it replaces\n",
            "the config the leftover names was written through rather than left alone"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# composed\n"
        );
    }

    #[test]
    fn a_background_read_names_job_control_rather_than_an_io_fault() {
        // TERMIOS(4): a background process that BLOCKS SIGTTIN, which the
        // hidden read does, gets EIO from the read "and no signal is sent",
        // where an unblocked one would have stopped and could be resumed.
        // Passed through raw, `pns setup &` blames an I/O fault for what is
        // job control, and hides the one thing the operator can do about it.
        let eio = std::io::Error::from_raw_os_error(libc::EIO);
        assert!(
            read_failure(&eio, true).contains("bring it to the foreground with fg"),
            "a backgrounded walk was not told why the terminal cannot be read"
        );
        // A HUNG-UP TERMINAL ANSWERS EIO TOO, and that read really did fail
        // for its own reason rather than for job control.
        assert!(
            read_failure(&eio, false).contains("the answers could not be read"),
            "an EIO in the foreground was blamed on job control"
        );
        // AND A BACKGROUND JOB'S OTHER FAILURES KEEP THEIR OWN REASON: a
        // non-UTF-8 paste still has to say that is what happened.
        let other = std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8",
        );
        assert!(
            read_failure(&other, true).contains("valid UTF-8"),
            "a background job's real read failure was replaced by the job-control line"
        );
    }

    #[test]
    fn a_same_second_backup_collision_names_the_backup_it_could_not_claim() {
        // THE NAME IS CLAIMED WITH `create_new`, so a second forced run inside
        // the same second finds its own stamp already taken; this pre-creates
        // that collision instead of running two forced publishes back to back
        // and hoping they land in the same wall-clock second.
        //
        // THE MOMENT IS NAMED, NOT READ, on both sides: `keep_aside_at`
        // takes the epoch, so this test and the code under it cannot
        // disagree about which second they are in, and exactly one backup
        // name is in play.
        const FIXED_EPOCH: u64 = 1_700_000_000;
        let home = scratch("setup-keep-aside-collision");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(path.parent().expect("the directory")).expect("the directory");
        std::fs::write(&path, "# the one it replaces\n").expect("the config");
        let claimed = pns::setup::backup_path(&path, FIXED_EPOCH).expect("the backup name");
        std::fs::write(&claimed, "# an earlier run's own backup\n").expect("the earlier backup");

        let refusal =
            keep_aside_at(&path, FIXED_EPOCH).expect_err("the backup name is already claimed");
        assert!(
            refusal.contains(&claimed.display().to_string()),
            "the refusal does not name the pre-claimed backup: {refusal}"
        );
        assert!(
            refusal.contains("already claimed"),
            "the reason is a raw io::Error instead of naming the same-second collision: {refusal}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("the config"),
            "# the one it replaces\n",
            "the config was moved even though its backup name could not be claimed"
        );
        assert_eq!(
            std::fs::read_to_string(&claimed).expect("the earlier backup"),
            "# an earlier run's own backup\n",
            "an earlier run's own backup was overwritten rather than left alone"
        );
    }

    #[test]
    fn a_claim_that_fails_for_another_reason_is_not_blamed_on_a_same_second_run() {
        // THE CLAIM FAILS, BUT NOT BECAUSE THE NAME IS TAKEN: the config's own
        // directory is missing, so `create_new` cannot open the backup name at
        // all. Only AlreadyExists is the same-second collision; any other
        // failure must carry its own reason rather than blame an earlier run
        // that never happened.
        let home = scratch("setup-keep-aside-other-reason");
        let path = home.join(".config/pns/config.toml");

        let refusal = keep_aside(&path).expect_err("the backup name cannot be claimed");
        assert!(
            refusal.contains("could not be claimed"),
            "the refusal does not say the claim itself failed: {refusal}"
        );
        assert!(
            !refusal.contains("this same second"),
            "a missing directory was blamed on a same-second collision: {refusal}"
        );
    }

    #[test]
    fn a_directory_at_the_config_path_is_named_rather_than_the_backup_it_could_not_replace() {
        // THE RENAME IS WHAT FAILS HERE, not the claim: the backup file is
        // created fine (it is a fresh name), and then a directory cannot be
        // renamed onto it. The refusal is about `path`, the thing that could
        // not be moved, not about `backup`, which was never the problem.
        let home = scratch("setup-keep-aside-directory");
        let path = home.join(".config/pns/config.toml");
        std::fs::create_dir_all(&path).expect("a directory standing where the config belongs");

        let refusal =
            keep_aside(&path).expect_err("a directory cannot be renamed onto a plain file");
        assert!(
            refusal.contains(&path.display().to_string()),
            "the refusal does not name the config path: {refusal}"
        );
        // `backup`'s own display string always carries `path`'s as a prefix
        // (`backup_path` appends `.<stamp>.backup` to the config's name), so
        // checking for the FULL backup string is what actually tells apart a
        // refusal that blames the backup from one that blames the path.
        assert!(
            !refusal.contains(".backup"),
            "the refusal blames the backup file it could not replace path with, \
             rather than the path it could not move: {refusal}"
        );
        assert!(
            path.is_dir(),
            "the directory standing at the config path was moved"
        );
        // THE CLAIMED BACKUP NAME IS RELEASED, not left behind empty: the
        // rename that would have moved the directory onto it never happened,
        // so a `.backup` entry surviving here would be a claim this run made
        // and never used.
        let leftover = leftovers(&path);
        assert!(
            leftover.is_empty(),
            "a backup claim was left behind after the refusal: {leftover:?}"
        );
    }
}
