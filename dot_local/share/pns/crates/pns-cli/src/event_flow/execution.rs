use super::*;

pub(super) fn execute(
    event: &pns_domain::EventArgs,
    probes: &SystemProbes<SystemCommandRunner>,
    payload: &HookPayload,
    attempt: Attempt,
    pulse: PulseSink<'_>,
    producer: Option<&super::submit::ProducerRequest>,
) -> Result<pns_application::Submitted, pns_application::LedgerFailure> {
    let json = producer.is_some();
    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    let silence_policy = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => config.silence_policy(
            producer
                .and_then(|request| request.class.as_ref())
                .map(pns_protocol::Name::as_str),
        ),
        _ => pns_domain::SilencePolicy::Respect,
    };
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
            pns_adapters::parse_presence(config).ok().flatten(),
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
            pns_adapters::Recap::default(),
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
    let durable_route = selection.iter().any(|plugin| matches!(
        plugin.kind,
        pns_domain::registry::PluginKind::Channel(routing) if routing.durable && routing.event_dispatched
    ));

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

    let decision = pns_application::decide(
        probes,
        &selection,
        &overrides,
        pns_domain::DecisionRequest {
            observation: event.state == "observation",
            scope: event.scope,
            pane: &event.pane,
            now_secs,
            long_running: event.long_running,
            mobile_watch_card: mobile.watch_card,
            silence_policy,
        },
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

    let store = pns_adapters::SqliteStore::for_records(state_dir());
    let identity = producer
        .map(|producer| producer.identity.clone())
        .or_else(|| {
            delivery_runtime::fresh_identity()
                .map_err(|_| {
                    delivery_runtime::delivery_notice("identity unavailable");
                })
                .ok()
        });
    let initial = pns_domain::Record {
        event,
        decision: &decision,
        overrides: &overrides,
        legs: &[],
        nag: attempt == Attempt::Nudge,
        permission_mode: &payload.permission_mode,
        agent_id: &payload.agent_id,
        tool_name: &payload.tool_name,
    };
    let submitted = delivery_runtime::DeliveryRuntime {
        store: &store,
        selection: &selection,
        home: &home,
        mobile: &mobile,
        hermes_key: hermes_key.clone(),
        json,
    }
    .submit_request(
        &delivery_runtime::SubmissionInput {
            identity: identity.as_ref(),
            producer_request: producer.map(|producer| producer.encoded.as_str()),
            event,
            legs: &decision.legs,
            pane_dropped: decision.pane_dropped,
            record: Some(&initial),
        },
        &|| now_secs,
    );
    let submitted =
        submitted.inspect_err(|_| delivery_runtime::delivery_notice("submission refused"))?;
    let outcomes = match &submitted {
        pns_application::Submitted::Attempted { outcomes, .. } => decision
            .legs
            .iter()
            .zip(outcomes)
            .map(|(leg, (_, delivered))| (*leg, delivered.clone()))
            .collect::<Vec<_>>(),
        pns_application::Submitted::Existing(_) => return Ok(submitted),
    };
    for (leg, delivered) in &outcomes {
        if let Some(line) = delivered.clone().line_for(leg.mode) {
            if json {
                eprintln!("pns: {line}");
            } else {
                println!("pns: {line}");
            }
        }
    }

    let records = EventRecords {
        moment: store,
        home: &home,
        selection: &selection,
        hue_table: hue_table.as_ref(),
        lights: lights.as_deref(),
        mobile: &mobile,
        hermes_key: hermes_key.clone(),
        recap,
        durable_route,
        json,
        pulse,
    };
    let lamps_live = lights.is_some() && hue_table.is_some();
    pns_application::SubmitNotification { ports: &records }.record(&pns_application::Submission {
        identity: identity.as_ref(),
        event,
        decision: &decision,
        overrides: &overrides,
        legs: &outcomes,
        attempt: match attempt {
            Attempt::First => pns_application::Attempt::First,
            Attempt::Nudge => pns_application::Attempt::Nudge,
            Attempt::Observation => pns_application::Attempt::Observation,
        },
        session_id: &payload.session_id,
        lamps_live,
        lights_declared: lights.is_some(),
        presence: presence_at_decision.as_ref(),
    });
    Ok(submitted)
}
