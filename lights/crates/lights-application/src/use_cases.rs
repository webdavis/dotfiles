use crate::{BrightnessChange, LightController, LightsError};
use lights_domain::{Action, RoomName, Rotation};

pub struct TogglePower;
impl TogglePower {
    pub fn run<C: LightController>(controller: &C, room: &RoomName) -> Result<Action, LightsError> {
        let state = controller.room(room)?;
        let on = !state.on;
        controller.set_power(&state.room, on)?;
        Ok(Action::PowerSet {
            room: room.clone(),
            on,
        })
    }
}
pub struct SetPower;
impl SetPower {
    pub fn run<C: LightController>(
        controller: &C,
        room: &RoomName,
        on: bool,
    ) -> Result<Action, LightsError> {
        let state = controller.room(room)?;
        controller.set_power(&state.room, on)?;
        Ok(Action::PowerSet {
            room: room.clone(),
            on,
        })
    }
}
pub struct AdjustBrightness;
impl AdjustBrightness {
    pub fn run<C: LightController>(
        controller: &C,
        room: &RoomName,
        change: BrightnessChange,
    ) -> Result<Action, LightsError> {
        let state = controller.room(room)?;
        controller.set_brightness(&state.room, change)?;
        Ok(match change {
            BrightnessChange::Absolute(level) => Action::BrightnessSet {
                room: room.clone(),
                level,
            },
            BrightnessChange::Step { direction, .. } => Action::BrightnessStepped {
                room: room.clone(),
                direction,
            },
        })
    }
}
pub enum SceneSelection<'a> {
    Named(&'a str),
    Next(&'a Rotation),
    Previous(&'a Rotation),
}
pub struct SetScene;
impl SetScene {
    pub fn run<C: LightController>(
        controller: &C,
        room: &RoomName,
        selection: SceneSelection<'_>,
    ) -> Result<Action, LightsError> {
        let state = controller.room(room)?;
        let scenes = controller.scenes(&state.room)?;
        let current = scenes.iter().find(|s| s.active).map(|s| s.name.as_str());
        let name = match selection {
            SceneSelection::Named(name) => name,
            SceneSelection::Next(rotation) => rotation.next(current),
            SceneSelection::Previous(rotation) => rotation.previous(current),
        };
        let scene =
            scenes
                .iter()
                .find(|s| s.name == name)
                .ok_or_else(|| LightsError::UnknownScene {
                    name: name.into(),
                    room: room.as_str().into(),
                })?;
        controller.set_scene(&scene.scene)?;
        Ok(Action::SceneSet {
            room: room.clone(),
            scene: scene.name.clone(),
        })
    }
}
pub struct ReportStatus;
impl ReportStatus {
    pub fn run<C: LightController>(controller: &C, room: &RoomName) -> Result<Action, LightsError> {
        let state = controller.room(room)?;
        let scene = controller
            .scenes(&state.room)?
            .into_iter()
            .find(|s| s.active)
            .map(|s| s.name);
        Ok(Action::Reported {
            room: room.clone(),
            on: state.on,
            brightness: state.brightness,
            scene,
        })
    }
}

#[cfg(test)]
mod tests;
