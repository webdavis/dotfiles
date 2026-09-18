//! What the pinned verifier saw, for a caller that cannot be handed an error.
//!
//! THE `Bridge` TRAIT IS LOSSY ON PURPOSE: `get` answers `Option` and `put`
//! answers nothing, so a refused handshake reaches no caller as a value. A
//! wrong certificate is the one transport failure that is news, so the verifier
//! records it here and whoever ran the call reads it back afterwards. That
//! keeps the trait's shape and teaches no caller a new error.
//!
//! PROCESS SCOPED, because the bridge is built inside closures the application
//! layer calls and there is no handle to read back through. Every call through a
//! mismatched pin fails, so the FIRST one a process sees is the report and the
//! rest are counted.

use pns_domain::CertificatePin;
use std::sync::Mutex;

/// One mismatch, as the reporting needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    /// Where the connection was addressed, as the operator would type it.
    pub address: String,
    pub expected: CertificatePin,
    pub presented: CertificatePin,
}

/// The record one process keeps.
#[derive(Default)]
pub struct Observer {
    seen: Mutex<Option<(Mismatch, u64)>>,
}

impl Observer {
    const fn new() -> Self {
        Self {
            seen: Mutex::new(None),
        }
    }

    /// Record one refused handshake.
    pub fn record(&self, mismatch: Mismatch) {
        // A POISONED LOCK IS NOT WORTH A PANIC HERE. This runs inside a TLS
        // handshake on a notification path: losing the record costs a report,
        // and panicking would cost the process.
        let Ok(mut seen) = self.seen.lock() else {
            return;
        };
        let spoken_for = seen.as_ref().map_or(0, |(_, spoken_for)| *spoken_for);
        *seen = Some((mismatch, spoken_for));
    }

    /// The mismatch nothing has spoken for yet, and never twice.
    pub fn unreported(&self) -> Option<Mismatch> {
        let mut seen = self.seen.lock().ok()?;
        let (mismatch, spoken_for) = seen.as_mut()?;
        if *spoken_for > 0 {
            return None;
        }
        *spoken_for = 1;
        Some(mismatch.clone())
    }

    /// Whether this process refused a handshake at all, spoken for or not.
    /// The doctor's pin row reads this rather than consuming the report.
    pub fn refused(&self) -> Option<Mismatch> {
        let seen = self.seen.lock().ok()?;
        seen.as_ref().map(|(mismatch, _)| mismatch.clone())
    }
}

static OBSERVER: Observer = Observer::new();

pub(super) fn record(address: &str, expected: CertificatePin, presented: CertificatePin) {
    OBSERVER.record(Mismatch {
        address: address.to_string(),
        expected,
        presented,
    });
}

/// The mismatch this process has not reported yet.
pub fn unreported_mismatch() -> Option<Mismatch> {
    OBSERVER.unreported()
}

/// The mismatch this process refused, however many times it has been read.
pub fn refused_mismatch() -> Option<Mismatch> {
    OBSERVER.refused()
}

/// What the handshake refusal says, verbatim, so the text ureq hands back and
/// the text the report carries are one sentence.
pub(super) fn refusal(
    address: &str,
    expected: CertificatePin,
    presented: CertificatePin,
) -> String {
    format!("the bridge at {address} presented {presented}, not the pinned {expected}")
}

#[cfg(test)]
#[path = "mismatch/tests.rs"]
mod tests;
