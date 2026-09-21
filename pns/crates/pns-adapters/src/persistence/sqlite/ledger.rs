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

    /// How many failing legs are dead-lettered, unbounded by any listing
    /// depth: what the recap's open section counts.
    pub fn dead_lettered_leg_count(&self) -> Result<u64, LedgerFailure> {
        self.failing(failing::dead_lettered_count)
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

    /// Acknowledges every dead-lettered failing leg and answers how many it
    /// cleared, which is the only way a leg the retry policy gave up on leaves
    /// `pns failures`.
    ///
    /// DEAD-LETTERED ONLY. A failing leg still inside its retry budget may yet
    /// arrive, so clearing it would hide a delivery that is still being chased.
    /// The rows stay and `ledger_attempts` is untouched, so what was tried is
    /// still readable afterwards.
    pub fn drain_deadlettered_legs(&self) -> Result<u64, LedgerFailure> {
        self.ledger_result(self.transaction(|transaction| {
            let drained = transaction.execute(
                "UPDATE ledger_legs SET acknowledged = 1, owner = NULL, token = NULL,
                 lease_until = NULL
                 WHERE acknowledged = 0 AND deadlettered_at IS NOT NULL",
                [],
            )?;
            Ok(drained as u64)
        }))
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

/// Whether a retry names the same submission the ledger already recorded.
/// Every field but `producer_request` compares by value; that one compares
/// by decoded meaning, so a producer re-encoding the same request with a
/// newer pns (which may drop bytes a canonical render never wrote, like an
/// absent optional field) still recognizes its own retry instead of
/// conflicting with itself.
fn submissions_match(existing: &LedgerSubmission, new: &LedgerSubmission) -> bool {
    existing.identity == new.identity
        && existing.event == new.event
        && existing.legs == new.legs
        && producer_requests_match(
            existing.producer_request.as_deref(),
            new.producer_request.as_deref(),
        )
}

/// Two producer_request texts match verbatim, or, when they differ, by
/// decoding to the same `Request`. Falls back to the byte compare (already
/// known to fail) when either side does not decode, so unparseable legacy
/// metadata still conflicts rather than being waved through.
fn producer_requests_match(existing: Option<&str>, new: Option<&str>) -> bool {
    match (existing, new) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a == b
                || matches!(
                    (
                        pns_protocol::decode_request(a.as_bytes()),
                        pns_protocol::decode_request(b.as_bytes()),
                    ),
                    (Ok(a), Ok(b)) if a.request == b.request
                )
        }
        _ => false,
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
            PreparedSubmission::Existing(ref record)
                if !submissions_match(&record.submission, submission) =>
            {
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

impl SqliteStore {
    /// The newest command the shell notifier timed, which is the last thing
    /// the operator ran long enough for the engine to hear about.
    ///
    /// THE AGENT NAMES THE PRODUCER. The notifier submits every timed command
    /// as the `shell` producer, so that column is what separates its events
    /// from a harness's, and the ledger is where they land.
    ///
    /// READ ONLY, AND EVERY FAILURE IS NO COMMAND, the reasoning
    /// `newest_wait` states: a report neither creates a database nor refuses
    /// to print because it could not read one.
    pub fn newest_shell_command(&self) -> Option<String> {
        let connection = self.read_only().ok()?;
        connection
            .query_row(
                "SELECT detail FROM ledger_events
                  WHERE agent = 'shell' AND detail <> ''
                  ORDER BY seq DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
    }
}
