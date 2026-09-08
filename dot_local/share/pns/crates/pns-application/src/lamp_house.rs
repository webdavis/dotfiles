use crate::{AgentWork, LampHouseRecords, LampMarkers};

/// What one tick found: the states the house is holding, and whether anything
/// is still in flight that could become one before the next tick.
///
/// TWO ANSWERS OFF ONE READING, because the tick's own lease is a function of
/// both. A lamp that is ON has to be re-armed; a run of work that has NOT yet
/// reached its threshold has to still be watched when it does, and taking that
/// as a second reading would be a second sweep of the same directories.
pub struct Standing {
    pub house: pns_domain::lights::held::House,
    /// A run of work or a lease that is live and has not lit a lamp YET.
    pub in_flight: bool,
}

pub struct ReadLampHouse<'a, W, M, R> {
    pub work: &'a W,
    pub markers: &'a M,
    pub records: &'a R,
}
impl<W: AgentWork, M: LampMarkers, R: LampHouseRecords> ReadLampHouse<'_, W, M, R> {
    /// The states the house is in, taken off the machine.
    ///
    /// THE STREAK IS ADVANCED HERE, which is the one reading that WRITES: a run of
    /// work is a duration, and a duration needs somewhere to have started.
    pub fn read(
        &self,
        lights: &pns_domain::lamps::config::Lights,
        now: u64,
        last_interaction: impl FnOnce() -> Option<u64>,
    ) -> Standing {
        // THE SAME CALL THE VISIBILITY MODEL MAKES, bounded the same way, and read
        // for a different field. A herdr that is missing, wedged or answering
        // something this cannot parse yields no working workspace, which is the
        // fail-toward-dark direction.
        let statuses = self.work.statuses();
        // THE SHELL'S OWN MARKERS, which each interactive shell writes while a
        // plain command runs in it. Nothing in this crate writes them.
        let shell_since = self.markers.shell_since();
        // BOTH SOURCES ARE WORK IN FLIGHT (operator ruling), which is the question
        // the UNREAD lamp asks: news that arrives while anything is still running is
        // not news anybody has missed yet.
        let working = pns_domain::lights::streak::any_working(&statuses, shell_since);
        // AND THE STREAK IS THE AGENTS' ALONE, because it exists to supply a start
        // that herdr does not give: a status word carries no clock. The shell
        // publishes the second its command began, so pooling the two had a fresh
        // command inherit an agent's finished run and a long build restart its own.
        let agents_working = pns_domain::lights::streak::any_working(&statuses, None);
        let streak = self.records.advance_streak(agents_working, now);
        let leases = self.markers.leases(now, lights.looping.lease_timeout_secs);
        Standing {
            // WORK THAT HAS NOT REACHED ITS THRESHOLD IS STILL IN FLIGHT, and this
            // is the reading that keeps the tick alive long enough to see it get
            // there: the automatic trigger's default is five minutes and the
            // operator's is six, both of them PAST the ordinary lease an event
            // leaves behind.
            in_flight: streak.is_some() || shell_since.is_some() || !leases.is_empty(),
            house: pns_domain::lights::held::House {
                blocked: pns_domain::lights::held::any_blocked(
                    &self.markers.blocked(now, lights.blocked.give_up_after_secs),
                    now,
                    lights.blocked.give_up_after_secs,
                ),
                looping: pns_domain::lights::looping::loop_running(
                    &pns_domain::lights::looping::Loop {
                        streak: streak.as_ref(),
                        agents_working,
                        shell_since,
                        leases: &leases,
                        now,
                        threshold_secs: lights.looping.threshold_secs,
                        lease_timeout_secs: lights.looping.lease_timeout_secs,
                    },
                ),
                unread: pns_domain::lights::unread::unread_arming(
                    &self.records.news(),
                    last_interaction(),
                    working,
                    now,
                    lights.unread.after_secs,
                ),
            },
        }
    }
}
