use super::*;

impl<L, R, D> SubmissionDelivery<'_, L, R, D>
where
    L: DeliveryLedger + Sync,
    L::Claim: Sync,
    R: DecisionOutcomes + Sync,
    D: NotificationDestination,
{
    pub(super) fn attempt(
        &self,
        request: &DeliveryRequest<'_>,
        claim: Option<(&L::Claim, LeaseWindow)>,
        destination: &str,
        clock: &(impl Clock + Sync),
        notice: &(impl Fn(&str) + Sync),
    ) -> Delivery {
        let record = |request: &DeliveryRequest<'_>, delivery: &Delivery| {
            if let Some((claim, lease)) = claim {
                let at = clock.now_secs().unwrap_or(lease.now);
                let completion = completion(delivery, at, lease.until.saturating_sub(lease.now));
                if let Err(error) = self.ledger.record(claim, &completion, at) {
                    notice(&format!(
                        "could not record the {destination} delivery: {error:?}"
                    ));
                }
            } else if let Some(request_id) = request.request_id {
                let identity = crate::SubmissionIdentity {
                    producer: request.producer.into(),
                    request_id: request_id.into(),
                };
                if let Err(error) = self.decisions.revise(&identity, destination, delivery) {
                    notice(&format!(
                        "could not revise the {destination} decision: {error}"
                    ));
                }
            }
            Ok::<(), String>(())
        };
        match self.destinations.get(destination) {
            Some(channel) => Recorded::new(channel, record).deliver(request),
            None => {
                let delivery = self.destinations.deliver(destination, request);
                let _ = record(request, &delivery);
                delivery
            }
        }
    }
}

fn completion(delivery: &Delivery, at: u64, delay: u64) -> LedgerCompletion {
    let (outcome, detail) = match delivery {
        Delivery::Delivered(detail) => {
            return LedgerCompletion::Acknowledged {
                detail: detail.clone(),
            };
        }
        Delivery::Failed(detail) => (UnconfirmedDelivery::Failed, detail.clone()),
        Delivery::Unlaunched(detail) => (UnconfirmedDelivery::Unlaunched, detail.clone()),
        Delivery::Silent => (UnconfirmedDelivery::Unknown, String::new()),
    };
    LedgerCompletion::Retry {
        outcome,
        detail,
        retry_at: at.saturating_add(delay),
    }
}
