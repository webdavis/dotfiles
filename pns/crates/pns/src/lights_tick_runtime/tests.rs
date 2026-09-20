use super::*;
use std::{fs, path::PathBuf};

struct Fixture {
    home: PathBuf,
    records: pns_adapters::SqliteStore,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let home = crate::runtime_test_support::scratch(name);
        let records = pns_adapters::SqliteStore::for_records(home.join("state"));
        Self { home, records }
    }

    fn silenced(&self, modes: &[&str], now: u64) -> bool {
        status_lights_silenced(
            &self.records,
            self.home.to_str().unwrap(),
            &modes
                .iter()
                .map(|mode| (*mode).to_string())
                .collect::<Vec<_>>(),
            now,
        )
    }

    fn focus(&self, mode: &str, name: &str) {
        let db = self.home.join("Library/DoNotDisturb/DB");
        fs::create_dir_all(&db).unwrap();
        fs::write(db.join("Assertions.json"), format!(
            r#"{{"data":[{{"storeAssertionRecords":[{{"assertionDetails":{{"assertionDetailsModeIdentifier":"{mode}"}}}}]}}]}}"#
        )).unwrap();
        fs::write(db.join("ModeConfigurations.json"), format!(
            r#"{{"data":[{{"modeConfigurations":{{"{mode}":{{"mode":{{"modeIdentifier":"{mode}","name":"{name}"}}}}}}}}]}}"#
        )).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

#[test]
fn quiet_silences_status_until_expiry_or_explicit_off() {
    let fixture = Fixture::new("status-quiet");
    assert!(!fixture.silenced(&[], 100));
    fixture.records.set_mute_expiry(Some(150)).unwrap();
    assert!(fixture.silenced(&[], 149));
    assert!(!fixture.silenced(&[], 150));
    fixture.records.set_mute_expiry(Some(200)).unwrap();
    assert!(fixture.silenced(&[], 150));
    fixture.records.set_mute_expiry(None).unwrap();
    assert!(!fixture.silenced(&[], 150));
}

#[test]
fn focus_silences_only_explicit_names_or_identifiers_and_tracks_transitions() {
    let fixture = Fixture::new("status-focus");
    fixture.focus("fixture.sleep", "Sleep");
    assert!(!fixture.silenced(&[], 100));
    assert!(fixture.silenced(&["sleep"], 100));
    assert!(fixture.silenced(&["fixture.sleep"], 100));
    assert!(!fixture.silenced(&["Work"], 100));
    fixture.focus("fixture.work", "Work");
    assert!(!fixture.silenced(&["Sleep"], 101));
    assert!(fixture.silenced(&["Work"], 101));
}

#[test]
fn both_mutes_must_end_before_status_can_resume() {
    let fixture = Fixture::new("status-both-mutes");
    fixture.records.set_mute_expiry(Some(150)).unwrap();
    fixture.focus("fixture.sleep", "Sleep");
    assert!(fixture.silenced(&["Sleep"], 149));
    assert!(fixture.silenced(&["Sleep"], 150));
    fixture.focus("fixture.work", "Work");
    assert!(fixture.silenced(&["Sleep"], 149));
    assert!(!fixture.silenced(&["Sleep"], 150));
}

#[test]
fn unknown_focus_keeps_its_fail_open_direction_without_overriding_quiet() {
    let fixture = Fixture::new("status-unknown-focus");
    assert!(!fixture.silenced(&["Sleep"], 100));
    fixture.focus("fixture.sleep", "Sleep");
    let assertions = fixture.home.join("Library/DoNotDisturb/DB/Assertions.json");
    fs::write(&assertions, "invalid").unwrap();
    assert!(!fixture.silenced(&["Sleep"], 100));
    fixture.records.set_mute_expiry(Some(150)).unwrap();
    assert!(fixture.silenced(&["Sleep"], 100));
}

#[test]
fn an_unreadable_catalog_still_allows_an_explicit_focus_identifier() {
    let fixture = Fixture::new("status-unknown-catalog");
    fixture.focus("fixture.sleep", "Sleep");
    fs::write(
        fixture
            .home
            .join("Library/DoNotDisturb/DB/ModeConfigurations.json"),
        "invalid",
    )
    .unwrap();
    assert!(!fixture.silenced(&["Sleep"], 100));
    assert!(fixture.silenced(&["fixture.sleep"], 100));
}

#[test]
fn an_unreadable_quiet_store_does_not_override_a_known_focus() {
    let fixture = Fixture::new("status-unknown-quiet");
    fs::write(fixture.home.join("state"), "not a directory").unwrap();
    assert!(!fixture.silenced(&[], 100));
    fixture.focus("fixture.sleep", "Sleep");
    assert!(fixture.silenced(&["Sleep"], 100));
}
