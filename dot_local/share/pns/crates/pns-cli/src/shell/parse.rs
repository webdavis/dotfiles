pub(super) enum Action<'a> {
    Begin {
        pid: u32,
        command: &'a str,
    },
    End {
        pid: u32,
        command: &'a str,
        exit_code: u8,
        elapsed: u64,
    },
}

pub(super) fn parse(argv: &[String]) -> Result<Action<'_>, &'static str> {
    let (verb, fields) = argv.split_first().ok_or("requires begin or end")?;
    if !matches!(verb.as_str(), "begin" | "end") {
        return Err("requires begin or end");
    }
    let mut pid = None;
    let mut command = None;
    let mut exit = None;
    let mut elapsed = None;
    let mut pairs = fields.chunks_exact(2);
    for pair in &mut pairs {
        let slot = match pair[0].as_str() {
            "--pid" => &mut pid,
            "--command" => &mut command,
            "--exit" if verb == "end" => &mut exit,
            "--elapsed" if verb == "end" => &mut elapsed,
            _ => return Err("has an unknown argument"),
        };
        if slot.replace(pair[1].as_str()).is_some() {
            return Err("has a repeated argument");
        }
    }
    if !pairs.remainder().is_empty() {
        return Err("requires a value after every flag");
    }
    let pid = decimal(pid)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|&n| n > 1 && n <= i32::MAX as u32)
        .ok_or("--pid requires a positive process id")?;
    let command = command.ok_or("requires --command")?;
    if verb == "begin" {
        return Ok(Action::Begin { pid, command });
    }
    let exit_code = decimal(exit)
        .and_then(|n| u8::try_from(n).ok())
        .ok_or("--exit requires a status from 0 to 255")?;
    let elapsed = decimal(elapsed).ok_or("--elapsed requires nonnegative whole seconds")?;
    Ok(Action::End {
        pid,
        command,
        exit_code,
        elapsed,
    })
}

fn decimal(value: Option<&str>) -> Option<u64> {
    let value = value?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

#[cfg(test)]
mod tests;
