use super::*;
use std::cell::RefCell;

#[derive(Default)]
struct RecordingLightController {
    writes: RefCell<Vec<bool>>,
}
impl LightController for RecordingLightController {
    fn room(&self, name: &RoomName) -> Result<RoomState, LightControlError> {
        if name.as_str() != "Studio" {
            return Err(LightControlError::UnknownRoom {
                name: name.as_str().into(),
            });
        }
        Ok(RoomState {
            room: RoomRef::from_index(0),
            on: true,
            brightness: Some(lights_domain::ReportedBrightness::new(42.75).unwrap()),
        })
    }
    fn scenes(&self, room: &RoomRef) -> Result<Vec<SceneState>, LightControlError> {
        if room.index() != 0 {
            return Err(LightControlError::InvalidReference);
        }
        Ok(vec![SceneState {
            scene: SceneRef::from_index(5),
            name: "Read".into(),
            active: true,
        }])
    }
    fn set_power(&self, room: &RoomRef, on: bool) -> Result<(), LightControlError> {
        if room.index() != 0 {
            return Err(LightControlError::InvalidReference);
        }
        self.writes.borrow_mut().push(on);
        Ok(())
    }
    fn set_brightness(&self, room: &RoomRef, _: BrightnessChange) -> Result<(), LightControlError> {
        if room.index() != 0 {
            return Err(LightControlError::InvalidReference);
        }
        Ok(())
    }
    fn set_scene(&self, scene: &SceneRef) -> Result<(), LightControlError> {
        if scene.index() != 5 {
            return Err(LightControlError::InvalidReference);
        }
        Ok(())
    }
}
fn controller_contract(controller: &impl LightController) {
    let state = controller.room(&room()).unwrap();
    assert!(state.on);
    assert_eq!(state.brightness.unwrap().percent(), 42.75);
    assert_eq!(
        controller.room(&RoomName::new("Missing").unwrap()),
        Err(LightControlError::UnknownRoom {
            name: "Missing".into()
        })
    );
    let scenes = controller.scenes(&state.room).unwrap();
    assert_eq!(
        scenes
            .iter()
            .filter(|s| s.active)
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Read"]
    );
    controller.set_power(&state.room, true).unwrap();
    controller.set_power(&state.room, false).unwrap();
    let invalid = RoomRef::from_index(usize::MAX);
    assert_eq!(
        controller.set_power(&invalid, true),
        Err(LightControlError::InvalidReference)
    );
    assert_eq!(
        controller.set_scene(&SceneRef::from_index(usize::MAX)),
        Err(LightControlError::InvalidReference)
    );
}
#[test]
fn light_controller_contract_recording() {
    let controller = RecordingLightController::default();
    controller_contract(&controller);
    assert_eq!(*controller.writes.borrow(), [true, false]);
}
#[test]
fn light_controller_contract_hue() {
    let ok = json!({"errors":[],"data":[]});
    let (controller, requests) = setup(vec![(200, fixture()), (200, ok.clone()), (200, ok)]);
    controller_contract(&controller);
    assert_eq!(requests.lock().unwrap().len(), 3);
}
