use crate::*;

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
pub(crate) fn hook_mode(event: &str) -> i32 {
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
        // stdout), so without this the lamp stayed blocked until the turn's Stop,
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
