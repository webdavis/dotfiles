mod render;

use lights_adapters::settings::{self, HueSettings, Settings};
use lights_adapters::{FilePositionStore, SystemClock};
use lights_application::{
    AdjustBrightness, ApplyPreset, BrightnessChange, ChoosePreset, LightController, LightsError,
    Notifier, ReportStatus, SceneMemory, SceneSelection, SetPower, SetScene, TogglePower,
};
use lights_domain::{Action, Brightness, Direction};
use lights_protocol::{BrightnessRequest, Command};
use std::path::Path;

#[derive(Debug, PartialEq)]
pub struct Response {
    pub exit: u8,
    pub stdout: String,
    pub stderr: String,
}

pub fn run<C: LightController>(
    args: &[String],
    config: &Path,
    state: &Path,
    notifier: &impl Notifier,
    controller: impl FnOnce(&HueSettings) -> C,
) -> Response {
    let request = match lights_protocol::parse(args) {
        Ok(request) => request,
        Err(message) => return failure(1, &format!("{message}\n{}", lights_protocol::HELP)),
    };
    if request.command == Command::Help {
        return success(lights_protocol::HELP.into());
    }
    let settings = match settings::load(config) {
        Ok(settings) => settings,
        Err(error) => return failure(5, &error.0),
    };
    let notify = request.notify || settings.notify;
    let store = FilePositionStore::new(state);
    let memory = SceneMemory::new(&store, settings.remember_position);
    if matches!(request.command, Command::Preset(_) | Command::PresetNow) {
        let name = match &request.command {
            Command::Preset(None) => {
                let names = render::preset_names(&settings.presets);
                // Listing nothing is not an error, but silence on a fresh
                // config is indistinguishable from a broken binary, so say
                // which it is.
                return if names.is_empty() {
                    failure(0, "no presets configured")
                } else {
                    success(names)
                };
            }
            Command::Preset(Some(name)) => name.clone(),
            // THE CLOCK CHOOSES, and a clock that names nothing is a refusal:
            // one key that quietly does nothing is worse than one that says why.
            _ => match ChoosePreset::run(&SystemClock, &settings.preset_windows) {
                Ok(name) => name.to_owned(),
                Err(refusal) => return failure(1, &render::no_preset_now(refusal)),
            },
        };
        let Some(plan) = settings.presets.plan(&name) else {
            return failure(1, &format!("unknown preset {name:?}"));
        };
        let results = ApplyPreset::run(
            &controller(&settings.controller),
            plan,
            &settings.rotation,
            &memory,
        );
        if notify {
            for action in results.iter().flatten() {
                notifier.announce(action);
            }
        }
        return render::per_room(&results);
    }
    let controller = controller(&settings.controller);
    // EVERY CONFIGURED ROOM, each read and written on its own, so `scene next`
    // leaves each room one step past where that room actually was.
    if request.all {
        let results = settings
            .aliases
            .rooms()
            .iter()
            .map(|room| execute(&controller, &settings, &memory, room, &request.command))
            .collect::<Vec<_>>();
        if notify {
            for action in results.iter().flatten() {
                notifier.announce(action);
            }
        }
        return render::per_room(&results);
    }
    let room = match request.room.as_deref() {
        Some(name) => match settings.aliases.resolve(name) {
            Ok(room) => room,
            Err(error) => return failure(1, error.0),
        },
        None => settings.default_room.clone(),
    };
    match execute(&controller, &settings, &memory, &room, &request.command) {
        Ok(action) => {
            if notify && !matches!(action, Action::Reported { .. }) {
                notifier.announce(&action);
            }
            success(lights_adapters::render_action(&action))
        }
        Err(error) => {
            let (code, message) = render::error(error);
            failure(code, &message)
        }
    }
}

fn execute<C: LightController>(
    controller: &C,
    settings: &Settings,
    memory: &SceneMemory<'_>,
    room: &lights_domain::RoomName,
    command: &Command,
) -> Result<Action, LightsError> {
    match command {
        Command::Toggle => TogglePower::run(controller, room),
        Command::On => SetPower::run(controller, room, true),
        Command::Off => SetPower::run(controller, room, false),
        Command::Brightness(value) => {
            let change = match value {
                BrightnessRequest::Absolute(n) => BrightnessChange::Absolute(Brightness::new(*n)),
                BrightnessRequest::Up => BrightnessChange::Step {
                    direction: Direction::Up,
                    percent: settings.step,
                },
                BrightnessRequest::Down => BrightnessChange::Step {
                    direction: Direction::Down,
                    percent: settings.step,
                },
            };
            AdjustBrightness::run(controller, room, change)
        }
        Command::Scene(name) => SetScene::run(
            controller,
            room,
            &settings.rotation,
            memory,
            match name.as_str() {
                "next" => SceneSelection::Next,
                "previous" => SceneSelection::Previous,
                name => SceneSelection::Named(name),
            },
        ),
        Command::Status => ReportStatus::run(controller, room),
        Command::Help | Command::Preset(_) | Command::PresetNow => {
            unreachable!("help and presets return before single-room composition")
        }
    }
}

fn success(stdout: String) -> Response {
    Response {
        exit: 0,
        stdout,
        stderr: String::new(),
    }
}
fn failure(exit: u8, message: &str) -> Response {
    Response {
        exit,
        stdout: String::new(),
        stderr: format!("lights: {message}\n"),
    }
}
