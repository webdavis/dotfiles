use crate::{RaiseNotification, Router, StalenessMemory};
use pns_domain::EventArgs;
use pns_domain::home::{self, DeviceIdentity, HomeReading, Staleness};

/// One reading from typed router facts, judged by the domain.
pub fn read_home<R: Router>(router: &R, device: &DeviceIdentity) -> HomeReading {
    home::home_reading(router.clients(), device)
}

pub struct ReadHomeProbe<'a, R, M, N> {
    pub router: &'a R,
    pub memory: &'a M,
    pub notifier: &'a N,
}

impl<R: Router, M: StalenessMemory, N: RaiseNotification> ReadHomeProbe<'_, R, M, N> {
    pub fn run(
        &self,
        device: &DeviceIdentity,
        alert_route: String,
        report: impl FnOnce(&HomeReading, Option<&Staleness>),
    ) -> HomeReading {
        let reading = read_home(self.router, device);
        // ONE DERIVATION, ONE DECISION. The episode is spelled once and the news
        // decided once, then the SAME value is what gets printed and what gets
        // remembered: two derivations of one fact, one in the print and one in
        // the write, can only stay in step for as long as neither grows a
        // condition of its own.
        let staleness = home::stale_identifiers(&reading);
        let episode = staleness.as_ref().map(home::episode_id);
        let news = home::is_new_staleness(self.memory.remembered().as_deref(), episode.as_deref());
        // ONE VALUE FEEDS BOTH SURFACES. The sentence the terminal prints and the
        // sentence the alert carries come out of this same Option, so there is no
        // second condition that could deliver what was not printed, or print what
        // was not delivered. It is Some only for a HOME reading with a
        // disagreement that is news, which is what keeps away, unreadable and
        // already-told runs silent without a guard of their own.
        let alert = staleness.as_ref().filter(|_| news);
        report(&reading, alert);
        // THE WARNING, DELIVERED. An ordinary event ABOUT the reading, handed to
        // the one event path: presence, surface and the leg plan decide where it
        // lands exactly as they do for a finished agent turn. Nothing narrows it
        // and it is not long-running, so it raises no pulse.
        //
        // DISPATCH BEFORE REMEMBER, AND THE ORDER IS LOAD-BEARING. Tidied into
        // remember-then-dispatch it would silently LOSE an alert: a crash, a
        // wedged channel or a kill between the two leaves the episode recorded
        // and never delivered, and the next run reads it as already told. This
        // way round the same interruption re-alerts instead, and two overlapping
        // hand runs that both read the memory before either writes both alert.
        // Duplicates are the direction to fail in.
        //
        // THE COST, ACCEPTED: the delivery OUTCOME is not consulted before the
        // write either, so a post the gateway rejected consumes the episode just
        // as a delivered one does. Fire-and-forget is this engine's contract for
        // every producer, and the printed line above has already told the one
        // human who typed the command.
        if let Some(staleness) = alert {
            self.notifier.raise(&EventArgs {
                agent: "pns".to_string(),
                state: "stale".to_string(),
                detail: home::stale_warning(staleness),
                channel: alert_route,
                ..Default::default()
            });
        }
        // ONLY A HOME READING HAS AN OPINION ABOUT THE IDENTIFIERS. NotHome and
        // Unknown both hand `stale_identifiers` a None, and writing that back
        // would read "the disagreement resolved" out of a trip to the shops or a
        // five-second router timeout: the same invention as reading a failed
        // fetch as NotHome, one layer up. Away and unreadable leave the memory
        // untouched, so the warning stays once per STATE rather than once per
        // homecoming.
        if matches!(reading.presence, home::HomePresence::Home { .. }) {
            self.memory.remember(episode.as_deref());
        }
        reading
    }
}

#[cfg(test)]
mod tests;
