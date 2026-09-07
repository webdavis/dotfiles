use crate::*;

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
pub(crate) fn model_switch_detail(from_model: &str, to_model: &str) -> Option<String> {
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
pub(crate) fn config_change_detail(source: &str, file_path: &str) -> Option<String> {
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
pub(crate) fn record_policy_settings_change(session_id: &str, file_path: &str, now: Option<u64>) {
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
