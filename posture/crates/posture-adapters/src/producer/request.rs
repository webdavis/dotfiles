use posture_application::{Alert, AlertSignal};
use posture_producer_wire::{Name, Rejection, Request, RequestId, Signal, Violation};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

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
    let seed = alert.occurrence_id.clone().unwrap_or_else(|| {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        // Separate calls, including separate producer instances, never collapse identical findings.
        format!(
            "{:?}:{}:{}",
            SystemTime::now(),
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    });
    let digest: String = Sha256::digest(seed.as_bytes())
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let identity =
        RequestId::new(format!("posture-{digest}")).map_err(|_| EncodeFailure::Invalid)?;
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
    let encoded = request.encode().map_err(|error| match error.reason {
        // Identifiers were validated above. Only the rendered detail is unbounded here.
        Rejection::Bound(Violation::Text { .. } | Violation::Bytes { .. }) => {
            EncodeFailure::Oversized
        }
        _ => EncodeFailure::Invalid,
    })?;
    Ok((identity, encoded + "\n"))
}
