//! `[notify] mode = "off"`: the page is raised on the local banner and posted
//! nowhere.
//!
//! OFF TURNS OFF DELIVERY, NOT THE PAGE. A security tool that answers a config
//! word by discarding findings would advance its cursor over a finding nobody
//! ever saw, which is the one failure this pipeline is built to refuse. The
//! banner needs no gateway, no key and no engine, so it is what remains when
//! every way off the machine is switched off, and it is the whole delivery
//! here rather than a fallback after one failed.

use posture_application::{Alert, AlertSink, IndependentAlarm, Submission, SubmissionFailure};

pub struct BannerOnly<A> {
    alarm: A,
}

impl<A: IndependentAlarm> BannerOnly<A> {
    pub fn new(alarm: A) -> Self {
        Self { alarm }
    }
}

impl<A: IndependentAlarm> AlertSink for BannerOnly<A> {
    /// The banner is the destination, so a banner that fired is an acceptance
    /// and the caller's state may advance. A banner that did not leaves the
    /// finding unacknowledged, exactly as an undelivered post does.
    fn submit(&mut self, alert: &Alert) -> Submission {
        match self.alarm.alarm(&alert.title, &alert.detail) {
            Ok(()) => Submission::Accepted,
            Err(_) => Submission::NotAccepted(SubmissionFailure::Failed),
        }
    }
}

#[cfg(test)]
mod tests;
