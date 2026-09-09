mod render;

use lights_adapters::settings::{self, HueSettings, Settings};
use lights_application::{
    AdjustBrightness, BrightnessChange, LightController, LightsError, Notifier, ReportStatus,
    SceneSelection, SetPower, SetScene, TogglePower,
};
use lights_domain::{Action, Brightness, Direction};
use lights_protocol::{BrightnessRequest, Command, Request};
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
    let room = match request.room.as_deref() {
        Some(name) => match settings.aliases.resolve(name) {
            Ok(room) => room,
            Err(error) => return failure(1, error.0),
        },
        None => settings.default_room.clone(),
    };
    let notify = request.notify || settings.notify;
    match execute(&controller(&settings.controller), &settings, &room, request) {
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
    room: &lights_domain::RoomName,
    request: Request,
) -> Result<Action, LightsError> {
    match request.command {
        Command::Toggle => TogglePower::run(controller, room),
        Command::On => SetPower::run(controller, room, true),
        Command::Off => SetPower::run(controller, room, false),
        Command::Brightness(value) => {
            let change = match value {
                BrightnessRequest::Absolute(n) => BrightnessChange::Absolute(Brightness::new(n)),
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
            match name.as_str() {
                "next" => SceneSelection::Next(&settings.rotation),
                "previous" => SceneSelection::Previous(&settings.rotation),
                name => SceneSelection::Named(name),
            },
        ),
        Command::Status => ReportStatus::run(controller, room),
        Command::Help => unreachable!("help returns before composition"),
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
