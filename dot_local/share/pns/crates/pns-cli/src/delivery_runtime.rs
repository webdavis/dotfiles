use crate::{Mobile, channel_dispatch, now_secs, state_dir};
use pns_adapters::SqliteStore;
use pns_application::{
    Clock, DeliveryRequest, LeaseWindow, LedgerFailure, LedgerLeg, LedgerSubmission,
    SubmissionDelivery, SubmissionIdentity, Submitted,
};
use pns_domain::{
    EventArgs, Record,
    registry::Selection,
    routing::{Leg, ReportMode},
};
use std::io::Read;

mod retry;
pub(crate) use retry::retry_pending;

pub(crate) struct DeliveryRuntime<'a> {
    pub(crate) store: &'a SqliteStore,
    pub(crate) selection: &'a Selection,
    pub(crate) home: &'a str,
    pub(crate) mobile: &'a Mobile,
    pub(crate) hermes_key: Option<String>,
    pub(crate) json: bool,
}

pub(crate) struct SubmissionInput<'a> {
    pub(crate) identity: Option<&'a SubmissionIdentity>,
    pub(crate) producer_request: Option<&'a str>,
    pub(crate) event: &'a EventArgs,
    pub(crate) legs: &'a [Leg],
    pub(crate) pane_dropped: bool,
    pub(crate) record: Option<&'a Record<'a>>,
}

impl DeliveryRuntime<'_> {
    pub(crate) fn submit(
        &self,
        identity: &SubmissionIdentity,
        event: &EventArgs,
        legs: &[Leg],
        pane_dropped: bool,
        record: Option<&Record<'_>>,
    ) -> Result<Submitted, LedgerFailure> {
        self.submit_request(
            &SubmissionInput {
                identity: Some(identity),
                producer_request: None,
                event,
                legs,
                pane_dropped,
                record,
            },
            &now_secs,
        )
    }

    pub(crate) fn submit_request(
        &self,
        input: &SubmissionInput<'_>,
        clock: &(impl Clock + Sync),
    ) -> Result<Submitted, LedgerFailure> {
        let destinations = channel_dispatch::destinations_with_output(
            self.selection,
            &input.event.channel,
            self.home,
            self.mobile,
            self.hermes_key.clone(),
            self.json,
        );
        self.attempt(input, clock, &destinations)
    }

    fn attempt<D: pns_application::NotificationDestination>(
        &self,
        input: &SubmissionInput<'_>,
        clock: &(impl Clock + Sync),
        destinations: &pns_application::Destinations<D>,
    ) -> Result<Submitted, LedgerFailure> {
        let event = if input.legs.is_empty() {
            channel_dispatch::rendered_event_quiet(input.event, input.pane_dropped)
        } else {
            channel_dispatch::rendered_event(input.event, input.pane_dropped)
        };
        let legs = input
            .legs
            .iter()
            .map(|leg| LedgerLeg {
                destination: leg.name.into(),
                route: input.event.channel.clone(),
                mode: leg.mode,
                decorative: leg.decorative,
            })
            .collect::<Vec<_>>();
        let workflow = SubmissionDelivery {
            ledger: self.store,
            decisions: self.store,
            destinations,
        };
        match (
            input.identity,
            clock.now_secs().and_then(|now| lease(now).ok()),
        ) {
            (Some(identity), Some(window)) => workflow.submit(
                &LedgerSubmission {
                    identity: identity.clone(),
                    producer_request: input.producer_request.map(str::to_owned),
                    event,
                    legs,
                },
                input.record,
                window,
                clock,
                &delivery_notice,
            ),
            _ => {
                delivery_notice("retention identity or clock unavailable");
                Ok(workflow.attempt_live(
                    &DeliveryRequest {
                        producer: input
                            .identity
                            .map_or("pns", |identity| identity.producer.as_str()),
                        request_id: input.identity.map(|identity| identity.request_id.as_str()),
                        event: &event,
                        route: "",
                        mode: ReportMode::Silent,
                    },
                    &legs,
                    input.record,
                    &delivery_notice,
                ))
            }
        }
    }
}

pub(crate) fn fresh_identity() -> std::io::Result<SubmissionIdentity> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(SubmissionIdentity {
        producer: "pns".into(),
        request_id: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
    })
}

pub(crate) fn lease(now: u64) -> Result<LeaseWindow, LedgerFailure> {
    Ok(LeaseWindow {
        now,
        until: now.checked_add(30).ok_or(LedgerFailure::InvalidLease)?,
    })
}

pub(crate) fn delivery_notice(_: &str) {
    SqliteStore::new(state_dir()).report_delivery_gap();
}

#[cfg(test)]
mod tests;
