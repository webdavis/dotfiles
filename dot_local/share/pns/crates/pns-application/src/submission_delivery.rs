use crate::{
    Clock, DecisionOutcomes, DeliveryLedger, DeliveryRequest, Destinations, LeaseWindow,
    LedgerCompletion, LedgerFailure, LedgerLeg, LedgerSubmission, NotificationDestination,
    PreparedSubmission, Recorded, SubmissionRecord, UnconfirmedDelivery,
};
use pns_domain::{Delivery, Record};

#[derive(Debug)]
pub enum Submitted {
    Attempted {
        sequence: Option<u64>,
        outcomes: Vec<(LedgerLeg, Delivery)>,
    },
    Existing(Box<SubmissionRecord>),
}

pub struct SubmissionDelivery<'a, L, R, D> {
    pub ledger: &'a L,
    pub decisions: &'a R,
    pub destinations: &'a Destinations<D>,
}

impl<L, R, D> SubmissionDelivery<'_, L, R, D>
where
    L: DeliveryLedger + Sync,
    L::Claim: Sync,
    R: DecisionOutcomes + Sync,
    D: NotificationDestination,
{
    pub fn submit(
        &self,
        submission: &LedgerSubmission,
        record: Option<&Record>,
        lease: LeaseWindow,
        clock: &(impl Clock + Sync),
        notice: &(impl Fn(&str) + Sync),
    ) -> Result<Submitted, LedgerFailure> {
        let (sequence, legs) = match self.ledger.prepare(submission, lease) {
            Ok(PreparedSubmission::Existing(record)) => return Ok(Submitted::Existing(record)),
            Ok(PreparedSubmission::Created { sequence, legs }) => (
                Some(sequence),
                legs.into_iter()
                    .map(|entry| (entry.leg, Some(entry.claim)))
                    .collect::<Vec<_>>(),
            ),
            Err(LedgerFailure::Unavailable(error)) => {
                notice(&format!(
                    "delivery ledger unavailable; delivery is not queued: {error}"
                ));
                return Ok(self.attempt_live(
                    &DeliveryRequest {
                        producer: &submission.identity.producer,
                        request_id: Some(&submission.identity.request_id),
                        event: &submission.event,
                        route: "",
                        mode: pns_domain::routing::ReportMode::Silent,
                    },
                    &submission.legs,
                    record,
                    notice,
                ));
            }
            Err(error) => return Err(error),
        };
        if let Some(record) = record {
            let initial = Record {
                legs: &[],
                ..*record
            };
            if let Err(error) = self.decisions.begin(&submission.identity, &initial) {
                notice(&format!("could not begin the delivery decision: {error}"));
            }
        }
        let outcomes = legs
            .into_iter()
            .map(|(leg, claim)| {
                let delivery = self.attempt(
                    &DeliveryRequest {
                        producer: &submission.identity.producer,
                        request_id: Some(&submission.identity.request_id),
                        event: &submission.event,
                        route: &leg.route,
                        mode: leg.mode,
                    },
                    claim.as_ref().map(|claim| (claim, lease)),
                    &leg.destination,
                    clock,
                    notice,
                );
                (leg, delivery)
            })
            .collect();
        Ok(Submitted::Attempted { sequence, outcomes })
    }

    pub fn retry(
        &self,
        lease: LeaseWindow,
        clock: &(impl Clock + Sync),
        notice: &(impl Fn(&str) + Sync),
    ) -> Result<Option<(LedgerLeg, Delivery)>, LedgerFailure> {
        let Some(retry) = self.ledger.claim_retry(lease, Default::default())? else {
            return Ok(None);
        };
        Ok(Some(self.attempt_retry(retry, lease, clock, notice)))
    }

    pub fn attempt_retry(
        &self,
        retry: crate::RetryDelivery<L::Claim>,
        lease: LeaseWindow,
        clock: &(impl Clock + Sync),
        notice: &(impl Fn(&str) + Sync),
    ) -> (LedgerLeg, Delivery) {
        let delivery = self.attempt(
            &DeliveryRequest {
                producer: &retry.identity.producer,
                request_id: Some(&retry.identity.request_id),
                event: &retry.event,
                route: &retry.leg.route,
                mode: retry.leg.mode,
            },
            Some((&retry.claim, lease)),
            &retry.leg.destination,
            clock,
            notice,
        );
        (retry.leg, delivery)
    }

    pub fn attempt_live(
        &self,
        request: &DeliveryRequest<'_>,
        legs: &[LedgerLeg],
        record: Option<&Record>,
        notice: &(impl Fn(&str) + Sync),
    ) -> Submitted {
        if let (Some(request_id), Some(record)) = (request.request_id, record) {
            let identity = crate::SubmissionIdentity {
                producer: request.producer.into(),
                request_id: request_id.into(),
            };
            let initial = Record {
                legs: &[],
                ..*record
            };
            if let Err(error) = self.decisions.begin(&identity, &initial) {
                notice(&format!("could not begin the delivery decision: {error}"));
            }
        }
        let outcomes = legs
            .iter()
            .map(|leg| {
                let delivery = self.attempt(
                    &DeliveryRequest {
                        route: &leg.route,
                        mode: leg.mode,
                        ..*request
                    },
                    None,
                    &leg.destination,
                    &|| None,
                    notice,
                );
                (leg.clone(), delivery)
            })
            .collect();
        Submitted::Attempted {
            sequence: None,
            outcomes,
        }
    }
}

mod attempt;

#[cfg(test)]
mod tests;
