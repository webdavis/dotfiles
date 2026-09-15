use super::super::*;
use super::*;

#[test]
fn named_scene_returns_typed_action_after_write() {
    let c = RecordingLightController::new(true);
    let store = store(None);
    assert_eq!(
        SetScene::run(
            &c,
            &room(),
            &rotation(),
            &SceneMemory::new(&store, false),
            SceneSelection::Named("Read"),
            Fade::INSTANT
        ),
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
    let store = store(None);
    assert_eq!(
        SetScene::run(
            &c,
            &room(),
            &rotation(),
            &SceneMemory::new(&store, false),
            SceneSelection::Named("Missing"),
            Fade::INSTANT
        ),
        Err(LightsError::UnknownScene {
            name: "Missing".into(),
            room: "Studio".into()
        })
    );
    assert_eq!(*c.calls.borrow(), [Call::Room, Call::Scenes]);
}
#[test]
fn a_remembered_place_is_used_only_when_the_bridge_reports_no_active_scene() {
    let c = RecordingLightController::with_scenes(vec![
        scene(12, "Rest", false),
        scene(14, "Relax", false),
        scene(11, "Read", false),
    ]);
    let store = store(Some("Rest"));
    assert_eq!(
        SetScene::run(
            &c,
            &room(),
            &rotation(),
            &SceneMemory::new(&store, true),
            SceneSelection::Next,
            Fade::INSTANT
        ),
        Ok(Action::SceneSet {
            room: room(),
            scene: "Relax".into()
        })
    );
    assert_eq!(
        *c.calls.borrow(),
        [Call::Room, Call::Scenes, Call::Scene(14)]
    );
    assert_eq!(
        *store.calls.borrow(),
        [
            StoreCall::Remembered("Studio".into()),
            StoreCall::Remember("Studio".into(), "Relax".into()),
        ]
    );
}
#[test]
fn an_active_scene_outside_the_rotation_still_means_the_fallback() {
    let c = RecordingLightController::with_scenes(vec![
        scene(13, "CC Halo Amber", true),
        scene(12, "Rest", false),
        scene(11, "Read", false),
    ]);
    let store = store(Some("Rest"));
    for selection in [SceneSelection::Next, SceneSelection::Previous] {
        assert_eq!(
            SetScene::run(
                &c,
                &room(),
                &rotation(),
                &SceneMemory::new(&store, true),
                selection,
                Fade::INSTANT
            ),
            Ok(Action::SceneSet {
                room: room(),
                scene: "Read".into()
            })
        );
    }
    // The bridge answered, so the memory was never asked.
    assert_eq!(
        *store.calls.borrow(),
        [
            StoreCall::Remember("Studio".into(), "Read".into()),
            StoreCall::Remember("Studio".into(), "Read".into()),
        ]
    );
}
#[test]
fn a_scene_outside_the_rotation_records_nothing() {
    let c = RecordingLightController::with_scenes(vec![
        scene(11, "Read", true),
        scene(13, "CC Halo Amber", false),
    ]);
    let store = store(None);
    assert_eq!(
        SetScene::run(
            &c,
            &room(),
            &rotation(),
            &SceneMemory::new(&store, true),
            SceneSelection::Named("CC Halo Amber"),
            Fade::INSTANT
        ),
        Ok(Action::SceneSet {
            room: room(),
            scene: "CC Halo Amber".into()
        })
    );
    assert!(store.calls.borrow().is_empty());
}
#[test]
fn a_preset_step_records_the_place_it_set() {
    let c = RecordingLightController::with_scenes(vec![
        scene(12, "Rest", false),
        scene(11, "Read", false),
    ]);
    let store = store(None);
    let steps = preset(&[("Studio", PresetTarget::Scene("Rest".into()))]);
    assert_eq!(
        ApplyPreset::run(&c, &steps, &rotation(), &SceneMemory::new(&store, true)),
        vec![Ok(Action::SceneSet {
            room: room(),
            scene: "Rest".into()
        })]
    );
    // A named scene never consults the rotation, so it never asks the memory.
    assert_eq!(
        *store.calls.borrow(),
        [StoreCall::Remember("Studio".into(), "Rest".into())]
    );
}
