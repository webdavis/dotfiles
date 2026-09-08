#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Toggle,
    On,
    Off,
    Brightness(BrightnessRequest),
    Scene(String),
    Status,
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
}

pub fn parse(args: &[String]) -> Result<Request, String> {
    let mut room = None;
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
        _ => return Err("unknown command, flag or extra argument".into()),
    };
    Ok(Request { command, room })
}

#[cfg(test)]
mod tests;
