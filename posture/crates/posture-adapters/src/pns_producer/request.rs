use posture_pns_wire::{Name, Request, RequestId, Signal};
use posture_application::{Alert, AlertSignal};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

pub(super) fn encode(alert: &Alert, route: Option<Name>) -> Result<(RequestId, String), ()> {
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
    let identity = RequestId::new(format!("posture-{digest}")).map_err(|_| ())?;
    let signal = match alert.signal {
        AlertSignal::NeedsAttention => Signal::NeedsAttention,
        AlertSignal::Observation => Signal::Observation,
    };
    let mut request = Request::new(
        identity.clone(),
        Name::new("posture").map_err(|_| ())?,
        Name::new(alert.event).map_err(|_| ())?,
        signal,
    );
    request.occurred_at = alert.occurred_at;
    request.detail = format!("{}\n{}", alert.title, alert.detail);
    request.route = route;
    if alert.signal == AlertSignal::NeedsAttention {
        request.class = Some(Name::new("security").map_err(|_| ())?);
    }
    Ok((identity, request.encode().map_err(|_| ())? + "\n"))
}
