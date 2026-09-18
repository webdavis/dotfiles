use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let args = match std::env::args_os()
        .skip(1)
        .map(|argument| argument.into_string())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(args) => args,
        Err(_) => {
            eprintln!("morning: arguments must be valid Unicode");
            return ExitCode::from(2);
        }
    };
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
        .join("morning/config.toml");
    let response = morning::run(&args, &config, &home);
    print!("{}", response.stdout);
    eprint!("{}", response.stderr);
    ExitCode::from(response.exit)
}
