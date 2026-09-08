use super::{SqliteStore, StoreError};
use pns_application::{
    ClaimedLeg, DeliveryLedger, LeaseWindow, LedgerCompletion, LedgerFailure, LedgerLeg,
    LedgerSubmission, LegAttempt, PreparedSubmission, RetryDelivery, SubmissionIdentity,
    SubmissionRecord, UnconfirmedDelivery,
};
mod claims;
mod completion;
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
        completion: &LedgerCompletion,
        at: u64,
    ) -> Result<(), LedgerFailure> {
        let recorded = self.ledger_result(self.transaction(|transaction| {
            let recorded = outcomes::record(transaction, claim, completion, at)?;
            if recorded {
                completion::revise_decision(transaction, claim, completion)?;
            }
            Ok(recorded)
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
