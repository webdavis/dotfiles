use crate::*;

/// Text safe to render or store, ON TOP OF `flattened`: whitespace and
/// control characters collapsed as `flattened` already does, and Unicode
/// format characters (`recap::is_invisible`) stripped besides.
///
/// STRIPS `recap::is_invisible` ON TOP OF `flattened`, never inside it:
/// `flattened` is shared by every other rendered field on this path, and
/// `model_switch_detail` has a reason a format character must not survive at
/// all rather than merely render inertly. It compares two names for equality,
/// which a reordering character could defeat silently (a name that reads the
/// same but compares unequal, or the reverse). Widening `flattened` itself for
/// one caller would let every other field silently start allowing format
/// characters through too.
fn rendered_plainly(text: &str) -> String {
    flattened(text)
        .chars()
        .filter(|character| !pns_domain::recap::sanitize::is_invisible(*character))
        .collect()
}
/// The automatic model-switch card's detail, or `None` when there is no
/// transition worth one: either name empty once flattened and stripped of
/// invisible characters, or the two equal once stripped.
pub(crate) fn model_switch_detail(from_model: &str, to_model: &str) -> Option<String> {
    let from = rendered_plainly(from_model);
    let to = rendered_plainly(to_model);
    if from.is_empty() || to.is_empty() || from == to {
        return None;
    }
    Some(format!("automatic session model change: {from} to {to}"))
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
pub(crate) fn quota_observation_detail(notification_type: &str, message: &str) -> Option<String> {
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
/// by the same definition every other blocked lamp here uses. So this calls the
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
/// a blocked lamp for a session that is working again, held until that turn's own
/// Stop. Ordering cannot CLOSE that race, which is the harness's to close, but
/// it shrinks the window from a plan of network legs to one file write.
///
/// KEYED BY SESSION, like every other wait: `blocked_marker_action("blocked")`
/// is `Action::Start` (it is one of `pulse::LAMP_BLOCKED`), so this reuses the
/// exact mechanism `blocking_event` uses rather than inventing a second one.
pub(crate) fn arm_quota_stale_wait(session_id: &str, probes: &SystemProbes<SystemCommandRunner>) {
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
/// The notification type every approval dialog arrives under, tool approvals
/// included: the dialog host defaults a typeless registry entry to it, and the
/// `Notification` matcher matches the type, so the declaration's matcher can
/// narrow this arm no further than every permission prompt on the machine.
const PERMISSION_PROMPT: &str = "permission_prompt";
/// The sandbox network dialog's own notification text, EXACTLY, measured
/// against Claude Code 2.1.272. It is the only thing that separates this
/// dialog from a tool approval `PermissionRequest` has already reported, so a
/// wording change upstream turns the alert OFF rather than misfiring it. The
/// literal is duplicated in `tests/hooks/sandbox_network.rs`, which pins the
/// same string rather than reading the bundle, so an upstream wording change
/// is caught by neither side and must be re-measured by hand on upgrade.
const SANDBOX_NETWORK_MESSAGE: &str = "A sandboxed command needs network access";
/// A sandbox network approval card's detail, or `None` for every other
/// permission prompt, in `quota_observation_detail`'s own style.
///
/// THE ALERT IS APPROXIMATE, AND THAT IS THE PLATFORM. `SandboxNetworkPrompts`
/// calls the dialog host directly with the host and port it wants, but the
/// registry text it raises is static and the hook payload carries neither, so
/// the card says what the harness said and stops there. Anything more specific
/// would be invented.
pub(crate) fn sandbox_network_detail(notification_type: &str, message: &str) -> Option<String> {
    (notification_type == PERMISSION_PROMPT && message == SANDBOX_NETWORK_MESSAGE)
        .then(|| SANDBOX_NETWORK_MESSAGE.to_string())
}
