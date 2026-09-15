use super::*;
use lights_domain::RoomName;

fn store(name: &str) -> (FilePositionStore, PathBuf) {
    let dir = std::env::temp_dir().join(format!("lights-position-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("lights/position.toml");
    (FilePositionStore::new(&path), path)
}
fn room(name: &str) -> RoomName {
    RoomName::new(name).unwrap()
}

#[test]
fn a_recorded_room_survives_a_reread_and_leaves_the_others_alone() {
    let (writer, path) = store("round-trip");
    assert_eq!(writer.remembered(&room("3F - Studio")), None);
    writer.remember(&room("3F - Studio"), "Rest");
    writer.remember(&room("2F - Kitchen"), "Relax");
    writer.remember(&room("3F - Studio"), "Dimmed");

    let reader = FilePositionStore::new(&path);
    assert_eq!(
        reader.remembered(&room("3F - Studio")).as_deref(),
        Some("Dimmed")
    );
    assert_eq!(
        reader.remembered(&room("2F - Kitchen")).as_deref(),
        Some("Relax")
    );
    assert_eq!(reader.remembered(&room("3F - MBedroom")), None);
    let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
}
#[test]
fn a_corrupt_file_counts_as_no_memory_and_the_next_write_replaces_it() {
    let (store, path) = store("corrupt");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "3F - Studio = [unclosed\n").unwrap();
    assert_eq!(store.remembered(&room("3F - Studio")), None);
    store.remember(&room("3F - Studio"), "Rest");
    assert_eq!(
        FilePositionStore::new(&path)
            .remembered(&room("3F - Studio"))
            .as_deref(),
        Some("Rest")
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
}
