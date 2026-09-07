use crate::*;

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
pub(crate) fn gate_mode(subcommand: &str) -> i32 {
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
pub(crate) fn blocking_event(payload: &HookPayload, agent: &str, payload_json: &str) -> i32 {
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
pub(crate) fn moshi_hook_bin() -> String {
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
