mod controller;
mod notification;
mod use_cases;

pub use controller::{
    BrightnessChange, LightControlError, LightController, RoomRef, RoomState, SceneRef, SceneState,
};
pub use notification::Notifier;
pub use use_cases::{
    AdjustBrightness, ReportStatus, SceneSelection, SetPower, SetScene, TogglePower,
};
pub type LightsError = LightControlError;
