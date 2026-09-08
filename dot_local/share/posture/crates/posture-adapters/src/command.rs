use posture_application::InspectionFailure;
use std::ffi::OsStr;
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, Stdio};
mod child;
mod input;
mod terminal;
use child::OwnedChild;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandIo<'a> {
    Inspection { merge_stderr: bool },
    CaptureStdout,
    Input(&'a [u8]),
    InheritAll,
}

pub trait CommandRunner {
    fn run(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<Vec<u8>, InspectionFailure> {
        let completed = self.run_completed(program, args, io)?;
        if completed.exit == 0 {
            Ok(completed.bytes)
        } else {
            Err(InspectionFailure::Failed)
        }
    }
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure>;
}

#[derive(Debug, PartialEq, Eq)]
pub struct CommandOutput {
    pub bytes: Vec<u8>,
    pub exit: i32,
}

pub struct SystemRunner {
    budget: Budget,
}
enum Budget {
    Total(Instant),
    PerCommand(Duration),
}
impl SystemRunner {
    pub fn per_command(budget: Duration) -> Self {
        Self {
            budget: Budget::PerCommand(budget),
        }
    }
    pub fn new(budget: Duration) -> Self {
        Self {
            budget: Budget::Total(Instant::now() + budget),
        }
    }
}

impl CommandRunner for SystemRunner {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        let expires = match self.budget {
            Budget::Total(expires) => expires,
            Budget::PerCommand(duration) => Instant::now() + duration,
        };
        if Instant::now() >= expires {
            return Err(InspectionFailure::TimedOut);
        }
        let interactive = matches!(io, CommandIo::CaptureStdout | CommandIo::InheritAll);
        let mut command = Command::new(program);
        command.args(args).process_group(0);
        command.stdin(if interactive {
            Stdio::inherit()
        } else {
            Stdio::null()
        });
        let mut input = match io {
            CommandIo::Input(bytes) => Some(input::PendingInput::prepare(&mut command, bytes)?),
            _ => None,
        };
        let mut reader = if io == CommandIo::InheritAll {
            command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
            None
        } else {
            let (reader, writer) = io::pipe().map_err(|_| InspectionFailure::Unavailable)?;
            input::nonblocking(&reader)?;
            let stderr = match io {
                CommandIo::Inspection { merge_stderr: true } => Stdio::from(
                    writer
                        .try_clone()
                        .map_err(|_| InspectionFailure::Unavailable)?,
                ),
                CommandIo::CaptureStdout => Stdio::inherit(),
                _ => Stdio::null(),
            };
            command.stdout(Stdio::from(writer)).stderr(stderr);
            Some(reader)
        };
        let mut child = OwnedChild(Some(
            command
                .spawn()
                .map_err(|_| InspectionFailure::Unavailable)?,
        ));
        // Command retains parent copies of the writers until dropped; EOF depends on this.
        drop(command);
        let mut foreground = if interactive {
            terminal::Foreground::take(
                child.0.as_ref().ok_or(InspectionFailure::Failed)?.id() as libc::pid_t
            )?
        } else {
            None
        };
        let mut output = Vec::new();
        let mut eof = reader.is_none();
        loop {
            if Instant::now() >= expires {
                return Err(InspectionFailure::TimedOut);
            }
            let mut bytes = [0_u8; 4096];
            if let Some(reader) = reader.as_mut().filter(|_| !eof) {
                match reader.read(&mut bytes) {
                    Ok(0) => eof = true,
                    Ok(count) => output.extend_from_slice(&bytes[..count]),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => return Err(InspectionFailure::Failed),
                }
            }
            if let Some(input) = &mut input {
                input.write_pending()?;
            }
            if eof && child.exited()? {
                let status = child.finish()?;
                if let Some(foreground) = &mut foreground {
                    foreground.restore()?;
                }
                let exit = status
                    .code()
                    .or_else(|| status.signal().map(|signal| 128 + signal))
                    .ok_or(InspectionFailure::Failed)?;
                return Ok(CommandOutput {
                    bytes: output,
                    exit,
                });
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests;
