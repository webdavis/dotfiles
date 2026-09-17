//! The direct path: posture signs one page and posts it to a hermes webhook
//! route itself, with one key per route.
//!
//! ONE KEY PER ROUTE, AND A KEYLESS ROUTE IS REFUSED. The gateway reads a
//! route's secret as a literal, so a page signed with another route's key
//! answers 401 and vanishes. A route this machine holds no key for therefore
//! refuses the page and raises the local banner, exactly as a broken engine
//! does: a security finding that went nowhere is never reported as delivered.
//!
//! THE BODY SERVES EVERY PROMPT SHAPE the gateway's routes are written in.
//! Hermes renders `{placeholder}` out of the posted JSON and emits an unknown
//! one as its own literal text, so a body missing a key the route names
//! delivers that key's braces to Discord. A route is retemplated on the
//! gateway rather than here, so the body carries all three shapes at once and
//! a retemplated route keeps rendering. See `body` for the list.

mod body;
mod window;

use crate::request_id;
use crate::signed_post::{PostOutcome, SignedPost, delivered, sign};
use crate::sink::{delivery_failed, tier_route};
use crate::wire::Name;
use posture_application::{Alert, AlertSink, IndependentAlarm, Submission, SubmissionFailure};
use posture_domain::Severity;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

/// What one page's post may take. It matches the budget every caller gives the
/// producer path, because the caller waits the same way for either.
const POST_DEADLINE: Duration = Duration::from_secs(5);

/// The title the local banner carries when a page could not be posted at all.
const ALARM_TITLE: &str = "Posture page could not be delivered";

/// The title the local banner carries when the page was delivered but its copy
/// could not be posted. SEPARATE FROM THE PAGE'S OWN TITLE, because the
/// finding did reach the operator and a reader who cannot tell the two apart
/// would go looking for a page that is already in the channel.
const COPY_ALARM_TITLE: &str = "Posture page copy could not be posted";

/// How many characters of one finding the combined message lists. Enough to
/// tell two findings apart, short enough that a storm's list stays one
/// readable message.
const LISTED_FINDING_CHARACTERS: usize = 120;

/// A second route a CRITICAL page is copied to, verbatim, once its own post
/// came back delivered, and the file the rolling hour of those copies is
/// recorded in.
///
/// THIS IS A DESTINATION, NOT A FEATURE. posture holds route names and a key
/// for each; what reads a route is the far side's business and is named
/// nowhere in this tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriticalCopy {
    pub route: Name,
    pub window: PathBuf,
}

pub struct HermesWebhook<P, A> {
    post: P,
    base_url: String,
    keys: BTreeMap<String, String>,
    route: Name,
    copy: Option<CriticalCopy>,
    alarm: A,
}

impl<P: SignedPost, A: IndependentAlarm> HermesWebhook<P, A> {
    pub fn new(
        post: P,
        base_url: String,
        keys: BTreeMap<String, String>,
        route: Name,
        alarm: A,
    ) -> Self {
        Self {
            post,
            base_url,
            keys,
            route,
            copy: None,
            alarm,
        }
    }

    /// The same sink, copying every delivered critical page to `copy`'s route.
    /// A sink built without this posts one page and nothing else, which is the
    /// behavior of every machine that names no copy route.
    pub fn copying(mut self, copy: CriticalCopy) -> Self {
        self.copy = Some(copy);
        self
    }

    /// `<base>/<route>`, which is how the gateway addresses one route.
    fn url(&self, route: &Name) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), route.as_str())
    }
}

impl<P: SignedPost, A: IndependentAlarm> AlertSink for HermesWebhook<P, A> {
    fn submit(&mut self, alert: &Alert) -> Submission {
        let route = tier_route(alert).unwrap_or_else(|| self.route.clone());
        let body = body::encode(alert, &route);
        let page_id = request_id::derive(&request_id::seed(alert));
        // A ROUTE WITH NO KEY REFUSES THE PAGE AND SAYS SO. An unsigned post
        // is rejected by the gateway, and an empty key is the not-set-up case
        // rather than a signature, so both land here.
        let Some(signature) = self
            .keys
            .get(route.as_str())
            .and_then(|key| sign(key, &body))
        else {
            return delivery_failed(
                &mut self.alarm,
                ALARM_TITLE,
                alert,
                SubmissionFailure::Refused,
            );
        };
        let outcome = self.post.post(
            &self.url(&route),
            &body,
            &signature,
            &page_id,
            Some(POST_DEADLINE),
        );
        if delivered(outcome) {
            self.copy_delivered_page(alert, &body, &page_id);
            return Submission::Accepted;
        }
        delivery_failed(
            &mut self.alarm,
            ALARM_TITLE,
            alert,
            match outcome {
                // The gateway answered, so the request was made and refused:
                // a wrong key, a route the gateway does not serve, or a
                // body it would not take. All three are a config to fix.
                PostOutcome::Status(_) | PostOutcome::NoStatus => SubmissionFailure::Failed,
                PostOutcome::NoResponse => SubmissionFailure::Unavailable,
            },
        )
    }
}

impl<P: SignedPost, A: IndependentAlarm> HermesWebhook<P, A> {
    /// Post the second copy a delivered critical page earns, and say so on the
    /// local banner when it is withheld or refused.
    ///
    /// THE PAGE'S OWN OUTCOME IS ALREADY SETTLED. Everything here happens
    /// after the gateway took the page, so no branch of it may answer the
    /// caller: a copy that could not be posted is a configuration to fix, not
    /// a finding to re-deliver.
    fn copy_delivered_page(&mut self, alert: &Alert, body: &str, page_id: &str) {
        let Some(copy) = self.copy.clone() else {
            return;
        };
        if alert.severity != Some(Severity::Critical) {
            return;
        }
        // A KEYLESS COPY ROUTE IS NOT A SILENT ONE. An unsigned post is
        // refused by the gateway and an empty key is the not-set-up case, so
        // both mean this leg is configured and cannot run, which is exactly
        // the state that would otherwise switch the feature off unnoticed.
        // Checked before the hour is claimed, so a leg that cannot post
        // spends none of it.
        let Some(key) = self
            .keys
            .get(copy.route.as_str())
            .filter(|key| !key.is_empty())
            .cloned()
        else {
            self.report_copy(COPY_ALARM_TITLE, alert);
            return;
        };
        // THE ID IS DISTINCT FROM THE PAGE'S EITHER WAY. The gateway's
        // duplicate cache is keyed on the delivery id alone across every
        // route, so a second post sharing the page's id would be read as the
        // page arriving twice and dropped.
        let (payload, request_id) = match window::claim(
            &copy.window,
            &finding_key(alert),
            &listed_finding(alert),
            window::now(),
        ) {
            window::Claim::Granted => (body.to_string(), request_id::derive_copy(page_id)),
            // A REPEAT IS NOT A REFUSAL. This finding is already in the hour,
            // so there is nothing to tell anyone. Neither is a finding that
            // arrives after the storm message: the message said the machine
            // is in trouble and the finding's own page is in its channel.
            window::Claim::AlreadyCopied | window::Claim::Storming => return,
            // ONE MESSAGE FOR THE WHOLE HOUR. Past the threshold the useful
            // message is that the machine is in trouble, with the findings
            // listed, rather than the next finding explained on its own.
            window::Claim::Storm(findings) => (
                body::encode_storm(&findings, &copy.route),
                request_id::derive_storm(page_id),
            ),
        };
        let Some(signature) = sign(&key, &payload) else {
            self.report_copy(COPY_ALARM_TITLE, alert);
            return;
        };
        let outcome = self.post.post(
            &self.url(&copy.route),
            &payload,
            &signature,
            &request_id,
            Some(POST_DEADLINE),
        );
        if !delivered(outcome) {
            self.report_copy(COPY_ALARM_TITLE, alert);
        }
    }

    /// Say on the local banner what happened to one page's copy.
    fn report_copy(&mut self, title: &str, alert: &Alert) {
        let _ = self
            .alarm
            .alarm(title, &format!("{}\n{}", alert.title, alert.detail));
    }
}

/// What makes one finding the same finding an hour later: WHAT IT SAYS, not
/// which batch of log said it.
///
/// A page's `occurrence_id` is the byte range it was judged from, so two
/// batches reporting one unchanged finding carry different occurrences and
/// counting those would count pages. The digest folds its own repeats by
/// content for the same reason. Hashed rather than stored whole, so identity
/// is content-based and the key stays a fixed width regardless of what the
/// finding says; the bounded label stored beside it is `listed_finding`, not
/// this key.
fn finding_key(alert: &Alert) -> String {
    request_id::derive(&format!("{}:{}:{}", alert.event, alert.title, alert.detail))
}

/// The one line a combined message lists this finding as: what it says, on one
/// line, bounded. Stored in the window file beside the finding's key, because
/// the process that sends the combined message is not the one that saw the
/// findings before it.
fn listed_finding(alert: &Alert) -> String {
    let first_line = alert.detail.lines().next().unwrap_or_default();
    let line = format!("{}: {first_line}", alert.title);
    match line.char_indices().nth(LISTED_FINDING_CHARACTERS) {
        Some((at, _)) => format!("{}...", &line[..at]),
        None => line,
    }
}

#[cfg(test)]
mod tests;
