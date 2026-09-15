use crate::{BrightnessChange, Fade, LightController, LightsError, SceneMemory};
use lights_domain::{Action, PresetStep, PresetTarget, RoomName, Rotation};

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
        fade: Fade,
    ) -> Result<Action, LightsError> {
        let state = controller.room(room)?;
        controller.set_brightness(&state.room, change, fade)?;
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
    Next,
    Previous,
}
pub struct SetScene;
impl SetScene {
    pub fn run<C: LightController>(
        controller: &C,
        room: &RoomName,
        rotation: &Rotation,
        memory: &SceneMemory<'_>,
        selection: SceneSelection<'_>,
        fade: Fade,
    ) -> Result<Action, LightsError> {
        let state = controller.room(room)?;
        let scenes = controller.scenes(&state.room)?;
        let active = scenes.iter().find(|s| s.active).map(|s| s.name.as_str());
        // A scene the bridge reports is the room's real place, in the rotation
        // or out of it. Memory only answers the question the bridge stopped
        // answering, which is what a pause between presses produces, and only
        // for a selection that reads the room's place at all.
        let rotating = matches!(selection, SceneSelection::Next | SceneSelection::Previous);
        let remembered = (rotating && active.is_none())
            .then(|| memory.recall(room))
            .flatten();
        let current = active.or(remembered.as_deref());
        let name = match selection {
            SceneSelection::Named(name) => name,
            SceneSelection::Next => rotation.next(current),
            SceneSelection::Previous => rotation.previous(current),
        };
        let scene =
            scenes
                .iter()
                .find(|s| s.name == name)
                .ok_or_else(|| LightsError::UnknownScene {
                    name: name.into(),
                    room: room.as_str().into(),
                })?;
        controller.set_scene(&scene.scene, fade)?;
        memory.record(room, &scene.name, rotation);
        Ok(Action::SceneSet {
            room: room.clone(),
            scene: scene.name.clone(),
        })
    }
}
/// Walks a preset's plan in order. A failed room is reported and the walk
/// continues: the point of one key is that the other rooms still change.
pub struct ApplyPreset;
impl ApplyPreset {
    pub fn run<C: LightController>(
        controller: &C,
        plan: &[PresetStep],
        rotation: &Rotation,
        memory: &SceneMemory<'_>,
    ) -> Vec<Result<Action, LightsError>> {
        plan.iter()
            .map(|step| match &step.target {
                PresetTarget::Scene(name) => SetScene::run(
                    controller,
                    &step.room,
                    rotation,
                    memory,
                    SceneSelection::Named(name),
                    Fade::INSTANT,
                ),
                PresetTarget::Off => SetPower::run(controller, &step.room, false),
            })
            .collect()
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
