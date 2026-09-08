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
    pns_application::RequestApproval {
        ports: &MoshiApprovalForwarder,
    }
    .forward_only(subcommand, &payload)
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
    // WHICH AGENTS FORWARD AT ALL, whether the payload is whole and whether
    // the operator is reachable are the composition root's three reads; the
    // ORDER of what follows is the use case's.
    let subcommand = moshi_subcommand(agent)
        .filter(|_| payload_is_whole(payload_json))
        .filter(|_| forward_to_moshi(&probes));
    let approval = MoshiRaiseNotification {
        probes: &probes,
        payload,
    };
    pns_application::RequestApproval { ports: &approval }.run(
        &event,
        &payload.session_id,
        subcommand.as_deref(),
        payload_json,
    )
}
/// THE COMPOSITION ROOT'S SIDE OF ONE APPROVAL: the spawn, the wait, the
/// environment variable and the notification, each behind the port the use
/// case orders them through.
///
/// THE ASSOCIATED HANDLE IS THE OWNED CHILD. Application code transfers it
/// without naming a process type; completion consumes it once at this adapter.
struct MoshiRaiseNotification<'a> {
    probes: &'a SystemProbes<SystemCommandRunner>,
    payload: &'a HookPayload,
}

impl pns_application::ApprovalForwarder for MoshiRaiseNotification<'_> {
    type Forwarded = std::process::Child;

    fn forward(&self, subcommand: &str, payload_json: &str) -> Option<Self::Forwarded> {
        pns_application::ApprovalForwarder::forward(
            &MoshiApprovalForwarder,
            subcommand,
            payload_json,
        )
    }

    fn answer(&self, child: Self::Forwarded) -> i32 {
        pns_application::ApprovalForwarder::answer(&MoshiApprovalForwarder, child)
    }
}

impl pns_application::PhoneSuppression for MoshiRaiseNotification<'_> {
    fn suppress(&self) {
        // SAFETY: the surface read joined every probe it started. Remaining
        // command-pipe workers and the moshi payload writer use only captured
        // pipes and owned buffers; they read no environment and spawn nothing.
        // Notification channel threads have not started yet.
        unsafe { std::env::set_var("PNS_SKIP_PHONE", "1") };
    }
}

impl pns_application::NagSchedule for MoshiRaiseNotification<'_> {
    fn arm(&self, session_id: &str, event: &pns::args::EventArgs) {
        arm_nag(session_id, event);
    }
}

impl pns_application::RaiseNotification for MoshiRaiseNotification<'_> {
    fn raise(&self, event: &pns::args::EventArgs) {
        run_event(event, self.probes, self.payload, Attempt::First);
    }
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
