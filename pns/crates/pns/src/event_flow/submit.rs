use super::*;
use pns_application::{SubmissionIdentity, Submitted};
use pns_protocol::{DecodedRequest, DeliveryScope, RequestEnvelope, ResultEnvelope, State, Status};

mod mapping;
mod receipt;

pub(super) struct ProducerRequest {
    pub(super) identity: SubmissionIdentity,
    pub(super) encoded: String,
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
///
/// THE RECEIPT IS DISCARDED, not printed: both callers (the GitHub poll, the
/// `resume --notify` automation) already have the exit code, and a caller
/// that wanted the page asked for `--json` or the terminal page instead.
pub(crate) fn submit_encoded(encoded: &[u8]) -> i32 {
    submit_reading(&["--json".to_string()], encoded, std::io::sink())
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
            // BEFORE THE DELIVERY, which is the blocked hook's own order: the
            // record this arms is what a later answer clears, and an answer
            // landing between the card and the arming would leave a record
            // nothing clears. A refusal is said and the event still goes: a
            // nudge nobody could resolve must not cost the delivery the rest
            // of the envelope still earns.
            match mapping::reminder(request) {
                Ok(reminder) => arm_remind(&payload.session_id, &event, reminder),
                Err(refusal) => eprintln!("pns: {refusal}"),
            }
            execution::execute(
                &event,
                &system_probes(),
                &payload,
                attempt,
                &|table, lights, flash, presence| {
                    fire_pulse_for_event(table, lights, flash, presence)
                },
                Some(producer),
            )
        })
    }) {
        Ok(status) => exit_code(status),
        // A receipt that could not even be written is input this path cannot
        // honour, on the code every other refusal earns.
        Err(_) => crate::legacy::REFUSED_INPUT,
    }
}

/// THE STATUS IS THE EXIT CODE. A page that reached only some of its
/// destinations is a broken destination the producer would not otherwise hear
/// about, so it earns the same `1` as one that reached none.
fn exit_code(status: Status) -> i32 {
    match status {
        Status::Delivered => 0,
        Status::Partial | Status::Undelivered => crate::invocation::EVENT_NOT_DELIVERED,
        Status::Rejected => crate::legacy::REFUSED_INPUT,
    }
}

fn accept(
    decoded: DecodedRequest,
    submit: impl FnOnce(&RequestEnvelope, &ProducerRequest) -> Result<Submitted, NotSubmitted>,
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
        github,
    };
    let mut result = receipt::result(submit(&request, &producer));
    result.request_id = Some(request.request_id);
    // The fields this envelope recognizes but acts on nowhere. Verbatim: the
    // decoder already bounded these names, and a prefix could push an
    // otherwise valid one beyond the text cap.
    result.ignored_fields = decoded.ignored;
    result
}

#[cfg(test)]
mod exit_code_tests {
    use super::{Status, exit_code};

    #[test]
    fn every_status_earns_the_exit_code_its_delivery_deserves() {
        assert_eq!(exit_code(Status::Delivered), 0);
        assert_eq!(exit_code(Status::Partial), 1);
        assert_eq!(exit_code(Status::Undelivered), 1);
        assert_eq!(exit_code(Status::Rejected), 2);
    }
}

#[cfg(test)]
mod accept_tests {
    use super::{NotSubmitted, ResultEnvelope, accept};

    fn answered(request: &str) -> ResultEnvelope {
        let decoded = pns_protocol::decode_request(request.as_bytes()).expect("a valid request");
        accept(decoded, |_, _| {
            Err(NotSubmitted::UnknownDeliveryClass("never-used".into()))
        })
    }

    /// A field version 1 does not define never reaches this path: the decode
    /// refuses it by name, so nothing here can name it in a list instead.
    #[test]
    fn an_unrecognized_field_is_refused_by_the_decode_rather_than_answered() {
        let refusal = pns_protocol::decode_request(
            br#"{"schema":"pns.request/1","request_id":"r-1","producer":"test","state":"observation","detial":"typo"}"#,
        )
        .expect_err("an unknown field is refused");
        assert!(
            format!("{:?}", refusal.reason).contains("detial"),
            "{refusal:?}"
        );
    }

    #[test]
    fn a_request_with_only_known_fields_answers_an_empty_ignored_list() {
        let result = answered(
            r#"{"schema":"pns.request/1","request_id":"r-1","producer":"test","state":"observation"}"#,
        );
        assert!(
            result.ignored_fields.is_empty(),
            "{:?}",
            result.ignored_fields
        );
    }
}
