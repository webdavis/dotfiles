use lights_domain::{RoomName, Rotation};

/// A room's place in the scene rotation, kept between invocations. The bridge
/// reports no active scene once a room has sat untouched for a while, and this
/// is what stands in for the scene it reported before.
pub trait PositionStore {
    fn remembered(&self, room: &RoomName) -> Option<String>;
    fn remember(&self, room: &RoomName, scene: &str);
}

/// A store and the setting that governs it. Off is the default, and off means
/// the store is neither read nor written.
pub struct SceneMemory<'a> {
    store: &'a dyn PositionStore,
    enabled: bool,
}

impl<'a> SceneMemory<'a> {
    pub fn new(store: &'a dyn PositionStore, enabled: bool) -> Self {
        Self { store, enabled }
    }
    pub(crate) fn recall(&self, room: &RoomName) -> Option<String> {
        self.enabled.then(|| self.store.remembered(room)).flatten()
    }
    /// Only a scene the rotation holds is a place to continue from, so a scene
    /// outside it leaves the room's last recorded place alone.
    pub(crate) fn record(&self, room: &RoomName, scene: &str, rotation: &Rotation) {
        if self.enabled && rotation.contains(scene) {
            self.store.remember(room, scene);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
