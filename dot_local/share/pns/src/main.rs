//! The pns binary: the composition root, and the only place with a main.
//!
//! Everything here is WIRING. The roster is one constant and one constructor
//! in `registry`, so there is no second construction of it to diverge; the
//! environment and the config are read once at this edge, and every decision
//! is delegated to the library. It exits 0 on every path, because a
//! notification must never fail the work it reports on.

use std::collections::BTreeMap;
use std::io::Write;
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
use pns::system::{SystemCommandRunner, SystemProbes, local_minutes_since_midnight, run_bounded};

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
/// place a non-zero exit is correct: there it is the operator's decision.
fn gate_mode(subcommand: &str) -> i32 {
    if !pns::hooks::is_harness_subcommand(subcommand) || !forward_to_moshi(&system_probes()) {
        return 0;
    }
    let Some(payload) = read_payload().filter(|payload| payload_is_whole(payload)) else {
        return 0;
    };
    spawn_moshi_hook(subcommand, &payload).map_or(0, moshi_decision)
}

/// A harness event, from the payload on stdin.
///
/// THE EXIT CONTRACT AND ITS ONE EXCEPTION. Every path here is a notification,
/// and a notification that cannot be delivered must never fail the turn it
/// reports on, so every path returns 0. The forwarded blocking path is the
/// exception: there the exit code is the OPERATOR'S DECISION, and answering it
/// here would answer the permission prompt for them.
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
        "blocked" => return blocking_event(&payload, &agent, &payload_json),
        "asked" | "plan-ready" => run_event(
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
    std::fs::write(&pending, format!("{line}\n"))?;
    if let Err(error) = std::fs::rename(&pending, path) {
        // Nothing half-written is left in the state directory for the next
        // run to trip over.
        let _ = std::fs::remove_file(&pending);
        return Err(error);
    }
    Ok(())
}

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
    match run_bounded(command, Some(&condenser_prompt(reply)), deadline)
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
    run_bounded(command, None, GIT_DEADLINE)
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
    forwarded.map_or(0, moshi_decision)
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
    let moshi = std::env::var("MOSHI_HOOK_BIN")
        .unwrap_or_else(|_| "/opt/homebrew/bin/moshi-hook".to_string());
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

/// Become moshi's answer. NO deadline and NO default: this waits on a human,
/// and the code it returns is their decision.
fn moshi_decision(mut child: std::process::Child) -> i32 {
    child
        .wait()
        .ok()
        .and_then(|status| status.code())
        .unwrap_or(0)
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
    // settings, the plan needs moshi's card toggle, and the two network
    // channels need their secrets.
    let (hue_table, watch_card, moshi_token, hermes_key) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (
            enabled_hue_table(config),
            mobile_watch_card(config),
            plugin_settings(config, "moshi").and_then(moshi_secret),
            plugin_settings(config, "hermes").and_then(hermes_secret),
        ),
        // A config that could not be read falls back to the DEFAULTS of all
        // four, and deliberately disagrees with the plugin selection below,
        // which falls back to the whole roster. Selection keeps notifications
        // working through a broken config; these say what an operator asked
        // for, and an unreadable file asked for nothing: with no secrets, the
        // network channels are simply not set up.
        _ => (None, false, None, None),
    };
    let (selection, warning) = select_plugins(&roster(), loaded);
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }

    let overrides = overrides_from_env();
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs());

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

    if decision.legs.is_empty() {
        // A verdict that must be SAID, but only for the contradiction the
        // caller asked for: a silent exit is indistinguishable from delivery.
        if event.local_only && event.remote_only {
            println!(
                "pns: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent"
            );
        }
    } else {
        dispatch_legs(&decision, event, &home, moshi_token, hermes_key);
    }

    // THE PULSE GOES LAST, after every channel the operator might be waiting
    // on. It is part of the PLAN rather than a second invocation (the shell
    // used to call `pns pulse` alongside the notification, so the tier was
    // decided twice and could disagree with itself), but it talks to a bridge
    // over the network under a ten-second deadline, and nothing an operator
    // reads should queue behind decoration. It still fires for a plan that
    // reached no channel at all: the lights are not a leg.
    if decision.pulse {
        // The state IS the exit code here: the shell notifier derives
        // --state from `$?`, and an agent turn that did not fail succeeded.
        fire_pulse_unless_quiet(hue_table, if event.state == "failed" { "1" } else { "0" });
    }
}

/// Every leg to its destination, in the registry's delivery order.
fn dispatch_legs(
    decision: &pns::engine::Decision,
    event: &pns::args::EventArgs,
    home: &str,
    moshi_token: Option<String>,
    hermes_key: Option<String>,
) {
    // Sanitized ONCE here rather than per channel: a channel may be written in
    // any language and cannot be expected to share the guard. Warned about
    // only now, because a scrub nobody was going to receive is not news.
    let pane = if decision.pane_dropped {
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

    for leg in &decision.legs {
        let delivered = deliver_leg(
            leg,
            &rendered,
            &banner,
            &moshi,
            &hermes,
            native_first(channels_dir_override.is_some()),
            &channels_dir,
        );
        // THE ONE PLACE a delivery reaches the operator. A channel says what
        // happened; whether anyone hears it is the leg's reporting mode, and
        // that rule lives here rather than in three channels.
        if let Some(line) = delivered.line_for(leg.mode) {
            println!("{line}");
        }
    }
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

/// The lights signal, from whichever mode asked for it.
fn fire_pulse(hue_table: Option<toml::Table>, exit_code: &str) {
    let Some(hue) = hue_table.and_then(|settings| {
        hue_settings(&settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())
    }) else {
        return;
    };
    HuePulse {
        bridge: UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
        },
        rooms: hue.rooms,
    }
    .run(exit_code);
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
/// SILENT BY DESIGN, and deliberately so now that it says so in the type: the
/// common failure here is a channel nobody installed, and reporting that on
/// every event would be noise. The status is still dropped; what changed is
/// that it is dropped in one visible place instead of implicitly.
fn deliver(channel: &Path, event: &str) -> Delivery {
    let Ok(mut child) = Command::new(channel).stdin(Stdio::piped()).spawn() else {
        return Delivery::Silent;
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
        reread_attempts_from, reread_interval_from, resolve_path,
    };
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
}
