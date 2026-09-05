//! The two outbound boundaries: an alert through the pns engine, and the
//! weekly record posted to the hermes gateway.
//!
//! BOTH ARRIVE AS TRAITS, never the concrete client: a refused delivery can
//! only be exercised through a real socket failure otherwise, and the alert it
//! fires is invisible to anything but a real engine.

use std::process::Command;
use std::time::Duration;

use pns::channels::hermes::{SignedPost, delivered, outcome_line, sign};
use unattended_upgrades::alert::{Alerter, alert_argv};
use unattended_upgrades::config::Records;
use unattended_upgrades::record::AGENT;

use crate::system::host;

/// How long one signed record POST may take. Nobody is waiting on the answer,
/// so this only bounds how long the job lingers on a gateway that stopped
/// listening; it matches the deadline pns gives its own unwatched posts.
const RECORD_DEADLINE: Duration = Duration::from_secs(10);

/// The pns engine, as a client: flags in, nothing read back.
pub struct PnsAlerter;

impl Alerter for PnsAlerter {
    fn alert(&self, binary: &str, args: &[String]) -> Result<(), String> {
        match Command::new(binary).args(args).status() {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!("`{binary}` answered {status}")),
            Err(error) => Err(format!("`{binary}` could not be run: {error}")),
        }
    }
}

/// One alert, FAIL OPEN at every rung: no `[alerts]` block, an engine that is
/// not there, and an engine that refused are each stated here and none of them
/// ends the run.
///
/// ANSWERS WHETHER THE ALERT IS OWED ANY LONGER, which is not the same as
/// whether an engine ran. With no `[alerts]` block nothing was owed and the
/// log line IS the delivery, so that is `true`; only a configured engine that
/// refused leaves something still to say. The per-run failure alert ignores
/// this, because it fires again next run either way; the staleness alert
/// fires once per streak and has to know.
pub fn send_alert(alerter: &dyn Alerter, engine: Option<&str>, lane: &str, summary: &str) -> bool {
    let Some(binary) = engine else {
        println!("uu: no [alerts] block; `{lane}: {summary}` was logged and nothing else");
        return true;
    };
    let argv = alert_argv(&host(), lane, summary);
    if let Err(why) = alerter.alert(binary, &argv) {
        println!("uu: the alert for `{lane}` was NOT delivered ({why}); it is logged here instead");
        return false;
    }
    true
}

/// The record, posted in process, ANSWERING WHETHER IT LANDED. The caller
/// needs that verdict: an entry the gateway never received is a failed run,
/// whatever the lanes did.
///
/// FAIL LOUD: a refused delivery is printed AND alerted, because a silent
/// record channel is indistinguishable from a machine whose jobs stopped
/// running, which is the one failure the record cannot report about itself.
pub fn deliver_record(
    post: &dyn SignedPost,
    alerter: &dyn Alerter,
    records: &Records,
    body: String,
    engine: Option<&str>,
) -> bool {
    let Some(signature) = sign(&records.key, &body) else {
        println!("uu: the [records] key is empty, so nothing could be signed or posted");
        return false;
    };
    let outcome = post.post(&records.url, &body, &signature, Some(RECORD_DEADLINE));
    println!("uu: {}", outcome_line(outcome));
    if delivered(outcome) {
        return true;
    }
    send_alert(
        alerter,
        engine,
        AGENT,
        &format!(
            "the weekly record could NOT be delivered to {} ({}); until this is fixed that \
             channel is silent for a reason that has nothing to do with the jobs it reports on",
            records.url,
            outcome_line(outcome)
        ),
    );
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use pns::channels::hermes::PostOutcome;
    use std::cell::RefCell;

    /// A `SignedPost` stub that always answers the same fixed outcome. It
    /// never touches a socket, so both directions below run in well under a
    /// second and neither depends on a real gateway being up or down.
    struct AnswerWith(PostOutcome);

    impl SignedPost for AnswerWith {
        fn post(
            &self,
            _url: &str,
            _body: &str,
            _signature_hex: &str,
            _deadline: Option<Duration>,
        ) -> PostOutcome {
            self.0
        }
    }

    /// An `Alerter` that records every call instead of spawning anything, so
    /// a test can assert whether the alert path fired at all.
    #[derive(Default)]
    struct SpyAlerter {
        calls: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl Alerter for SpyAlerter {
        fn alert(&self, binary: &str, args: &[String]) -> Result<(), String> {
            self.calls
                .borrow_mut()
                .push((binary.to_string(), args.to_vec()));
            Ok(())
        }
    }

    fn stub_records() -> Records {
        Records {
            url: "http://127.0.0.1:0/wherever".to_string(),
            key: "k".to_string(),
        }
    }

    #[test]
    fn a_refused_post_reports_failure_and_alerts_through_the_given_alerter() {
        let spy = SpyAlerter::default();
        let delivered = deliver_record(
            &AnswerWith(PostOutcome::NoResponse),
            &spy,
            &stub_records(),
            "body".to_string(),
            Some("engine"),
        );
        assert!(!delivered);
        let calls = spy.calls.borrow();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(calls[0].0, "engine", "{calls:?}");
        assert!(
            calls[0]
                .1
                .iter()
                .any(|arg| arg.contains(&stub_records().url)),
            "{calls:?}"
        );
    }

    #[test]
    fn a_delivered_post_reports_success_and_never_touches_the_alerter() {
        let spy = SpyAlerter::default();
        let delivered = deliver_record(
            &AnswerWith(PostOutcome::Status(200)),
            &spy,
            &stub_records(),
            "body".to_string(),
            Some("engine"),
        );
        assert!(delivered);
        assert!(spy.calls.borrow().is_empty(), "{:?}", spy.calls.borrow());
    }
}
