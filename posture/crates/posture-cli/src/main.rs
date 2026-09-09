use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(posture_cli::run(
        &std::env::args_os().skip(1).collect::<Vec<_>>(),
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ))
}
