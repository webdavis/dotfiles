use crate::*;

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
pub(crate) fn blocking_event(
    payload: &HookPayload,
    agent: &str,
    payload_json: &str,
    reminder: Reminder,
) -> i32 {
    let event = pns_domain::EventArgs {
        agent: agent.to_string(),
        state: "blocked".to_string(),
        detail: payload.message.clone(),
        pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
        ..attribution(payload, agent)
    };
    // THE ROW BEFORE THE FORWARD, because a payload handed to moshi never
    // reaches `raise` and the wait still happened.
    crate::activity::record(&event, payload);
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
        reminder,
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
    /// This call's reminder, already resolved from its own `--remind` switch
    /// and the producer's config entry. A zero delay arms nothing.
    reminder: Reminder,
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

impl pns_application::RemindSchedule for MoshiRaiseNotification<'_> {
    fn arm(&self, session_id: &str, event: &pns_domain::EventArgs) {
        arm_remind(session_id, event, self.reminder);
    }
}

impl pns_application::RaiseNotification for MoshiRaiseNotification<'_> {
    fn raise(&self, event: &pns_domain::EventArgs) {
        run_event(event, self.probes, self.payload, Attempt::First);
    }
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
    // THE SAME CLOCK THE DELIVERY PLAN READS BELOW, off this probe set's own
    // memoized cell rather than a fresh wall-clock read: see R4-1. Two reads
    // of the wall clock for one event is the boundary that drifted a phone
    // reading and a desk reading apart.
    pns_application::operator_surface(probes, &overrides_from_env(), probes.now_secs())
        != pns_domain::surface::Surface::Desk
}
