use crate::*;

/// A harness event, from the payload on stdin.
///
/// THE REMINDER IS SWITCHED ON BY THE CALL. `--remind`, `--remind=<duration>`
/// and `--no-remind` sit after the event word, and a switch this binary cannot
/// honour is REFUSED with exit 2 before anything is delivered: it is argv the
/// caller typed wrong, which is the one thing this path treats as an error.
/// The two arming arms resolve it and nothing else pays for the read, which
/// keeps `resolved` to a payload read, a parse and two file operations.
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
    let agent = std::env::var("PNS_PRODUCER").unwrap_or_else(|_| "claude".to_string());

    match event {
        // AND THE WAIT ENDS HERE TOO, beside the turn marker. A prompt is the
        // operator typing, which answers ANY live wait their session could be
        // holding: `resolved`'s PostToolBatch signal never fires for a
        // PermissionRequest (Claude Code decides that off this hook's own
        // stdout), so without this the lamp stayed blocked until the turn's Stop,
        // one whole tool call after the operator had already answered.
        // AND THE SESSION IS NAMED HERE, once: the first prompt of a session
        // is what the header's second line carries for every event after it,
        // and a later prompt does not relabel the ones before it. It goes
        // LAST of the three, because the two above are the marker writes an
        // operator is waiting on and this one is a row nobody reads until
        // the session's next event.
        "prompt" => {
            start_of_turn(&payload);
            end_blocked_wait(&payload.session_id, now_secs());
            name_session(&payload, &agent);
        }
        "stop" => end_of_turn(&payload, &agent),
        "stop-failure" => failed_turn(&payload, &agent),
        "blocked" => {
            return match remind_after(&agent) {
                Ok(after_secs) => blocking_event(&payload, &agent, &payload_json, after_secs),
                Err(code) => code,
            };
        }
        // EVERY CLEARING SIGNAL THE HARNESS HAS, on one arm. Five
        // declarations reach it: `PostToolBatch`, whichever way the operator
        // answered (a denial still produces a `tool_result` and so still
        // resolves the batch); `PostToolUse` for `AskUserQuestion` and for
        // `ExitPlanMode`, where the tool IS the dialog and returns at the
        // answer; `ElicitationResult`, which is an elicitation's own answer
        // signal; and `SubagentStop`. NONE OF THEM CARDS THE OPERATOR,
        // because none of them is news: the answer is the thing they just
        // gave.
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
        // `SubagentStop` IS THE ONE EXCEPTION, and it needs one: it carries
        // `agent_id` as well (measured in the 2.1.272 bundle), and it reports
        // the subagent that was holding the wait FINISHING, which is what
        // bounds a subagent's wait at its own end rather than at the parent's
        // Stop. RESIDUAL, STATED HONESTLY: a subagent ending while the parent
        // itself waits on the operator clears the parent's marker too, since
        // one marker is keyed by the session the two share, and the parent's
        // next event re-publishes it.
        // AND THESE ARMS ARE ASYNC, so each is UNORDERED against the next
        // PermissionRequest; `update_blocked_marker`'s End refuses to remove a
        // wait armed after the moment being cleared for, which is what keeps a
        // late clear from taking a newer wait's marker.
        "resolved" => {
            clear_remind(&payload.session_id);
            if !payload.in_subagent || payload.hook_event_name == "SubagentStop" {
                end_blocked_wait(&payload.session_id, now_secs());
            }
        }
        // MID-TURN NEWS FROM A SERVER THAT STOPPED TO ASK. It reports
        // something that happened INSIDE a turn that is still running, so it
        // does not touch the turn marker: the clock belongs to the Stop or the
        // StopFailure that ends the turn, and restarting it here would make a
        // long turn report itself short and lose the tier it earned. It does
        // not forward to moshi either, because an elicitation is answered at
        // the pane the harness is already holding open.
        //
        // IT SERVES `Elicitation` ALONE now. The two `PostToolUse` matchers
        // that used to arrive here fire AFTER the dialog was answered, so they
        // re-armed a wait the operator had just ended and carded them with
        // their own choice read back to them; both are declared to `resolved`.
        // `Elicitation` is the one genuine pre-answer wait of the three.
        "asked" => drop(run_event(
            &pns_domain::EventArgs {
                agent: agent.clone(),
                state: event.to_string(),
                detail: payload.message.clone(),
                pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
                ..attribution(&payload, &agent)
            },
            &system_probes(),
            &payload,
            Attempt::First,
        )),
        // A CALL THE HARNESS REFUSED ON ITS OWN, as an OBSERVATION. Nobody is
        // waiting on an answer: the decision has already been taken, which is
        // why this never forwards to moshi either, and a marker-neutral
        // routing is what stops it colouring a lamp that says a session is
        // waiting and what stops it taking a wait a real question armed beside
        // it. It states no message of its own, so its detail resolves through
        // `parse_payload`'s existing chain to the tool request.
        "denied" => drop(run_event(
            &pns_domain::EventArgs {
                agent: agent.clone(),
                state: event.to_string(),
                detail: payload.message.clone(),
                pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
                ..attribution(&payload, &agent)
            },
            &system_probes(),
            &payload,
            Attempt::Observation,
        )),
        // THE SANDBOX NETWORK APPROVAL DIALOG, behind an exact allowlist of
        // the messages this binary has verified. It reaches the dialog host
        // with no `PermissionRequest` at all, and the host defaults its
        // typeless notification to `permission_prompt`, which is also every
        // ordinary tool approval's type, so the declaration's matcher cannot
        // separate the two and `sandbox_network_detail` is what does. Routed
        // as a wait through `Attempt::First` with a `LAMP_BLOCKED` state word,
        // so `run_event` arms the marker itself, plus the reminder, which is
        // `blocking_event`'s shape without the moshi forward: there is no
        // permission-request payload to hand moshi, and Claude Code already
        // carries this dialog to a phone over its own remote-control bridge.
        // THE CARD CANNOT NAME THE HOST, because the registry text is static
        // and the payload carries neither host nor port.
        "waiting" => {
            if let Some(detail) =
                sandbox_network_detail(&payload.notification_type, &payload.message)
            {
                let event = pns_domain::EventArgs {
                    agent: agent.clone(),
                    state: "blocked".to_string(),
                    detail,
                    pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
                    ..attribution(&payload, &agent)
                };
                // BEFORE THE NOTIFICATION, never after, which is
                // `RequestApproval`'s own order: the record this arms is what
                // a later answer clears, and an answer landing between the
                // card and the arming would leave a record nothing clears.
                let after_secs = match remind_after(&agent) {
                    Ok(after_secs) => after_secs,
                    Err(code) => return code,
                };
                arm_remind(&payload.session_id, &event, after_secs);
                let _ = run_event(&event, &system_probes(), &payload, Attempt::First);
            }
        }
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
                    &pns_domain::EventArgs {
                        agent: agent.clone(),
                        state: event.to_string(),
                        detail,
                        pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
                        ..attribution(&payload, &agent)
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
                    &pns_domain::EventArgs {
                        agent: agent.clone(),
                        state: event.to_string(),
                        detail,
                        pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
                        ..attribution(&payload, &agent)
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
                    &pns_domain::EventArgs {
                        agent: agent.clone(),
                        state: event.to_string(),
                        detail,
                        pane: std::env::var("HERDR_PANE_ID").unwrap_or_default(),
                        ..attribution(&payload, &agent)
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

/// This call's resolved reminder delay, or the exit code to answer with once
/// the refusal has been said.
///
/// ON STDERR, NEVER STDOUT, like every other line this path writes: Claude
/// Code reads this hook's stdout as moshi's decision object.
fn remind_after(agent: &str) -> Result<u64, i32> {
    remind_delay(&crate::arguments_after_verb(), agent).map_err(|refusal| {
        eprintln!("pns: {refusal}");
        2
    })
}

/// What `pns hook` takes, which is one harness event per run.
pub(crate) const HOOK_USAGE: &str = "pns: usage: pns hook prompt | stop | \
stop-failure | blocked | asked | denied | waiting | resolved | model-switch | \
quota | config-change [--remind[=<duration>] | --no-remind] \
(the harness payload arrives on stdin)";
