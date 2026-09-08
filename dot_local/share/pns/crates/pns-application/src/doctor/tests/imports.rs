use super::*;

#[test]
fn doctor_names_retained_import_failures_after_history_without_changing_delivery_health() {
    let history = History {
        imports: Ok(vec![(
            "lights-held".into(),
            "legacy record could not be read".into(),
        )]),
        ..History::default()
    };
    let (code, lines) = report(&history, sent(), Outcome::Signalled(1), Pairing::NoAnswer);
    assert_eq!(code, 0);
    assert_eq!(
        lines.last().unwrap(),
        "pns doctor: state import lights-held: legacy record could not be read; legacy file retained."
    );
    assert!(lines[lines.len() - 2].contains("missed"));
    let (code, failed) = report(
        &history,
        Vec::new(),
        Outcome::Signalled(1),
        Pairing::NoAnswer,
    );
    assert_eq!(code, 1);
    assert_eq!(failed.last(), lines.last());
}

#[test]
fn doctor_reports_an_unavailable_import_check_without_claiming_healthy_state_or_failing_delivery() {
    let history = History {
        imports: Err("database busy".into()),
        ..History::default()
    };
    let (code, lines) = report(&history, sent(), Outcome::Signalled(1), Pairing::NoAnswer);
    assert_eq!(code, 0);
    assert_eq!(
        lines.last().unwrap(),
        "pns doctor: state import status could not be read (database busy)."
    );
    assert!(lines[lines.len() - 2].contains("missed"));
}
