use crate::*;

/// One recap posted to one route, and what the route had to say about it.
///
/// IT SAYS WHAT HAPPENED, which is what `ReportMode::ReportOutcome` was for
/// and what it never actually did: the destination registry RETURNS its outcomes and
/// prints nothing, so the mode only ever moved the deadline. MEASURED against
/// a dead endpoint, `pns recap --since ... --until ...` printed nothing and
/// exited 0, which is exactly the drill an operator runs by hand to check a
/// route they have just prepared, against exactly the failure it is most
/// likely to meet.
///
/// THE SAME LINE `run_event` PRINTS, prefix and all, because a second spelling
/// of one report is a second thing to keep in step. The detached child's
/// stdout is `/dev/null`, so this costs the event path nothing.
/// The project this recap is about: the repository the composing directory
/// belongs to, by the same rule every session event's project is read by.
///
/// THE CWD, WHICH IS WHERE THE RECAP WAS WRITTEN. A recap covers a window of
/// time rather than one repository, so there is no field on it to read a
/// project off; what there is, is the checkout the operator or the skill ran
/// it in, which is the repository the window was about in every case that has
/// ever produced one. A recap composed outside a repository names no project
/// and lands on the engine's own channel, which is where every recap goes
/// today, so the ambiguous case degrades to the current behaviour rather than
/// guessing.
fn recap_project() -> String {
    let cwd = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    crate::named_project(&pns_adapters::git_checkout(&cwd).repository, &cwd)
}

pub(crate) fn deliver_recap(
    body: &str,
    channel: &str,
    home: &str,
    hermes_keys: &HermesKeys,
    discord: &DiscordSettings,
) -> Vec<(pns_domain::routing::Leg, Delivery)> {
    use pns_application::NotificationDestination;
    let selection = roster().all();
    let mobile = Mobile::default();
    let destinations =
        channel_dispatch::destinations(&selection, channel, home, &mobile, hermes_keys, discord);
    let Some(destination) = destinations.durable() else {
        return Vec::new();
    };
    let leg = pns_domain::routing::Leg {
        name: destination.id().as_str(),
        mode: pns_domain::routing::ReportMode::ReportOutcome,
        decorative: false,
    };
    let event = pns_domain::EventArgs {
        agent: "pns".to_string(),
        state: "recap".to_string(),
        detail: body.to_string(),
        channel: channel.to_string(),
        project: recap_project(),
        ..Default::default()
    };
    let identity = delivery_runtime::fresh_identity()
        .map_err(|_| {
            delivery_runtime::delivery_notice("identity unavailable");
        })
        .ok();
    let result = delivery_runtime::DeliveryRuntime {
        store: &pns_adapters::SqliteStore::for_records(state_dir()),
        selection: &selection,
        home,
        mobile: &mobile,
        hermes_keys,
        discord,
        json: false,
    }
    .submit_request(
        &delivery_runtime::SubmissionInput {
            identity: identity.as_ref(),
            producer_request: None,
            event: &event,
            legs: &[leg],
            pane_dropped: false,
            record: None,
        },
        &now_secs,
    )
    .map_err(|_| ());
    let delivered = match result {
        Ok(pns_application::Submitted::Attempted { outcomes, .. }) => outcomes
            .into_iter()
            .next()
            .map(|(_, delivered)| delivered)
            .unwrap_or_else(|| Delivery::Unlaunched("no durable delivery was attempted".into())),
        Ok(pns_application::Submitted::Existing(_)) => return Vec::new(),
        Err(()) => Delivery::Failed("delivery state unavailable".into()),
    };
    let outcomes = vec![(leg, delivered)];
    for (leg, delivered) in &outcomes {
        if let Some(line) = delivered.clone().line_for(leg.mode) {
            println!("pns: {line}");
        }
    }
    outcomes
}
