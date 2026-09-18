use super::*;
use pns_application::{SubmissionIdentity, Submitted};
use pns_protocol::{
    DecodedRequest, DeliveryScope, Interaction, InteractionResult, Request, ResultEnvelope, Status,
};

mod mapping;
mod receipt;

pub(super) struct ProducerRequest {
    pub(super) identity: SubmissionIdentity,
    pub(super) encoded: String,
    pub(super) class: Option<pns_protocol::Name>,
    /// The `github` extension this request carried, if any. A MALFORMED one
    /// is none of them: the refusal is printed and the event takes the
    /// ordinary path, because a colour nobody could read must not cost the
    /// delivery the rest of the envelope still earns.
    pub(super) github: Option<pns_domain::github::GithubEvent>,
}

pub(crate) fn submit_mode(args: &[String]) -> i32 {
    submit_reading(args, std::io::stdin().lock(), std::io::stdout().lock())
}

/// One envelope this binary built itself, through the SAME path a producer's
/// own submission takes.
///
/// NOT A SECOND SUBMISSION PATH. The GitHub poll runs inside this binary, so
/// spawning a child of itself to hand the envelope over a pipe would buy
/// nothing but a process: reading the encoded bytes here runs the identical
/// decode, ledger, policy and dispatch, which is what keeps the poll an
/// ordinary producer rather than a privileged one.
pub(crate) fn submit_encoded(encoded: &[u8]) -> i32 {
    submit_reading(&["--json".to_string()], encoded, std::io::stdout().lock())
}

fn submit_reading(args: &[String], input: impl std::io::Read, output: impl std::io::Write) -> i32 {
    match crate::submit::run(args, input, output, |decoded| {
        accept(decoded, |request, producer| {
            let (event, attempt) = mapping::event(request);
            let payload = HookPayload {
                session_id: request
                    .session
                    .as_ref()
                    .map_or_else(String::new, |session| session.as_str().into()),
                ..HookPayload::default()
            };
            execution::execute(
                &event,
                &system_probes(),
                &payload,
                attempt,
                &|table, lights, flash, presence| {
                    fire_pulse_unless_quiet(table, lights, flash, presence)
                },
                Some(producer),
            )
        })
    }) {
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
    let github = match pns_adapters::github_event(&request.extensions) {
        Ok(github) => github,
        Err(refusal) => {
            eprintln!("pns: {refusal}");
            None
        }
    };
    let producer = ProducerRequest {
        identity: SubmissionIdentity {
            producer: request.producer.as_str().into(),
            request_id: request.request_id.as_str().into(),
        },
        encoded,
        class: request.class.clone(),
        github,
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
