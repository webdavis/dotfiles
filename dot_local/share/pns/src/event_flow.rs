use crate::*;

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
pub(crate) enum Attempt {
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
pub(crate) fn run_event(
    event: &pns::args::EventArgs,
    probes: &SystemProbes<SystemCommandRunner>,
    payload: &HookPayload,
    attempt: Attempt,
) {
    run_event_pulsing(
        event,
        probes,
        payload,
        attempt,
        &|table, lights, behaviour, presence| {
            fire_pulse_unless_quiet(table, lights, behaviour, presence);
        },
    );
}
/// Where this event's pulse ends up. THE REAL PULSE IN PRODUCTION and a
/// recorder in the one test that is about the ORDERING of this path rather
/// than about a lamp.
///
/// A SEAM AND NOT AN ABSTRACTION. There is exactly one implementation besides
/// the real pulse, it exists because the readings the pulse is handed are
/// taken hundreds of lines above it, and no other reading on this path is
/// injectable. A probe set is already the seam for everything else.
type PulseSink<'a> = &'a dyn Fn(
    Option<toml::Table>,
    Option<&pns::config::Lights>,
    pns::config::Behaviour,
    Option<&pns::presence_policy::Snapshot>,
);
fn run_event_pulsing(
    event: &pns::args::EventArgs,
    probes: &SystemProbes<SystemCommandRunner>,
    payload: &HookPayload,
    attempt: Attempt,
    pulse: PulseSink<'_>,
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
    let (hue_table, lights, mobile, hermes_key, recap, focus_silence, presence) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (
            enabled_hue_table(config),
            config.lights.clone(),
            read_mobile(config),
            plugin_settings(config, "hermes").and_then(hermes_secret),
            config.recap.clone(),
            config.focus_silence.clone(),
            // A TABLE NOBODY COULD PARSE IS NO READING, never a room: the
            // refusal was already printed, and inventing a room out of
            // settings nobody could read is the fail-open the whole reading is
            // shaped to avoid.
            pns::config::parse_presence(config).ok().flatten(),
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
            None,
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

    // THE LAMPS' OWN READINGS, TAKEN HERE AND NOT AT THE PULSE BELOW. The
    // pulse is the last thing this path does: every channel has dispatched,
    // the record is written and the catch-up has replayed, which is anywhere
    // from a millisecond to a network deadline later. The desk ages this
    // carries come off THIS decision, so a snapshot built down there describes
    // a moment the plan beside it never saw. The reading itself was taken
    // before this probe set held a clock at all (`with_presence_path`), so
    // what is assembled here is one moment rather than three.
    //
    // TAKEN FOR EVERY EVENT, not only the ones that reach a lamp: the gate
    // below is two booleans read off this same decision, and moving the
    // reading behind it would put the boundary back where it was. It costs one
    // memoized file read on an armed machine and nothing at all on a machine
    // with no `[plugins.presence]` table.
    let presence_at_decision = presence_snapshot(
        presence.as_ref(),
        probes,
        decision.inputs.desk_input_age,
        decision.inputs.screen_locked,
        home_presence(),
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
    // on the operator holds blocked whether or not it ran long.
    //
    // IT NEEDS A `[lights]` TABLE, which is the opt-in, and the opt-in is read
    // off the BEHAVIOUR rather than tested a second time here: `state_behaviour`
    // only answers blocked for a mapped machine, so the colour a lamp shows
    // and the gate that lets it fire cannot come out disagreeing about one
    // event. Without the map there is no blocked lamp to show, and a long-running
    // blocked turn keeps the green it has flashed since the bash.
    //
    // AND IT RESPECTS THE SILENCE, through the same predicate arbitration uses
    // rather than a second copy of it: a muted operator gets no lamp, which is
    // the shipped rule that the lights are decoration too.
    //
    // THIS FLASH IS NOT WHAT HOLDS THE LAMP BLOCKED. `pulse_render` answers
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
        // THE DECISION'S OWN READINGS, handed down rather than taken again:
        // this event's plan and the room its lamp narrows to have to describe
        // one moment. The snapshot was BUILT beside the decision, before any
        // channel ran, for the reason stated there.
        pulse(
            hue_table.clone(),
            lights.as_deref(),
            behaviour,
            presence_at_decision.as_ref(),
        );
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

#[cfg(test)]
#[path = "event_flow/tests.rs"]
mod event_flow_tests;
