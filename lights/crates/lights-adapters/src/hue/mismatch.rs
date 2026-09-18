//! What the pinned verifier saw, for a caller the handshake error reaches
//! stripped of its detail.
//!
//! WHY A RECORD AND NOT JUST THE ERROR. `LightControlError` carries a detail
//! string, and every ureq failure collapses into one of two sentences on its
//! way there, because a bridge that is merely unplugged must not print a wall
//! of transport prose. A wrong certificate is the one transport failure that is
//! news, so the verifier records it here and the failure path reads it back.
//!
//! PROCESS SCOPED, and spoken for ONCE: every call through a mismatched pin
//! fails the same way, so a walk over five rooms would otherwise print five
//! copies of the same two fingerprints.

use lights_domain::CertificatePin;
use std::sync::Mutex;

/// One refused handshake, as the report needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    /// Where the connection was addressed, as the operator would type it.
    pub address: String,
    pub expected: CertificatePin,
    pub presented: CertificatePin,
}

/// The record one process keeps.
#[derive(Default)]
pub struct Record {
    seen: Mutex<Option<(Mismatch, bool)>>,
}

impl Record {
    const fn new() -> Self {
        Self {
            seen: Mutex::new(None),
        }
    }
    /// Keep one refused handshake.
    pub fn keep(&self, mismatch: Mismatch) {
        // A POISONED LOCK IS NOT WORTH A PANIC. This runs inside a TLS
        // handshake: losing the record costs a report, panicking costs the
        // process the operator pressed a key to run.
        let Ok(mut seen) = self.seen.lock() else {
            return;
        };
        let spoken = seen.as_ref().is_some_and(|(_, spoken)| *spoken);
        *seen = Some((mismatch, spoken));
    }
    /// The mismatch nothing has spoken for yet, and never twice.
    pub fn unspoken(&self) -> Option<Mismatch> {
        let mut seen = self.seen.lock().ok()?;
        let (mismatch, spoken) = seen.as_mut()?;
        if *spoken {
            return None;
        }
        *spoken = true;
        Some(mismatch.clone())
    }
    /// Whether this process refused a handshake at all, spoken for or not. The
    /// status line reads this rather than consuming the report.
    pub fn refused(&self) -> Option<Mismatch> {
        let seen = self.seen.lock().ok()?;
        seen.as_ref().map(|(mismatch, _)| mismatch.clone())
    }
}

static RECORD: Record = Record::new();

pub(super) fn record(address: &str, expected: CertificatePin, presented: CertificatePin) {
    RECORD.keep(Mismatch {
        address: address.to_string(),
        expected,
        presented,
    });
}

/// The mismatch this process has not spoken for yet.
pub fn unspoken_mismatch() -> Option<Mismatch> {
    RECORD.unspoken()
}

/// The mismatch this process refused, however many times it has been read.
pub fn refused_mismatch() -> Option<Mismatch> {
    RECORD.refused()
}

/// What the handshake refusal says, verbatim, so the text rustls hands back and
/// the text the report carries are one sentence.
pub(super) fn refusal(
    address: &str,
    expected: CertificatePin,
    presented: CertificatePin,
) -> String {
    format!("the bridge at {address} presented {presented}, not the pinned {expected}")
}

/// What the operator is told, and what to do about it. BOTH FINGERPRINTS,
/// because the next move is to enroll again and compare.
pub fn report(mismatch: &Mismatch) -> String {
    format!(
        "the bridge at {} presented a certificate lights is not pinned to, so every \
call is refused\nexpected:  {}\npresented: {}\nfix: run `lights enroll --bridge-id <id>`, \
check the name it prints is the bridge you expect, and save the line it prints onto the \
vault entry the config reads [controller] certificate from",
        mismatch.address, mismatch.expected, mismatch.presented
    )
}

#[cfg(test)]
#[path = "mismatch/tests.rs"]
mod tests;
