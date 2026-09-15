use std::{ffi::OsString, io::Write};
mod configuration;
mod native;
use configuration::Configuration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verb {
    Install,
    Verify,
    Reload,
    Rollback,
}

const USAGE: &str = "usage: posture ssh install|verify|reload|rollback|print-config|print-path\n";
pub(super) fn run(args: &[OsString], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let config = Configuration::read(|key| std::env::var_os(key));
    execute(
        args,
        &config,
        |verb, output| native::run(verb, &config, output),
        stdout,
        stderr,
    )
}
fn execute(
    args: &[OsString],
    config: &Configuration,
    perform: impl FnOnce(Verb, &mut posture_application::SshOutput<'_>) -> u8,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    use std::os::unix::ffi::OsStrExt;
    let word = (args.len() == 1).then(|| args[0].to_str()).flatten();
    let verb = match word {
        Some("install") => Verb::Install,
        Some("verify") => Verb::Verify,
        Some("reload") => Verb::Reload,
        Some("rollback") => Verb::Rollback,
        Some("print-config") => {
            return u8::from(
                stdout
                    .write_all(posture_domain::ssh_config().as_bytes())
                    .is_err(),
            );
        }
        Some("print-path") => {
            let path = posture_domain::ssh_dropin_path(&config.dropins);
            return u8::from(
                stdout
                    .write_all(path.as_os_str().as_bytes())
                    .and_then(|()| stdout.write_all(b"\n"))
                    .is_err(),
            );
        }
        Some("--help" | "-h") => return u8::from(stdout.write_all(USAGE.as_bytes()).is_err()),
        _ => {
            let _ = stderr.write_all(USAGE.as_bytes());
            return 2;
        }
    };
    perform(verb, &mut posture_application::SshOutput { stdout, stderr })
}

#[cfg(test)]
mod tests;
