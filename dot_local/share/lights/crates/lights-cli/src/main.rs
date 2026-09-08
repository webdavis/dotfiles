use lights_adapters::HueLightController;
use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let args = match std::env::args_os()
        .skip(1)
        .map(|s| s.into_string())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(args) => args,
        Err(_) => {
            eprintln!("lights: arguments must be valid Unicode");
            return ExitCode::from(1);
        }
    };
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        .map(|p| p.join("lights/config.toml"))
        .unwrap_or_default();
    let response = lights_cli::run(&args, &config, HueLightController::new);
    print!("{}", response.stdout);
    eprint!("{}", response.stderr);
    ExitCode::from(response.exit)
}
