mod controller;
mod memory;
mod notification;
mod schedule;
mod use_cases;

pub use controller::{
    BrightnessChange, LightControlError, LightController, RoomRef, RoomState, SceneRef, SceneState,
};
pub use memory::{PositionStore, SceneMemory};
pub use notification::Notifier;
pub use schedule::{ChoosePreset, Clock, NoPresetNow};
pub use use_cases::{
    AdjustBrightness, ApplyPreset, ReportStatus, SceneSelection, SetPower, SetScene, TogglePower,
};
pub type LightsError = LightControlError;
