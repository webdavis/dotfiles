//! `pns lights enroll`: what the bridge presents, and the line to save.
//!
//! IT PRINTS AND NEVER WRITES. The deployed config is a chezmoi target, so a
//! command that edited it would be reverted by the next apply; the value belongs
//! in the vault entry the config reads its bridge and key from.
//!
//! THE IDENTITY CHECK IS THE ONE THING THIS COMMAND IS FOR BESIDES THE
//! FINGERPRINT. A pin taken from whatever answered the address pins whatever
//! answered the address, so enrollment compares the certificate's common name
//! against the bridge id the same host reports and against the one the operator
//! read off the device, when they pass it.

use crate::*;

pub(crate) const ENROLL_USAGE: &str = "pns: usage: pns lights enroll [--bridge-id <id>]; prints the certificate \
line to save and writes nothing";

/// Read the bridge's certificate, and print the line to save.
pub(crate) fn lights_enroll() -> i32 {
    let arguments: Vec<String> = crate::arguments_after_verb();
    let stated_id = match arguments.as_slice() {
        [] => None,
        [flag, id] if flag == "--bridge-id" => Some(id.clone()),
        _ => {
            eprintln!("{ENROLL_USAGE}");
            return 2;
        }
    };
    let home = std::env::var("HOME").unwrap_or_default();
    let Some(address) = bridge_address(&home) else {
        eprintln!(
            "pns: no [plugins.hue] bridge in the config, so there is no address to enroll \
against"
        );
        return 2;
    };
    let enrollment = match pns_adapters::enroll(&address, pns_adapters::BRIDGE_DEADLINE) {
        Ok(enrollment) => enrollment,
        Err(refusal) => {
            eprintln!("{refusal}");
            return 1;
        }
    };
    let paint = pns_adapters::style::Paint::for_stdout();
    for line in pns_adapters::style::header(
        paint,
        "pns lights enroll",
        &[
            pns_adapters::style::HeaderLine {
                label: "Writes",
                text: "nothing; the line below is yours to save",
            },
            pns_adapters::style::HeaderLine {
                label: "Address",
                text: &address,
            },
        ],
    ) {
        println!("{line}");
    }
    for line in readings(&enrollment) {
        println!("{line}");
    }
    match verdict(&enrollment, stated_id.as_deref()) {
        Ok(warning) => {
            if let Some(warning) = warning {
                println!("{}", paint.accent(&warning));
            }
            println!();
            println!("certificate = \"{}\"", enrollment.pin);
            0
        }
        Err(refusal) => {
            eprintln!("{refusal}");
            1
        }
    }
}

/// What was read, as lines, so the refusal and the acceptance print the same
/// readings.
fn readings(enrollment: &pns_adapters::Enrollment) -> Vec<String> {
    vec![
        format!("certificate common name: {}", enrollment.common_name),
        format!(
            "bridge id it reports:    {}",
            enrollment.reported_id.as_deref().unwrap_or("none")
        ),
        format!(
            "model it reports:        {}",
            enrollment.model.as_deref().unwrap_or("none")
        ),
        format!("fingerprint:             {}", enrollment.pin),
    ]
}

/// Whether this enrollment may be trusted, and what it costs when the identity
/// was not checked.
///
/// THREE ANSWERS, and the middle one is the operator's own choice. An id read
/// off the device's label closes the enrollment-time impostor window; the
/// bridge's own answer only makes it harder, because whoever is answering for
/// the address answers that too. So a disagreement REFUSES, a bridge that
/// agrees with itself WARNS, and only an out-of-band id passes clean.
fn verdict(
    enrollment: &pns_adapters::Enrollment,
    stated_id: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(stated) = stated_id {
        if !stated.eq_ignore_ascii_case(&enrollment.common_name) {
            return Err(format!(
                "pns: the certificate names {}, not the {stated} you gave; nothing to \
enroll, because whatever answered this address is not the bridge you read off the label",
                enrollment.common_name
            ));
        }
        return Ok(None);
    }
    if !enrollment.identities_agree() {
        return Err(format!(
            "pns: the certificate names {} and the host reports {}; nothing to enroll, \
because a bridge does not disagree with itself and an impostor does",
            enrollment.common_name,
            enrollment.reported_id.as_deref().unwrap_or("nothing")
        ));
    }
    Ok(Some(
        "warning: the identity was not checked out of band, so this enrolls whatever \
answered the address. An impostor present right now would be pinned and everything \
afterwards would look correct. Read the bridge id off the device and rerun with \
--bridge-id <id> to close that."
            .to_string(),
    ))
}

/// The configured bridge address, or None when no table names one.
fn bridge_address(home: &str) -> Option<String> {
    let LoadOutcome::Loaded(config) = load_config(&config_path(home)).ok()? else {
        return None;
    };
    pns_adapters::enabled_hue_table(&config)?
        .get("bridge")?
        .as_str()
        .filter(|address| !address.is_empty())
        .map(String::from)
}

#[cfg(test)]
#[path = "command_enroll/tests.rs"]
mod tests;
