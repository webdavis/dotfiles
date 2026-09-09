//! `tailnet-pin <fqdn> <ip> <short>`: converge ONE `/etc/hosts` record for a
//! MagicDNS fallback pin, as root.
//!
//! A PIN IS THE FALLBACK that answers for a tailnet name when MagicDNS is
//! unavailable, so it must be exactly right or not there at all: a stale pin
//! resolves confidently to the wrong host, which is worse than no pin.
//!
//! IT RUNS AS ROOT, once per pin, from the system-setup runner. Every refusal
//! goes to stderr with a non-zero status and changes nothing, because a run
//! that cannot be certain is a run that must not rewrite this file.

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

use tailnet_pin_adapters::{PathFault, RealHostsFile};
use tailnet_pin_application::{Outcome, Refusal, reconcile};
use tailnet_pin_domain::Pin;

mod seam;

const USAGE: &str = "usage: tailnet-pin <fqdn> <ip> <short>";

/// A refusal, and a run that could not be certain, both exit here.
const REFUSED: u8 = 1;

/// Argv this does not serve.
const USAGE_FAULT: u8 = 2;

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    let [fqdn, ip, short] = arguments.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(USAGE_FAULT);
    };

    let path = match seam::hosts_file_path() {
        Ok(path) => path,
        Err(refusal) => return refuse(&refusal),
    };

    // THE FIELDS ARE CHECKED BEFORE THE FILE IS OPENED, so a pin that could
    // never be written costs no read of a file it will not touch.
    let pin = match Pin::new(fqdn.as_bytes(), ip.as_bytes(), short.as_bytes()) {
        Ok(pin) => pin,
        Err(field) => {
            return refuse(&format!(
                "refusing to edit {}: pin {} is not a single hosts column",
                path.display(),
                field.as_str()
            ));
        }
    };

    let file = match RealHostsFile::open(&path) {
        Ok(file) => file,
        Err(PathFault::UnresolvedSymlink) => {
            return refuse(&format!(
                "refusing to edit {} for {}: its symlink chain does not resolve",
                path.display(),
                name(&pin)
            ));
        }
        Err(PathFault::NotAFile) => {
            return refuse(&format!(
                "refusing to edit {} for {}: it is not a regular file",
                path.display(),
                name(&pin)
            ));
        }
    };

    report(&file, &pin, reconcile(&file, &pin))
}

fn report(file: &RealHostsFile, pin: &Pin, outcome: Outcome) -> ExitCode {
    let where_it_is = file.description();
    match outcome {
        Outcome::Converged => {
            println!(
                "tailnet-pin: MagicDNS fallback pin {} already converged in {where_it_is}",
                name(pin)
            );
            ExitCode::SUCCESS
        }
        Outcome::Written => {
            println!(
                "tailnet-pin: MagicDNS fallback pin {} written to {where_it_is}",
                name(pin)
            );
            ExitCode::SUCCESS
        }
        Outcome::Refused(Refusal::Unreadable) => refuse(&format!(
            "refusing to edit {where_it_is} for {}: it could not be read, and an unreadable hosts file is not an empty one",
            name(pin)
        )),
        Outcome::Refused(Refusal::LostLoopback) => refuse(&format!(
            "refusing to rewrite {where_it_is} for {}: the filtered result lost its loopback entry (no line this rebuild KEEPS maps 127.0.0.1 to a name written before a #, so nothing is left for localhost to resolve through)",
            name(pin)
        )),
        Outcome::Refused(Refusal::NotInstalled(reason)) => refuse(&format!(
            "refusing to report success for {}: installing the rebuilt {where_it_is} failed: {reason}",
            name(pin)
        )),
    }
}

fn refuse(message: &str) -> ExitCode {
    eprintln!("tailnet-pin: {message}");
    ExitCode::from(REFUSED)
}

/// The pin's name as a message writes it. A name that is not UTF-8 was already
/// refused as a hosts column, so nothing lossy reaches a message that matters.
fn name(pin: &Pin) -> String {
    String::from_utf8_lossy(pin.fqdn()).into_owned()
}
