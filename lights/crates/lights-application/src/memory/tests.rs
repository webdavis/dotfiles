use super::*;
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
pub(crate) enum StoreCall {
    Remembered(String),
    Remember(String, String),
}
#[derive(Default)]
pub(crate) struct RecordingPositionStore {
    pub(crate) held: Option<String>,
    pub(crate) calls: RefCell<Vec<StoreCall>>,
}
impl PositionStore for RecordingPositionStore {
    fn remembered(&self, room: &RoomName) -> Option<String> {
        self.calls
            .borrow_mut()
            .push(StoreCall::Remembered(room.as_str().into()));
        self.held.clone()
    }
    fn remember(&self, room: &RoomName, scene: &str) {
        self.calls
            .borrow_mut()
            .push(StoreCall::Remember(room.as_str().into(), scene.into()));
    }
}
pub(crate) fn rotation() -> Rotation {
    Rotation::new(vec!["Rest".into(), "Read".into()], "Read".into()).unwrap()
}
fn room() -> RoomName {
    RoomName::new("Studio").unwrap()
}

#[test]
fn the_setting_off_neither_reads_nor_writes_the_store() {
    let store = RecordingPositionStore {
        held: Some("Rest".into()),
        ..Default::default()
    };
    let memory = SceneMemory::new(&store, false);
    assert_eq!(memory.recall(&room()), None);
    memory.record(&room(), "Rest", &rotation());
    assert!(store.calls.borrow().is_empty());
}
#[test]
fn the_setting_on_reads_what_the_store_holds() {
    let store = RecordingPositionStore {
        held: Some("Rest".into()),
        ..Default::default()
    };
    let memory = SceneMemory::new(&store, true);
    assert_eq!(memory.recall(&room()).as_deref(), Some("Rest"));
    assert_eq!(
        *store.calls.borrow(),
        [StoreCall::Remembered("Studio".into())]
    );
}
#[test]
fn a_scene_outside_the_rotation_is_never_recorded() {
    let store = RecordingPositionStore::default();
    let memory = SceneMemory::new(&store, true);
    memory.record(&room(), "CC Halo Amber", &rotation());
    assert!(store.calls.borrow().is_empty());
    memory.record(&room(), "Rest", &rotation());
    assert_eq!(
        *store.calls.borrow(),
        [StoreCall::Remember("Studio".into(), "Rest".into())]
    );
}
