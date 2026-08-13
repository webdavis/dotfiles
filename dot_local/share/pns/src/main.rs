//! The relay binary: the composition root, and the only place with a main.
//!
//! Everything here is WIRING. The registrations below are the roster, the
//! environment and the config are read once at this edge, and every decision
//! is delegated to the library. It exits 0 on every path, because a
//! notification must never fail the work it reports on.

use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pns::args::parse_args;
use pns::channels::banner::BannerChannel;
use pns::channels::hermes::{
    DEFAULT_HERMES_URL, HermesChannel, UreqSignedPost, hermes_secret, remote_deadline,
};
use pns::channels::hue::{Bridge, HuePulse, hue_settings};
use pns::channels::moshi::{DEFAULT_MOSHI_URL, MoshiChannel, UreqPost, moshi_secret, read_auth};
use pns::channels::{Delivery, native_first};
use pns::config::{LoadOutcome, config_path, load_config};
use pns::engine::{Overrides, decide};
use pns::hooks::{
    HookPayload, condenser_prompt, condenser_verdict, moshi_subcommand, parse_payload,
    transcript_reply,
};
use pns::registry::{Registry, select_plugins};
use pns::render;
use pns::system::{SystemCommandRunner, SystemProbes};

fn main() {
    // The pulse is a MODE, not a leg: it fires on a long command's exit code
    // rather than on an event, so it leaves before any of the event wiring.
    let first = std::env::args_os().nth(1).unwrap_or_default();
    if first == *"pulse" {
        pulse_mode();
        return;
    }
    // The gate moshi's OWN extension calls: pi and omp spawn
    // `helperBinary pi-hook`, which never passes through a pns hook.
    if first == *"gate" {
        std::process::exit(gate_mode(
            &std::env::args_os()
                .nth(2)
                .unwrap_or_default()
                .to_string_lossy(),
        ));
    }
    if first == *"hook" {
        std::process::exit(hook_mode(
            &std::env::args_os()
                .nth(2)
                .unwrap_or_default()
                .to_string_lossy(),
        ));
    }
    event_mode();
}

/// A presence-gated pass-through to moshi-hook, for the harnesses that reach
/// it directly rather than through a pns hook.
///
/// EXIT 0 MEANS "NOT FORWARDED" on every path that declines (no moshi, the
/// operator at the desk, a subcommand this will not vouch for), which is the
/// harness's "no opinion, prompt as usual". The forwarded path is the one
/// place a non-zero exit is correct: there it is the operator's decision.
fn gate_mode(subcommand: &str) -> i32 {
    if !pns::hooks::is_harness_subcommand(subcommand) || !forward_to_moshi() {
        return 0;
    }
    let mut payload = String::new();
    let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut payload);
    forward_to_moshi_hook(subcommand, &payload)
}

/// A harness event, from the payload on stdin.
///
/// THE EXIT CONTRACT AND ITS ONE EXCEPTION. Every path here is a notification,
/// and a notification that cannot be delivered must never fail the turn it
/// reports on, so every path returns 0. The forwarded blocking path is the
/// exception: there the exit code is the OPERATOR'S DECISION, and answering it
/// here would answer the permission prompt for them.
fn hook_mode(event: &str) -> i32 {
    let mut payload_json = String::new();
    let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut payload_json);
    let payload = parse_payload(&payload_json);
    let agent = std::env::var("RELAY_AGENT").unwrap_or_else(|_| "claude".to_string());

    match event {
        "prompt" => start_of_turn(&payload),
        "stop" => end_of_turn(&payload, &agent),
        "blocked" => return blocking_event(&payload, &agent, &payload_json),
        "asked" | "plan-ready" => run_event(&pns::args::EventArgs {
            agent,
            state: event.to_string(),
            project: project_of(&payload.cwd),
            detail: payload.message.clone(),
            pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
            ..Default::default()
        }),
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
    let home = std::env::var("HOME").unwrap_or_default();
    let state_dir = resolve_path(
        std::env::var("PNS_STATE_DIR").ok().as_deref(),
        &format!("{home}/.local/state/pns"),
    );
    Some(state_dir.join(format!("session-{session_id}.start")))
}

/// How long the finished turn ran, consuming the marker. The marker is
/// VALIDATED before it reaches arithmetic: a truncated write or a hand edit
/// must be a decision, not a crash.
fn consume_turn_marker(session_id: &str) -> Option<u64> {
    let marker = turn_marker(session_id)?;
    let started = std::fs::read_to_string(&marker).ok()?;
    let _ = std::fs::remove_file(&marker);
    let started: u64 = started.trim().parse().ok()?;
    Some(now_secs()?.saturating_sub(started))
}

/// The Stop hook: what the turn said, and whether it ran long enough to earn
/// the lights.
fn end_of_turn(payload: &HookPayload, agent: &str) {
    let reply = turn_reply(payload);
    let (state, detail) = match reply.is_empty() {
        true => ("done".to_string(), String::new()),
        false => condense(&reply),
    };
    let elapsed = consume_turn_marker(&payload.session_id);
    run_event(&pns::args::EventArgs {
        agent: agent.to_string(),
        state,
        project: project_of(&payload.cwd),
        branch: git_branch(&payload.cwd),
        detail,
        pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
        long_running: pns::pulse::session_was_long(elapsed, Some(pulse_threshold_secs())),
        ..Default::default()
    });
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
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map(|meta| meta.len()).unwrap_or_default();
    let _ = file.seek(SeekFrom::Start(
        length.saturating_sub(TRANSCRIPT_TAIL_BYTES),
    ));
    let mut tail = Vec::new();
    let _ = file.read_to_end(&mut tail);
    String::from_utf8_lossy(&tail).into_owned()
}

/// The turn condensed to a state and a sentence, by a cheap model when one
/// answers and by trimming the reply when it does not.
fn condense(reply: &str) -> (String, String) {
    let fallback = || ("done".to_string(), pns::render::preview(reply));
    // The re-entry guard: the condenser is itself an agent run, and its own
    // Stop hook would call this again. The stripped home below installs no
    // hooks at all, which is the hard guarantee; this is the cheap one.
    if std::env::var("RELAY_SUMMARIZING").is_ok() {
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
        .env("RELAY_SUMMARIZING", "1")
        .env("CODEX_HOME", &home);
    match run_bounded(command, Some(&condenser_prompt(reply)), CONDENSER_DEADLINE)
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
/// own, which is the hard guarantee against a relay-to-codex-to-relay loop.
/// It is created owner-only, because it points at the live Codex credentials.
fn condenser_home() -> Option<std::path::PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let user_home = std::env::var("HOME").unwrap_or_default();
    let home = resolve_path(
        std::env::var("RELAY_CODEX_HOME").ok().as_deref(),
        &format!("{user_home}/.config/relay/codex-home"),
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

/// A blocking event: the notification first, then the operator's decision.
///
/// The notification goes out with the phone leg suppressed, because moshi is
/// about to raise the actionable card and pns's own push would be the same
/// event twice. Then the payload is written back BYTE FOR BYTE: this hook
/// consumed stdin, and a consumed-but-not-forwarded stream leaves moshi with
/// an empty parse, after which it silently does nothing.
fn blocking_event(payload: &HookPayload, agent: &str, payload_json: &str) -> i32 {
    let event = pns::args::EventArgs {
        agent: agent.to_string(),
        state: "blocked".to_string(),
        project: project_of(&payload.cwd),
        detail: payload.message.clone(),
        pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
        ..Default::default()
    };
    let Some(subcommand) = moshi_subcommand(agent).filter(|_| forward_to_moshi()) else {
        run_event(&event);
        return 0;
    };
    // Suppressed here rather than by the plan: the caller is about to raise
    // the card itself, which the surface model cannot know.
    unsafe { std::env::set_var("RELAY_SKIP_PHONE", "1") };
    run_event(&event);
    forward_to_moshi_hook(&subcommand, payload_json)
}

/// Whether the operator can answer from the phone at all. THE SURFACE decides:
/// on mobile or away the card is the only way to reach them, and at the desk
/// the harness prompt in front of them already is one.
fn forward_to_moshi() -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    let probes = SystemProbes::new(
        SystemCommandRunner,
        resolve_path(
            std::env::var("PNS_PHONE_MARKER_FILE").ok().as_deref(),
            &format!("{home}/.local/state/pns/phone-attention.marker"),
        )
        .to_string_lossy()
        .into_owned(),
    );
    pns::engine::operator_surface(&probes, &overrides_from_env(), now_secs())
        != pns::surface::Surface::Desk
}

/// Hand moshi the stream and become its answer.
fn forward_to_moshi_hook(subcommand: &str, payload_json: &str) -> i32 {
    let moshi = std::env::var("MOSHI_HOOK_BIN")
        .unwrap_or_else(|_| "/opt/homebrew/bin/moshi-hook".to_string());
    let Ok(mut child) = Command::new(&moshi)
        .arg(subcommand)
        .stdin(Stdio::piped())
        .spawn()
    else {
        // Not installed is "no opinion": the harness prompts as usual.
        return 0;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload_json.as_bytes());
    }
    // NO deadline and NO default: this waits on a human, and the code it
    // returns is their answer.
    child
        .wait()
        .ok()
        .and_then(|status| status.code())
        .unwrap_or(0)
}

/// Run a command with a deadline, returning its stdout on success.
///
/// There is no wait-with-timeout in the standard library and macOS ships no
/// `timeout(1)`, so the wait happens on a thread and the child is killed when
/// the window closes. Every spawn on a notification path is bounded: the
/// notification is worth less than the turn it reports on.
fn run_bounded(
    mut command: Command,
    stdin_text: Option<&str>,
    deadline: Duration,
) -> Option<String> {
    let mut child = command
        .stdin(if stdin_text.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let (Some(text), Some(mut stdin)) = (stdin_text, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    let mut stdout = child.stdout.take()?;
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut output = String::new();
        let _ = std::io::Read::read_to_string(&mut stdout, &mut output);
        let _ = sender.send(output);
    });
    match receiver.recv_timeout(deadline) {
        Ok(output) => {
            let _ = child.wait();
            Some(output)
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
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
    std::env::var("PNS_REPLY_REREAD_ATTEMPTS")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(DEFAULT_REREAD_ATTEMPTS)
}

fn reread_interval() -> Duration {
    std::env::var("PNS_REPLY_REREAD_INTERVAL")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .map(Duration::from_secs_f64)
        .unwrap_or(DEFAULT_REREAD_INTERVAL)
}

/// How long a turn must run to earn the lights.
fn pulse_threshold_secs() -> u64 {
    std::env::var("PNS_PULSE_THRESHOLD_SECS")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(pns::pulse::DEFAULT_LONG_SESSION_SECS)
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
        eprintln!("relay: {warning}");
    }
    run_event(&event);
}

/// One notification, end to end: decide, render, dispatch. THE one event path,
/// whether the event came from argv or from a harness hook.
fn run_event(event: &pns::args::EventArgs) {
    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    // Read off the config before selection consumes it: the pulse needs hue's
    // settings and the plan needs moshi's card toggle.
    let (hue_table, watch_card) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (enabled_hue_table(config), mobile_watch_card(config)),
        _ => (None, false),
    };
    let (selection, warning) = select_plugins(&roster(), loaded);
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }

    let overrides = overrides_from_env();
    let probes = SystemProbes::new(
        SystemCommandRunner,
        resolve_path(
            std::env::var("PNS_PHONE_MARKER_FILE").ok().as_deref(),
            &format!("{home}/.local/state/pns/phone-attention.marker"),
        )
        .to_string_lossy()
        .into_owned(),
    );
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since_epoch| since_epoch.as_secs());

    let decision = decide(
        &probes,
        &selection,
        &overrides,
        event.local_only,
        event.remote_only,
        &event.pane,
        now_secs,
        event.long_running,
        watch_card,
    );

    // The pulse is part of the PLAN now, not a second invocation: the shell
    // used to call `pns pulse` alongside the notification, which meant the
    // tier was decided twice and could disagree with itself.
    if decision.pulse {
        // The state IS the exit code here: the shell notifier derives
        // --state from `$?`, and an agent turn that did not fail succeeded.
        fire_pulse(hue_table, if event.state == "failed" { "1" } else { "0" });
    }

    if decision.legs.is_empty() {
        // A verdict that must be SAID, but only for the contradiction the
        // caller asked for: a silent exit is indistinguishable from delivery.
        if event.local_only && event.remote_only {
            println!(
                "relay: post SKIPPED -- --local-only and --remote-only were both given, which suppresses every channel; nothing was sent"
            );
        }
        return;
    }

    // Sanitized ONCE here rather than per channel: a channel may be written in
    // any language and cannot be expected to share the guard. Warned about
    // only now, because a scrub nobody was going to receive is not news.
    let pane = if decision.pane_dropped {
        eprintln!(
            "relay: dropped a pane id with shell metacharacters; no channel will focus a pane"
        );
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
    let auth = AuthFile {
        path: resolve_path(
            std::env::var("RELAY_AUTH_FILE").ok().as_deref(),
            &format!("{home}/.config/relay/auth.json"),
        ),
        contents: OnceCell::new(),
    };
    let banner = banner_channel();

    for leg in &decision.legs {
        let delivered = deliver_leg(
            leg,
            &rendered,
            &banner,
            &auth,
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
fn mobile_watch_card(config: &pns::config::Config) -> bool {
    config
        .plugins
        .get("moshi")
        .and_then(|moshi| moshi.settings.get("mobile_watch_card")?.as_bool())
        .unwrap_or(false)
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

/// The auth file, read AT MOST ONCE and only if a leg asks for it.
///
/// Eager reading made every notification wait on a file no plan might need:
/// point RELAY_AUTH_FILE at a FIFO nobody writes and an executable-only
/// delivery blocks forever. One read is still the rule, so two native legs
/// cannot sign against two different versions of the file.
struct AuthFile {
    path: std::path::PathBuf,
    contents: OnceCell<Option<String>>,
}

impl AuthFile {
    fn contents(&self) -> Option<&str> {
        self.contents
            .get_or_init(|| read_auth(&self.path))
            .as_deref()
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

/// The moshi push, with the token this event already read.
fn moshi_channel(auth: Option<&str>) -> MoshiChannel<UreqPost> {
    MoshiChannel {
        http: UreqPost::default(),
        token: auth.and_then(moshi_secret),
        url: url_from_env("RELAY_MOSHI_URL", DEFAULT_MOSHI_URL),
    }
}

/// The hermes post, with the key this event already read. The path comes with
/// it only so the not-set-up line can name the file that had no key in it.
fn hermes_channel(
    auth: Option<&str>,
    auth_path: std::path::PathBuf,
) -> HermesChannel<UreqSignedPost> {
    HermesChannel {
        post: UreqSignedPost,
        key: auth.and_then(hermes_secret),
        auth_path,
        url: url_from_env("RELAY_HERMES_URL", DEFAULT_HERMES_URL),
        sync_deadline: remote_deadline(std::env::var("RELAY_REMOTE_TIMEOUT").ok().as_deref()),
    }
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
    auth: &AuthFile,
    native_wins: bool,
    channels_dir: &Path,
) -> Delivery {
    if native_wins {
        // The two network channels are built HERE rather than up front,
        // because building one is what reads the auth file.
        match leg.name {
            "macos-banner" => return banner.deliver(rendered, leg.mode),
            "moshi" => return moshi_channel(auth.contents()).deliver(rendered, leg.mode),
            "hermes" => {
                return hermes_channel(auth.contents(), auth.path.clone())
                    .deliver(rendered, leg.mode);
            }
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

/// THE ROSTER, and the only statement of delivery order. A destination is
/// added to `registry::ROSTER`, never to policy; this turns those
/// declarations into the registry both modes select from.
fn roster() -> Registry {
    let mut registry = Registry::new();
    for (name, routing) in pns::registry::ROSTER {
        if let Err(error) = registry.register(name, routing) {
            eprintln!("pns: {error:?}");
        }
    }
    registry
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

/// The `pulse` mode: read the hue table, take the single-pulse lock, and run
/// the sequence against the bridge. Every absence is a silent exit 0.
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
    use super::resolve_path;

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
