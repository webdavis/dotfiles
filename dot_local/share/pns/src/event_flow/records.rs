use super::*;

/// THE COMPOSITION ROOT'S SIDE OF THE RECORD TAIL: one adapter per port,
/// each one this binary's existing function with the values a use case has no
/// use for bound in.
///
/// ONE STRUCT IMPLEMENTING TEN TRAITS rather than ten structs. Every one of
/// them writes into the same state directory for the same event, and ten
/// zero-sized types would be ten names for one moment.
pub(super) struct EventRecords<'a> {
    pub(super) moment: pns_adapters::SqliteStore,
    pub(super) home: &'a str,
    pub(super) selection: &'a pns_domain::registry::Selection,
    pub(super) hue_table: Option<&'a toml::Table>,
    pub(super) lights: Option<&'a pns::config::Lights>,
    pub(super) mobile: &'a Mobile,
    pub(super) hermes_key: Option<String>,
    pub(super) recap: pns::config::Recap,
    pub(super) durable_route: bool,
    pub(super) json: bool,
    /// The pulse seam, carried because the readings it is handed are taken
    /// hundreds of lines above the call.
    pub(super) pulse: PulseSink<'a>,
}

impl pns_application::Journal for EventRecords<'_> {
    fn journal(
        &self,
        event: &pns_domain::EventArgs,
        now: Option<u64>,
        identity: Option<&pns_application::SubmissionIdentity>,
    ) {
        pns_application::Journal::journal(&self.moment, event, now, identity);
    }
    fn read(&self) -> Result<Option<String>, String> {
        pns_application::Journal::read(&self.moment)
    }
}
impl pns_application::ActivityRing for EventRecords<'_> {
    fn record(&self, event: &pns_domain::EventArgs, now: Option<u64>) {
        pns_application::ActivityRing::record(&self.moment, event, now);
    }
    fn entries_between(&self, since: u64, until: u64) -> Vec<pns_domain::missed::Entry> {
        pns_application::ActivityRing::entries_between(&self.moment, since, until)
    }
}

impl pns_application::BlockedMarker for EventRecords<'_> {
    fn update(&self, session_id: &str, event_state: &str, lamps_live: bool, now: Option<u64>) {
        update_blocked_marker(&state_dir(), session_id, event_state, lamps_live, now);
    }
}

impl pns_application::LoopLease for EventRecords<'_> {
    fn renew(&self, pane: &str, now: Option<u64>) {
        renew_loop_lease(&state_dir(), pane, now);
    }
}

impl pns_application::LampRecords for EventRecords<'_> {
    fn news(&self, behaviour: pns::config::Behaviour, now: Option<u64>) {
        let _ = self.moment.record_news(behaviour, now);
    }
    fn clear_held(&self) {
        clear_held_lamps(self.hue_table);
    }
}

impl pns_application::ReturnMoment for EventRecords<'_> {
    fn complete(&self) {
        pns_application::ReturnMoment::complete(&self.moment);
    }
    fn claim(&self, now: Option<u64>, take_journal: bool) -> Option<pns_application::Claim> {
        // SubmitNotification already checked presence. The repository
        // retains the absent-clock and newer-edge checks for this exact now.
        if take_journal {
            return pns_application::ReturnMoment::claim(&self.moment, now, true);
        }
        if let Some(now) = now {
            let _ = self.moment.mark_present(now);
        }
        None
    }
}

impl pns_application::LightsTick for EventRecords<'_> {
    fn register(&self, decision: &pns::engine::Decision, actual_miss: bool) {
        pns_application::register_lights_tick(
            &pns_adapters::FileJobSpool::new(state_dir()),
            self.lights,
            decision,
            actual_miss,
        );
    }
}

impl pns_application::LampSignal for EventRecords<'_> {
    fn pulse(
        &self,
        behaviour: pns::config::Behaviour,
        presence: Option<&pns::presence_policy::Snapshot>,
    ) {
        (self.pulse)(self.hue_table.cloned(), self.lights, behaviour, presence);
    }
}

impl pns_application::MissedReplay for EventRecords<'_> {
    fn replay(&self, decision: &pns::engine::Decision) {
        replay_missed(
            self.recap.clone(),
            decision,
            self.durable_route,
            delivery_runtime::DeliveryRuntime {
                store: &self.moment,
                selection: self.selection,
                home: self.home,
                mobile: self.mobile,
                hermes_key: self.hermes_key.clone(),
                json: self.json,
            },
        );
    }
}
