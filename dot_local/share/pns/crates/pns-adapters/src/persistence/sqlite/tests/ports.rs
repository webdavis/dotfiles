use super::{SqliteStore, state};
use pns_application::{
    HeldLamps, LampComplaint, LampComplaints, LampHouseRecords, LampMutes, PresenceDecisions,
    StalenessMemory,
};
use pns_domain::lights::{mute::Muted, phase::HeldEntry, streak::Streak};
#[test]
fn lamp_repository_ports_preserve_unknown_reads_and_refuse_unwritten_changes() {
    let path = state();
    std::fs::create_dir_all(path.join("pns.db")).unwrap();
    let store = SqliteStore::new(path);
    assert_eq!(HeldLamps::read(&store), None);
    assert!(HeldLamps::remember(&store, &[HeldEntry::bare("light/new")]).is_err());
    let (held, complaints) = LampMutes::read(&store);
    assert!(held.is_empty());
    assert_eq!(complaints.len(), 1);
    assert!(
        LampMutes::write(
            &store,
            &[Muted {
                expiry: 8,
                place: "Kitchen".into()
            }]
        )
        .is_err()
    );
    let healthy = SqliteStore::new(state());
    HeldLamps::remember(&healthy, &[HeldEntry::bare("light/known")]).unwrap();
    assert_eq!(HeldLamps::read(&healthy).unwrap()[0].path, "light/known");
    LampMutes::write(
        &healthy,
        &[Muted {
            expiry: 8,
            place: "Kitchen".into(),
        }],
    )
    .unwrap();
    assert_eq!(LampMutes::read(&healthy).0[0].place, "Kitchen");
    healthy
        .connect()
        .unwrap()
        .execute("UPDATE lamp_mutes SET line = 'torn record'", [])
        .unwrap();
    let complaints = LampMutes::read(&healthy).1;
    assert_eq!(complaints[0].matches("pns: state error").count(), 1);
    assert!(complaints[0].ends_with("the next pns lights quiet write replaces the stored record"));
}
#[test]
fn a_busy_streak_publication_still_returns_the_computed_next_streak_without_claiming_it_was_stored()
{
    let mut store = SqliteStore::new(state());
    store.busy_timeout = std::time::Duration::from_millis(5);
    store.advance_streak(true, 10).unwrap();
    let mut connection = store.connect().unwrap();
    let _held = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    assert_eq!(
        LampHouseRecords::advance_streak(&store, true, 20),
        Some(Streak {
            since: 10,
            last_seen: 20
        })
    );
    let body: String = _held
        .query_row("SELECT body FROM lamp_streak", [], |row| row.get(0))
        .unwrap();
    assert_eq!(body, "10 10");
}
#[test]
fn typed_complaint_and_staleness_memories_keep_their_independent_values_and_clear_only_the_named_one()
 {
    let store = SqliteStore::new(state());
    LampComplaints::remember(&store, LampComplaint::Tick, Some("tick"));
    LampComplaints::remember(&store, LampComplaint::Quiet, Some("quiet"));
    StalenessMemory::remember(&store, Some("episode"));
    assert_eq!(
        LampComplaints::remembered(&store, LampComplaint::Tick),
        "tick"
    );
    assert_eq!(
        LampComplaints::remembered(&store, LampComplaint::Quiet),
        "quiet"
    );
    assert_eq!(
        StalenessMemory::remembered(&store).as_deref(),
        Some("episode")
    );
    LampComplaints::remember(&store, LampComplaint::Quiet, None);
    assert_eq!(LampComplaints::remembered(&store, LampComplaint::Quiet), "");
    assert_eq!(
        LampComplaints::remembered(&store, LampComplaint::Tick),
        "tick"
    );
}
#[test]
fn the_presence_port_records_the_callers_snapshot_and_the_house_port_reads_committed_news() {
    let store = SqliteStore::new(state());
    let snapshot = pns_domain::Snapshot {
        status: pns_domain::PresenceStatus::Nowhere { poll_age_secs: 3 },
        desk_idle_secs: None,
        screen_locked: None,
        home: pns_domain::home::HomePresence::NotHome,
        desk_room: None,
        desk_stale_after_secs: 120,
        now: Some(7),
    };
    PresenceDecisions::record(
        &store,
        &snapshot,
        &pns_domain::Narrowing::To("Kitchen".into()),
    );
    let entry = crate::presence_journal::last(&store.presence_history().unwrap().unwrap()).unwrap();
    assert_eq!(entry.at, Some(7));
    assert_eq!(entry.room.as_deref(), Some("Kitchen"));
    store
        .record_news(pns_domain::lamps::config::Behaviour::Failed, Some(9))
        .unwrap();
    assert_eq!(LampHouseRecords::news(&store).failed_at, Some(9));
}
