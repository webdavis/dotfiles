//! The second post a DELIVERED critical page earns, and every reason it is
//! not made.

use super::*;

/// A sink that copies a critical page to the `explain` route, with its hour
/// recorded inside `sandbox` so one test's window is never another's.
fn copying(
    keys: BTreeMap<String, String>,
    outcome: PostOutcome,
    sandbox: &crate::test_sandbox::Sandbox,
) -> HermesWebhook<Posts, Alarm> {
    subject(keys, outcome).copying(CriticalCopy {
        route: Name::new("explain").unwrap(),
        window: sandbox.path().join("state/critical-copy-window.json"),
    })
}

#[test]
fn a_delivered_critical_page_is_copied_once_to_the_copy_route_signed_with_its_key() {
    let sandbox = crate::test_sandbox::Sandbox::new("critical-copy");
    let mut sut = copying(keys(), PostOutcome::Status(204), &sandbox);
    assert_eq!(
        sut.submit(&alert(Some(Severity::Critical))),
        Submission::Accepted
    );
    let sent = sut.post.sent.borrow();
    assert_eq!(sent.len(), 2, "the page and one copy");
    assert_eq!(sent[0].url, "http://127.0.0.1:8644/webhooks/priority");
    assert_eq!(sent[1].url, "http://127.0.0.1:8644/webhooks/explain");
    assert_eq!(sent[1].body, sent[0].body, "the copy is the page verbatim");
    assert_eq!(
        sent[1].signature,
        sign("key-explain", &sent[1].body).unwrap()
    );
    assert_ne!(sent[1].signature, sent[0].signature);
    assert!(sut.alarm.calls.is_empty());
}

#[test]
fn a_page_below_the_critical_tier_is_never_copied() {
    let sandbox = crate::test_sandbox::Sandbox::new("critical-copy");
    for severity in [Some(Severity::Notice), Some(Severity::Info), None] {
        let mut sut = copying(keys(), PostOutcome::Status(204), &sandbox);
        assert_eq!(sut.submit(&alert(severity)), Submission::Accepted);
        assert_eq!(sut.post.sent.borrow().len(), 1, "{severity:?}");
        assert!(sut.alarm.calls.is_empty(), "{severity:?}");
    }
}

#[test]
fn a_critical_page_whose_own_post_failed_is_not_copied_and_fails_exactly_as_before() {
    let sandbox = crate::test_sandbox::Sandbox::new("critical-copy");
    for (outcome, failure) in [
        (PostOutcome::Status(401), SubmissionFailure::Failed),
        (PostOutcome::NoStatus, SubmissionFailure::Failed),
        (PostOutcome::NoResponse, SubmissionFailure::Unavailable),
    ] {
        let mut sut = copying(keys(), outcome, &sandbox);
        assert_eq!(
            sut.submit(&alert(Some(Severity::Critical))),
            Submission::NotAccepted(failure)
        );
        assert_eq!(sut.post.sent.borrow().len(), 1, "only the page was tried");
        assert_eq!(
            sut.alarm.calls,
            vec![(
                ALARM_TITLE.to_string(),
                "Security finding\nline one".to_string()
            )]
        );
    }
}

#[test]
fn a_copy_route_this_machine_holds_no_key_for_says_so_and_leaves_the_page_delivered() {
    let sandbox = crate::test_sandbox::Sandbox::new("critical-copy");
    for held in [
        BTreeMap::from([("priority".to_string(), "key-priority".to_string())]),
        BTreeMap::from([
            ("priority".to_string(), "key-priority".to_string()),
            ("explain".to_string(), String::new()),
        ]),
    ] {
        let mut sut = copying(held, PostOutcome::Status(204), &sandbox);
        assert_eq!(
            sut.submit(&alert(Some(Severity::Critical))),
            Submission::Accepted,
            "the page was delivered and stays delivered"
        );
        assert_eq!(sut.post.sent.borrow().len(), 1, "no copy is attempted");
        assert_eq!(
            sut.alarm.calls,
            vec![(
                COPY_ALARM_TITLE.to_string(),
                "Security finding\nline one".to_string()
            )],
            "a copy leg that is configured but cannot post must not be silent"
        );
    }
}

#[test]
fn a_copy_the_gateway_refused_says_so_and_still_leaves_the_page_delivered() {
    let sandbox = crate::test_sandbox::Sandbox::new("critical-copy");
    // The page's own post has to succeed for the copy to be attempted at all,
    // so the double answers the page and refuses everything after it.
    let mut sut = copying(keys(), PostOutcome::Status(204), &sandbox);
    sut.post.after_first = Some(PostOutcome::Status(404));
    assert_eq!(
        sut.submit(&alert(Some(Severity::Critical))),
        Submission::Accepted,
        "the page itself was taken"
    );
    assert_eq!(sut.post.sent.borrow().len(), 2, "the copy was attempted");
    assert_eq!(
        sut.alarm.calls,
        vec![(
            COPY_ALARM_TITLE.to_string(),
            "Security finding\nline one".to_string()
        )]
    );
}

/// One critical page per distinct finding, told apart by its detail.
fn finding(detail: &str) -> Alert {
    let mut page = alert(Some(Severity::Critical));
    page.detail = detail.into();
    page
}

#[test]
fn a_storm_gets_one_combined_message_listing_every_finding_instead_of_one_each() {
    let sandbox = crate::test_sandbox::Sandbox::new("critical-copy");
    let mut sut = copying(keys(), PostOutcome::Status(204), &sandbox);
    for index in 0..window::STORM_THRESHOLD {
        assert_eq!(
            sut.submit(&finding(&format!("finding {index}"))),
            Submission::Accepted
        );
    }
    assert_eq!(
        sut.post.sent.borrow().len(),
        window::STORM_THRESHOLD * 2,
        "inside the threshold every finding is explained on its own"
    );

    assert_eq!(
        sut.submit(&finding("the finding that tipped it")),
        Submission::Accepted
    );
    let sent = sut.post.sent.borrow();
    assert_eq!(
        sent.len(),
        window::STORM_THRESHOLD * 2 + 2,
        "the crossing page, and one message for the whole hour"
    );
    let storm = sent.last().expect("the combined message");
    assert_eq!(storm.url, "http://127.0.0.1:8644/webhooks/explain");
    assert_eq!(
        storm.signature,
        sign("key-explain", &storm.body).unwrap(),
        "signed with the copy route's own key"
    );
    let body: serde_json::Value = serde_json::from_str(&storm.body).expect("a JSON body");
    assert_eq!(
        body["header"],
        serde_json::json!("6 distinct critical findings in one hour")
    );
    let listed = body["body"].as_str().expect("the list");
    for index in 0..window::STORM_THRESHOLD {
        assert!(
            listed.contains(&format!("Security finding: finding {index}")),
            "every finding of the hour is listed, missing {index}: {listed}"
        );
    }
    assert!(listed.contains("Security finding: the finding that tipped it"));
    assert_ne!(storm.request_id, sent[sent.len() - 2].request_id);
    assert!(sut.alarm.calls.is_empty());
}

#[test]
fn a_finding_after_the_combined_message_is_still_paged_and_is_not_explained_again() {
    let sandbox = crate::test_sandbox::Sandbox::new("critical-copy");
    let mut sut = copying(keys(), PostOutcome::Status(204), &sandbox);
    for index in 0..=window::STORM_THRESHOLD {
        assert_eq!(
            sut.submit(&finding(&format!("finding {index}"))),
            Submission::Accepted
        );
    }
    let before = sut.post.sent.borrow().len();
    for index in 0..4 {
        assert_eq!(
            sut.submit(&finding(&format!("a later finding {index}"))),
            Submission::Accepted,
            "the page itself is unaffected"
        );
    }
    assert_eq!(
        sut.post.sent.borrow().len(),
        before + 4,
        "four pages and nothing else: the hour's one message already said so"
    );
    assert!(sut.alarm.calls.is_empty());
}

#[test]
fn repeats_of_one_finding_spend_no_budget_and_crowd_out_no_other_findings_copy() {
    let sandbox = crate::test_sandbox::Sandbox::new("critical-copy");
    let mut sut = copying(keys(), PostOutcome::Status(204), &sandbox);
    for _ in 0..7 {
        assert_eq!(
            sut.submit(&alert(Some(Severity::Critical))),
            Submission::Accepted
        );
    }
    assert_eq!(
        sut.post.sent.borrow().len(),
        8,
        "seven pages and the one copy the first occurrence earned"
    );
    // The rest of the threshold still fits, so the repeats cost none of the
    // hour's budget.
    for index in 0..window::STORM_THRESHOLD - 1 {
        let mut page = alert(Some(Severity::Critical));
        page.detail = format!("a different finding {index}");
        assert_eq!(sut.submit(&page), Submission::Accepted);
    }
    assert_eq!(
        sut.post.sent.borrow().len(),
        8 + (window::STORM_THRESHOLD - 1) * 2
    );
    assert!(sut.alarm.calls.is_empty());
}

#[test]
fn a_sink_with_no_copy_route_posts_the_critical_page_once_and_nothing_else() {
    let mut sut = subject(keys(), PostOutcome::Status(204));
    assert_eq!(
        sut.submit(&alert(Some(Severity::Critical))),
        Submission::Accepted
    );
    assert_eq!(sut.post.sent.borrow().len(), 1);
    assert!(sut.alarm.calls.is_empty());
}

#[test]
fn every_post_carries_a_request_id_and_a_page_and_its_copy_carry_different_ones() {
    let sandbox = crate::test_sandbox::Sandbox::new("critical-copy");
    let mut sut = copying(keys(), PostOutcome::Status(204), &sandbox);
    assert_eq!(
        sut.submit(&alert(Some(Severity::Critical))),
        Submission::Accepted
    );
    let sent = sut.post.sent.borrow();
    // The page's id is the one the producer path would have put on the wire for
    // the same occurrence, so a retry of this page is recognized as a repeat.
    assert_eq!(
        sent[0].request_id,
        "posture-d28d5af268c004d795ce0240f35f5218"
    );
    assert!(sent[1].request_id.starts_with("posture-"));
    assert_ne!(
        sent[1].request_id, sent[0].request_id,
        "the gateway's duplicate cache is keyed on the id alone across routes, \
         so a shared id would swallow the copy for an hour"
    );
}
