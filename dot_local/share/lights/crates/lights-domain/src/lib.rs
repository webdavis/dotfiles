mod brightness;
mod rooms;
mod rotation;

pub use brightness::{Brightness, Direction, ReportedBrightness};
pub use rooms::{Aliases, RoomName};
pub use rotation::Rotation;

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
