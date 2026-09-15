#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Toggle,
    On,
    Off,
    Brightness(BrightnessRequest),
    Scene(String),
    Status,
    /// `None` lists the configured presets; `Some` applies one by name.
    Preset(Option<String>),
    /// Applies the preset the configured clock windows give this minute.
    PresetNow,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrightnessRequest {
    Up,
    Down,
    Absolute(u64),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub command: Command,
    pub room: Option<String>,
    /// Act on every room the alias table names rather than on one.
    pub all: bool,
    pub notify: bool,
}

pub fn parse(args: &[String]) -> Result<Request, String> {
    let mut room = None;
    let mut all = false;
    let mut notify = false;
    let mut words = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--room" {
            let value = args
                .next()
                .filter(|s| !s.starts_with('-') && !s.trim().is_empty())
                .ok_or("--room requires a name")?;
            if room.replace(value.clone()).is_some() {
                return Err("duplicate --room".into());
            }
        } else if arg == "--all" {
            if all {
                return Err("duplicate --all".into());
            }
            all = true;
        } else if arg == "--notify" {
            if notify {
                return Err("duplicate --notify".into());
            }
            notify = true;
        } else {
            words.push(arg.as_str());
        }
    }
    let command = match words.as_slice() {
        ["--help"] if room.is_none() => Command::Help,
        [] | ["toggle"] => Command::Toggle,
        ["on"] => Command::On,
        ["off"] => Command::Off,
        ["status"] => Command::Status,
        ["scene", name] if !name.is_empty() && !name.starts_with('-') => {
            Command::Scene((*name).into())
        }
        ["brightness", value] => Command::Brightness(match *value {
            "up" => BrightnessRequest::Up,
            "down" => BrightnessRequest::Down,
            value if !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit()) => {
                BrightnessRequest::Absolute(
                    value.parse().map_err(|_| "brightness integer overflow")?,
                )
            }
            _ => return Err("brightness requires up, down or a non-negative integer".into()),
        }),
        ["preset"] => Command::Preset(None),
        // A LITERAL WORD, so a preset the operator named `now` is reached as
        // `preset now` would reach the clock instead. The help says so.
        ["preset", "now"] => Command::PresetNow,
        ["preset", name] if !name.is_empty() && !name.starts_with('-') => {
            Command::Preset(Some((*name).into()))
        }
        _ => return Err("unknown command, flag or extra argument".into()),
    };
    // A PRESET NAMES ITS OWN ROOMS. Accepting `--room` here could only
    // discard it silently.
    if room.is_some() && matches!(command, Command::Preset(_) | Command::PresetNow) {
        return Err("--room does not apply to a preset".into());
    }
    // ONE OF THEM HAS TO LOSE, so neither does: a pair that names one room and
    // every room is a mistake to report rather than a preference to guess at.
    if all && room.is_some() {
        return Err("--all and --room cannot be combined".into());
    }
    if all && !matches!(command, Command::Scene(_) | Command::Brightness(_)) {
        return Err("--all applies to scene and brightness only".into());
    }
    Ok(Request {
        command,
        room,
        all,
        notify,
    })
}

#[cfg(test)]
mod tests;
