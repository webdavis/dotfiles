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
use pns::config::tick_bridge_deadline;
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

/// Everything this binary answers to, and the flags a producer states an event
/// with. Printed on request and on a refusal, which is why it is one text: an
/// operator who mistyped and an operator who asked have the same question.
const USAGE: &str = "\
pns: usage:
  pns [<producer flags>]           one notification, stated in argv
  pns hook <event>                 a harness hook: prompt, stop, stop-failure,
                                   blocked, asked, plan-ready, denied, resolved,
                                   model-switch, quota, config-change
  pns gate <harness>-hook          presence-gated pass-through to moshi-hook
  pns <harness>-hook               the same gate, spelled the way moshi calls it
  pns pulse <exit-code>            signal the lamps by hand
  pns quiet [<duration>|off]       the operator's mute
  pns daemon run|schedule|cancel   the clock
  pns lights tick|quiet            the lamps' upkeep
  pns loop begin|end               take the loop lamp by hand, and give it back
  pns nag                          card every outstanding approval
  pns recap --since <epoch> --until <epoch>
  pns setup [--force]              write a first config, one question at a time
  pns doctor                       one test send through every channel
  pns home                         one reading of the router, said out loud
  pns --help, -h                   this text

producer flags: --agent <name> --state <word> --project <name> --branch <name>
                --detail <text> --pane <id> --channel <route>
                --local-only --remote-only --long-running
";

/// Whether argv is a PRODUCER invocation rather than a mistyped subcommand.
///
/// IT READS THE WHOLE OF ARGV, not just the leading word, and that is the
/// point. The parser deliberately accepts a stray token in front of the real
/// flags, so a leading word alone does not make an invocation a typo: what does
/// is argv carrying no producer flag, and no `--help`/`-h`, anywhere. Refusing
/// on the first word alone would drop real notifications, which is the exact
/// mirror of the bug this refusal exists to fix.
///
/// AN EMPTY ARGV is the bare invocation `args` calls a valid empty event.
/// A DASH-LED FIRST WORD IS NO LONGER A FREE PASS: that used to make ANY
/// dash-led argv[1] a producer invocation, so a mistyped flag (`--wat`,
/// `-help`, `--agent=claude`) delivered an empty event in silence, the `pns
/// stpo` bug reopened for a typo that happens to start with a dash.
/// `--help`/`-h` ARE COUNTED, so a producer invocation that only adds
/// `--help` still reaches the parser below, which is where the help arm
/// actually prints the usage and returns.
fn is_producer_argv(argv: &[String]) -> bool {
    argv.is_empty()
        || argv
            .iter()
            .any(|token| pns::args::is_producer_flag(token) || pns::args::is_help_flag(token))
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

/// Text safe to render or store, ON TOP OF `flattened`: whitespace and
/// control characters collapsed as `flattened` already does, and Unicode
/// format characters (`recap::is_invisible`) stripped besides.
///
/// STRIPS `recap::is_invisible` ON TOP OF `flattened`, never inside it:
/// `flattened` is shared by every other rendered field on this path, and this
/// crate has two callers with a reason a format character must not survive at
/// all rather than merely render inertly. `model_switch_detail` compares two
/// names for equality, which a reordering character could defeat silently (a
/// name that reads the same but compares unequal, or the reverse); the
/// config-change arm writes a path into a durable state file as well as a
/// card, and an invisible character there would round-trip identically on
/// every future read. Widening `flattened` itself for two callers would let
/// every other field silently start allowing format characters through too.
fn rendered_plainly(text: &str) -> String {
    flattened(text)
        .chars()
        .filter(|character| !pns::recap::is_invisible(*character))
        .collect()
}

/// The automatic model-switch card's detail, or `None` when there is no
/// transition worth one: either name empty once flattened and stripped of
/// invisible characters, or the two equal once stripped.
fn model_switch_detail(from_model: &str, to_model: &str) -> Option<String> {
    let from = rendered_plainly(from_model);
    let to = rendered_plainly(to_model);
    if from.is_empty() || to.is_empty() || from == to {
        return None;
    }
    Some(format!("automatic session model change: {from} to {to}"))
}

/// A `ConfigChange` payload field, rendered plainly and CUT, with the cut
/// marked: `clipped` says it happened rather than handing a reader a path
/// that silently is not the one on disk.
///
/// THE CUT IS WHAT KEEPS THE AUDIT TRAIL: both fields this arm reads are
/// harness text bounded only by `MAX_PAYLOAD_BYTES` (1 MB), and both land in
/// a ring whose prune runs on a read-back capped at `RING_READ_MAX` (256
/// KiB). One oversized path makes that read-back fail, and the heal then
/// collapses the whole trail to the single line just written, losing every
/// policy change recorded before it. `decision_log`'s `IDENTITY_MAX` is the
/// same defence at the same boundary, for the same reason.
fn config_field(text: &str, max_chars: usize) -> String {
    render::clipped(&rendered_plainly(text), max_chars)
}

/// The longest path a `ConfigChange` field carries into a card or the audit
/// trail. THE CARD AND AUDIT BUDGET, not a claim about every real path: it is
/// macOS's own `PATH_MAX`, but Linux's is 4096, so a genuinely long Linux path
/// IS visibly clipped here, with the cut marked rather than silent. Short
/// enough that the trail's own arithmetic holds regardless: see
/// `POLICY_SETTINGS_AUDIT_KEPT`.
const CONFIG_PATH_MAX_CHARS: usize = 1024;

/// The longest session id the audit trail carries. A session id is a UUID in
/// every harness this serves; the cap is what stops one nobody validated from
/// filling a line.
const CONFIG_SESSION_MAX_CHARS: usize = 64;

/// The five documented `ConfigChange` sources, and nothing else: an exact
/// allowlist, matching the exact matcher declared beside it in
/// `modify_settings.json`. THIS IS THE RUST-SIDE BACKSTOP the declaration's
/// matcher alone cannot be trusted to be: `parse_payload` accepts any string
/// under this key, so a direct invocation, a drifted declaration, or a future
/// value Claude Code adds would otherwise reach a card for a source this
/// binary has never verified. A `ConfigChange` carrying any other `source`
/// yields `None`, in `quota_label`'s own style.
fn config_source_label(source: &str) -> Option<&'static str> {
    match source {
        "user_settings" => Some("user settings changed"),
        "project_settings" => Some("project settings changed"),
        "local_settings" => Some("local settings changed"),
        "policy_settings" => Some("policy settings changed"),
        "skills" => Some("skills changed"),
        _ => None,
    }
}

/// A configuration-change card's detail: which of the five sources changed,
/// and the file Claude Code named, when it named one. `None` for an
/// unmatched source, in `quota_observation_detail`'s own style.
///
/// NEVER "WHAT CHANGED": the payload carries no key, no old or new value and
/// no actor, so the detail says only WHICH SOURCE and, optionally, WHICH
/// FILE. `file_path` is untrusted text that lands in a banner and a card, so
/// it goes through `rendered_plainly` exactly as a hostile model name does.
fn config_change_detail(source: &str, file_path: &str) -> Option<String> {
    let label = config_source_label(source)?;
    let path = config_field(file_path, CONFIG_PATH_MAX_CHARS);
    Some(if path.is_empty() {
        label.to_string()
    } else {
        format!("{label}: {path}")
    })
}

/// How many received `policy_settings` changes the audit trail remembers,
/// comfortably past the five-entry decision ring (`decision_log::KEPT`): a
/// policy change is rarer and more consequential than an ordinary observed
/// event, and it must outlive more than a handful of intervening turns rather
/// than vanish with them the moment the ring rolls over.
///
/// THE ARITHMETIC `append_ring_line` ASKS EVERY CALLER FOR, against the
/// `RING_READ_MAX` this passes beside it: a line is a timestamp, a session cut
/// to `CONFIG_SESSION_MAX_CHARS` and a path cut to `CONFIG_PATH_MAX_CHARS`, so
/// its worst case is about 4.4 KB of UTF-8 and twenty of them about 88 KB,
/// comfortably inside the reader's 256 KiB ceiling. Without both cuts the
/// depth alone would not bound the FILE, and a ring past that ceiling can
/// never be pruned again: the heal fires and the trail collapses to one line.
const POLICY_SETTINGS_AUDIT_KEPT: usize = 20;

/// The policy-settings audit trail's file name, beside `DECISIONS` and
/// `ACTIVITY`.
const POLICY_SETTINGS_AUDIT: &str = "policy-settings-audit";

/// Append one received `policy_settings` change to a bounded, state-only
/// audit record, so it outlives the five-entry decision ring an ordinary
/// observed event is logged to. STATE-ONLY, in `record_missed`'s style: no
/// card of its own, no marker, no lease; the routing this rides beside stays
/// marker-neutral, and this is purely a durable trace of receipt for a class
/// of change worth remembering past the next few turns.
///
/// FAIL-QUIET, in `record_decision`'s exact style and for its exact reason:
/// an event path whose stdout a harness hook reads must not gain a line about
/// the state directory, and a record that did not land costs a read of this
/// file later, never a card.
fn record_policy_settings_change(session_id: &str, file_path: &str, now: Option<u64>) {
    let now = now.unwrap_or_default();
    let session = config_field(session_id, CONFIG_SESSION_MAX_CHARS);
    let path = config_field(file_path, CONFIG_PATH_MAX_CHARS);
    let path = if path.is_empty() { "none" } else { &path };
    let line = format!("{now} session={session} file={path}");
    let _ = append_ring_line(
        &state_dir().join(POLICY_SETTINGS_AUDIT),
        &line,
        POLICY_SETTINGS_AUDIT_KEPT,
        RING_READ_MAX,
    );
}

/// The three quota-notification labels this binary recognises, and nothing
/// else: an exact allowlist, matching the exact matcher declared beside it in
/// `modify_settings.json`. A `Notification` carrying any other
/// `notification_type` (a permission prompt, an elicitation dialog, the
/// deferred `agent_needs_input` and `agent_completed`) yields `None`, which is
/// silence, never a guess at what the harness meant.
fn quota_label(notification_type: &str) -> Option<&'static str> {
    match notification_type {
        "quota_auto_resume_fired" => Some("quota auto-resume fired"),
        "quota_auto_resume_stale" => Some("quota auto-resume stale"),
        "quota_auto_resume_disabled" => Some("quota auto-resume disabled"),
        _ => None,
    }
}

/// A quota-notification card's detail: which of the three happened, and the
/// message Claude Code stated about it. `None` for an unmatched type, in
/// `model_switch_detail`'s own style.
fn quota_observation_detail(notification_type: &str, message: &str) -> Option<String> {
    let label = quota_label(notification_type)?;
    Some(if message.is_empty() {
        label.to_string()
    } else {
        format!("{label}: {message}")
    })
}

/// Arm the needs marker for a stale quota auto-resume wait, the one exception
/// among the three quota types.
///
/// `Attempt::Observation` never reaches `update_blocked_marker` (`run_event`
/// returns before it for anything but `Attempt::First`), which is the whole
/// point for `fired` and `disabled`: neither reports a session waiting on the
/// operator, so neither should colour a lamp that says one is. `stale` does:
/// Claude Code's interactive-mode reference documents that after a sleep of
/// more than about thirty minutes the session stops and reads `Your usage
/// limit has reset - press enter to continue`, which is a wait on the operator
/// by the same definition every other blue lamp here uses. So this calls the
/// marker's own Start operation directly, a state-only file write in D1's
/// style, rather than routing the whole event through `Attempt::First` and
/// picking up the journal, the presence edge and the loop-lease renewal that
/// come with it.
///
/// AND WHAT CLEARS IT IS NOT THE PROMPT HOOK, or not only. The reference says
/// Claude Code continues by sending Claude a fixed prompt of its own; it does
/// NOT say whether that internal prompt reaches the `UserPromptSubmit` hook,
/// and this repository has no capture that settles it either way, so a marker
/// whose only clear was `pns hook prompt` would be a bet on an undocumented
/// detail of another program. It is not one: EVERY event from that session
/// except the four that start a wait ends one (`blocked_marker_action`), so
/// the continued turn's own Stop clears this marker whether or not the
/// continuation ever reached the prompt hook, and the operator typing anything
/// at all clears it sooner. The prompt hook is the FAST path and the Stop is
/// the guarantee, which is why both are pinned by a test.
///
/// AND IT RUNS BEFORE THE DELIVERY, not after it. The declaration is
/// `async: true`, so this hook runs BESIDE the session it reports on while the
/// screen is already telling the operator to press Enter. Arming after the
/// delivery plan would let an Enter inside that window clear nothing, because
/// there would be no marker yet, and then take a marker published behind it:
/// a blue lamp for a session that is working again, held until that turn's own
/// Stop. Ordering cannot CLOSE that race, which is the harness's to close, but
/// it shrinks the window from a plan of network legs to one file write.
///
/// KEYED BY SESSION, like every other wait: `blocked_marker_action("blocked")`
/// is `Action::Start` (it is one of `pulse::LAMP_BLOCKED`), so this reuses the
/// exact mechanism `blocking_event` uses rather than inventing a second one.
fn arm_quota_stale_wait(session_id: &str, probes: &SystemProbes<SystemCommandRunner>) {
    let home = std::env::var("HOME").unwrap_or_default();
    let lamps_live = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => {
            enabled_hue_table(&config).is_some() && config.lights.is_some()
        }
        _ => false,
    };
    update_blocked_marker(
        &state_dir(),
        session_id,
        "blocked",
        lamps_live,
        probes.now_secs(),
    );
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
        // AND THE WAIT ENDS HERE TOO, beside the turn marker. A prompt is the
        // operator typing, which answers ANY live wait their session could be
        // holding: `resolved`'s PostToolBatch signal never fires for a
        // PermissionRequest (Claude Code decides that off this hook's own
        // stdout), so without this the lamp stayed blue until the turn's Stop,
        // one whole tool call after the operator had already answered.
        "prompt" => {
            start_of_turn(&payload);
            end_blocked_wait(&payload.session_id);
        }
        "stop" => end_of_turn(&payload, &agent),
        "stop-failure" => failed_turn(&payload, &agent),
        "blocked" => return blocking_event(&payload, &agent, &payload_json),
        // The PostToolBatch clearing signal. The batch this session was blocked
        // on has RESOLVED, whichever way the operator answered: a denial still
        // produces a `tool_result` and so still resolves the batch.
        //
        // IT LOADS NO CONFIG AND DELIVERS NOTHING. A record exists only because
        // the feature was on when the approval arrived, so clearing it is right
        // regardless of what the config says now, and that keeps this per-batch
        // path to a payload read, a parse and at most two file operations.
        //
        // AND THE WAIT ENDS HERE TOO, GUARDED. `agent_id` is present only
        // inside a subagent call, so a batch carrying the KEY (whatever its
        // value; a malformed one is not proof of the main thread) resolved a
        // SUBAGENT'S tool, not the parent session's own wait on the operator;
        // clearing on it anyway would go dark on a wait nobody has answered.
        // RESIDUAL, STATED HONESTLY: the parent's marker then stays lit until
        // its own Stop, same as before this fix.
        // AND THIS ARM IS ASYNC (PostToolBatch, `async: true`), so it is
        // UNORDERED against the next PermissionRequest and the batch's own
        // `asked`: a late End can unlink a newer wait's marker, an early one
        // can leave an answered `asked` lit. The same one-file-per-session
        // limit `update_blocked_marker` states; bounded the same way, by the
        // backstop and the session's next event.
        "resolved" => {
            clear_nag(&payload.session_id);
            if !payload.in_subagent {
                end_blocked_wait(&payload.session_id);
            }
        }
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
            &payload,
            Attempt::First,
        ),
        // `PostModelSwitch`, restricted to the one `source` that is news:
        // `command`, `picker` and `sdk` are the operator or the harness
        // choosing a model on purpose, and `resume`, which the harness also
        // does on its own, is D4b's own follow-up (a state-only audit record,
        // not a notification). Only `auto` is routed, and it is routed as an
        // OBSERVATION: it is news about the session, not a turn needing the
        // operator's attention, so it must not clear a wait, renew a lease or
        // claim the return moment. Labelled "automatic session model
        // change", never "fallback": the payload cannot tell a fallback
        // chain apart from every other automatic change.
        // NEITHER NAME IS AN OPINION WORTH A CARD, so the arm writes nothing
        // at all when `model_switch_detail` finds equal names once flattened
        // and stripped, or either side empty.
        "model-switch" if payload.source == "auto" => {
            if let Some(detail) = model_switch_detail(&payload.from_model, &payload.to_model) {
                run_event(
                    &pns::args::EventArgs {
                        agent,
                        state: event.to_string(),
                        project: project_of(&payload.cwd),
                        detail,
                        pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
                        ..Default::default()
                    },
                    &system_probes(),
                    &payload,
                    Attempt::Observation,
                );
            }
        }
        "model-switch" => {}
        // The one `Notification` arm, covering the ONE exact allowlist
        // declared beside it in `modify_settings.json`:
        // `quota_auto_resume_fired`, `quota_auto_resume_stale` and
        // `quota_auto_resume_disabled`. `agent_needs_input` and
        // `agent_completed` are deliberately unwired (D7): the former may
        // duplicate an ordinary asked or blocked event and the latter
        // combines success and failure in one notification type, so either
        // needs a live capture before it can be mapped honestly. Routed as an
        // OBSERVATION like the model-switch arm beside it: quota events are
        // news about the session, not a turn needing the operator's
        // attention, so delivery must not clear a wait, renew a lease or
        // claim the return moment on its own.
        "quota" => {
            if let Some(detail) =
                quota_observation_detail(&payload.notification_type, &payload.message)
            {
                let probes = system_probes();
                // THE ONE EXCEPTION, AND IT GOES FIRST: see
                // `arm_quota_stale_wait` for both halves of why.
                if payload.notification_type == "quota_auto_resume_stale" {
                    arm_quota_stale_wait(&payload.session_id, &probes);
                }
                run_event(
                    &pns::args::EventArgs {
                        agent,
                        state: event.to_string(),
                        project: project_of(&payload.cwd),
                        detail,
                        pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
                        ..Default::default()
                    },
                    &probes,
                    &payload,
                    Attempt::Observation,
                );
            }
        }
        // `ConfigChange`, restricted to the FIVE DOCUMENTED SOURCES via an
        // exact Rust-side allowlist (`config_source_label`) that mirrors,
        // rather than trusts, the declaration's own exact matcher: a direct
        // invocation, a drifted declaration, or a future value Claude Code
        // adds must not reach a card this binary never verified. Routed as an
        // OBSERVATION, like the model-switch and quota arms beside it: this
        // is a configuration audit trail, not a turn needing the operator's
        // attention, so delivery must not clear a wait, renew a lease, or
        // claim the return moment. ONE CARD PER RECEIVED EVENT, deliberately:
        // there is no once-per-something guarantee to keep, because a
        // corrupt-file recovery, several live sessions, or a changed skill
        // can each produce their own event, so this fires again for every
        // distinct invocation rather than coalescing them.
        "config-change" => {
            if let Some(detail) = config_change_detail(&payload.source, &payload.file_path) {
                let probes = system_probes();
                // THE ONE SOURCE THAT OUTLIVES THE CARD: see
                // `record_policy_settings_change` for why a policy change
                // gets a bounded audit line on top of the ordinary decision
                // ring every observation is logged to.
                if payload.source == "policy_settings" {
                    record_policy_settings_change(
                        &payload.session_id,
                        &payload.file_path,
                        probes.now_secs(),
                    );
                }
                run_event(
                    &pns::args::EventArgs {
                        agent,
                        state: event.to_string(),
                        project: project_of(&payload.cwd),
                        detail,
                        pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
                        ..Default::default()
                    },
                    &probes,
                    &payload,
                    Attempt::Observation,
                );
            }
        }
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
    // NO CLOCK IS NO MARKER, never a marker at epoch zero: the same rule
    // `update_blocked_marker` states beside its own clock. A marker at zero
    // would measure the turn from 1970, so `consume_turn_marker` would call a
    // two-second turn long-running and it would earn the watch card and the
    // pulse; no marker measures nothing, and `session_was_long` reads that as
    // not long.
    if !marker.exists()
        && let Some(now) = now_secs()
    {
        let _ = std::fs::write(&marker, now.to_string());
    }
}

/// The spool name the tick job is registered under. ONE JOB FOR THE WHOLE
/// HOUSE, not one per lamp: the tick derives every state from scratch and
/// writes every fixture, so a second job would be a second writer of the same
/// bulbs.
const LIGHTS_JOB: &str = "lights";

/// How long the tick runs on after an ordinary event. A working loop emits
/// events constantly, so five minutes covers an agent's thinking gap without
/// covering a stall.
const ORDINARY_LEASE_SECS: u64 = 300;

/// And after a journalled one, which is an operator who is away or muted. The
/// glow has to survive the whole absence, and the absence is precisely when no
/// further event arrives to refresh this.
const JOURNALLED_LEASE_SECS: u64 = 12 * 60 * 60;

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

/// Start or end this session's wait on the operator, which is what the blue
/// lamp is derived from.
///
/// ONE FILE PER WAITING SESSION, named by the session id through
/// `lights::blocked_marker`, so a harness id that cannot be a filename writes
/// nothing at all rather than escaping the state directory.
///
/// EVERY EVENT ENDS A WAIT EXCEPT THE FOUR THAT START ONE, which is
/// `blocked_marker_action`'s rule and not a second copy of it here.
///
/// THE LAG, NAMED RATHER THAN HIDDEN: the marker clears at the NEXT event from
/// that session, never at the instant the operator answered, because no event
/// reports the answer itself. STOP IS THE LAST OF THE ARMS THAT GET THERE, not
/// the only one: `prompt` clears on the operator typing and `resolved` on the
/// tool batch coming back, and each is why the two arms carry a comment of
/// their own. The worst case left is a wait whose session produces neither
/// before its turn ends, and the SUBAGENT RESIDUAL, which `resolved` skips by
/// design and which therefore does hold blue until the parent's own Stop. The
/// tick's own bound is what stops an abandoned session holding it forever, and
/// the day item 21's rebuild wires a real answered signal this consumes it at
/// the same call site.
///
/// STARTING ONE RIDES BEHIND THE `[lights]` TABLE, and ENDING ONE DOES NOT. A
/// machine that never asked for the lamps must not start accumulating files
/// about them, and nothing would ever sweep them there: the tick is the only
/// sweeper and it does not run without the table. Removal is one unlink with
/// nothing to accumulate, and gating it too meant a wait that ended while the
/// lamps were off kept its marker: switching hue back on inside the configured
/// backstop then put blocked on a lamp for a session nobody was waiting on.
///
/// THE OLDER STOP CAN REMOVE THE NEWER WAIT'S MARKER, and that is a stated
/// limit rather than a rule. One file per SESSION carries no generation, so a
/// blocked event that publishes a new wait while the previous Stop is still
/// condensing loses it when that Stop reaches this line. Unlink cannot
/// arbitrate on this filesystem (concurrent unlink reports success to every
/// caller on APFS), so telling the two apart would need a generation IN the
/// marker and a compare-and-swap publish over it. The damage is bounded by the
/// backstop above and closed by the session's next event, which re-publishes
/// the wait it is still in.
///
/// THE BACKSTOP CANNOT SWEEP A MARKER THE NAG HAS NOT YET NUDGED, and that is
/// held at CONFIG LOAD rather than here: `[lights.blocked] give_up_after_secs`
/// shorter than `[nag] after_secs` is refused by name (`config::parse_config`),
/// because it is a config that gives up on a wait before it ever nudges about
/// it. Nothing at this level re-publishes a swept marker, so nothing here has
/// to tell an abandoned session from a live one.
///
/// FAIL-QUIET, in `record_missed`'s exact style and for its exact reason.
fn update_blocked_marker(
    state_dir: &Path,
    session_id: &str,
    event_state: &str,
    lamps_live: bool,
    now: Option<u64>,
) {
    let Some(marker) = pns::lights::blocked_marker(state_dir, session_id) else {
        return;
    };
    match pns::lights::blocked_marker_action(event_state) {
        pns::lights::Action::Start if !lamps_live => {}
        pns::lights::Action::Start => {
            // THE DECISION'S OWN CLOCK, as record_news beside it: this reads
            // the moment the decision was made for, never a fresh one taken
            // inside this function. NO CLOCK IS NO MARKER, never a marker at
            // epoch zero: the bound that expires an abandoned wait is
            // measured against this number, and a zero would be expired the
            // moment it was written or, read the other way, would be a wait
            // nobody could age out.
            if let Some(now) = now {
                let _ = publish_state_line(&marker, &now.to_string());
            }
        }
        // The failure is DROPPED here and nowhere else: see the doc comment.
        pns::lights::Action::End => {
            let _ = std::fs::remove_file(&marker);
        }
    }
}

/// End this session's wait on the operator directly: a state-only file move
/// in `clear_nag`'s style, with no event built, no config loaded and no
/// decision made.
///
/// TWO CALLERS NEED EXACTLY THIS, both in `hook_mode`: `prompt`, because the
/// operator answering a live wait by typing is not `resolved`'s signal
/// (PermissionRequest is decided off this hook's stdout, never off a later
/// PostToolBatch), and `resolved` itself, guarded there against a subagent's
/// batch. Ending is unconditional, unlike starting one: see
/// `update_blocked_marker`'s comment on why an End never checks the lamp
/// switches.
fn end_blocked_wait(session_id: &str) {
    if let Some(marker) = pns::lights::blocked_marker(&state_dir(), session_id) {
        let _ = std::fs::remove_file(&marker);
    }
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
    mobile: &Mobile,
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
    let _ = dispatch_legs(&decision.legs, false, &replay, home, mobile, hermes_key);
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

/// How many times an append waits for a ring's own lock before giving up
/// rather than risk the very race the lock exists to prevent.
///
/// A HANDFUL OF SHORT SLEEPS PAST WHAT THE CRITICAL SECTION ITSELF EVER
/// TAKES: the whole locked span is one small read, one rewrite and one
/// rename, so a live holder clears in microseconds. Giving up costs the ONE
/// event that could not get in, in `record_decision`'s own fail-quiet style;
/// it never risks publishing over a sibling's newer state, which is the loss
/// this lock exists to prevent.
const RING_LOCK_ATTEMPTS: u32 = 200;

/// How long a ring's own lock is believed before a holder that died on it is
/// read as an orphan. Long past any real critical section, so this only ever
/// fires for a crash, in `lights_tick_stale_secs`'s own style for its own
/// job.
const RING_LOCK_STALE_SECS: u64 = 5;

/// The path beside a ring's own that arbitrates between two processes
/// touching it at once.
fn ring_lock_path(path: &Path) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    std::path::PathBuf::from(name)
}

/// One ring's lock, WAITED FOR rather than skipped: unlike a lights tick,
/// which safely stands down from a busy window and picks the lamp up again
/// next interval, standing down here means silently losing whichever event
/// is mid-append. Reuses `claim_lock`, the one shape every lock in this
/// binary uses (see its own doc comment), rather than a second mechanism.
/// Bounded anyway, in this binary's own style: `RING_LOCK_ATTEMPTS` short
/// sleeps, and a hold that outlasts all of them is read as broken rather than
/// waited on forever.
fn claim_ring_lock(path: &Path) -> Option<HeldLock> {
    let lock = ring_lock_path(path);
    // A CLOCK THAT CANNOT BE READ COUNTS AS ZERO, which is `lock_aged_out`'s
    // own safe direction under a different name: a held lock is never read as
    // older than it is, so a broken clock can stand this caller down but
    // never lets it steal a live holder's claim.
    let now = now_secs().unwrap_or(0);
    for attempt in 0..RING_LOCK_ATTEMPTS {
        if claim_lock(&lock, now, RING_LOCK_STALE_SECS) {
            return Some(HeldLock(lock));
        }
        if attempt + 1 < RING_LOCK_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    None
}

/// The append and the prune behind it, for ANY of this tool's bounded state
/// rings. The caller names the file and its own depth; everything below is
/// one hardening serving every one of them, because a second hand-written
/// copy of it is how one ring ends up without the FIFO guard.
///
/// THE WHOLE OPERATION IS ONE CLAIM: append, read-back, prune and publish all
/// happen while this process alone holds the ring's own lock. Two events
/// firing at once (a Stop hook and the long-running notifier are a normal
/// pair) used to be safe only for the append itself; the prune's read and its
/// publish were NOT one atomic step, so a racer that read before a sibling's
/// append could still publish its stale, smaller window AFTER the sibling
/// published a newer one, silently dropping the sibling's line and keeping
/// the wrong oldest entry. The lock is what makes the four steps indivisible,
/// which is also what retires the old accepted limit below: an append can no
/// longer land during a sibling's rename, because no sibling is ever inside
/// this section at the same time.
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
/// `read_max` IS THE CALLER'S TOO, and it travels with `kept` because the two
/// are one decision. The prune runs on the READ-BACK, so a ring deep enough to
/// exceed the reader's ceiling can never be pruned again: the heal fires and
/// the file collapses to the one line just written, silently, exactly when it
/// is fullest. Every caller states both numbers together, and the doc comment
/// on each depth does the arithmetic.
fn append_ring_line(path: &Path, line: &str, kept: usize, read_max: u64) -> std::io::Result<()> {
    // BEFORE THE CLAIM: the lock lives beside the ring, so a state directory
    // that does not exist yet fails the lock's own exclusive create with the
    // same `NotFound` the ring's own open used to paper over here. A first
    // event has nowhere else to make this directory.
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Some(_lock) = claim_ring_lock(path) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "the ring's lock stayed held past every attempt",
        ));
    };
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
    // A TEST-ONLY STALL, in `env_deadline`'s own words: it exists so a test
    // can prove this section is exclusive rather than hope a real race lands
    // in a window that is normally microseconds wide. Unset in every real
    // invocation, so production takes no delay here at all.
    if let Some(delay) = env_deadline("PNS_RING_LOCK_TEST_DELAY_MS") {
        std::thread::sleep(delay);
    }
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
    // AND THE FREE CLEARING SIGNAL WITH IT. A turn cannot end while one of its
    // own approvals is unanswered, so a turn end proves resolution. It costs one
    // function call, no hook declaration and no apply, and it is the backstop
    // for a batch payload over the 1MB cap, an operator who escaped the prompt
    // instead of answering it, and the window between this merge and the apply
    // that installs the PostToolBatch entry.
    clear_nag(&payload.session_id);
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
        payload,
        Attempt::First,
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
    // The same free clear `end_of_turn` takes, for the same reason: StopFailure
    // fires INSTEAD of Stop, so without it a dead turn leaves its approval armed.
    clear_nag(&payload.session_id);
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
        payload,
        Attempt::First,
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
    // AFTER THE FORWARD IS STARTED AND BEFORE THE NOTIFICATION. The forward is
    // the operator-facing round trip and nothing may sit in front of its spawn;
    // arming is a config read, three file operations and a spool write, taken
    // here so the clock starts at the true prompt time and so a notification
    // that dies still leaves a timer armed, which is the direction that helps
    // the operator.
    //
    // AND THAT CONFIG READ IS THE THIRD ON THIS PATH, said plainly because the
    // other two are. `run_event` loads it, the wait below loads it again, and
    // `arm_nag` loads it here; each is one open and one TOML parse of a file
    // measured in kilobytes, off local disk, with no network and no subprocess
    // in any of them. It is named for honesty rather than as a cost worth
    // routing around: threading one view through would change three signatures
    // for a value each caller reads at the moment it needs it.
    arm_nag(&payload.session_id, &event);
    run_event(&event, &probes, payload, Attempt::First);
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
/// what makes this reading and the delivery plan's reading the SAME one FOR
/// `blocking_event`: they are two questions about one moment, and a boundary
/// crossed between two measurements cards a phone with no round trip behind
/// it. `pns gate <harness>-hook` (see `gate_mode`) calls this with its own
/// throwaway probe set and runs no delivery plan at all, so the claim does
/// not extend to that caller.
fn forward_to_moshi(probes: &SystemProbes<SystemCommandRunner>) -> bool {
    // FOR `blocking_event`, THE SAME CLOCK THE DELIVERY PLAN READS BELOW, off
    // this probe set's own memoized cell rather than a fresh wall-clock read:
    // see R4-1. Two reads of the wall clock for one event is the boundary
    // that drifted a phone reading and a desk reading apart. `gate_mode`
    // calls this with its own throwaway probe set and runs no delivery plan.
    pns::engine::operator_surface(probes, &overrides_from_env(), probes.now_secs())
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
/// `[plugins.mobile] submit_deadline_secs`, then the default.
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

/// One notification from argv, or a usage print when `--help`/`-h` reached
/// the parse in FLAG position.
fn event_mode(argv: &[String]) {
    let (event, warnings) = parse_args(argv.iter().cloned());
    // HELP WINS BEFORE ANYTHING ELSE ON THIS PATH: no config load, no probe.
    // It used to reach EVERYTHING when it fell through this same parser as an
    // unknown token, which notified about an empty event and raised a banner
    // titled "pns · done". Nothing about printing the commands needs the
    // machine read.
    if event.help {
        print!("{USAGE}");
        return;
    }
    for warning in &warnings {
        eprintln!("pns: {warning}");
    }
    // ARGV CARRIES NO PAYLOAD, which is the honest no-identity case.
    run_event(
        &event,
        &system_probes(),
        &HookPayload::default(),
        Attempt::First,
    );
}

/// Whether this is the event's FIRST delivery, a NUDGE about one already
/// recorded, or an OBSERVATION.
///
/// ONE ARGUMENT RATHER THAN A SECOND EVENT PATH. A nudge is an ordinary event
/// in every respect an operator can see (the mute, the named Focus modes, the
/// quiet window, the surface and visibility plan, fresh probes taken in the
/// nudge's own process); what it is not is a second OCCURRENCE, and the
/// contiguous tail of `run_event` is what records occurrences.
///
/// AN OBSERVATION IS THE SAME KIND OF NON-OCCURRENCE, for a different reason:
/// it is a harness telling pns about something that happened rather than a
/// turn needing the operator's attention, so it changes no workflow or marker
/// state and is routed marker-neutral through the same tail a nudge skips.
/// It is still recorded as a decision (`record_decision` runs before the
/// guard for every attempt), just with `nag=no`.
///
/// AN OBSERVATION SHAPED LIKE A `PermissionRequest` IS TOO LATE TO GATE HERE.
/// `blocking_event` forwards to moshi and arms the nag before `run_event`
/// ever runs, so this guard cannot undo either one; a caller on that path
/// must refuse the observation at the top of `blocking_event` itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Attempt {
    First,
    Nudge,
    Observation,
}

/// One notification, end to end: decide, render, dispatch. THE one event path,
/// whether the event came from argv or from a harness hook.
///
/// THE PAYLOAD RIDES BESIDE THE EVENT RATHER THAN INSIDE IT, and the split is
/// the point: `EventArgs` is the ARGV contract, and argv has no spelling for a
/// session id, a permission mode, a subagent id or a raw tool name. Every one
/// of those arrives in a harness payload or not at all, so the hook arms pass
/// what they were given and every other caller passes `HookPayload::default()`,
/// which is honestly no identity rather than fields nothing can fill. The
/// lamps' needs marker and the decision line are its readers.
fn run_event(
    event: &pns::args::EventArgs,
    probes: &SystemProbes<SystemCommandRunner>,
    payload: &HookPayload,
    attempt: Attempt,
) {
    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    // Read off the config before selection consumes it: the pulse needs hue's
    // settings, the plan needs the mobile card toggle, the catch-up needs the
    // whole `[recap]` table, and the two network channels need their secrets.
    //
    // THE RECAP TRAVELS AS ONE NAMED VALUE, never as a row of loose booleans.
    // Three of its four fields are bools; spread into this tuple they would sit
    // adjacent here and in the call below, which is a swap nothing would catch,
    // and a struct with named fields cannot be transposed.
    //
    // AND THE MOBILE TABLE'S VERDICT DOES TOO, for a second reason on top of
    // that one: its token, its toggle and its refusal are three answers to ONE
    // question, and reading them separately is what let the refusal be dropped
    // on the way to a leg that then delivered anyway.
    let (hue_table, lights, mobile, hermes_key, recap, focus_silence) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (
            enabled_hue_table(config),
            config.lights.clone(),
            read_mobile(config),
            plugin_settings(config, "hermes").and_then(hermes_secret),
            config.recap.clone(),
            config.focus_silence.clone(),
        ),
        // A config that is absent or could not be read falls back to the
        // DEFAULTS of all five, and deliberately disagrees with the plugin
        // selection below, which falls back to the CORE. Selection keeps
        // notifications working through a broken config; these say what an
        // operator asked for, and a file nobody could read asked for nothing:
        // with no secrets, the network channels are simply not set up.
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
        // THE LAMPS FALL BACK TO ABSENT, which is the same reading as hue's own
        // table beside it: a file nobody could parse named no family, and a map
        // this could not read must not be replaced with a guess about which
        // lamps are whose.
        _ => (
            None,
            None,
            Mobile::default(),
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
    // off the config directly, so this and dispatch answer one question once.
    // A machine that turned the durable channel off has said there is nowhere
    // for a recap to go, and a card reading "recap in #pns" against an empty
    // channel is the one thing the card's own spawn check exists to prevent.
    //
    // A MACHINE WITH NO CONFIG NOW HAS NO DURABLE ROUTE EITHER, and that falls
    // straight out of the core fallback: hermes needs a key stood up before it
    // can carry anything, so it is not in the core and no recap is promised
    // against it.
    let durable_route = selection.iter().any(|plugin| plugin.name == "hermes");

    // THE SAME CLOCK `forward_to_moshi` READS, off this probe set's own
    // memoized cell: see R4-1. On the blocked path that read came first and
    // this answers the same second; on every other path this is the first and
    // only read. A second wall-clock read here is exactly the boundary that
    // let a phone reading and a desk reading about one event disagree.
    let now_secs = probes.now_secs();
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
        mobile.watch_card,
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
            &mobile,
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
        nag: attempt == Attempt::Nudge,
        permission_mode: &payload.permission_mode,
        agent_id: &payload.agent_id,
        tool_name: &payload.tool_name,
    });
    // AND THE CONTIGUOUS TAIL BELOW BELONGS TO THE FIRST DELIVERY. A nudge or
    // an observation returns here, so it writes no journal entry, no
    // activity-ring line, never claims the return moment through
    // `mark_present`, never triggers `replay_missed` and never pulses.
    //
    // EACH IS A DEFECT AVOIDED RATHER THAN TIDINESS. The recap counts
    // activity-ring lines toward `min_events`, so a nudge or an observation
    // that rang would inflate the operator's own recap with pns's noise;
    // neither is evidence of presence, so neither must move the last-present
    // marker; and the pulse falling out here is how "escalation is not a
    // colour" stays enforced without touching the lights at all.
    //
    // A SUPPRESSED NUDGE IS THEREFORE LOST, deliberately, and AN OBSERVATION
    // NEVER RENEWS A LEASE OR ARMS A LAMP, for the same reason from the other
    // side: it is not an occurrence to replay later. Muted, inside a named
    // Focus, or planned to nothing means the nudge does not happen and is not
    // journaled for replay: a "still waiting" card replayed hours later, about
    // a question answered long ago, is worse than silence.
    if attempt != Attempt::First {
        return;
    }
    // THE JOURNAL GOES WITH IT, inheriting the ordering contract stated above
    // rather than restating it: same site, same accepted price, and both
    // branches reach it, including the empty-plan branch, which is where most
    // misses live.
    record_missed(event, &decision, &overrides);
    // AND THE LAMPS' NEEDS MARKER BESIDE IT, under the same ordering contract
    // and the same fail-quiet rule: a marker that did not land costs one lamp
    // its colour and never a card.
    // THE LAMPS ARE LIVE ONLY WITH BOTH SWITCHES: a map, and the transport
    // enabled. `[lights]` is policy and `[plugins.hue]` is how it reaches a
    // bulb, so a table with hue switched off lights nothing, runs no tick, and
    // must not accumulate markers nothing will ever sweep.
    let lamps_live = lights.is_some() && hue_table.is_some();
    update_blocked_marker(
        &state_dir(),
        &payload.session_id,
        &event.state,
        lamps_live,
        decision.inputs.now_secs,
    );
    // AND THE NEWS RECORD BESIDE IT, under the same ordering contract and the
    // same fail-quiet rule. It is what arms the unread lamp, and it is written
    // WHATEVER THE DELIVERY DID: a card that was suppressed, muted or dropped is
    // exactly the news that lamp exists to carry.
    //
    // THE PULSE'S OWN MAPPING decides what counts, so the colour a lamp flashes
    // and the record that arms the unread lamp cannot disagree about one event.
    //
    // AND IT IS NOT GATED ON THE LAMP SWITCHES EITHER, which is the difference
    // between this record and the wait marker beside it. A marker is a file per
    // session that only the tick ever sweeps, so a machine with no lamps must
    // not start accumulating them; this is ONE line rewritten in place, it can
    // never grow, and what it holds is the plain fact that a turn finished or
    // died. Written only while a map and a transport were both live, an
    // operator who switched hue off for an evening came back to a lamp with
    // nothing to say about the evening.
    record_news(
        &state_dir(),
        pns::pulse::state_behaviour(&event.state, true),
        decision.inputs.now_secs,
    );
    // AND THE LOOP LEASE THIS PANE HOLDS, if it holds one. The renewal is the
    // pane's own ordinary traffic, which is what makes the lease a liveness
    // signal rather than a timer. It CREATES nothing, so a machine with no lamps
    // pays one failed open and keeps no state.
    renew_loop_lease(&state_dir(), &event.pane, decision.inputs.now_secs);
    // AND THE ACTIVITY RING WITH IT, at the same site and under the same
    // ordering contract and the same fail-quiet rule. It records
    // UNCONDITIONALLY, which is the whole difference between it and the
    // journal above: the recap's window is every event, delivered or not.
    record_activity(event, &decision);

    // THE CATCH-UP GOES AFTER BOTH RECORDS AND BEFORE THE PULSE, inheriting
    // the ordering contract stated above rather than restating it: a slow
    // replay must not cost either record, and a card the operator may be
    // waiting on outranks decoration.
    replay_missed(recap, &decision, &home, &mobile, hermes_key, durable_route);
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
    //
    // THE LAMPS HAVE A SECOND GATE, beside the plan's rather than inside it.
    // `plan.pulse` is `long_running` and it is what the decision log records;
    // widening it would change what every card, banner and log line says about
    // an event that earned no card. The blocked lamp is not a delivery, it is
    // a colour on a bulb, so it earns its own condition here: an agent waiting
    // on the operator lights blue whether or not it ran long.
    //
    // IT NEEDS A `[lights]` TABLE, which is the opt-in, and the opt-in is read
    // off the BEHAVIOUR rather than tested a second time here: `state_behaviour`
    // only answers blocked for a mapped machine, so the colour a lamp shows
    // and the gate that lets it fire cannot come out disagreeing about one
    // event. Without the map there is no blue to show, and a long-running
    // blocked turn keeps the green it has flashed since the bash.
    //
    // AND IT RESPECTS THE SILENCE, through the same predicate arbitration uses
    // rather than a second copy of it: a muted operator gets no lamp, which is
    // the shipped rule that the lights are decoration too.
    //
    // THIS FLASH IS NOT WHAT HOLDS THE LAMP BLUE. `pulse_render` answers
    // `None` for every held behaviour, Blocked included, so this call fires
    // once, at the moment the wait begins, and does nothing after. The
    // TICK lights it off the marker `update_blocked_marker` just published,
    // on its next successful run, scheduled `refresh_secs` after the last
    // one; a stopped daemon lights nothing. That reading takes `pns lights
    // quiet` and each room's own dim window, and never this event's own
    // silence or a macOS Focus: those gate the flash and the cards, not the
    // sustained breath.
    let behaviour = pns::pulse::state_behaviour(&event.state, lights.is_some());
    let blocked_lamp = behaviour == pns::config::Behaviour::Blocked && !overrides.silenced();
    if decision.plan.pulse || blocked_lamp {
        fire_pulse_unless_quiet(hue_table.clone(), lights.as_deref(), behaviour);
    }
    // AND THE OPERATOR'S RETURN PUTS OUT WHATEVER A GLOW IS STILL HOLDING.
    // The steady write is the one body on this path that does not expire, so
    // something has to put it out, and this is where the condition behind it
    // stops being true: `is_present` is the same predicate that advances the
    // return edge the glow is derived from, so the lamp and the marker cannot
    // disagree about whether the operator came back.
    //
    // NO DAEMON IS INVOLVED, which is half of what pays for the steady write.
    // The held paths were recorded when they were written, so this is one PUT
    // each with no listing to resolve, and it works on a machine where the
    // tick has not run for hours.
    if lamps_live && pns::missed_notifications::is_present(&decision) {
        clear_held_lamps(hue_table.as_ref());
    }
    // AND THE TICK'S LEASE IS REFRESHED LAST, by every event, which is what
    // makes a stalled loop go dark for free: nothing renews its own lease, so
    // a machine that stopped producing events stops re-arming its lamps.
    if lamps_live {
        register_lights_tick(lights.as_deref(), &decision, &overrides);
    }
}

/// Put out whatever a steady glow write is still holding, and forget it.
///
/// THE FILE IS THE FENCE. An ordinary event reads whether it exists and stops
/// there, so every event that is not a return from an absence costs one failed
/// open and no network at all.
///
/// IT FORGETS EVEN THOUGH THE WRITE MIGHT HAVE FAILED, and the cost is stated
/// rather than coded around: `put` is fire and forget, so a refused clear is
/// invisible and the lamp stays lit with nothing recorded to put it out. That
/// is the same exposure the steady write already carries by not expiring, and
/// the alternative is worse: a record kept until somebody proved the write
/// landed would have every later event re-clearing a lamp that is already
/// dark, forever, on a machine whose daemon is down.
fn clear_held_lamps(settings: Option<&toml::Table>) {
    let state = state_dir();
    // A RECORD THIS CANNOT READ NAMES NO LAMP TO PUT OUT, and it is KEPT: the
    // clear works off names alone, so there is nothing to write, and forgetting
    // the file would take the tick's only chance of repairing it with it.
    let Some(held) = held_lamps(&state) else {
        return;
    };
    if held.is_empty() {
        return;
    }
    let Some(hue) = settings.and_then(|settings| {
        hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())
    }) else {
        return;
    };
    pns::channels::hue::clear_held(
        &UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
            deadline: BRIDGE_DEADLINE,
        },
        &held,
    );
    // The failure is DROPPED here, in this function's own stated style: the
    // PUTs are already out, so the worst a failed forget costs is one more
    // clear of a lamp that is already dark.
    let _ = remember_held(&state, &[]);
}

/// Register the repeating tick, or drop the refusal.
///
/// THE FAILURE IS DROPPED, exactly as `record_decision`'s is and for the same
/// reason: a lamp that did not re-arm must never cost a card, a line of stdout
/// or an exit code. `daemon::schedule` returns its error rather than printing
/// it precisely so each caller can state its own direction, and this one drops
/// it.
///
/// IT CANNOT BLOCK. The registration is one file written by rename into a
/// directory; there is no connection, no handshake and nothing to wait on, so
/// a daemon that is dead, wedged or mid-restart changes nothing about this
/// call.
///
/// TWO LEASE LENGTHS, off ONE question: was this event journalled. An ordinary
/// event means the operator is here and a working loop emits events
/// constantly, so five minutes covers an agent's thinking gap without covering
/// a stall. A journalled one means they are away or muted, which is exactly
/// when no further event will arrive to refresh this, and the glow has to
/// survive the whole absence.
///
/// THE DUE SECOND IS KEPT WHEN ONE IS ALREADY PENDING, and that is not
/// decoration: re-registering replaces the job by name, so an event storm that
/// pushed `due` out to `now + refresh` every time would keep moving the tick
/// away from itself and a busy machine's lamps would never be re-armed at all.
/// The lease is what every event refreshes; the schedule is left where the
/// last tick put it.
fn register_lights_tick(
    lights: Option<&pns::config::Lights>,
    decision: &pns::engine::Decision,
    overrides: &Overrides,
) {
    // THE DECISION'S OWN CLOCK, like record_news and renew_loop_lease beside
    // this call: a fresh wall-clock read here would be a second reading of the
    // same moment, which is exactly the boundary R4-1 exists to close. NO
    // CLOCK IS NO REGISTRATION, never a job due at epoch zero.
    let (Some(lights), Some(now)) = (lights, decision.inputs.now_secs) else {
        return;
    };
    let lease = if pns::missed_notifications::was_missed(decision, overrides) {
        JOURNALLED_LEASE_SECS
    } else {
        ORDINARY_LEASE_SECS
    };
    schedule_lights_tick(&state_dir(), lights, now, lease);
}

/// The tick registered to run for the next `lease_secs`, keeping whatever due
/// second is already pending.
///
/// THREE CALLERS AND ONE REGISTRATION, because the tick's lease is what decides
/// whether a lamp can EVER light, and three spellings of it would be three
/// answers. An event refreshes it, a lease taken by hand starts it, and the
/// tick renews its own while anything is still in flight.
///
/// THE DUE SECOND IS KEPT WHEN ONE IS ALREADY PENDING, and that is not
/// decoration: re-registering replaces the job by name, so an event storm that
/// pushed `due` out to `now + refresh` every time would keep moving the tick
/// away from itself and a busy machine's lamps would never be re-armed at all.
/// The lease is what every caller refreshes; the schedule is left where the
/// last tick put it.
fn schedule_lights_tick(state: &Path, lights: &pns::config::Lights, now: u64, lease_secs: u64) {
    let pending =
        match pns::daemon::peek(&pns::daemon::spool_dir(state).join(LIGHTS_JOB), LIGHTS_JOB) {
            pns::daemon::Peeked::Job(job) => Some(job.due),
            _ => None,
        };
    let due = pending
        .filter(|due| *due > now)
        .unwrap_or_else(|| now.saturating_add(lights.refresh_secs));
    let job = pns::daemon::Job {
        id: LIGHTS_JOB.to_string(),
        due,
        // AT LEAST AS FAR AS THE DUE SECOND, because a lease that ended before
        // its own job's first run is a record `validate_shape` refuses, and a
        // refused registration is a lamp that never re-arms with nothing said
        // anywhere. It bites for any refresh interval longer than the ordinary
        // lease, which the config permits up to a day.
        until: due.max(now.saturating_add(lease_secs)),
        every: Some(lights.refresh_secs),
        unless_marker: None,
        args: vec!["lights".to_string(), "tick".to_string()],
    };
    // The failure is DROPPED here and nowhere else: see the doc comment.
    let _ = pns::daemon::schedule(state, &job, now);
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
    mobile: &Mobile,
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
    let moshi = moshi_channel(mobile.token.clone());
    let hermes = hermes_channel(hermes_key, hermes_url_for(&event.channel));

    // NO `?` AND NO EARLY RETURN: one channel's failure costs the others
    // nothing, and every channel above was constructed before the first
    // delivery, so a leg cannot be lost to a sibling's refusal.
    legs.iter()
        .map(|leg| {
            // THE MOBILE LEG IS GATED ON THE BACKEND VERDICT, ahead of the
            // dispatch that picks a seam and so ahead of BOTH of them. The
            // gate used to sit on the TOKEN, which only feeds the native
            // channel: with an executable channel of the same name installed,
            // the card went out under a backend nobody named while stderr
            // said "no card is pushed". A sentence that is printed has to be
            // true wherever the leg is dispatched.
            //
            // IT SITS HERE RATHER THAN IN `deliver_leg` because this is the
            // one site that dispatches any leg at all, so the two are the
            // same fence; a refused leg also runs nothing, so there is no
            // panic to catch below and nothing to unwind.
            if leg.name == "mobile"
                && let Some(reason) = mobile.refusal.as_deref()
            {
                return (*leg, Delivery::Failed(refused_backend_line(reason)));
            }
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
///
/// IT IS HANDED THE ARMED TABLE rather than the config, because every read of
/// `[plugins.mobile]` goes through one accessor: a toggle honoured under a
/// table whose backend was refused would be one setting of a refused table
/// still in force.
fn watch_card(settings: &toml::Table) -> bool {
    let Some(stated) = settings.get("mobile_watch_card") else {
        return false;
    };
    stated.as_bool().unwrap_or_else(|| {
        eprintln!(
            "pns: config error ([plugins.mobile] mobile_watch_card is {}, not a boolean); the mobile watching card stays off",
            stated.type_str()
        );
        false
    })
}

/// What reading `[plugins.mobile]` decided, carried whole rather than
/// collapsed into an absent token.
///
/// THE COMPLAINT TRAVELS WITH THE OUTCOME. A backend nobody answers and a
/// token nobody wrote are two different edits, and folding both into `None`
/// made the doctor name `token` for a fault that was `type`, on a machine
/// whose token was already correct.
#[derive(Default)]
struct Mobile {
    /// The push token, when the table is armed and states one. `None` is the
    /// not-set-up case, which the deliver seam names its own config key for.
    token: Option<String>,
    /// Why no card can be pushed: the table is switched on and names a backend
    /// nothing compiled in answers. The mobile leg fails with these words
    /// wherever it is dispatched.
    refusal: Option<String>,
    /// Whether a card fires while the operator is watching the pane.
    watch_card: bool,
}

/// The one read of `[plugins.mobile]`, and the one place its refusal reaches
/// stderr.
///
/// THE COMPLAINT IS PRINTED HERE because this is the composition root, which is
/// where every other returned warning becomes a line. ONCE, whatever the table
/// is going to be read for, because the table is read once: the token, the
/// toggle and the refusal come out of a single verdict instead of three
/// readers that each had to remember to ask the same question.
fn read_mobile(config: &pns::config::Config) -> Mobile {
    let settings = match pns::config::armed_mobile(config) {
        Ok(settings) => settings,
        Err(reason) => {
            eprintln!("pns: config error ({reason}); no card is pushed");
            return Mobile {
                refusal: Some(reason),
                ..Mobile::default()
            };
        }
    };
    let Some(settings) = settings else {
        return Mobile::default();
    };
    Mobile {
        token: moshi_secret(settings),
        refusal: None,
        watch_card: watch_card(settings),
    }
}

/// One line about a table the event path deliberately never refuses.
///
/// A DISABLED TABLE IS INERT (operator ruling 2026-08-31). Nothing at load and
/// nothing on the event path enforces the `type` under a switched-off table,
/// because a line about a channel the operator turned off, printed on every
/// event, is noise. It is still a misconfiguration waiting for the moment the
/// switch flips, so the DIAGNOSTIC says it, once, where diagnostics live and
/// where the operator is standing there reading.
///
/// ON STDERR, with the config complaints and not with the census: the doctor's
/// stdout is one line per registered plugin plus its summary, and this is
/// about a table rather than about a check. It moves no exit code, which is
/// the same rule the Focus and daemon lines keep: a switch nobody flipped is
/// not a broken notifier.
fn disabled_backend_warning(table: &str, only_type: &str) -> String {
    format!(
        "pns: [plugins.{table}] is switched off and names no backend this binary answers \
         (the only type is {only_type:?}); nothing refuses it until it is enabled"
    )
}

/// Every switched-off table whose `type` names no compiled-in backend, in the
/// order the roster registers them.
fn disabled_backend_warnings(config: &pns::config::Config) -> Vec<String> {
    let switched_off = |name: &str| {
        config
            .plugins
            .get(name)
            .filter(|entry| !entry.enabled)
            .map(|entry| &entry.settings)
    };
    let mut warnings = Vec::new();
    // THE TYPE ALONE on both tables. `router_settings` settles the type before
    // it reads anything else, which is why only its two type refusals count
    // here: a switched-off table naming a backend that DOES answer, with a
    // missing `router_url` under it, is a different edit and not this
    // warning's business.
    if switched_off("router").is_some_and(|settings| {
        matches!(
            pns::home::router_settings(settings),
            Err(pns::home::SetupFailure::NoType | pns::home::SetupFailure::UnknownType(_))
        )
    }) {
        warnings.push(disabled_backend_warning("router", pns::home::UNIFI_TYPE));
    }
    if switched_off("mobile").is_some_and(|settings| mobile_backend(settings).is_err()) {
        warnings.push(disabled_backend_warning("mobile", MOSHI_TYPE));
    }
    warnings
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
fn fire_pulse_unless_quiet(
    hue_table: Option<toml::Table>,
    lights: Option<&pns::config::Lights>,
    behaviour: pns::config::Behaviour,
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
) -> Vec<String> {
    let Some(hue) = hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()) else {
        return Vec::new();
    };
    let bridge = UreqBridge {
        base: format!("https://{}/clip/v2/resource", hue.bridge),
        key: hue.key,
        deadline: BRIDGE_DEADLINE,
    };
    run_pulse_writes(&bridge, lights, behaviour, reading, held)
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
    lights: &pns::config::Lights,
    behaviour: pns::config::Behaviour,
    reading: &pns::channels::hue::Reading<'_>,
    held: Option<&[String]>,
) -> Vec<String> {
    // A BRIDGE THAT ANSWERED NOTHING RESOLVES NOTHING, and says nothing here.
    // The doctor is where an unreachable bridge is reported; a warning on every
    // notification for the rest of a machine's life is noise.
    let Some(routing) = pns::channels::hue::resolve_on_bridge(bridge, lights) else {
        return Vec::new();
    };
    let complaints = routing_complaints(&routing);
    for routed in &routing.lamps {
        let path = pns::channels::hue::Fixture::Light(routed.lamp.id.clone()).path();
        let lamp_is_held = held.is_none_or(|held| held.contains(&path));
        if pns::channels::hue::muted_now(&routed.lamp, reading.muted)
            || !pns::lights::pulse_fires(&routed.shows, behaviour, lamp_is_held)
        {
            continue;
        }
        let showing =
            pns::channels::hue::dim_showing(routed.dim.as_ref(), behaviour, reading.minutes_now);
        if let Some((color, pulse, brightness)) =
            pns::channels::hue::pulse_render(behaviour, lights, showing)
        {
            bridge.put(
                &path,
                &pns::channels::hue::pulse_body(&pulse, color, brightness),
            );
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
            "mobile" => return moshi.deliver(rendered, leg.mode),
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
/// renames because a plain unlink does not arbitrate on APFS: measured, eight
/// racers were every one of them told they had succeeded.
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

/// One named lock taken, or false when somebody live already holds it.
///
/// THE SHAPE EVERY LOCK IN THIS BINARY USES, and it is one function because its
/// two halves are only correct together: an exclusive create arbitrates between
/// racers, and the age rule is what stops a holder that died from wedging the
/// path forever. What differs between callers is the NAME and how long a holder
/// is believed, so those are the parameters and the mechanism is not repeated.
fn claim_lock(lock: &Path, now: u64, stale_secs: u64) -> bool {
    if publish_lock(lock).is_ok() {
        return true;
    }
    // Somebody holds it. A live holder is one this process stands down for.
    if !lock_aged_out(lock, now, stale_secs) {
        return false;
    }
    // THE DEAD LOCK IS TAKEN BY RENAME AND NEVER BY REMOVE, which is the one
    // place arbitration is still needed on this path: a remove reports success
    // to EVERY racer on APFS (measured, eight racers all told they had
    // succeeded), so two processes clearing one dead lock would each then create
    // a fresh one and both would own the window. A rename does arbitrate.
    let claim = pns::nag::claim_path(lock, std::process::id());
    if std::fs::symlink_metadata(&claim).is_ok() {
        return false;
    }
    if std::fs::rename(lock, &claim).is_err() {
        return false;
    }
    let _ = std::fs::remove_file(&claim);
    publish_lock(lock).is_ok()
}

/// The lock published, or an error when somebody already holds it.
///
/// EXCLUSIVE, so of any number of processes racing this exactly one is told it
/// succeeded, and it NEVER FOLLOWS A LINK: an exclusive create fails on a
/// symlink at the path rather than opening what it points at.
fn publish_lock(lock: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(STATE_FILE_MODE)
        .open(lock)
        .map(|_| ())
}

/// Whether a lock already on disk is old enough to be the leavings of a crash.
///
/// A LOCK WHOSE OWN CLOCK CANNOT BE READ COUNTS AS LIVE and stands the caller
/// down. That is the safe direction (one window lost, never two holders), and
/// the case behind it is a lock that vanished between the failed create and the
/// question, which the next attempt resolves anyway.
fn lock_aged_out(lock: &Path, now: u64, stale_secs: u64) -> bool {
    std::fs::symlink_metadata(lock)
        .ok()
        .as_ref()
        .and_then(modified_at)
        .is_some_and(|at| now.saturating_sub(at.as_secs()) > stale_secs)
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
/// leaves the violet breathing for the whole timeout with nothing behind it,
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
/// does not arbitrate on this filesystem: it reports success to every caller, so
/// a sweep that read an expired epoch and then unlinked could delete a FRESH
/// marker a racing event had published in between, and both would believe they
/// had removed the old one. Taking the file by rename first means what this
/// removes is what this took, and the epoch is READ AGAIN off the claim: a
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
    let (Some(lights), Some(now)) = (config.lights.as_deref(), now_secs()) else {
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
    breath: pns::config::Breath,
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
        let Some(routing) = pns::channels::hue::resolve_on_bridge(bridge, lights) else {
            return complaints;
        };
        complaints.extend(routing_complaints(&routing));
        for routed in &routing.lamps {
            if pns::channels::hue::muted_now(&routed.lamp, reading.muted) {
                continue;
            }
            let Some(held) = pns::lights::shown(active, &routed.shows) else {
                continue;
            };
            let showing = pns::channels::hue::dim_showing(
                routed.dim.as_ref(),
                held.behaviour(),
                reading.minutes_now,
            );
            if showing == pns::channels::hue::Showing::Dark {
                continue;
            }
            let (color, breath) = pns::channels::hue::held_render(held, lights, showing);
            let path = pns::channels::hue::Fixture::Light(routed.lamp.id.clone()).path();
            // A LAMP NOT NAMED IN LAST TICK'S RECORD, OR NAMED THERE WITH NO
            // PHASE, RESUMES AT THE DEFAULT: a fresh arm, an external switch,
            // a killed child's bare token and a dim-window shape change all
            // read the same way, and all cost at most one fade of motion.
            let previous =
                held_before.and_then(|entries| entries.iter().find(|entry| entry.path == path));
            let resume = pns::lights::resume_from(previous, now_ms, held, &breath);
            breathing.push(Breathing {
                path,
                held,
                breath,
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
                    .map(|(path, end, end_relative_ms)| pns::lights::HeldEntry {
                        path: path.clone(),
                        resume: Some(pns::lights::Phase {
                            end_unix_ms: now_ms + spent_ms + end_relative_ms,
                            end: *end,
                            held: entry.held,
                        }),
                    })
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
/// reported at is exactly the end its own last ISSUED fade targeted.
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
) -> Vec<(String, pns::lights::End, u64)> {
    // (due millisecond, the lamp this fade belongs to, the end it moves toward,
    // body), in the order they are due.
    let mut schedule: Vec<(u64, &Breathing, pns::lights::End, String)> = Vec::new();
    for entry in breathing {
        let fades = pns::lights::breath_fades(budget_ms, &entry.breath, entry.resume);
        for (index, fade) in fades.iter().enumerate() {
            // THE FIRST FADE CARRIES THE COLOUR AND THE `on`, which is what arms
            // the lamp; every one after it states brightness and duration alone,
            // so the bridge has nothing else to reconcile mid-transition. THIS
            // HOLDS ON A RESUMED TICK TOO: an externally switched-off lamp comes
            // back on with its first fade whichever end the record names.
            let body = if index == 0 {
                pns::channels::hue::breath_arm_body(entry.color, fade, entry.breath.duration_ms)
            } else {
                pns::channels::hue::fade_body(fade, entry.breath.duration_ms)
            };
            let end = if fade.brightness == entry.breath.high {
                pns::lights::End::High
            } else {
                pns::lights::End::Low
            };
            schedule.push((fade.start_ms, entry, end, body));
        }
    }
    schedule.sort_by(|left, right| (left.0, &left.1.path).cmp(&(right.0, &right.1.path)));
    let mut landings: Vec<(String, pns::lights::End, u64)> = Vec::new();
    for (due_ms, entry, end, body) in schedule {
        // SATURATING, so a write that ran long simply issues the next fade at
        // once rather than sleeping a wrapped duration.
        let now_ms = elapsed_ms();
        if due_ms > now_ms {
            sleep(Duration::from_millis(due_ms - now_ms));
        }
        // READ AGAIN AFTER THE SLEEP, because the sleep is the one thing here
        // that is allowed to overshoot, and this is the moment the write starts.
        let at_ms = elapsed_ms();
        if at_ms >= budget_ms {
            break;
        }
        bridge.put(&entry.path, &body);
        let landing = (entry.path.clone(), end, at_ms + entry.breath.duration_ms);
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

/// A lock held for as long as this value is alive, and given back when it is
/// dropped. Shared by every `claim_lock` caller with more than one exit path
/// (the lights tick and a ring append today), not just the tick: a second
/// hand-written guard is how one of them ends up leaking its lock on a path
/// the other already covered.
///
/// A GUARD RATHER THAN A RELEASE AT EVERY EXIT: the lights tick stands down
/// from four places and a ring append from several early returns, and a lock
/// left behind stands every later claimant down for a whole stale window.
/// `Drop` is the one exit all of them share.
///
/// THE MESSAGE NAMES NEITHER CALLER, deliberately: it is printed by the type
/// both share, and naming one subsystem in it would misdescribe the other's
/// failure the day this is reused a third time.
struct HeldLock(std::path::PathBuf);

impl Drop for HeldLock {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.0) {
            eprintln!(
                "pns: the lock {} could not be given up ({error}); \
                 the next claimant waits it out",
                self.0.display()
            );
        }
    }
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
        if ticks.is_multiple_of(SWITCH_TICKS) && !daemon_enabled() {
            println!("pns daemon: disabled in the config; exiting");
            return 0;
        }
        daemon_pass(
            &spool,
            &state,
            now_secs(),
            tick,
            &mut children,
            &mut reported,
        );
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

#[cfg(test)]
mod tests {
    use super::{
        Bounded, Breathing, CONFIG_FILE_MODE, DEFAULT_REREAD_ATTEMPTS, DEFAULT_REREAD_INTERVAL,
        LIGHTS_HELD, LIGHTS_JOB, LIGHTS_NEWS, LIGHTS_SAID, LIGHTS_SHELL_DIR, LIGHTS_TICK_LOCK,
        MAX_REREAD_ATTEMPTS, MAX_REREAD_INTERVAL, STATE_FILE_MODE, ad_hoc_quiet, answered,
        asks_the_bridge, blocked_lamp, child_bound, daemon_pass, drive_breaths, end_lease,
        held_lamps, keep_aside, keep_aside_at, lights_report, list, matches_glob, means_yes,
        muted_state, publish_config, publish_state_line, read_failure, read_held, read_news,
        read_note, recap_bounds, record_news, remember_held, renew_loop_lease, republish_after,
        reread_attempts_from, reread_interval_from, resolve_path, router_backend, run_pulse_writes,
        run_tick_writes, say_lights_once, sweep_blocked, sweep_leases, sweep_legacy_state,
        sweep_markers, sweep_shell_markers, tick_bridge_deadline, update_blocked_marker,
    };
    use std::cell::RefCell;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::Duration;

    /// A scratch state directory of this test's own, named so two tests and two
    /// runs of one test never share a file.
    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "pns-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos())
        ));
        std::fs::create_dir_all(&directory).expect("the scratch directory");
        directory
    }

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
                end: pns::lights::End::High,
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

    /// The bridge the tick's writes are driven against: three listings,
    /// answered or not, and every PUT recorded IN ORDER.
    ///
    /// A SEQUENCE RATHER THAN A SET, because the order is the whole question
    /// here: an arm followed by an off is a lamp the tick put out after telling
    /// it to breathe, and a set cannot tell that from a lamp that was only ever
    /// armed.
    struct ScriptedBridge {
        listings: Option<()>,
        puts: RefCell<Vec<(String, String)>>,
    }

    impl pns::channels::hue::Bridge for ScriptedBridge {
        fn get(&self, path: &str) -> Option<String> {
            self.listings?;
            Some(
                match path {
                    "light" => ONE_LAMP,
                    "zone" => r#"{"data":[]}"#,
                    _ => ONE_ROOM,
                }
                .to_string(),
            )
        }
        fn put(&self, path: &str, body: &str) {
            self.puts
                .borrow_mut()
                .push((path.to_string(), body.to_string()));
        }
    }

    /// The clock a tick test hands the driver when it asserts on WHAT was
    /// written and to which lamp rather than on when: nothing takes any time,
    /// so every fade the schedule holds is issued and none is dropped at the
    /// budget. A test that asserts a PHASE hands over a `FakeClock` instead,
    /// because a phase is a moment and this clock has none.
    fn no_time_passes() -> impl FnMut() -> u64 {
        || 0
    }

    /// A monotonic clock and its sleeper over one cell. THE SLEEPER IS THE ONLY
    /// THING THAT ADVANCES IT, so a whole tick plays out at the milliseconds its
    /// own schedule names, with no wall clock in the test at all.
    #[derive(Default)]
    struct FakeClock(std::cell::Cell<u64>);

    impl FakeClock {
        fn elapsed_ms(&self) -> u64 {
            self.0.get()
        }

        fn slept(&self, waited: Duration) {
            self.0
                .set(self.0.get() + u64::try_from(waited.as_millis()).unwrap_or(0));
        }
    }

    /// A bridge whose calls cost the tick real time on the tick's own clock,
    /// which is what a slow LAN does to a synchronous schedule. The two costs
    /// are separate because they buy different failures: a slow resolve eats
    /// the budget before a single fade is issued, and a slow write pushes every
    /// later fade past the moment it was due.
    struct SlowBridge<'a> {
        clock: &'a FakeClock,
        get_cost_ms: u64,
        put_cost_ms: u64,
        answers: bool,
        puts: RefCell<Vec<(String, String)>>,
    }

    impl pns::channels::hue::Bridge for SlowBridge<'_> {
        fn get(&self, path: &str) -> Option<String> {
            self.clock.slept(Duration::from_millis(self.get_cost_ms));
            self.answers.then(|| {
                match path {
                    "light" => ONE_LAMP,
                    "zone" => r#"{"data":[]}"#,
                    _ => ONE_ROOM,
                }
                .to_string()
            })
        }
        fn put(&self, path: &str, body: &str) {
            self.puts
                .borrow_mut()
                .push((path.to_string(), body.to_string()));
            self.clock.slept(Duration::from_millis(self.put_cost_ms));
        }
    }

    fn scripted(answers: bool) -> ScriptedBridge {
        ScriptedBridge {
            listings: answers.then_some(()),
            puts: RefCell::new(Vec::new()),
        }
    }

    const ONE_ROOM: &str = r#"{"data":[
      {"id":"r1","type":"room","metadata":{"name":"3F - Studio"},
       "children":[{"rid":"dev-1","rtype":"device"}],
       "services":[{"rid":"g1","rtype":"grouped_light"}]}
    ]}"#;

    const ONE_LAMP: &str = r#"{"data":[
      {"id":"l1","type":"light","owner":{"rid":"dev-1","rtype":"device"},
       "metadata":{"name":"3F - Studio - HCL1"}}
    ]}"#;

    const LAMP_PATH: &str = "light/l1";
    const CLEAR_BODY: &str = r#"{"on":{"on":false}}"#;

    /// A room routed for every held state, which is the map these tick tests
    /// resolve through.
    ///
    /// THE SHORTEST LEGAL INTERVAL, deliberately, because it is the tightest
    /// budget a tick can be handed and these tests are about what a tick does
    /// with one. It tracks `MIN_REFRESH_SECS`, which became twelve on
    /// 2026-09-02 when the loop breath was slowed: ten no longer leaves a
    /// resumed six-second shape anywhere to put a fade.
    fn held_lights() -> pns::config::Lights {
        *pns::config::parse_config(
            "[lights]\nrefresh_secs = 12\n\
             [lights.room.\"3F - Studio\"]\nshows = [\"blocked\", \"unread\", \"loop\"]\n",
        )
        .expect("the test's own config parses")
        .lights
        .expect("and carries a lights table")
    }

    /// The clock and the mutes a tick that is testing something else is judged
    /// against: noon, and nothing muted.
    fn noon(muted: &pns::channels::hue::Muting) -> pns::channels::hue::Reading<'_> {
        pns::channels::hue::Reading {
            minutes_now: Some(12 * 60),
            muted,
        }
    }

    /// The ordinary mute: a machine that has never typed the command.
    fn nothing_muted() -> pns::channels::hue::Muting {
        pns::channels::hue::Muting::Places(Vec::new())
    }

    /// One place the operator quieted by hand.
    fn quieted(place: &str) -> pns::channels::hue::Muting {
        pns::channels::hue::Muting::Places(vec![place.to_string()])
    }

    /// What the held record says right now.
    fn recorded(state: &std::path::Path) -> Option<String> {
        std::fs::read_to_string(state.join(LIGHTS_HELD))
            .ok()
            .map(|line| line.trim().to_string())
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
                    end: pns::lights::End::High,
                    held: pns::lights::Held::Blocked,
                }),
            }]),
            0,
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
        // EVERY LOCKED SHAPE AND NOT JUST THE FASTEST ONE, because the fade
        // ceiling is only safe if the SLOWEST shape it admits still breathes:
        // an empty schedule is a lamp that stops moving, and a loop lamp that
        // stops moving looks exactly like the daemon dying, which is the one
        // thing that lamp exists to say. Held on the fastest shape alone, this
        // passes a driver that has gone back to refusing any fade whose whole
        // DURATION will not fit, which is what it did before the seamless
        // turn-around and which the slow shapes are the ones to catch.
        let shipped = pns::config::Lights::default();
        for refresh_secs in [10, 12, 20, 30] {
            let three = tick_bridge_deadline(refresh_secs).as_millis() * 3;
            let interval = u128::from(refresh_secs) * 1000;
            assert!(
                three < interval,
                "refresh {refresh_secs}s: three calls at {three}ms do not fit"
            );
            let left = u64::try_from(interval - three).expect("a budget in milliseconds");
            for (named, breath) in [
                ("blocked", shipped.blocked.breath),
                ("unread", shipped.unread.breath),
                ("loop", shipped.looping.breath),
                ("dim", shipped.dim),
            ] {
                assert!(
                    !pns::lights::breath_fades(left, &breath, pns::lights::Resume::default())
                        .is_empty(),
                    "refresh {refresh_secs}s: the {left}ms left over will not hold a fade \
                     of the locked {named} shape ({}ms)",
                    breath.duration_ms
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
        // TWO SHAPES THIS TEST OWNS, DELIBERATELY NOT THE LOCKED DEFAULTS. The
        // interleave asserted below is the exact due-order these two durations
        // produce, so reading either from `Lights::default()` would rewrite the
        // expected order every time a cadence is retuned and this test would
        // start failing for a reason it is not about. The 4000 here is NOT the
        // loop default (that is 6000): leave it alone when a cadence change
        // sends you grepping for 4000.
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
                    breath: quick,
                    color: pns::pulse::BLOCKED_COLOR,
                    resume: pns::lights::Resume::default(),
                },
                Breathing {
                    path: "light/b".to_string(),
                    held: pns::lights::Held::Looping,
                    breath: slow,
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
                breath: pns::config::Breath {
                    duration_ms: 2_000,
                    high: 100,
                    low: 30,
                },
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
            vec![("light/a".to_string(), pns::lights::End::High, 11_000)],
            "the last write really happened at 9,000ms and its fade runs 2,000ms \
             from there, so the next tick resumes off 11,000ms rather than off the \
             13,700ms the nominal schedule would have claimed"
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
                    end: pns::lights::End::High,
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
                    end: pns::lights::End::Low,
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
        // before its first blue body reached the lamp, because the first fade of
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
            || clock.elapsed_ms(),
            |waited| clock.slept(waited),
        );
        assert!(complaints.is_empty(), "{complaints:?}");
        let held_after_the_loop = read_held(&state).expect("a record this tick wrote");

        // TICK TWO holds BLOCKED instead. Resumed off the loop's phase it would
        // sleep 3,400ms before its first blue body; it starts down at once
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
            "the first blue fade is issued before anything is slept for, so the \
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
            "[lights]\nrefresh_secs = 12\n\
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
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            Some(&[]),
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
            &lights,
            pns::config::Behaviour::Done,
            &noon(&quieted("3F - Studio")),
            Some(&[]),
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
            &lights,
            pns::config::Behaviour::Done,
            &noon(&pns::channels::hue::Muting::Everything),
            Some(&[]),
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
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            Some(&[LAMP_PATH.to_string()]),
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
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            held_lamps(&state).as_deref(),
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
            &lights,
            pns::config::Behaviour::Done,
            &noon(&nothing_muted()),
            None,
        );
        assert!(
            unreadable.puts.borrow().is_empty(),
            "a held record nobody could read let the pulse fire anyway: {:?}",
            unreadable.puts.borrow()
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
                &lights,
                pns::config::Behaviour::Done,
                &noon(&nothing_muted()),
                Some(&[]),
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

    #[test]
    fn a_wait_that_ended_loses_its_marker_whether_or_not_the_lamps_are_live() {
        // REMOVAL IS CHEAP AND CREATION IS NOT, which is why one gate cannot
        // serve both. Gating the whole update on the feature switches stopped
        // the marker being CLEARED as well: a wait that ended while hue was off
        // stayed on disk, and re-enabling hue inside the backstop bound
        // put blocked back on a lamp for a session nobody is waiting on.
        let state = scratch("needs-marker-end-ungated");
        let marker = pns::lights::blocked_marker(&state, "s1").expect("a usable session id");
        std::fs::create_dir_all(marker.parent().expect("the needs directory"))
            .expect("the needs directory");
        std::fs::write(&marker, "1000\n").expect("a wait in progress");

        update_blocked_marker(&state, "s1", "done", false, Some(1_000));
        assert!(
            !marker.exists(),
            "the wait ended, so the marker goes, lamps live or not: it is one \
             unlink and it clears a leftover from when they were"
        );

        update_blocked_marker(&state, "s1", "blocked", false, Some(1_000));
        assert!(
            !marker.exists(),
            "but STARTING one stays gated: a machine that never asked for the \
             lamps must not accumulate files that nothing will ever sweep"
        );

        update_blocked_marker(&state, "s1", "blocked", true, Some(1_000));
        assert!(
            marker.exists(),
            "and a machine with them live starts the wait, which is what makes \
             the two assertions above a difference rather than a dead path"
        );
        assert_eq!(
            std::fs::read_to_string(&marker).expect("the marker"),
            "1000\n",
            "the marker holds the DECISION's clock, not a fresh wall-clock read \
             taken inside this function"
        );

        // NO CLOCK IS NO MARKER: an unreadable clock must not default to
        // epoch zero, which would write a marker that reads as already
        // expired the moment it lands, or that never ages out at all read
        // the other way. SEEDED, not absent: a `None` case starting with no
        // marker on disk cannot tell "correctly wrote nothing" apart from a
        // `None => remove_file(marker)` mutant, since removing a file that
        // was never there is itself a silent no-op.
        let unreadable_clock_marker =
            pns::lights::blocked_marker(&state, "s2").expect("a usable session id");
        std::fs::create_dir_all(
            unreadable_clock_marker
                .parent()
                .expect("the needs directory"),
        )
        .expect("the needs directory");
        std::fs::write(&unreadable_clock_marker, "999\n").expect("a wait already in progress");
        update_blocked_marker(&state, "s2", "blocked", true, None);
        assert_eq!(
            std::fs::read_to_string(&unreadable_clock_marker).expect("the marker"),
            "999\n",
            "an unreadable clock must touch no marker at all, neither writing \
             one at epoch zero nor removing the one already there"
        );
    }

    /// A process id nothing is using: a child run to completion and reaped, so
    /// the kernel has already answered for it. STATED BY THE MACHINE rather
    /// than guessed at, because a made-up number can be live.
    fn a_reaped_pid() -> u32 {
        let mut child = std::process::Command::new("/usr/bin/true")
            .spawn()
            .expect("a child");
        let gone = child.id();
        child.wait().expect("the child is waitable");
        gone
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
