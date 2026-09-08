use lights_application::LightsError;
use lights_domain::{Action, Direction};
use lights_protocol::Output;

pub(super) fn action(action: &Action) -> String {
    match action {
        Action::PowerSet { room, on } => Output::Power {
            room: room.as_str(),
            on: *on,
        },
        Action::BrightnessSet { room, level } => Output::Absolute {
            room: room.as_str(),
            percent: level.percent(),
        },
        Action::BrightnessStepped { room, direction } => Output::Step {
            room: room.as_str(),
            up: *direction == Direction::Up,
        },
        Action::SceneSet { room, scene } => Output::Scene {
            room: room.as_str(),
            scene,
        },
        Action::Reported {
            room,
            on,
            brightness,
            scene,
        } => Output::Status {
            room: room.as_str(),
            on: *on,
            brightness: brightness.map(|n| n.percent()),
            scene: scene.as_deref(),
        },
    }
    .render()
}

pub(super) fn error(error: LightsError) -> (u8, String) {
    match error {
        LightsError::UnknownRoom { name } => (2, format!("unknown room {name:?}")),
        LightsError::UnknownScene { name, room } => {
            (3, format!("unknown scene {name:?} in room {room:?}"))
        }
        LightsError::Unreachable { detail }
        | LightsError::Refused { detail }
        | LightsError::Malformed { detail } => (4, detail),
        LightsError::InvalidReference => (4, "invalid controller reference".into()),
    }
}
