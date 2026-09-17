//! The identity one submission is known by, derived once so every delivery
//! path spells it the same way.
//!
//! BOTH PATHS DERIVE THE SAME ID FOR THE SAME PAGE. The producer path puts it
//! in the request envelope and the hermes path sends it as `X-Request-ID`, and
//! a page that took one path on Monday and the other on Tuesday is the same
//! page to whatever reads those ids back. That is why the derivation lives
//! here rather than inside either path.

use posture_application::Alert;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// The id for a seed: the first sixteen bytes of its SHA-256, in lowercase
/// hex, under this tool's own prefix. Pure, so the same seed always answers
/// the same id and a derived id can be derived again.
pub(crate) fn derive(seed: &str) -> String {
    let digest: String = Sha256::digest(seed.as_bytes())
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("posture-{digest}")
}

/// The seed a page's own id is derived from: its occurrence when the page has
/// one, so a retry of the same page carries the same id and is recognized
/// rather than delivered twice.
///
/// A page with no occurrence gets a seed no other call can collide with,
/// because collapsing two different findings into one id would silence the
/// second.
pub(crate) fn seed(alert: &Alert) -> String {
    alert.occurrence_id.clone().unwrap_or_else(|| {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        format!(
            "{:?}:{}:{}",
            SystemTime::now(),
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    })
}

#[cfg(test)]
mod tests;
