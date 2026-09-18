//! `lights enroll`: what the bridge presents, and the line to save.
//!
//! IT PRINTS AND NEVER WRITES. The deployed config is a chezmoi target, so a
//! command that edited it would be reverted by the next apply; the value
//! belongs on the vault entry the config reads its bridge and key from.
//!
//! THE IDENTITY CHECK IS WHAT THIS COMMAND IS FOR, as much as the fingerprint.
//! A pin taken from whatever answered the address pins whatever answered the
//! address, so the certificate's common name is checked against the id the
//! operator read off the device when they pass one, and against the id the same
//! host reports when they do not.

use crate::Response;
use lights_adapters::settings::Endpoint;
use std::time::Duration;

/// What the operator gives up by skipping the out-of-band check, said in full
/// because the cost is invisible afterwards: everything looks correct.
const UNCHECKED: &str = "warning: the id was not checked against the device, so this enrolls \
whatever answered the address. An impostor present right now would be pinned and every call \
after it would look correct. Read the bridge id off the device and rerun with --bridge-id <id> \
to close that.";

pub(super) fn run(endpoint: &Endpoint, stated_id: Option<&str>) -> Response {
    let enrollment = match lights_adapters::enroll(
        &endpoint.address,
        Duration::from_secs(endpoint.timeout_secs),
    ) {
        Ok(enrollment) => enrollment,
        Err(refusal) => return crate::failure(4, &refusal),
    };
    let readings = format!(
        "address:                 {}\ncertificate common name: {}\nbridge id it reports:    \
{}\nmodel it reports:        {}\n",
        endpoint.address,
        enrollment.common_name,
        enrollment.reported_id.as_deref().unwrap_or("none"),
        enrollment.model.as_deref().unwrap_or("none"),
    );
    match verdict(&enrollment, stated_id) {
        Err(refusal) => Response {
            exit: 4,
            stdout: readings,
            stderr: format!("lights: {refusal}\n"),
        },
        Ok(warning) => Response {
            exit: 0,
            stdout: format!(
                "{readings}\nsave this line on the vault entry the config reads the bridge \
from:\ncertificate = \"{}\"\n",
                enrollment.pin
            ),
            stderr: warning
                .map(|text| format!("lights: {text}\n"))
                .unwrap_or_default(),
        },
    }
}

/// Whether this enrollment may be trusted, and what it costs when the identity
/// was not checked out of band.
///
/// THREE ANSWERS. An id read off the device closes the enrollment-time impostor
/// window; the host's own answer only raises its price, because whoever answers
/// for the address answers that too. So a disagreement REFUSES, a host that
/// agrees with itself WARNS, and only an out-of-band id passes clean.
fn verdict(
    enrollment: &lights_adapters::Enrollment,
    stated_id: Option<&str>,
) -> Result<Option<&'static str>, String> {
    if let Some(stated) = stated_id {
        if !stated.eq_ignore_ascii_case(&enrollment.common_name) {
            return Err(format!(
                "the certificate names {}, not the {stated} you gave; nothing to enroll, \
because whatever answered this address is not the bridge you read off the label",
                enrollment.common_name
            ));
        }
        return Ok(None);
    }
    if !enrollment.identities_agree() {
        return Err(format!(
            "the certificate names {} and the host reports {}; nothing to enroll, because a \
bridge does not disagree with itself and an impostor does",
            enrollment.common_name,
            enrollment.reported_id.as_deref().unwrap_or("nothing")
        ));
    }
    Ok(Some(UNCHECKED))
}

#[cfg(test)]
#[path = "enroll/tests.rs"]
mod tests;
