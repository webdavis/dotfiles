use super::*;

#[test]
fn doctor_pairs_reordered_outcomes_by_name_and_prints_every_section_in_order() {
    let history = History::default();
    let (code, lines) = report(&history, sent(), Outcome::Signalled(1), Pairing::NoAnswer);
    assert_eq!(code, 0);
    assert_eq!(
        &lines[..2],
        [
            "alpha: sent, alpha receipt",
            "beta: sent, this channel reports no outcome"
        ]
    );
    assert!(lines[2].starts_with("hue: signalled 1 room"));
    assert!(lines[3].starts_with("room:"));
    assert_eq!(lines[4], "off: skipped, configured off");
    assert_eq!(lines[5], "pns doctor: 3 sent, 0 failed, 2 skipped");
    assert!(lines[6].starts_with("pns doctor: moshi pairing:"));
    assert_eq!(lines[7], "pns doctor: moshi says: fixture server");
    assert_eq!(&lines[8..10], ["focus fixture", "daemon fixture"]);
    assert!(lines[10].starts_with("pns doctor: the nag "));
    assert!(lines[11].starts_with("pns doctor: lights:"));
    // The ledger, then the routes it would post to, then history. The fixture
    // has posted to no route, so the route section is its summary alone.
    assert!(lines[13].contains("no routes to check"));
    assert!(lines[14].contains("decision"));
    assert!(lines.last().unwrap().contains("missed"));
    assert_eq!(&*history.reads.borrow(), &["decisions", "journal"]);
}

#[test]
fn doctor_missing_or_unlaunched_send_fails_without_losing_later_report_lines() {
    for delivered in [
        vec![leg("beta", Delivery::Silent)],
        vec![
            leg("beta", Delivery::Silent),
            leg("alpha", Delivery::Unlaunched("cannot launch".into())),
        ],
    ] {
        let (code, lines) = report(
            &History::default(),
            delivered,
            Outcome::Signalled(1),
            Pairing::NoAnswer,
        );
        assert_eq!(code, 1);
        assert!(lines[0].starts_with("alpha: FAILED,"));
        assert!(lines.last().unwrap().contains("missed"));
    }
}

#[test]
fn doctor_unreadable_history_is_not_a_delivery_failure_and_keeps_error_kind() {
    let history = History {
        decisions: Err("permission denied".into()),
        journal: Err("invalid data".into()),
        ..History::default()
    };
    let (code, lines) = report(&history, sent(), Outcome::Signalled(1), Pairing::NoAnswer);
    assert_eq!(code, 0);
    assert!(
        lines.contains(
            &"pns doctor: the decision log could not be read (permission denied).".into()
        )
    );
    assert_eq!(
        lines.last().unwrap(),
        "pns doctor: the missed-notification journal could not be read (invalid data)."
    );
}

#[test]
fn doctor_does_not_render_private_missed_text_and_honours_replay_off() {
    let history = History {
        journal: Ok(Some("private notification payload\n".into())),
        ..History::default()
    };
    let (code, lines) = report(&history, sent(), Outcome::Signalled(1), Pairing::NoAnswer);
    assert_eq!(code, 0);
    assert!(!lines.join("\n").contains("private notification payload"));
    assert!(lines.last().unwrap().contains("1"));
    assert!(!lines.last().unwrap().contains("next event"));
}

#[test]
fn doctor_unpaired_host_and_failed_pulse_grade_the_whole_completed_report() {
    for (pulse, pairing) in [
        (Outcome::Signalled(1), Pairing::Unpaired),
        (Outcome::Failed("pulse failed".into()), Pairing::NoAnswer),
    ] {
        let (code, lines) = report(&History::default(), sent(), pulse, pairing);
        assert_eq!(code, 1);
        assert!(lines.last().unwrap().contains("missed"));
    }
}

/// Every route gets its own line and the summary counts them, which is what
/// makes the section usable when several routes are configured.
#[test]
fn each_route_gets_a_line_and_the_summary_counts_them_apart() {
    let history = History {
        routes: vec![
            ("pns".into(), pns_domain::doctor::RouteVerdict::Served),
            ("gone".into(), pns_domain::doctor::RouteVerdict::Missing),
            (
                "quiet".into(),
                pns_domain::doctor::RouteVerdict::Unknown("the gateway did not answer".into()),
            ),
        ],
        ..History::default()
    };
    let (_, lines) = report(&history, sent(), Outcome::Signalled(1), Pairing::NoAnswer);
    let routes: Vec<&String> = lines
        .iter()
        .filter(|line| line.starts_with("pns doctor: route "))
        .collect();
    assert_eq!(routes.len(), 3);
    assert!(routes[0].contains("served by the gateway"), "{}", routes[0]);
    assert!(
        routes[1].contains("THE GATEWAY HAS NO SUCH ROUTE"),
        "{}",
        routes[1]
    );
    assert!(
        routes[2].contains("unknown, the gateway did not answer"),
        "{}",
        routes[2]
    );
    assert!(
        lines
            .iter()
            .any(|line| line == "pns doctor: 3 route(s) checked, 1 missing, 1 unknown"),
        "{lines:?}"
    );
}

/// A missing route does NOT move the exit code. The roster is derived from what
/// pns has posted to, so a route retired on the gateway would fail the doctor
/// forever with nothing an operator could do to clear it, and a check that
/// cannot be satisfied is one they learn to ignore.
#[test]
fn a_missing_route_reports_loudly_without_moving_the_exit_code() {
    let history = History {
        routes: vec![("gone".into(), pns_domain::doctor::RouteVerdict::Missing)],
        ..History::default()
    };
    let (code, lines) = report(&history, sent(), Outcome::Signalled(1), Pairing::NoAnswer);
    assert_eq!(code, 0, "{lines:?}");
    assert!(
        lines
            .iter()
            .any(|line| line.contains("THE GATEWAY HAS NO SUCH ROUTE")),
        "the line still says it in words that cannot be skimmed past"
    );
}
