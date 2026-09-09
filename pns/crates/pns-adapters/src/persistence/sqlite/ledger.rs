use super::{SqliteStore, StoreError};
use pns_application::{
    ClaimedLeg, DeliveryLedger, LeaseWindow, LedgerCompletion, LedgerFailure, LedgerLeg,
    LedgerSubmission, LegAttempt, PreparedSubmission, RetryDelivery, StoredFailure,
    SubmissionIdentity, SubmissionRecord, UnconfirmedDelivery,
};
mod claims;
mod completion;
mod failing;
mod health;
mod outcomes;
mod prepare;
mod read;
pub(super) mod schema;

// An opaque capability belongs to one leg generation. The random token also
// separates equal row ids in different databases, including a replaced database.
#[derive(Debug)]
pub struct DeliveryClaim {
    leg: i64,
    generation: i64,
    token: [u8; 16],
}

impl SqliteStore {
    fn ledger_result<T>(&self, result: Result<T, StoreError>) -> Result<T, LedgerFailure> {
        result.map_err(|error| {
            self.report("delivery ledger", &error);
            LedgerFailure::Unavailable("delivery ledger storage unavailable".into())
        })
    }

    /// The newest failing legs, newest first. READ ONLY, like the health
    /// snapshot beside it and for the same reason: a report must not create,
    /// import or migrate a database, or reading it would change it.
    ///
    /// Inherent rather than on the `DeliveryLedger` trait, which is the port
    /// the delivery path speaks. Nothing about reporting belongs in a delivery
    /// contract, and putting it there would make every test double implement a
    /// method none of them use.
    pub fn failing_legs(&self, limit: u32) -> Result<Vec<StoredFailure>, LedgerFailure> {
        self.failing(|connection| failing::newest(connection, limit))
    }

    /// One failing leg by the id a listing showed, or `None` when that id names
    /// nothing failing: a leg that has since been acknowledged answers here the
    /// same way one that never existed does, because to the reader they are the
    /// same news.
    /// Every distinct route the ledger has posted to, which is the only roster
    /// of routes pns has: a producer names one at call time and nothing else
    /// records it.
    pub fn posted_routes(&self) -> Result<Vec<String>, LedgerFailure> {
        self.failing(failing::routes)
    }

    pub fn failing_leg(&self, id: u64) -> Result<Option<StoredFailure>, LedgerFailure> {
        let Ok(id) = i64::try_from(id) else {
            return Ok(None);
        };
        self.failing(|connection| failing::one(connection, id))
    }

    fn failing<T>(
        &self,
        read: impl FnOnce(&rusqlite::Connection) -> Result<T, StoreError>,
    ) -> Result<T, LedgerFailure> {
        (|| read(&self.read_only()?))().map_err(|_: StoreError| {
            LedgerFailure::Unavailable("delivery ledger unreadable".into())
        })
    }
}
fn validate_lease(lease: LeaseWindow) -> Result<(), LedgerFailure> {
    if lease.until <= lease.now {
        Err(LedgerFailure::InvalidLease)
    } else {
        Ok(())
    }
}
impl DeliveryLedger for SqliteStore {
    type Claim = DeliveryClaim;

    fn prepare(
        &self,
        submission: &LedgerSubmission,
        lease: LeaseWindow,
    ) -> Result<PreparedSubmission<Self::Claim>, LedgerFailure> {
        validate_lease(lease)?;
        let mut instances = std::collections::HashSet::new();
        if !submission
            .legs
            .iter()
            .all(|leg| instances.insert(&leg.destination))
        {
            return Err(LedgerFailure::InvalidPlan);
        }
        let result = self.ledger_result(
            self.transaction(|transaction| prepare::submission(transaction, submission, lease)),
        )?;
        match result {
            PreparedSubmission::Existing(ref record) if record.submission != *submission => {
                Err(LedgerFailure::ConflictingSubmission)
            }
            other => Ok(other),
        }
    }
    fn record(
        &self,
        claim: &Self::Claim,
        delivery: &pns_domain::Delivery,
        at: u64,
        backoff: pns_domain::retry::RetryBackoff,
    ) -> Result<(), LedgerFailure> {
        let recorded = self.ledger_result(self.transaction(|transaction| {
            let completion = outcomes::record(transaction, claim, delivery, at, backoff)?;
            if let Some(ref completion) = completion {
                completion::revise_decision(transaction, claim, completion)?;
            }
            Ok(completion.is_some())
        }))?;
        if recorded {
            Ok(())
        } else {
            Err(LedgerFailure::LostClaim)
        }
    }
    fn claim_retry(
        &self,
        lease: LeaseWindow,
        limits: pns_domain::retry::RetryLimits,
    ) -> Result<Option<RetryDelivery<Self::Claim>>, LedgerFailure> {
        validate_lease(lease)?;
        self.ledger_result(
            self.existing_transaction(|transaction| claims::next(transaction, lease, limits)),
        )
    }
    fn inspect(
        &self,
        identity: &SubmissionIdentity,
    ) -> Result<Option<SubmissionRecord>, LedgerFailure> {
        // The event, routes and attempt history are one read snapshot.
        self.ledger_result((|| {
            let mut connection = self.connect()?;
            let transaction = connection.transaction()?;
            read::find(&transaction, identity)
        })())
    }
}
#[cfg(test)]
mod tests;
