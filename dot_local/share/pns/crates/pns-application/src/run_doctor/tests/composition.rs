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
    assert!(lines[12].contains("decision"));
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
