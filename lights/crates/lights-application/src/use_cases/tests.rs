use super::*;
use crate::{LightControlError, RoomRef, RoomState, SceneRef, SceneState};
use lights_domain::{Brightness, Direction, ReportedBrightness};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Room,
    Scenes,
    Power(bool),
    Brightness(BrightnessChange),
    Scene(usize),
}
struct RecordingLightController {
    on: bool,
    calls: RefCell<Vec<Call>>,
    scene_error: bool,
}
impl RecordingLightController {
    fn new(on: bool) -> Self {
        Self {
            on,
            calls: RefCell::new(vec![]),
            scene_error: false,
        }
    }
}
impl LightController for RecordingLightController {
    fn room(&self, _: &RoomName) -> Result<RoomState, LightControlError> {
        self.calls.borrow_mut().push(Call::Room);
        Ok(RoomState {
            room: RoomRef::from_index(7),
            on: self.on,
            brightness: Some(ReportedBrightness::new(42.75).unwrap()),
        })
    }
    fn scenes(&self, room: &RoomRef) -> Result<Vec<SceneState>, LightControlError> {
        assert_eq!(room.index(), 7);
        self.calls.borrow_mut().push(Call::Scenes);
        if self.scene_error {
            return Err(LightControlError::Malformed {
                detail: "scenes".into(),
            });
        }
        Ok(vec![SceneState {
            scene: SceneRef::from_index(11),
            name: "Read".into(),
            active: true,
        }])
    }
    fn set_power(&self, room: &RoomRef, on: bool) -> Result<(), LightControlError> {
        assert_eq!(room.index(), 7);
        self.calls.borrow_mut().push(Call::Power(on));
        Ok(())
    }
    fn set_brightness(
        &self,
        room: &RoomRef,
        change: BrightnessChange,
    ) -> Result<(), LightControlError> {
        assert_eq!(room.index(), 7);
        self.calls.borrow_mut().push(Call::Brightness(change));
        Ok(())
    }
    fn set_scene(&self, scene: &SceneRef) -> Result<(), LightControlError> {
        self.calls.borrow_mut().push(Call::Scene(scene.index()));
        Ok(())
    }
}
fn room() -> RoomName {
    RoomName::new("Studio").unwrap()
}
#[test]
fn toggle_writes_opposite_aggregated_power() {
    for on in [true, false] {
        let c = RecordingLightController::new(on);
        assert_eq!(
            TogglePower::run(&c, &room()),
            Ok(Action::PowerSet {
                room: room(),
                on: !on
            })
        );
        assert_eq!(*c.calls.borrow(), [Call::Room, Call::Power(!on)]);
    }
}
fn explicit_power(on: bool) {
    for initial in [true, false] {
        let c = RecordingLightController::new(initial);
        assert_eq!(
            SetPower::run(&c, &room(), on),
            Ok(Action::PowerSet { room: room(), on })
        );
        assert_eq!(*c.calls.borrow(), [Call::Room, Call::Power(on)]);
    }
}
#[test]
fn explicit_on_ignores_existing_power() {
    explicit_power(true);
}
#[test]
fn explicit_off_ignores_existing_power() {
    explicit_power(false);
}
#[test]
fn explicit_power_resolves_once_without_decision_read() {
    explicit_power(true);
    explicit_power(false);
}
#[test]
fn absolute_write_has_no_readback_or_power_off() {
    let c = RecordingLightController::new(false);
    let level = Brightness::new(0);
    assert_eq!(
        AdjustBrightness::run(&c, &room(), BrightnessChange::Absolute(level)),
        Ok(Action::BrightnessSet {
            room: room(),
            level
        })
    );
    assert_eq!(
        *c.calls.borrow(),
        [
            Call::Room,
            Call::Brightness(BrightnessChange::Absolute(level))
        ]
    );
}
#[test]
fn relative_step_ignores_snapshot_level() {
    let c = RecordingLightController::new(true);
    let change = BrightnessChange::Step {
        direction: Direction::Down,
        percent: 5,
    };
    assert_eq!(
        AdjustBrightness::run(&c, &room(), change),
        Ok(Action::BrightnessStepped {
            room: room(),
            direction: Direction::Down
        })
    );
    assert_eq!(*c.calls.borrow(), [Call::Room, Call::Brightness(change)]);
}
#[test]
fn relative_write_has_no_readback_or_power_off() {
    relative_step_ignores_snapshot_level();
}
#[test]
fn named_scene_returns_typed_action_after_write() {
    let c = RecordingLightController::new(true);
    assert_eq!(
        SetScene::run(&c, &room(), SceneSelection::Named("Read")),
        Ok(Action::SceneSet {
            room: room(),
            scene: "Read".into()
        })
    );
    assert_eq!(
        *c.calls.borrow(),
        [Call::Room, Call::Scenes, Call::Scene(11)]
    );
}
#[test]
fn unknown_scene_has_no_write() {
    let c = RecordingLightController::new(true);
    assert_eq!(
        SetScene::run(&c, &room(), SceneSelection::Named("Missing")),
        Err(LightsError::UnknownScene {
            name: "Missing".into(),
            room: "Studio".into()
        })
    );
    assert_eq!(*c.calls.borrow(), [Call::Room, Call::Scenes]);
}
#[test]
fn status_has_no_write_or_notification() {
    let c = RecordingLightController::new(true);
    assert_eq!(
        ReportStatus::run(&c, &room()),
        Ok(Action::Reported {
            room: room(),
            on: true,
            brightness: Some(ReportedBrightness::new(42.75).unwrap()),
            scene: Some("Read".into())
        })
    );
    assert_eq!(*c.calls.borrow(), [Call::Room, Call::Scenes]);
}
#[test]
fn status_scene_lookup_failure_is_not_absence() {
    let mut c = RecordingLightController::new(true);
    c.scene_error = true;
    assert_eq!(
        ReportStatus::run(&c, &room()),
        Err(LightsError::Malformed {
            detail: "scenes".into()
        })
    );
    assert_eq!(*c.calls.borrow(), [Call::Room, Call::Scenes]);
}
