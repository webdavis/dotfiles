use std::time::Duration;

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
pub(super) fn answer_within(child: std::process::Child, deadline: Duration) -> i32 {
    let started = std::time::Instant::now();
    answer_within_on(child, deadline, || started.elapsed(), std::thread::sleep)
}

/// `answer_within` with its clock and its sleeper as parameters, for the
/// reason `drive_breaths` takes both: the wait fills its whole deadline BY
/// DESIGN, so a test that read the real clock and slept for real would live
/// the deadline too, and would be measuring the machine rather than the bound.
/// `elapsed` is how long the wait has been running; a fake that advances only
/// inside `sleep` makes the moment of the give-up exact.
fn answer_within_on(
    mut child: std::process::Child,
    deadline: Duration,
    mut elapsed: impl FnMut() -> Duration,
    mut sleep: impl FnMut(Duration),
) -> i32 {
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
        if elapsed() >= deadline {
            let _ = child.kill();
            // REAPED, not merely signalled: an unreaped child is a zombie
            // holding its slot until pns exits, and the wait is instant on a
            // process already killed.
            let _ = child.wait();
            // SAID OUT LOUD, AND THE BOUND IS IN THE SENTENCE. An expiry used
            // to be silent, so an operator whose daemon was wedged saw a
            // prompt appear late and was told nothing about why. The number is
            // the deadline THIS wait actually honoured, which is what makes
            // the sentence answer "how long did it wait, and was that the
            // bound I configured": a build that ignored the configured value
            // and used one of its own would say so here.
            //
            // IT NAMES THE SUBMISSION, NOT THE PHONE. What ran out here is the
            // local daemon acknowledging `moshi-hook`'s registration, the wait
            // the doc comment above describes; the operator's own answer never
            // travels this path, so a sentence about the phone not answering
            // would blame the wrong party.
            //
            // NO FREE TEXT, in the decision ring's spirit: one integer and
            // fixed words, so nothing of the operator's own content reaches a
            // channel the harness may surface.
            eprintln!(
                "pns: the moshi submission did not finish within {}ms; the prompt was released",
                deadline.as_millis()
            );
            return 0;
        }
        sleep(SUBMISSION_POLL_INTERVAL);
    }
}

/// How often that wait looks. Ten milliseconds is `run_bounded`'s own tick:
/// short enough to add no latency an operator could notice on a submission
/// answered in roughly 150, long enough not to spin a core.
const SUBMISSION_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[cfg(test)]
mod tests;
