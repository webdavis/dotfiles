use lights_domain::{Brightness, Direction, ReportedBrightness, RoomName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomRef(usize);
impl RoomRef {
    pub fn from_index(index: usize) -> Self {
        Self(index)
    }
    pub fn index(self) -> usize {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneRef(usize);
impl SceneRef {
    pub fn from_index(index: usize) -> Self {
        Self(index)
    }
    pub fn index(self) -> usize {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct RoomState {
    pub room: RoomRef,
    pub on: bool,
    pub brightness: Option<ReportedBrightness>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneState {
    pub scene: SceneRef,
    pub name: String,
    pub active: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrightnessChange {
    Absolute(Brightness),
    Step { direction: Direction, percent: u8 },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightControlError {
    Unreachable { detail: String },
    Refused { detail: String },
    UnknownRoom { name: String },
    UnknownScene { name: String, room: String },
    Malformed { detail: String },
    InvalidReference,
}

pub trait LightController {
    fn room(&self, name: &RoomName) -> Result<RoomState, LightControlError>;
    fn scenes(&self, room: &RoomRef) -> Result<Vec<SceneState>, LightControlError>;
    fn set_power(&self, room: &RoomRef, on: bool) -> Result<(), LightControlError>;
    fn set_brightness(
        &self,
        room: &RoomRef,
        change: BrightnessChange,
    ) -> Result<(), LightControlError>;
    fn set_scene(&self, scene: &SceneRef) -> Result<(), LightControlError>;
}
