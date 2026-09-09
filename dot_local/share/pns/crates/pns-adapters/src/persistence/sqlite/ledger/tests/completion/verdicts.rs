use super::*;
#[test]
fn completing_an_owned_leg_preserves_its_identity_and_exact_printable_verdict() {
    for (completion, verdict) in [
        (acknowledged(), "delivered"),
        (retry(30), "failed"),
        (
            LedgerCompletion::Retry {
                outcome: UnconfirmedDelivery::Unlaunched,
                detail: "never ran".into(),
                retry_at: 30,
            },
            "unlaunched",
        ),
        (
            LedgerCompletion::Retry {
                outcome: UnconfirmedDelivery::Unknown,
                detail: String::new(),
                retry_at: 30,
            },
            "silent",
        ),
    ] {
        let store = SqliteStore::new(state());
        let input = submission();
        let mut other_producer = submission();
        other_producer.identity.producer = "other-producer".into();
        created(&store, &other_producer);
        let mut other_request = submission();
        other_request.identity.request_id = "other-request".into();
        created(&store, &other_request);
        let legs = created(&store, &input);
        begin(&store, &input.identity);
        let original = line(&store);
        store.record(&legs[1].claim, &completion, 11).unwrap();
        assert_eq!(
            line(&store),
            original.replace("legs=none", &format!("legs=lights-desk:{verdict}")),
            "only the owned destination's printable verdict changes"
        );
        assert_eq!(
            store.inspect(&input.identity).unwrap().unwrap().attempts[1].completion,
            completion
        );
        assert!(matches!(
            store.inspect(&input.identity).unwrap().unwrap().attempts[0].completion,
            LedgerCompletion::Retry {
                outcome: UnconfirmedDelivery::Unknown,
                ..
            }
        ));
    }
}
#[test]
fn a_pruned_or_absent_decision_does_not_block_owned_completion_or_reappear() {
    for pruned in [false, true] {
        let store = SqliteStore::new(state());
        let input = submission();
        let legs = created(&store, &input);
        if pruned {
            begin(&store, &input.identity);
            for id in 0..5 {
                begin(
                    &store,
                    &SubmissionIdentity {
                        producer: "other".into(),
                        request_id: id.to_string(),
                    },
                );
            }
        }
        let before = line(&store);
        store.record(&legs[0].claim, &acknowledged(), 11).unwrap();
        assert_eq!(
            line(&store),
            before,
            "completion cannot recreate a pruned decision"
        );
        assert_eq!(
            store.inspect(&input.identity).unwrap().unwrap().attempts[0].completion,
            acknowledged()
        );
    }
}
