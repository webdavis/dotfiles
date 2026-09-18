mod brightness;
mod certificate_pin;
mod presets;
mod rooms;
mod rotation;
mod windows;

pub use brightness::{Brightness, Direction, ReportedBrightness};
pub use certificate_pin::CertificatePin;
pub use presets::{PresetStep, PresetTarget, Presets};
pub use rooms::{Aliases, RoomName};
pub use rotation::Rotation;
pub use windows::{MINUTES_PER_DAY, MinuteOfDay, PresetWindow, PresetWindows};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueError(pub &'static str);

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    PowerSet {
        room: RoomName,
        on: bool,
    },
    BrightnessSet {
        room: RoomName,
        level: Brightness,
    },
    BrightnessStepped {
        room: RoomName,
        direction: Direction,
    },
    SceneSet {
        room: RoomName,
        scene: String,
    },
    Reported {
        room: RoomName,
        on: bool,
        brightness: Option<ReportedBrightness>,
        scene: Option<String>,
    },
}
