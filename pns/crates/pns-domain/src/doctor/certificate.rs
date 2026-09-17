//! What the doctor says about the bridge's pinned certificate.
//!
//! IT REPORTS AND DOES NOT ENROLL. Doctor states what is true; producing a pin
//! is `pns lights enroll`'s job, and a diagnostic that sometimes changes things
//! becomes one nobody runs.

use super::{Item, Mark};
use crate::CertificatePin;

/// What this process learned about the pin while it ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinState {
    /// No `[plugins.hue]` table with a bridge, so no pin was read.
    Unconfigured,
    /// A pin is configured and no handshake was refused.
    Held,
    /// A pin is configured and a handshake was refused, so the bridge at that
    /// address is not the device this machine trusts.
    Mismatched {
        address: String,
        expected: CertificatePin,
        presented: CertificatePin,
    },
}

/// The pin row.
///
/// A MISMATCH IS AN ISSUE, not a warning, and it is the one thing in the lights
/// section that moves the exit code. Everything else here reports a dark lamp,
/// which is not a broken notifier; this reports that something other than the
/// operator's bridge is answering its address, which is the report's whole
/// reason to name a fingerprint at all.
pub fn certificate_row(state: &PinState) -> Item {
    match state {
        PinState::Unconfigured => Item::note(
            "certificate: no [plugins.hue] bridge, so no certificate is pinned".to_string(),
        ),
        PinState::Held => Item::row(
            Mark::Good,
            "certificate: pinned, and no handshake was refused".to_string(),
        ),
        PinState::Mismatched {
            address,
            expected,
            presented,
        } => Item::row(
            Mark::Bad,
            format!(
                "certificate: the bridge at {address} presented {presented}, not the pinned \
{expected}; every lamp call is refused. Run `pns lights enroll`, check the printed common name \
is the bridge you expect, and save the line it prints"
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(last: &str) -> CertificatePin {
        CertificatePin::parse(&format!("sha256:{}{last}", "0".repeat(62)))
            .expect("a well formed pin")
    }

    #[test]
    fn a_mismatch_is_an_issue_naming_both_fingerprints_and_the_enrollment_command() {
        let item = certificate_row(&PinState::Mismatched {
            address: "192.0.2.1".to_string(),
            expected: pin("01"),
            presented: pin("02"),
        });
        assert!(matches!(
            item,
            Item::Row {
                mark: Mark::Bad,
                ..
            }
        ));
        let text = item.text();
        assert!(text.contains(&pin("01").to_string()), "{text}");
        assert!(text.contains(&pin("02").to_string()), "{text}");
        assert!(text.contains("pns lights enroll"), "{text}");
    }

    #[test]
    fn a_pin_nothing_refused_is_good_and_no_bridge_at_all_is_a_reading() {
        assert!(matches!(
            certificate_row(&PinState::Held),
            Item::Row {
                mark: Mark::Good,
                ..
            }
        ));
        assert!(matches!(
            certificate_row(&PinState::Unconfigured),
            Item::Row {
                mark: Mark::Note,
                ..
            }
        ));
    }
}
