use super::{SqliteStore, StoreError, scalar::Scalar};
use pns_domain::lamps::config::Behaviour;
use pns_domain::lights::{streak::Streak, unread::News};
mod held;
mod muted;

// A loop's between-turn gap survives exactly the legacy two-minute grace.
pub(super) const WORKING_GRACE_SECS: u64 = 120;
impl SqliteStore {
    pub fn read_news(&self) -> Result<News, StoreError> {
        Ok(Scalar::News
            .read(&self.connect()?)?
            .as_deref()
            .and_then(crate::lights_codec::parse_news)
            .unwrap_or_default())
    }
    pub fn record_news(&self, behaviour: Behaviour, now: Option<u64>) -> Result<(), StoreError> {
        let Some(now) = now else {
            return Ok(());
        };
        if pns_domain::lights::unread::news_after(News::default(), behaviour, now).is_none() {
            return Ok(());
        }
        // Both fields are merged while holding one database transaction. A
        // second event cannot replace a failure with a stale success snapshot.
        self.transaction(|transaction| {
            let held = Scalar::News
                .stored(transaction)?
                .as_deref()
                .and_then(crate::lights_codec::parse_news)
                .unwrap_or_default();
            if let Some(next) = pns_domain::lights::unread::news_after(held, behaviour, now) {
                Scalar::News.write(transaction, Some(&crate::lights_codec::render_news(&next)))?;
            }
            Ok(())
        })
        .inspect_err(|error| self.report("lamp news", error))
    }
    pub fn advance_streak(&self, working: bool, now: u64) -> Result<Option<Streak>, StoreError> {
        self.transaction(|transaction| {
            let held = Scalar::Streak
                .stored(transaction)?
                .as_deref()
                .and_then(crate::lights_codec::parse_streak);
            let next =
                pns_domain::lights::streak::next_streak(held, working, now, WORKING_GRACE_SECS);
            Scalar::Streak.write(
                transaction,
                next.as_ref()
                    .map(crate::lights_codec::render_streak)
                    .as_deref(),
            )?;
            Ok(next)
        })
    }
}
