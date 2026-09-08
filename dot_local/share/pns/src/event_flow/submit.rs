use super::*;
use pns_application::{SubmissionIdentity, Submitted};
use pns_protocol::{
    DecodedRequest, DeliveryScope, Interaction, InteractionResult, Request, ResultEnvelope, Signal,
    Status,
};

mod mapping;
mod receipt;

pub(super) struct ProducerRequest {
    pub(super) identity: SubmissionIdentity,
    pub(super) encoded: String,
    pub(super) class: Option<pns_protocol::Name>,
}

pub(crate) fn submit_mode(args: &[String]) -> i32 {
    match pns_cli::submit::run(
        args,
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        |decoded| {
            accept(decoded, |request, producer| {
                let (event, attempt) = mapping::event(request);
                let payload = HookPayload {
                    session_id: request
                        .session
                        .as_ref()
                        .map_or_else(String::new, |session| session.id.as_str().into()),
                    ..HookPayload::default()
                };
                execution::execute(
                    &event,
                    &system_probes(),
                    &payload,
                    attempt,
                    &|table, lights, behaviour, presence| {
                        fire_pulse_unless_quiet(table, lights, behaviour, presence)
                    },
                    Some(producer),
                )
            })
        },
    ) {
        Ok(Status::Rejected) | Err(_) => 2,
        Ok(Status::Accepted | Status::Degraded) => 0,
    }
}

fn accept(
    decoded: DecodedRequest,
    submit: impl FnOnce(&Request, &ProducerRequest) -> Result<Submitted, pns_application::LedgerFailure>,
) -> ResultEnvelope {
    let request = decoded.request;
    let encoded = match request.encode() {
        Ok(encoded) => encoded,
        Err(mut refusal) => {
            refusal.request_id = Some(request.request_id.clone());
            return ResultEnvelope::rejected(&refusal);
        }
    };
    let producer = ProducerRequest {
        identity: SubmissionIdentity {
            producer: request.producer.as_str().into(),
            request_id: request.request_id.as_str().into(),
        },
        encoded,
        class: request.class.clone(),
    };
    let mut result = receipt::result(submit(&request, &producer));
    result.request_id = Some(request.request_id);
    result.interaction =
        (request.interaction == Interaction::AwaitDecision).then_some(InteractionResult::NoOpinion);
    // Unknown field names are already bounded by the decoder. Keep them verbatim,
    // without a prefix that could push an otherwise valid name beyond the text cap.
    if !decoded.ignored.is_empty() {
        result.diagnostics.push("ignored_fields".into());
        result.diagnostics.extend(decoded.ignored);
    }
    result
}
