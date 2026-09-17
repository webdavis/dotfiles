use crate::request_id;
use crate::wire::{Name, Oversized, Request, RequestId, Signal};
use posture_application::{Alert, AlertSignal};

pub(super) enum EncodeFailure {
    Invalid,
    Oversized,
}

pub(super) fn omission(alert: &Alert) -> Alert {
    Alert {
        occurrence_id: alert.occurrence_id.as_ref().map(|id| format!("notification-omitted:{id}")),
        event: "notification-omitted",
        signal: AlertSignal::NeedsAttention,
        // The notice stands in for the finding, so it goes where the finding would have.
        severity: alert.severity,
        occurred_at: alert.occurred_at,
        title: "Posture security alert omitted".into(),
        detail: "A security finding exceeded notification limits. The full alert was not submitted and remains unacknowledged. Inspect the originating posture check.".into(),
    }
}

pub(super) fn encode(
    alert: &Alert,
    route: Option<Name>,
) -> Result<(RequestId, String), EncodeFailure> {
    // ONE DERIVATION FOR BOTH DELIVERY PATHS, so a page carries the same
    // identity whether it was handed to a producer or posted to a route.
    let identity = RequestId::new(request_id::derive(&request_id::seed(alert)))
        .map_err(|_| EncodeFailure::Invalid)?;
    let signal = match alert.signal {
        AlertSignal::NeedsAttention => Signal::NeedsAttention,
        AlertSignal::Observation => Signal::Observation,
    };
    let mut request = Request::new(
        identity.clone(),
        Name::new("posture").map_err(|_| EncodeFailure::Invalid)?,
        Name::new(alert.event).map_err(|_| EncodeFailure::Invalid)?,
        signal,
    );
    request.occurred_at = alert.occurred_at;
    request.detail = format!("{}\n{}", alert.title, alert.detail);
    request.route = route;
    if alert.signal == AlertSignal::NeedsAttention {
        request.class = Some(Name::new("security").map_err(|_| EncodeFailure::Invalid)?);
    }
    // Identifiers were validated above. Only the rendered detail is unbounded
    // here, so a cap is the one refusal left and it is the oversized one.
    let encoded = request
        .encode()
        .map_err(|Oversized| EncodeFailure::Oversized)?;
    Ok((identity, encoded + "\n"))
}
